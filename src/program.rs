//! What it takes to be a program: draw a frame, take a key, decide when to
//! stop.
//!
//! The terminal owns the loop and calls into this, rather than a program
//! owning the loop and calling out. That is not a style choice: WebAssembly
//! cannot suspend a synchronous call, so a guest that ran its own loop would
//! block the page until it finished. Handing it one frame at a time is what
//! lets a program be interactive at all, and it is the same shape a WASI guest
//! will be driven through, so the two cannot drift.

use crate::screen::Screen;

/// What a program does with a frame.
pub enum Step {
    /// Carry on. The screen is drawn as it now stands.
    Running,
    /// Finished. The alternate screen goes away and the scrollback comes back
    /// exactly as it was.
    Done,
}

pub trait Program {
    /// Called once before the first frame, with the screen already sized.
    fn start(&mut self, _screen: &mut Screen) {}

    /// Draws one frame. `elapsed` is milliseconds since the last one, so a
    /// program moves at the same speed on any refresh rate.
    fn frame(&mut self, screen: &mut Screen, elapsed: f64) -> Step;

    /// One keypress, named the way a browser names it: a single character for
    /// an ordinary key, or a word like `ArrowLeft` or `Escape`.
    fn key(&mut self, _key: &str) {}
}

/// A program as the filesystem holds it: a name, a line about it, and a way to
/// open one.
///
/// There is no separate registry. Programs are entries in `~/bin`, so `ls`
/// lists them and completion offers them for the same reason it offers any
/// other file, and the tree is the only place that says what exists.
pub struct Listing {
    pub name: &'static str,
    pub summary: &'static str,
    pub spec: crate::args::Spec,
    /// Where the code is, relative to the page, so it does not care where the
    /// site is mounted.
    pub url: &'static str,
}

impl Listing {
    #[must_use]
    pub const fn about(&self) -> crate::args::About<'_> {
        crate::args::About {
            name: self.name,
            summary: self.summary,
            spec: &self.spec,
        }
    }
}

pub const LIFE: Listing = Listing {
    name: "life",
    summary: "conway's game of life",
    spec: crate::args::Spec::NONE,
    url: "bin/life.wasm",
};

pub const STARS: Listing = Listing {
    name: "stars",
    summary: "a drifting starfield",
    spec: crate::args::Spec::NONE,
    url: "bin/stars.wasm",
};

/// What `~/bin` holds. Adding a program is an entry here and a `Listing`.
pub const BIN: &[crate::fs::Entry] = &[
    crate::fs::Entry {
        name: LIFE.name,
        node: crate::fs::Node::Program(&LIFE),
    },
    crate::fs::Entry {
        name: STARS.name,
        node: crate::fs::Node::Program(&STARS),
    },
];

/// What opening a guest produced: something to drive, or something it printed
/// on its way out.
pub enum Opened {
    Draws(Box<dyn Program>),
    Printed(crate::shell::Output),
}

/// Fetches a guest and works out which shape it is.
///
/// A program that exports `frame` draws, and is driven a frame at a time. One
/// that only exports `_start` runs to completion here and now: it finishes in
/// microseconds, so there is nothing to yield to, and what it printed belongs
/// in the scrollback rather than on a screen of its own.
///
/// # Errors
///
/// Returns a message fit to print when the guest cannot be fetched or
/// instantiated, which includes asking for an import that is not provided.
#[cfg(feature = "hydrate")]
#[expect(
    clippy::future_not_send,
    reason = "the browser is single threaded and nothing here crosses a thread"
)]
pub async fn open_guest(url: &str) -> Result<Opened, String> {
    use crate::shell::{Output, Span, body_lines};
    use crate::wasi::Finished;

    let mut guest = crate::wasi::open(url).await?;
    if guest.draws() {
        return Ok(Opened::Draws(Box::new(Guest(guest))));
    }

    let (mut lines, note) = match guest.run() {
        // Only when it is worth saying: a program that worked says nothing
        // about having worked.
        Finished::Exited { code, output } => (
            body_lines(&output),
            (code != 0).then(|| Span::new(format!("exit {code}"), "dim")),
        ),
        Finished::Trapped { output } => (
            body_lines(&output),
            Some(Span::new("swagsh: the program stopped unexpectedly", "err")),
        ),
    };
    lines.extend(note.map(|span| vec![span]));
    Ok(Opened::Printed(Output::Lines(lines)))
}

/// The server fetches nothing and runs nothing. This exists so the terminal
/// that calls it compiles into its build.
///
/// # Errors
///
/// Always: there is no browser here to run anything in.
#[cfg(not(feature = "hydrate"))]
#[expect(clippy::unused_async, reason = "matches the client signature")]
pub async fn open_guest(_url: &str) -> Result<Opened, String> {
    Err("no browser to run a program in".to_owned())
}

/// A fetched guest, driven through the same interface a native program is, so
/// the terminal cannot tell the two apart.
#[cfg(feature = "hydrate")]
struct Guest(crate::wasi::Guest);

#[cfg(feature = "hydrate")]
impl Program for Guest {
    fn start(&mut self, screen: &mut Screen) {
        self.0.begin(screen);
    }

    fn frame(&mut self, screen: &mut Screen, elapsed: f64) -> Step {
        if self.0.frame(screen, elapsed) {
            Step::Done
        } else {
            Step::Running
        }
    }

    fn key(&mut self, key: &str) {
        self.0.press(key);
    }
}
