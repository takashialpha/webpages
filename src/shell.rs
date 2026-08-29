//! Command types, dispatch, and completion.
//!
//! Commands are plain functions in a `&'static` registry, so `help` and tab
//! completion are both generated from the same slice and cannot fall out of step
//! with what actually runs.

use crate::args::{Args, Spec};
use crate::commands::COMMANDS;
use crate::fs;

/// A run of text and the CSS class the terminal paints it with. An empty class
/// means the terminal's default foreground.
#[derive(Clone)]
pub struct Span {
    pub text: String,
    pub class: &'static str,
}

impl Span {
    pub fn new(text: impl Into<String>, class: &'static str) -> Self {
        Self {
            text: text.into(),
            class,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, "")
    }
}

/// One line of output.
pub type Line = Vec<Span>;

/// What a command hands back to the terminal.
#[derive(Clone)]
pub enum Output {
    Lines(Vec<Line>),
    /// Entries flowed into as many columns as the width allows, the way `ls`
    /// lays itself out. The count is left to CSS so it adapts to the viewport
    /// instead of assuming 80 columns.
    Columns(Vec<Line>),
    /// Empties the scrollback, banner and all, the way `clear` does.
    Clear,
    /// Ends the session: the shell prints its farewell and stops taking input,
    /// the way a closed ssh connection does.
    Exit,
    Nothing,
}

/// Where the shell currently is, as segments below the root.
#[derive(Clone)]
pub struct Session {
    pub cwd: Vec<&'static str>,
    /// Where it was before the last `cd`, which is where `cd -` goes back to.
    pub prev: Vec<&'static str>,
}

impl Session {
    pub const fn new() -> Self {
        Self {
            cwd: Vec::new(),
            prev: Vec::new(),
        }
    }

    /// The working directory as the prompt shows it.
    pub fn prompt_path(&self) -> String {
        fs::display_path(&self.cwd)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// Splits body text into output lines, turning bare URLs into link spans.
///
/// Terminal emulators make URLs clickable, so this is the one place output
/// stops being inert text. Everything else on the page is typed, not clicked.
pub fn body(text: &str) -> Output {
    Output::Lines(body_lines(text))
}

/// The lines [`body`] would print, for a command that decorates them first.
pub fn body_lines(text: &str) -> Vec<Line> {
    text.trim_end().lines().map(linkify).collect()
}

/// A single unstyled line.
pub fn line(text: impl Into<String>) -> Output {
    Output::Lines(vec![vec![Span::plain(text)]])
}

/// A shell-style error, named after the shell the site is pretending to be.
pub fn error(text: impl Into<String>) -> Output {
    Output::Lines(vec![error_line(text)])
}

/// One error line, for the errors that carry a second line under them.
pub fn error_line(text: impl Into<String>) -> Line {
    vec![Span::new(format!("swagsh: {}", text.into()), "err")]
}

/// Finds bare `http://` and `https://` runs and splits them into link spans.
fn linkify(text: &str) -> Line {
    let mut spans = Vec::new();
    let mut rest = text;

    loop {
        // Earliest of the two schemes, since `https://` never contains `http://`.
        let start = match (rest.find("https://"), rest.find("http://")) {
            (Some(secure), Some(plain)) => secure.min(plain),
            (Some(only), None) | (None, Some(only)) => only,
            (None, None) => break,
        };

        let tail = &rest[start..];
        let mut end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        // Sentence punctuation is not part of the URL.
        while end > 0 && matches!(tail.as_bytes().get(end - 1), Some(b'.' | b',' | b')')) {
            end -= 1;
        }

        if start > 0 {
            spans.push(Span::plain(&rest[..start]));
        }
        spans.push(Span::new(&tail[..end], "link"));
        rest = &tail[end..];
    }

    if !rest.is_empty() {
        spans.push(Span::plain(rest));
    }
    spans
}

/// Runs one line of input.
pub fn run(session: &mut Session, input: &str) -> Output {
    let mut parts = input.split_whitespace();
    let Some(name) = parts.next() else {
        return Output::Nothing;
    };
    let args: Vec<&str> = parts.collect();

    let Some(command) = COMMANDS.iter().find(|command| command.name == name) else {
        return error(format!("command not found: {name}"));
    };

    match crate::args::parse(command, &args) {
        Ok(parsed) => (command.run)(session, &parsed),
        // Either the line was rejected or `-h` was asked for; both are already
        // formatted, so there is nothing left to decide here.
        Err(output) => output,
    }
}

/// A command: one entry in [`COMMANDS`] and one function.
pub struct Command {
    pub name: &'static str,
    /// One line, shown by `help`.
    pub summary: &'static str,
    /// What the command accepts. Enforced before `run` is called, and read by
    /// `help` and tab completion, so the three cannot disagree.
    pub spec: Spec,
    pub run: fn(&mut Session, &Args<'_>) -> Output,
}

/// The result of pressing Tab.
pub struct Completion {
    /// The input line, extended as far as is unambiguous.
    pub line: String,
    /// Every match, when more than one exists, for the shell to list.
    pub candidates: Vec<String>,
}

/// Completes the last word of `input`: a command name in the first position, a
/// path in any other.
pub fn complete(session: &Session, input: &str) -> Completion {
    let (head, word) = input
        .rfind(' ')
        .map_or(("", input), |space| input.split_at(space + 1));

    let candidates = if head.is_empty() {
        COMMANDS
            .iter()
            .map(|command| command.name.to_owned())
            .filter(|name| name.starts_with(word))
            .collect::<Vec<_>>()
    } else if word.starts_with('-') {
        flag_candidates(head, word)
    } else {
        path_candidates(session, word)
    };

    let Some(prefix) = common_prefix(&candidates) else {
        return Completion {
            line: input.to_owned(),
            candidates: Vec::new(),
        };
    };

    // A lone match is finished, so add the separator a shell would: a space,
    // unless it is a directory, where you carry on typing the path.
    let completed = if candidates.len() == 1 {
        let sole = prefix.as_str();
        if sole.ends_with('/') {
            format!("{head}{sole}")
        } else {
            format!("{head}{sole} ")
        }
    } else {
        format!("{head}{prefix}")
    };

    Completion {
        line: completed,
        candidates: if candidates.len() > 1 {
            candidates
        } else {
            Vec::new()
        },
    }
}

/// Completions for a partial option, read from the spec of whichever command
/// the line starts with. Every command takes `-h`, so that is offered too.
fn flag_candidates(head: &str, word: &str) -> Vec<String> {
    let Some(name) = head.split_whitespace().next() else {
        return Vec::new();
    };
    let Some(command) = COMMANDS.iter().find(|command| command.name == name) else {
        return Vec::new();
    };

    let mut candidates = vec!["-h".to_owned(), "--help".to_owned()];
    for flag in command.spec.flags {
        candidates.push(format!("-{}", flag.short));
        if let Some(long) = flag.long {
            candidates.push(format!("--{long}"));
        }
    }
    candidates.retain(|candidate| candidate.starts_with(word));
    candidates
}

/// Completions for a partial path, relative to the working directory.
fn path_candidates(session: &Session, word: &str) -> Vec<String> {
    let (dir, stem) = word
        .rfind('/')
        .map_or(("", word), |slash| word.split_at(slash + 1));

    let Some(segments) = fs::resolve(&session.cwd, dir) else {
        return Vec::new();
    };
    let Some(node) = fs::node_at(&segments) else {
        return Vec::new();
    };

    node.entries()
        .iter()
        .filter(|entry| entry.name.starts_with(stem))
        .map(|entry| {
            let suffix = if matches!(entry.node, fs::Node::Dir(_)) {
                "/"
            } else {
                ""
            };
            format!("{dir}{}{suffix}", entry.name)
        })
        .collect()
}

/// The longest prefix shared by every candidate.
fn common_prefix(candidates: &[String]) -> Option<String> {
    let first = candidates.first()?;
    let mut end = first.len();

    for other in &candidates[1..] {
        end = first
            .char_indices()
            .zip(other.char_indices())
            .take_while(|((_, a), (_, b))| a == b)
            .map(|((index, a), _)| index + a.len_utf8())
            .last()
            .unwrap_or(0)
            .min(end);
    }
    first.get(..end).map(ToOwned::to_owned)
}
