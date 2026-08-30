//! Programs: draw a frame, take a key, say when to stop.
//!
//! The terminal owns the loop and calls in. It has to: WebAssembly cannot
//! suspend a synchronous call, so a guest running its own loop would freeze
//! the page until it finished.

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

/// A program, as the filesystem holds it.
///
/// There is no separate registry: programs are entries in `~/bin`, so the tree
/// is the only place that says what exists.
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

pub const SNAKE: Listing = Listing {
    name: "snake",
    summary: "eat, grow, do not bite yourself",
    spec: crate::args::Spec::NONE,
    url: "bin/snake.wasm",
};

pub const STARS: Listing = Listing {
    name: "stars",
    summary: "a drifting starfield",
    spec: crate::args::Spec::NONE,
    url: "bin/stars.wasm",
};

/// What `~/bin` holds.
pub const BIN: &[crate::fs::Entry] = &[
    crate::fs::Entry {
        name: LIFE.name,
        node: crate::fs::Node::Program(&LIFE),
    },
    crate::fs::Entry {
        name: SNAKE.name,
        node: crate::fs::Node::Program(&SNAKE),
    },
    crate::fs::Entry {
        name: STARS.name,
        node: crate::fs::Node::Program(&STARS),
    },
];

/// What opening a guest produced.
pub enum Opened {
    Draws(Box<dyn Program>),
    Printed(crate::shell::Output),
}

/// Fetches a guest and works out which shape it is.
///
/// One that exports `frame` draws, a frame at a time. One that only exports
/// `_start` runs to completion here: it takes microseconds, and what it
/// printed belongs in the scrollback.
///
/// # Errors
///
/// A message to print, if it cannot be fetched or instantiated.
#[cfg(feature = "hydrate")]
#[expect(
    clippy::future_not_send,
    reason = "the browser is single threaded and nothing here crosses a thread"
)]
pub async fn open_guest(url: &str) -> Result<Opened, String> {
    use crate::shell::{Output, Span, body_lines, error_line};
    use crate::wasi::Finished;

    let mut guest = crate::wasi::open(url).await?;
    if guest.draws() {
        return Ok(Opened::Draws(Box::new(guest)));
    }

    let (mut lines, note) = match guest.run() {
        // A program that worked says nothing about having worked.
        Finished::Exited { code, output } => (
            body_lines(&output),
            (code != 0).then(|| vec![Span::new(format!("exit {code}"), "dim")]),
        ),
        Finished::Trapped { output } => (
            body_lines(&output),
            Some(error_line("the program stopped unexpectedly")),
        ),
    };
    lines.extend(note);
    Ok(Opened::Printed(Output::Lines(lines)))
}

/// The server runs nothing. This exists so the terminal compiles there.
///
/// # Errors
///
/// Always.
#[cfg(not(feature = "hydrate"))]
#[expect(clippy::unused_async, reason = "matches the client signature")]
pub async fn open_guest(_url: &str) -> Result<Opened, String> {
    Err("no browser to run a program in".to_owned())
}

/// The trait exists so the terminal can hold a program without knowing
/// whether its build has a browser to run one.
#[cfg(feature = "hydrate")]
impl Program for crate::wasi::Guest {
    fn start(&mut self, screen: &mut Screen) {
        self.begin(screen);
    }

    fn frame(&mut self, screen: &mut Screen, elapsed: f64) -> Step {
        if self.advance(screen, elapsed) {
            Step::Done
        } else {
            Step::Running
        }
    }

    fn key(&mut self, key: &str) {
        self.press(key);
    }
}
