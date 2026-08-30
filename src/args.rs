//! Option parsing, shared by every command.
//!
//! A command declares what it takes in a [`Spec`] and [`parse`] enforces it
//! first, so no command parses its own line and none can quietly ignore what it
//! was given. `help` and completion read the same spec, so the usage text comes
//! from what actually runs.

use crate::shell::{Line, Output, Span, error_line};

/// What it takes to check a line and explain it.
///
/// Commands and programs are different things typed the same way, so they are
/// refused and explained the same way.
#[derive(Clone, Copy)]
pub struct About<'a> {
    pub name: &'a str,
    pub summary: &'a str,
    pub spec: &'a Spec,
}

/// One option. They are all on or off: nothing takes a value, so there is no
/// `--name=value` to parse.
pub struct Flag {
    pub short: char,
    /// `None` when there is no long form, the way `ls -1` has none.
    pub long: Option<&'static str>,
    /// One line, shown by `help <command>`.
    pub help: &'static str,
}

/// What Tab offers after a command: `cd` takes only directories, `pwd` takes
/// nothing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Completes {
    Paths,
    Dirs,
    Nothing,
}

/// What a command accepts: its options, and how many operands.
pub struct Spec {
    pub flags: &'static [Flag],
    /// Fewest operands accepted.
    pub min: usize,
    /// Most operands accepted, or `None` for any number.
    pub max: Option<usize>,
    /// What one operand is called, in usage text and in errors.
    pub operand: &'static str,
    /// What its operands can be, which is what Tab offers.
    pub completes: Completes,
}

impl Spec {
    /// A command that takes no options and no operands.
    pub const NONE: Self = Self {
        flags: &[],
        min: 0,
        max: Some(0),
        operand: "",
        completes: Completes::Nothing,
    };

    /// A command that takes no options and one optional directory.
    pub const fn dir(operand: &'static str) -> Self {
        Self {
            flags: &[],
            min: 0,
            max: Some(1),
            operand,
            completes: Completes::Dirs,
        }
    }
}

/// A checked command line: the options given, and the operands left over.
pub struct Args<'a> {
    /// Every option seen, by its short form, whichever form was typed.
    shorts: Vec<char>,
    operands: Vec<&'a str>,
}

impl<'a> Args<'a> {
    /// Whether an option was given, asked for by its short form even when the
    /// long one was typed.
    pub fn has(&self, short: char) -> bool {
        self.shorts.contains(&short)
    }

    pub fn operands(&self) -> &[&'a str] {
        &self.operands
    }

    /// The first operand, for commands that take at most one.
    pub fn first(&self) -> Option<&'a str> {
        self.operands.first().copied()
    }
}

/// Parses one command's arguments against its spec.
///
/// Options may be clustered, so `-a1` is `-a -1`. `--` ends them, and a lone
/// `-` is an operand, which is what lets `cd -` name a directory.
///
/// # Errors
///
/// What to print instead of running: usage for `-h`, an error otherwise.
pub fn parse<'a>(about: About<'_>, argv: &[&'a str]) -> Result<Args<'a>, Output> {
    let spec = about.spec;
    let mut shorts = Vec::new();
    let mut operands = Vec::new();
    let mut ended = false;

    for arg in argv {
        if ended || *arg == "-" {
            operands.push(*arg);
        } else if *arg == "--" {
            ended = true;
        } else if let Some(long) = arg.strip_prefix("--") {
            if long == "help" {
                return Err(usage(about));
            }
            let Some(flag) = spec.flags.iter().find(|flag| flag.long == Some(long)) else {
                return Err(fail(about, &format!("unrecognized option '--{long}'")));
            };
            shorts.push(flag.short);
        } else if let Some(cluster) = arg.strip_prefix('-') {
            for short in cluster.chars() {
                if short == 'h' {
                    return Err(usage(about));
                }
                let Some(flag) = spec.flags.iter().find(|flag| flag.short == short) else {
                    return Err(fail(about, &format!("invalid option -- '{short}'")));
                };
                shorts.push(flag.short);
            }
        } else {
            operands.push(*arg);
        }
    }

    if operands.len() < spec.min {
        return Err(fail(about, "missing operand"));
    }
    if spec.max.is_some_and(|max| operands.len() > max) {
        return Err(fail(about, "too many arguments"));
    }

    Ok(Args { shorts, operands })
}

/// The usage line, built from the spec.
pub fn usage_line(about: About<'_>) -> String {
    let spec = about.spec;

    let flags = if spec.flags.is_empty() {
        String::new()
    } else {
        let shorts: String = spec.flags.iter().map(|flag| flag.short).collect();
        format!(" [-{shorts}]")
    };

    // Square brackets optional, angle required, an ellipsis for any number,
    // the way a man page spells it.
    let operands = match (spec.min, spec.max) {
        (_, Some(0)) => String::new(),
        (0, Some(_)) => format!(" [{}]", spec.operand),
        (0, None) => format!(" [{}...]", spec.operand),
        (_, Some(_)) => format!(" <{}>", spec.operand),
        (_, None) => format!(" <{}...>", spec.operand),
    };

    format!("usage: {}{flags}{operands}", about.name)
}

/// What `-h` and `help <command>` both print.
pub fn usage(about: About<'_>) -> Output {
    let mut lines: Vec<Line> = vec![
        vec![Span::plain(usage_line(about))],
        vec![],
        vec![Span::plain(format!("  {}", about.summary))],
    ];

    if !about.spec.flags.is_empty() {
        let names: Vec<String> = about.spec.flags.iter().map(flag_names).collect();
        let width = names.iter().map(String::len).max().unwrap_or(0);

        lines.push(vec![]);
        lines.push(vec![Span::new("options", "dim")]);
        lines.push(vec![]);
        lines.extend(about.spec.flags.iter().zip(names).map(|(flag, names)| {
            vec![
                Span::new(format!("  {names:width$}"), "accent"),
                Span::plain(format!("  {}", flag.help)),
            ]
        }));
    }

    Output::Lines(lines)
}

/// `-a, --all`, or just `-1` when there is no long form.
pub fn flag_names(flag: &Flag) -> String {
    flag.long.map_or_else(
        || format!("-{}", flag.short),
        |long| format!("-{}, --{long}", flag.short),
    )
}

/// An error naming the command, with the way out underneath.
fn fail(about: About<'_>, message: &str) -> Output {
    Output::Lines(vec![
        error_line(format!("{}: {message}", about.name)),
        vec![Span::new(
            format!("try `{} --help` for more information.", about.name),
            "dim",
        )],
    ])
}
