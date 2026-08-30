//! What a command is, and what running a line does.
//!
//! Commands are functions in a `&'static` registry. `help` and completion both
//! read it, so neither can fall behind what actually runs. Tab is its own
//! module; everything else about a line is here.

use std::sync::Arc;

use leptos::prelude::{RwSignal, Set as _};

mod complete;

pub use complete::{Completion, complete};

use crate::args::{About, Args, Spec};
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
///
/// An email address is not made clickable. That would mean a `mailto:` href,
/// and the terminal builds an anchor straight out of the text it is handed.
pub fn body_lines(text: &str) -> Vec<Line> {
    let mut finder = linkify::LinkFinder::new();
    finder.kinds(&[linkify::LinkKind::Url]);

    text.trim_end()
        .lines()
        .map(|line| {
            finder
                .spans(line)
                .map(|span| {
                    if span.kind().is_some() {
                        Span::new(span.as_str(), "link")
                    } else {
                        Span::plain(span.as_str())
                    }
                })
                .collect()
        })
        .collect()
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

    // Checked like a command's line, so `bin/life --help` answers and
    // `bin/life nonsense` is refused rather than quietly ignored.
    //
    // Named by the path that was typed rather than by the file, because the
    // usage line and the `try ... --help` under an error both use the name,
    // and a bare `life` is not something you can run.
    let about = About {
        name,
        ..listing.about()
    };
    match crate::args::parse(about, args) {
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
