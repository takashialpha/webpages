//! The commands that walk the tree: `ls`, `cd`, `cat`, `pwd`.

use crate::args::Args;
use crate::fs::{self, Live, Node};
use crate::shell::{Line, Output, Session, Span, body_lines, error, error_line, line, pending};
use crate::wall;

pub fn ls(session: &mut Session, args: &Args<'_>) -> Output {
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
            // Naming a file lists the file, the way `ls` does. A live file is
            // still a file; reading it is `cat`'s job.
            Some(Node::File(_) | Node::Live(_) | Node::Program(_)) => {
                files.push(vec![Span::plain(*target)]);
            }
            Some(node) => dirs.push((target, node)),
        }
    }

    // The plain case keeps the grid: one directory, in columns.
    if missing.is_empty()
        && files.is_empty()
        && !args.has('1')
        && let [(_, node)] = dirs[..]
    {
        return Output::Columns(entry_lines(node, args.has('a')));
    }

    // Anything else is several blocks, and blocks need headers between them,
    // which one flowed grid cannot do.
    let mut lines = missing;
    lines.extend(files);

    // Only worth a header when there is more than one thing to tell apart.
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
        // Coloured the way `ls` colours anything you run rather than read.
        Node::Program(_) => vec![Span::new(entry.name, "accent")],
        Node::File(_) | Node::Live(_) => vec![Span::plain(entry.name)],
    }));
    lines
}

pub fn cd(session: &mut Session, args: &Args<'_>) -> Output {
    let target = args.first().unwrap_or("~");

    // `cd -` goes back and says where, the way bash does. It is why a lone
    // dash parses as an operand.
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

/// One operand of a `cat`, looked up but not yet printed.
///
/// A plan, because a live file has to be fetched and everything after it still
/// has to come out in order.
enum Piece {
    Text(&'static str),
    Live(Live),
    Failed(Line),
}

pub fn cat(session: &mut Session, args: &Args<'_>) -> Output {
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
                // Nothing to print. What it is instead is `ls`'s business.
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

    // Something has to be fetched, so the whole thing answers later. The board
    // is the only live file, so one request covers every mention of it.
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
    /// Cheap to copy: the text is `'static` and a failure is one line.
    fn clone_ref(&self) -> Self {
        match *self {
            Self::Text(text) => Self::Text(text),
            Self::Live(live) => Self::Live(live),
            Self::Failed(ref line) => Self::Failed(line.clone()),
        }
    }
}

/// Renders a plan. `-n` numbers across the whole thing, not per file.
fn spell(plan: &[Piece], board: &str, numbered: bool) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut count = 0_usize;

    for piece in plan {
        let body = match *piece {
            Piece::Text(text) => body_lines(text),
            // Raw, the way `cat` reads anything. The frame and the axes
            // belong to `wall`, which is what draws it.
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

pub fn pwd(session: &mut Session, _args: &Args<'_>) -> Output {
    line(session.prompt_path())
}
