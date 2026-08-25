//! The command set, and the registry `help` and tab completion are built from.
//!
//! Adding a command is one entry in [`COMMANDS`] and one function. Nothing else
//! needs touching: `help` lists whatever is here, and completion offers it.

use crate::fs::{self, Node};
use crate::shell::{Command, Line, Output, Session, Span, body, error, line};
use crate::theme;

pub const COMMANDS: &[Command] = &[
    Command {
        name: "help",
        summary: "list the commands",
        usage: "help [command]",
        run: help,
    },
    Command {
        name: "ls",
        summary: "list directory contents",
        usage: "ls [path]",
        run: ls,
    },
    Command {
        name: "cd",
        summary: "change directory",
        usage: "cd [path]",
        run: cd,
    },
    Command {
        name: "cat",
        summary: "print a file",
        usage: "cat <file>",
        run: cat,
    },
    Command {
        name: "pwd",
        summary: "print the working directory",
        usage: "pwd",
        run: pwd,
    },
    Command {
        name: "whoami",
        summary: "print the current user",
        usage: "whoami",
        run: whoami,
    },
    Command {
        name: "date",
        summary: "print the current utc time",
        usage: "date",
        run: date,
    },
    Command {
        name: "uptime",
        summary: "how long this server has been up",
        usage: "uptime",
        run: uptime,
    },
    Command {
        name: "theme",
        summary: "switch the palette",
        usage: "theme [name]",
        run: set_theme,
    },
    Command {
        name: "exit",
        summary: "close the connection",
        usage: "exit",
        run: exit,
    },
    Command {
        name: "clear",
        summary: "clear the screen",
        usage: "clear",
        run: clear,
    },
];

fn help(_session: &mut Session, args: &[&str]) -> Output {
    if let Some(name) = args.first() {
        return COMMANDS
            .iter()
            .find(|command| command.name == *name)
            .map_or_else(
                || error(format!("help: no such command: {name}")),
                |command| {
                    Output::Lines(vec![
                        vec![Span::plain(format!("usage: {}", command.usage))],
                        vec![],
                        vec![Span::plain(format!("  {}", command.summary))],
                    ])
                },
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
        "everything else is a file. run `ls` to look around.",
        "dim",
    )]);
    Output::Lines(lines)
}

fn ls(session: &mut Session, args: &[&str]) -> Output {
    let target = args.first().copied().unwrap_or(".");
    let Some(segments) = fs::resolve(&session.cwd, target) else {
        return error(format!("ls: {target}: No such file or directory"));
    };
    let Some(node) = fs::node_at(&segments) else {
        return error(format!("ls: {target}: No such file or directory"));
    };

    match *node {
        // Listing a file names the file, the way `ls` itself does.
        Node::File(_) => line(target.to_owned()),
        Node::Dir(entries) => Output::Columns(
            entries
                .iter()
                .map(|entry| match entry.node {
                    Node::Dir(_) => vec![Span::new(format!("{}/", entry.name), "dir")],
                    Node::File(_) => vec![Span::plain(entry.name)],
                })
                .collect(),
        ),
    }
}

fn cd(session: &mut Session, args: &[&str]) -> Output {
    let target = args.first().copied().unwrap_or("~");
    let Some(segments) = fs::resolve(&session.cwd, target) else {
        return error(format!("cd: {target}: No such file or directory"));
    };
    match fs::node_at(&segments) {
        Some(Node::Dir(_)) => {
            session.cwd = segments;
            Output::Nothing
        }
        Some(Node::File(_)) => error(format!("cd: {target}: Not a directory")),
        None => error(format!("cd: {target}: No such file or directory")),
    }
}

fn cat(session: &mut Session, args: &[&str]) -> Output {
    let Some(target) = args.first().copied() else {
        return error("cat: missing operand");
    };
    let Some(segments) = fs::resolve(&session.cwd, target) else {
        return error(format!("cat: {target}: No such file or directory"));
    };
    match fs::node_at(&segments) {
        Some(Node::File(text)) => body(text),
        Some(Node::Dir(_)) => error(format!("cat: {target}: Is a directory")),
        None => error(format!("cat: {target}: No such file or directory")),
    }
}

fn pwd(session: &mut Session, _args: &[&str]) -> Output {
    line(session.prompt_path())
}

fn whoami(_session: &mut Session, _args: &[&str]) -> Output {
    line(crate::USER)
}

const fn exit(_session: &mut Session, _args: &[&str]) -> Output {
    Output::Exit
}

const fn clear(_session: &mut Session, _args: &[&str]) -> Output {
    Output::Clear
}

fn set_theme(_session: &mut Session, args: &[&str]) -> Output {
    let Some(name) = args.first().copied() else {
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

fn date(_session: &mut Session, _args: &[&str]) -> Output {
    line(now_utc())
}

fn uptime(_session: &mut Session, _args: &[&str]) -> Output {
    line(format!("up {}", format_duration(uptime_secs())))
}

/// `4 days, 2:11`, the way `uptime` renders it.
fn format_duration(total: u64) -> String {
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

/// Current UTC, formatted the way `date -u` prints it.
#[cfg(feature = "hydrate")]
fn now_utc() -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let now = js_sys::Date::new_0();
    let day = DAYS
        .get(now.get_utc_day() as usize)
        .copied()
        .unwrap_or("???");
    let month = MONTHS
        .get(now.get_utc_month() as usize)
        .copied()
        .unwrap_or("???");

    format!(
        "{day} {month} {:2} {:02}:{:02}:{:02} UTC {}",
        now.get_utc_date(),
        now.get_utc_hours(),
        now.get_utc_minutes(),
        now.get_utc_seconds(),
        now.get_utc_full_year(),
    )
}

/// Commands only ever run in the browser, so this branch exists to keep the
/// server build compiling and is never reached.
#[cfg(not(feature = "hydrate"))]
fn now_utc() -> String {
    "unavailable".to_owned()
}

/// The server's uptime when the page was rendered, plus however long the page
/// has been open. Avoids a request, and stays correct as the tab sits there.
#[cfg(feature = "hydrate")]
fn uptime_secs() -> u64 {
    let at_render: f64 = leptos::prelude::document()
        .document_element()
        .and_then(|html| html.get_attribute("data-uptime"))
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();

    let since_load = (js_sys::Date::now() - crate::loaded_at()) / 1000.0;
    let seconds = (at_render + since_load).max(0.0);

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped non-negative, and no process runs for 10^19 seconds"
    )]
    {
        seconds as u64
    }
}

#[cfg(not(feature = "hydrate"))]
fn uptime_secs() -> u64 {
    crate::server_uptime_secs()
}
