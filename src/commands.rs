//! The command set.
//!
//! Adding one is an entry in [`COMMANDS`] and a function. `help` lists it,
//! completion offers it, and its [`Spec`] is checked first, so a command only
//! ever sees arguments it declared.
//!
//! The registry is here so it reads as one list. The functions are grouped
//! next door by what they touch: the tree, the box itself, or the board.

mod board;
mod files;
mod system;

use crate::args::{Args, Completes, Flag, Spec, usage};
use crate::shell::{Command, Line, Output, Session, Span, error};

use board::graffiti;
use files::{cat, cd, ls, pwd};
use system::{clear, date, exit, set_theme, uptime, whoami};

/// No `-l`: its columns are mode, owner, group and mtime, and this tree has
/// none of them.
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

    // Characters, not bytes: this is a column to pad to, and `format!` counts
    // the same way. They agree on the names there are, and would stop agreeing
    // on the first one that is not ascii.
    let width = COMMANDS
        .iter()
        .map(|command| command.name.chars().count())
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
