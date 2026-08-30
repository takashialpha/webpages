//! What the box says about itself: who you are, what time it is, how long it
//! has been up, and what it looks like.

use crate::args::Args;
use crate::clock;
use crate::shell::{Line, Output, Session, Span, error, line};
use crate::theme;

pub fn whoami(_session: &mut Session, _args: &Args<'_>) -> Output {
    line(crate::USER)
}

pub const fn exit(_session: &mut Session, _args: &Args<'_>) -> Output {
    Output::Exit
}

pub const fn clear(_session: &mut Session, _args: &Args<'_>) -> Output {
    Output::Clear
}

pub fn set_theme(_session: &mut Session, args: &Args<'_>) -> Output {
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

pub fn date(_session: &mut Session, _args: &Args<'_>) -> Output {
    line(clock::format_utc(clock::now_millis()))
}

pub fn uptime(_session: &mut Session, _args: &Args<'_>) -> Output {
    line(format!("up {}", format_duration(clock::uptime_secs())))
}

/// `4 days, 2:11`, the way `uptime` says it.
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
