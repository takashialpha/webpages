//! The command set, and the registry `help` and tab completion are built from.
//!
//! Adding a command is one entry in [`COMMANDS`] and one function. Nothing else
//! needs touching: `help` lists whatever is here, completion offers it, and the
//! [`Spec`] on the entry is checked before the function runs, so a command body
//! only ever sees arguments it declared.

use crate::args::{Args, Completes, Flag, Spec, usage};
use crate::clock;
use crate::fs::{self, Live, Node};
use crate::shell::{
    Command, Line, Output, Session, Span, body_lines, error, error_line, line, pending,
};
use crate::theme;
use crate::wall;

/// Options are kept to the ones that mean something here. `ls -l` is absent on
/// purpose: its columns are mode, owner, group, and mtime, and this tree has
/// none of those to report, so it could only make them up.
const LS_FLAGS: &[Flag] = &[
    Flag {
        short: 'a',
        long: Some("all"),
        help: "include . and ..",
    },
    Flag {
        short: '1',
        long: None,
        help: "one entry per line",
    },
];

const CAT_FLAGS: &[Flag] = &[Flag {
    short: 'n',
    long: Some("number"),
    help: "number the output lines",
}];

pub const COMMANDS: &[Command] = &[
    Command {
        name: "help",
        summary: "list the commands",
        spec: Spec {
            flags: &[],
            min: 0,
            max: Some(1),
            operand: "command",
            completes: Completes::Nothing,
        },
        run: help,
    },
    Command {
        name: "ls",
        summary: "list directory contents",
        spec: Spec {
            flags: LS_FLAGS,
            min: 0,
            max: None,
            operand: "path",
            completes: Completes::Paths,
        },
        run: ls,
    },
    Command {
        name: "cd",
        summary: "change directory",
        spec: Spec::dir("path"),
        run: cd,
    },
    Command {
        name: "cat",
        summary: "print a file",
        spec: Spec {
            flags: CAT_FLAGS,
            min: 1,
            max: None,
            operand: "file",
            completes: Completes::Paths,
        },
        run: cat,
    },
    Command {
        name: "pwd",
        summary: "print the working directory",
        spec: Spec::NONE,
        run: pwd,
    },
    Command {
        name: "whoami",
        summary: "print the current user",
        spec: Spec::NONE,
        run: whoami,
    },
    Command {
        name: "date",
        summary: "print the current utc time",
        spec: Spec::NONE,
        run: date,
    },
    Command {
        name: "uptime",
        summary: "how long this server has been up",
        spec: Spec::NONE,
        run: uptime,
    },
    Command {
        name: "wall",
        summary: "read and write the shared board",
        spec: Spec {
            flags: &[],
            min: 0,
            max: Some(3),
            operand: "x y char",
            completes: Completes::Nothing,
        },
        run: graffiti,
    },
    Command {
        name: "theme",
        summary: "switch the palette",
        spec: Spec {
            flags: &[],
            min: 0,
            max: Some(1),
            operand: "name",
            completes: Completes::Nothing,
        },
        run: set_theme,
    },
    Command {
        name: "exit",
        summary: "close the connection",
        spec: Spec::NONE,
        run: exit,
    },
    Command {
        name: "clear",
        summary: "clear the screen",
        spec: Spec::NONE,
        run: clear,
    },
];

fn help(_session: &mut Session, args: &Args<'_>) -> Output {
    if let Some(name) = args.first() {
        return COMMANDS
            .iter()
            .find(|command| command.name == name)
            .map_or_else(
                || error(format!("help: no such command: {name}")),
                |command| usage(command.about()),
            );
    }

    let width = COMMANDS
        .iter()
        .map(|command| command.name.len())
        .max()
        .unwrap_or(0);

    let mut lines: Vec<Line> = vec![vec![Span::new("available commands", "dim")], vec![]];
    lines.extend(COMMANDS.iter().map(|command| {
        vec![
            Span::new(format!("  {:width$}", command.name), "accent"),
            Span::plain(format!("  {}", command.summary)),
        ]
    }));
    lines.push(vec![]);
    lines.push(vec![Span::new(
        "every command takes -h for its own usage and options.",
        "dim",
    )]);
    lines.push(vec![Span::new(
        "`ls` and `cd` to look around. `bin/` holds programs.",
        "dim",
    )]);
    Output::Lines(lines)
}

fn ls(session: &mut Session, args: &Args<'_>) -> Output {
    let here = ["."];
    let targets = if args.operands().is_empty() {
        &here[..]
    } else {
        args.operands()
    };

    let mut missing: Vec<Line> = Vec::new();
    let mut files: Vec<Line> = Vec::new();
    let mut dirs: Vec<(&str, &'static Node)> = Vec::new();

    for target in targets {
        match fs::resolve(&session.cwd, target).and_then(|segments| fs::node_at(&segments)) {
            None => missing.push(error_line(format!(
                "ls: {target}: No such file or directory"
            ))),
            // Naming a file lists the file, the way `ls` itself does. A live
            // file is still a file: listing it says its name, and reading it is
            // `cat`'s job.
            Some(Node::File(_) | Node::Live(_) | Node::Program(_)) => {
                files.push(vec![Span::plain(*target)]);
            }
            Some(node) => dirs.push((target, node)),
        }
    }

    // The plain case keeps the grid: one directory, laid out in columns.
    if missing.is_empty()
        && files.is_empty()
        && !args.has('1')
        && let [(_, node)] = dirs[..]
    {
        return Output::Columns(entry_lines(node, args.has('a')));
    }

    // Anything else is more than one block, and blocks need headers between
    // them, which a single flowed grid cannot express.
    let mut lines = missing;
    lines.extend(files);

    // A header only makes sense when there is more than one thing to tell apart.
    let labeled = dirs.len() > 1 || !lines.is_empty();
    for (name, node) in dirs {
        if !lines.is_empty() {
            lines.push(vec![]);
        }
        if labeled {
            lines.push(vec![Span::plain(format!("{name}:"))]);
        }
        lines.extend(entry_lines(node, args.has('a')));
    }

    Output::Lines(lines)
}

/// One line per entry, directories marked with a trailing slash.
fn entry_lines(node: &Node, all: bool) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    if all {
        lines.push(vec![Span::new("./", "dir")]);
        lines.push(vec![Span::new("../", "dir")]);
    }
    lines.extend(node.entries().iter().map(|entry| match entry.node {
        Node::Dir(_) => vec![Span::new(format!("{}/", entry.name), "dir")],
        // Runnable, so it is coloured the way `ls` colours anything you can
        // run rather than read.
        Node::Program(_) => vec![Span::new(entry.name, "accent")],
        Node::File(_) | Node::Live(_) => vec![Span::plain(entry.name)],
    }));
    lines
}

fn cd(session: &mut Session, args: &Args<'_>) -> Output {
    let target = args.first().unwrap_or("~");

    // `cd -` goes back where it came from and prints where that was, the way
    // bash does. It is why a lone dash is parsed as an operand.
    if target == "-" {
        let back = std::mem::take(&mut session.prev);
        session.prev = std::mem::replace(&mut session.cwd, back);
        return line(session.prompt_path());
    }

    let Some(segments) = fs::resolve(&session.cwd, target) else {
        return error(format!("cd: {target}: No such file or directory"));
    };
    match fs::node_at(&segments) {
        Some(Node::Dir(_)) => {
            session.prev = std::mem::replace(&mut session.cwd, segments);
            Output::Nothing
        }
        Some(Node::File(_) | Node::Live(_) | Node::Program(_)) => {
            error(format!("cd: {target}: Not a directory"))
        }
        None => error(format!("cd: {target}: No such file or directory")),
    }
}

/// One operand of a `cat`, resolved but not yet rendered.
///
/// Kept as a plan rather than rendered as it goes, because a live file cannot
/// be read without asking the server, and the operands after it still have to
/// come out in the order they were given.
enum Piece {
    Text(&'static str),
    Live(Live),
    Failed(Line),
}

fn cat(session: &mut Session, args: &Args<'_>) -> Output {
    let numbered = args.has('n');
    let plan: Vec<Piece> = args
        .operands()
        .iter()
        .map(|target| {
            match fs::resolve(&session.cwd, target).and_then(|segments| fs::node_at(&segments)) {
                Some(Node::File(text)) => Piece::Text(text),
                Some(&Node::Live(live)) => Piece::Live(live),
                Some(Node::Dir(_)) => {
                    Piece::Failed(error_line(format!("cat: {target}: Is a directory")))
                }
                // Not text, so there is nothing to print. What it is instead
                // is `ls`'s business, not `cat`'s.
                Some(Node::Program(_)) => {
                    Piece::Failed(error_line(format!("cat: {target}: is a binary file")))
                }
                None => Piece::Failed(error_line(format!(
                    "cat: {target}: No such file or directory"
                ))),
            }
        })
        .collect();

    if !plan.iter().any(|piece| matches!(*piece, Piece::Live(_))) {
        return Output::Lines(spell(&plan, "", numbered));
    }

    // Something in there has to be fetched, so the whole thing answers later.
    // The board is the only live file, so one request covers however many times
    // it was named.
    pending(move |sink| {
        let plan: Vec<Piece> = plan.iter().map(Piece::clone_ref).collect();
        leptos::task::spawn_local(async move {
            sink.set(match wall::fetch(None).await {
                Ok((200, board)) => Output::Wide(spell(&plan, &board, numbered)),
                Ok((_, complaint)) => error(format!("cat: {}", complaint.trim())),
                Err(problem) => error(problem.replace("wall:", "cat:")),
            });
        });
    })
}

impl Piece {
    /// Cheap enough to copy: the text is `'static` and a failure is a line.
    fn clone_ref(&self) -> Self {
        match *self {
            Self::Text(text) => Self::Text(text),
            Self::Live(live) => Self::Live(live),
            Self::Failed(ref line) => Self::Failed(line.clone()),
        }
    }
}

/// Renders a resolved plan, numbering across the whole of it rather than
/// restarting per file, which is what `cat -n` on several files does.
fn spell(plan: &[Piece], board: &str, numbered: bool) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut count = 0_usize;

    for piece in plan {
        let body = match *piece {
            Piece::Text(text) => body_lines(text),
            // Raw, the way `cat` reads any other file. The frame and the axes
            // belong to `wall`, which is the thing that draws it.
            Piece::Live(Live::Wall) => board.lines().map(|row| vec![Span::plain(row)]).collect(),
            Piece::Failed(ref line) => vec![line.clone()],
        };

        for mut line in body {
            if numbered {
                count += 1;
                line.insert(0, Span::new(format!("{count:>6}  "), "dim"));
            }
            lines.push(line);
        }
    }
    lines
}

fn pwd(session: &mut Session, _args: &Args<'_>) -> Output {
    line(session.prompt_path())
}

fn whoami(_session: &mut Session, _args: &Args<'_>) -> Output {
    line(crate::USER)
}

const fn exit(_session: &mut Session, _args: &Args<'_>) -> Output {
    Output::Exit
}

const fn clear(_session: &mut Session, _args: &Args<'_>) -> Output {
    Output::Clear
}

fn set_theme(_session: &mut Session, args: &Args<'_>) -> Output {
    let Some(name) = args.first() else {
        let current = theme::current();
        let mut lines: Vec<Line> = vec![vec![Span::new("palettes", "dim")], vec![]];
        lines.extend(theme::PALETTES.iter().map(|palette| {
            let marker = if palette.name == current { "*" } else { " " };
            vec![
                Span::new(format!("  {marker} {}", palette.name), "accent"),
                Span::plain(format!("  {}", palette.summary)),
            ]
        }));
        return Output::Lines(lines);
    };

    theme::apply(name).map_or_else(
        || {
            error(format!(
                "theme: unknown palette: {name}. run `theme` for the list."
            ))
        },
        |palette| line(format!("theme set to {}", palette.name)),
    )
}

/// `wall` reads the board, `wall <x> <y> <char>` writes one cell of it.
///
/// Both answer later, because both are a request to the server. The entry is
/// already in the scrollback by then and the prompt never waits.
fn graffiti(_session: &mut Session, args: &Args<'_>) -> Output {
    let write = match *args.operands() {
        [] => None,
        // Coordinates and nothing else clears the cell. The line was split on
        // whitespace before it got here, so a space cannot arrive as an
        // argument, and leaving it out is the way to ask for one.
        [x, y] => Some(format!("{x} {y}")),
        [x, y, cell] => Some(format!("{x} {y} {cell}")),
        [..] => {
            return error("wall: one character at a time. `wall <x> <y> [char]`");
        }
    };

    pending(move |sink| {
        let write = write.clone();
        leptos::task::spawn_local(async move {
            sink.set(match wall::fetch(write.as_deref()).await {
                Ok((200, board)) => board_lines(&board),
                // The server explains a refusal in one sentence, and that
                // sentence is more use than the status code it came with.
                Ok((_, complaint)) => error(format!("wall: {}", complaint.trim())),
                Err(problem) => error(problem),
            });
        });
    })
}

/// The board in a frame, with its axes outside it.
///
/// Addressed the way it is drawn: `0,0` is the bottom left corner and `y`
/// counts upwards, so it reads as the first quadrant of a graph rather than as
/// lines of a document.
///
/// The frame is drawn in the box-drawing characters the VGA ROM font actually
/// carries, the same ones a bios screen is built from, rather than in `+` and
/// `-`. Its width is where the stylesheet's 85 column measure comes from.
fn board_lines(board: &str) -> Output {
    // Indexed rather than computed, so there is no integer cast in sight.
    const DIGITS: &[u8; 10] = b"0123456789";
    let digit = |n: usize| char::from(DIGITS[n % 10]);

    let gutter = " ".repeat(GUTTER);
    let rule = "─".repeat(wall::COLS);

    let mut lines: Vec<Line> = vec![vec![Span::new(format!("{gutter}┌{rule}┐"), "dim")]];

    // The server sends the board top row first, which is already the order it
    // is drawn in; only the number beside each row is counted the other way,
    // because the bottom row is row zero.
    lines.extend(
        board
            .lines()
            .take(wall::ROWS)
            .enumerate()
            .map(|(index, row)| {
                vec![
                    Span::new(
                        format!("{:>width$} ", wall::ROWS - 1 - index, width = GUTTER - 1),
                        "dim",
                    ),
                    Span::new("│", "dim"),
                    Span::new(format!("{row:width$}", width = wall::COLS), "accent"),
                    Span::new("│", "dim"),
                ]
            }),
    );

    lines.push(vec![Span::new(format!("{gutter}└{rule}┘"), "dim")]);

    // The x axis sits under the frame, indented past the border so a column
    // lines up with the cell above it. Tens below units, so a number is read
    // upwards out of the two rows.
    let axis = format!("{gutter} ");
    let units: String = (0..wall::COLS).map(digit).collect();
    let tens: String = (0..wall::COLS)
        .map(|x| if x % 10 == 0 { digit(x / 10) } else { ' ' })
        .collect();
    lines.push(vec![Span::new(format!("{axis}{units}"), "dim")]);
    lines.push(vec![Span::new(format!("{axis}{tens}"), "dim")]);

    // Kept inside the frame's own width: this line shares the box that scrolls
    // sideways, so a longer one would make the whole board scroll to read it.
    lines.push(vec![]);
    lines.push(vec![Span::new(
        format!(
            "{}x{}, 0,0 bottom left. `wall <x> <y> <char>` sets a cell; omit char to clear.",
            wall::COLS,
            wall::ROWS
        ),
        "dim",
    )]);
    Output::Wide(lines)
}

/// Width of the row-number column, including the space after it.
const GUTTER: usize = 3;

fn date(_session: &mut Session, _args: &Args<'_>) -> Output {
    line(clock::format_utc(clock::now_millis()))
}

fn uptime(_session: &mut Session, _args: &Args<'_>) -> Output {
    line(format!("up {}", format_duration(clock::uptime_secs())))
}

/// `4 days, 2:11`, the way `uptime` renders it.
fn format_duration(total: i64) -> String {
    let days = total / 86_400;
    let hours = (total % 86_400) / 3600;
    let minutes = (total % 3600) / 60;

    let clock = format!("{hours}:{minutes:02}");
    match days {
        0 => clock,
        1 => format!("1 day, {clock}"),
        _ => format!("{days} days, {clock}"),
    }
}
