//! The command set, and the registry `help` and tab completion are built from.
//!
//! Adding a command is one entry in [`COMMANDS`] and one function. Nothing else
//! needs touching: `help` lists whatever is here, completion offers it, and the
//! [`Spec`] on the entry is checked before the function runs, so a command body
//! only ever sees arguments it declared.

use crate::args::{Args, Flag, Spec, usage};
use crate::clock;
use crate::fs::{self, Node};
use crate::shell::{Command, Line, Output, Session, Span, body_lines, error, error_line, line};
use crate::theme;

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
        spec: Spec::one("command"),
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
        },
        run: ls,
    },
    Command {
        name: "cd",
        summary: "change directory",
        spec: Spec::one("path"),
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
        name: "theme",
        summary: "switch the palette",
        spec: Spec::one("name"),
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
            .map_or_else(|| error(format!("help: no such command: {name}")), usage);
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
        "everything else is a file. run `ls` to look around.",
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
            // Naming a file lists the file, the way `ls` itself does.
            Some(Node::File(_)) => files.push(vec![Span::plain(*target)]),
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
        Node::File(_) => vec![Span::plain(entry.name)],
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
        Some(Node::File(_)) => error(format!("cd: {target}: Not a directory")),
        None => error(format!("cd: {target}: No such file or directory")),
    }
}

fn cat(session: &mut Session, args: &Args<'_>) -> Output {
    let mut lines: Vec<Line> = Vec::new();
    // Numbering runs across the whole output rather than restarting per file,
    // which is what `cat -n` on several files does.
    let mut numbered = 0_usize;

    for target in args.operands() {
        match fs::resolve(&session.cwd, target).and_then(|segments| fs::node_at(&segments)) {
            Some(Node::File(text)) => {
                for mut line in body_lines(text) {
                    if args.has('n') {
                        numbered += 1;
                        line.insert(0, Span::new(format!("{numbered:>6}  "), "dim"));
                    }
                    lines.push(line);
                }
            }
            Some(Node::Dir(_)) => lines.push(error_line(format!("cat: {target}: Is a directory"))),
            None => lines.push(error_line(format!(
                "cat: {target}: No such file or directory"
            ))),
        }
    }

    Output::Lines(lines)
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
