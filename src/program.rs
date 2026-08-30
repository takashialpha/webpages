//! Programs: draw a frame, take a key, say when to stop.
//!
//! The terminal owns the loop and calls in. It has to: WebAssembly cannot
//! suspend a synchronous call, so a program running its own loop would hold the
//! page still until it finished.

use crate::screen::Screen;

/// What a program does with a frame.
pub enum Step {
    /// Carry on. The screen is drawn as it stands.
    Running,
    /// Finished. The alternate screen goes and the scrollback comes back as it
    /// was.
    Done,
}

pub trait Program {
    /// Called once before the first frame, with the screen already sized.
    fn start(&mut self, _screen: &mut Screen) {}

    /// One frame. `elapsed` is milliseconds since the last, so a program runs
    /// at the same speed on any refresh rate.
    fn frame(&mut self, screen: &mut Screen, elapsed: f64) -> Step;

    /// One keypress, named the way a browser names it: one character for an
    /// ordinary key, or a word like `ArrowLeft`.
    fn key(&mut self, _key: &str) {}
}

/// A program, as the tree holds it. There is no separate registry: `~/bin` is
/// the only place that says what exists.
pub struct Listing {
    pub name: &'static str,
    pub summary: &'static str,
    pub spec: crate::args::Spec,
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
};

pub const SNAKE: Listing = Listing {
    name: "snake",
    summary: "eat, grow, do not bite yourself",
    spec: crate::args::Spec::NONE,
};

pub const TETRIS: Listing = Listing {
    name: "tetris",
    summary: "stack the falling blocks",
    spec: crate::args::Spec::NONE,
};

pub const STARS: Listing = Listing {
    name: "stars",
    summary: "a drifting starfield",
    spec: crate::args::Spec::NONE,
};

/// The manifest `just guests` writes beside the programs: one `name: hash`
/// line each.
///
/// Read once at startup and stamped onto the document, which is how the
/// browser gets it. The names cannot be baked in: the site has to compile
/// without the programs having been built, or `just check` would have to build
/// them first. Same trick as `data-uname`.
///
/// Empty when there is no manifest, which leaves [`url`] naming them plainly.
#[cfg(not(feature = "hydrate"))]
#[must_use]
pub fn manifest(site_root: &str) -> String {
    use std::sync::OnceLock;

    static MANIFEST: OnceLock<String> = OnceLock::new();
    MANIFEST
        .get_or_init(|| {
            std::fs::read_to_string(std::path::Path::new(site_root).join("bin").join("hash.txt"))
                .unwrap_or_default()
        })
        .clone()
}

/// Reads back what the server stamped, so the browser asks for the same names.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn manifest(_site_root: &str) -> String {
    leptos::prelude::document()
        .document_element()
        .and_then(|html| html.get_attribute("data-programs"))
        .unwrap_or_default()
}

/// Where a program's code is, relative to the page, with its content hash in
/// the name.
///
/// Falls back to the plain name when there is no manifest, which is a build
/// that skipped `just guests`. That 404s when the program is run, and says so.
#[must_use]
pub fn url(listing: &Listing) -> String {
    manifest("")
        .lines()
        .find_map(|line| {
            let hash = line.strip_prefix(listing.name)?.strip_prefix(": ")?;
            Some(format!("bin/{}.{}.wasm", listing.name, hash.trim()))
        })
        .unwrap_or_else(|| format!("bin/{}.wasm", listing.name))
}

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
        name: TETRIS.name,
        node: crate::fs::Node::Program(&TETRIS),
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
/// `_start` runs to the end here: it takes microseconds, and what it printed
/// belongs in the scrollback.
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
        // Nothing to add when it worked.
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

/// The trait is what lets the terminal hold a program without knowing whether
/// its build has a browser to run one in.
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
