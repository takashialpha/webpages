//! Command types, dispatch, and completion.
//!
//! Commands are functions in a `&'static` registry. `help` and completion both
//! read it, so neither can fall behind what actually runs.

use std::sync::Arc;

use leptos::prelude::{RwSignal, Set as _};

use crate::args::{About, Args, Completes, Spec};
use crate::commands::COMMANDS;
use crate::fs;

/// A run of text and the class it is painted with. Empty means the default
/// foreground.
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

/// Where a command puts its output once it has some. Copy, so a task can carry
/// one into whatever callback answers.
#[derive(Clone, Copy)]
pub struct Sink(RwSignal<Output>);

impl Sink {
    pub const fn new(cell: RwSignal<Output>) -> Self {
        Self(cell)
    }

    /// Replaces whatever the entry was showing. Calling it again overwrites,
    /// which is what a command reporting progress wants.
    pub fn set(self, output: Output) {
        self.0.set(output);
    }
}

/// The work behind an [`Output::Pending`]: run once, given somewhere to put
/// the answer. Spawning is its own business.
pub type Task = Arc<dyn Fn(Sink) + Send + Sync>;

/// What a command hands back to the terminal.
#[derive(Clone)]
pub enum Output {
    Lines(Vec<Line>),
    /// Flowed into as many columns as fit, the way `ls` lays itself out. CSS
    /// decides how many, so it follows the screen rather than assuming 80.
    Columns(Vec<Line>),
    /// Too wide to wrap, so it scrolls sideways in its own box. A grid stops
    /// being a grid the moment it wraps.
    Wide(Vec<Line>),
    /// Nothing yet. The entry goes into the scrollback now and the task fills
    /// it in later, so the prompt comes back rather than waiting on a fetch.
    Pending(Task),
    /// Empties the scrollback, banner and all, the way `clear` does.
    Clear,
    /// Hands the screen to a program until it is done.
    Run(&'static crate::program::Listing),
    /// Ends the session, the way a closed ssh connection does.
    Exit,
    Nothing,
}

/// A command whose output arrives later.
pub fn pending(task: impl Fn(Sink) + Send + Sync + 'static) -> Output {
    Output::Pending(Arc::new(task))
}

/// Where the shell is, as segments below the top.
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

/// Text as lines, with bare URLs turned into links. The one place output is
/// clickable; everything else is typed.
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

/// An error, named after the shell the site is pretending to be.
pub fn error(text: impl Into<String>) -> Output {
    Output::Lines(vec![error_line(text)])
}

/// One error line, for the errors that carry a second line under them.
pub fn error_line(text: impl Into<String>) -> Line {
    vec![Span::new(format!("swagsh: {}", text.into()), "err")]
}

/// One line as plain runs and link runs.
///
/// URLs only. An email address would need a `mailto:` href, and the terminal
/// builds an anchor straight out of the text it is handed.
fn linkify(text: &str) -> Line {
    let mut finder = linkify::LinkFinder::new();
    finder.kinds(&[linkify::LinkKind::Url]);

    finder
        .spans(text)
        .map(|span| {
            if span.kind().is_some() {
                Span::new(span.as_str(), "link")
            } else {
                Span::plain(span.as_str())
            }
        })
        .collect()
}

/// Runs one line of input.
pub fn run(session: &mut Session, input: &str) -> Output {
    let mut parts = input.split_whitespace();
    let Some(name) = parts.next() else {
        return Output::Nothing;
    };
    let args: Vec<&str> = parts.collect();

    let Some(command) = COMMANDS.iter().find(|command| command.name == name) else {
        return launchable(session, name, &args);
    };

    match crate::args::parse(command.about(), &args) {
        Ok(parsed) => (command.run)(session, &parsed),
        // Rejected, or `-h` was asked for. Both are already written out.
        Err(output) => output,
    }
}

/// Works out what to do with a word that is not a builtin.
///
/// Only a path runs a program: `./bin/life` and `bin/life` do, a bare `life`
/// does not. There is no `PATH`, and inventing one would mean a name resolving
/// to something the tree never said was there.
fn launchable(session: &Session, name: &str, args: &[&str]) -> Output {
    let found = if name.contains('/') {
        match fs::resolve(&session.cwd, name).and_then(|segments| fs::node_at(&segments)) {
            Some(&fs::Node::Program(listing)) => Some(listing),
            // It is there, just not runnable. "not found" alone would send
            // someone hunting for a typo.
            Some(_) => {
                return Output::Lines(vec![
                    error_line(format!("command not found: {name}")),
                    vec![Span::new(
                        format!("{name} exists, but it is not a program."),
                        "dim",
                    )],
                ]);
            }
            None => None,
        }
    } else {
        None
    };

    let Some(listing) = found else {
        return error(format!("command not found: {name}"));
    };

    // Checked like a command's line, so `life --help` answers and `life
    // nonsense` is refused rather than quietly ignored.
    match crate::args::parse(listing.about(), args) {
        Ok(_) => Output::Run(listing),
        Err(output) => output,
    }
}

impl Command {
    #[must_use]
    pub const fn about(&self) -> About<'_> {
        About {
            name: self.name,
            summary: self.summary,
            spec: &self.spec,
        }
    }
}

/// A command: one entry in [`COMMANDS`] and one function.
pub struct Command {
    pub name: &'static str,
    /// One line, shown by `help`.
    pub summary: &'static str,
    /// What it accepts. Enforced before `run`, and read by `help` and by
    /// completion, so the three cannot disagree.
    pub spec: Spec,
    pub run: fn(&mut Session, &Args<'_>) -> Output,
}

/// The result of pressing Tab.
pub struct Completion {
    /// The input line, extended as far as is unambiguous.
    pub line: String,
    /// Every match, when there is more than one, for the shell to list.
    pub candidates: Vec<String>,
}

/// Completes the last word: a command name in the first position, a path
/// anywhere else.
pub fn complete(session: &Session, input: &str) -> Completion {
    let (head, word) = input
        .rfind(' ')
        .map_or(("", input), |space| input.split_at(space + 1));

    let candidates = if head.is_empty() {
        // Both, because running a program means typing a path to it. `bi`
        // and `bin/li` are on the way to the same thing.
        COMMANDS
            .iter()
            .map(|command| command.name.to_owned())
            .filter(|name| name.starts_with(word))
            .chain(path_candidates(session, word, Completes::Paths))
            .collect::<Vec<_>>()
    } else if word.starts_with('-') {
        flag_candidates(head, word)
    } else {
        path_candidates(session, word, wanted(head))
    };

    let Some(prefix) = common_prefix(&candidates) else {
        return Completion {
            line: input.to_owned(),
            candidates: Vec::new(),
        };
    };

    // One match is finished, so add what a shell would: a space, unless it is
    // a directory, where you carry on typing.
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

/// The options the command at the head of the line takes. Every one takes
/// `-h`.
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

/// What that command's operands can be.
fn wanted(head: &str) -> Completes {
    head.split_whitespace()
        .next()
        .and_then(|name| COMMANDS.iter().find(|command| command.name == name))
        .map_or(Completes::Paths, |command| command.spec.completes)
}

/// Completions for a partial path, relative to the working directory.
fn path_candidates(session: &Session, word: &str, wanted: Completes) -> Vec<String> {
    if wanted == Completes::Nothing {
        return Vec::new();
    }

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
        .filter(|entry| wanted != Completes::Dirs || matches!(entry.node, fs::Node::Dir(_)))
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
