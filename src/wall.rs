//! The graffiti board: a shared grid many people write to.
//!
//! Bounded on purpose. There is no free-form text anywhere: a write is a run of
//! printable ASCII at a coordinate, clipped to its row, checked at every edge.
//! The worst anyone can do is spell something across the grid that the next
//! visitor writes over. It is stored as plain text of exactly these dimensions,
//! which means moderating it is opening the file in an editor.

/// Eighty by twenty-four, which is what a terminal has always been.
///
/// It does not fit a phone, and is not meant to: the `wall` command puts the
/// board in a box that scrolls sideways, the way any wide output has to.
pub const COLS: usize = 80;
pub const ROWS: usize = 24;

/// An unwritten cell.
pub const BLANK: u8 = b' ';

/// Printable ASCII, and nothing else. Checked at every edge the grid has, so a
/// control character can never reach the file, the terminal, or anyone else.
#[must_use]
pub const fn printable(byte: u8) -> bool {
    byte >= 0x20 && byte <= 0x7e
}

/// The board itself. Fixed size, so it cannot grow whatever arrives.
#[derive(Clone)]
pub struct Grid([[u8; COLS]; ROWS]);

impl Grid {
    #[must_use]
    pub const fn blank() -> Self {
        Self([[BLANK; COLS]; ROWS])
    }

    /// Sets one cell, addressed the way the board is drawn: `0,0` is the bottom
    /// left and `y` counts upwards. Returns whether anything changed.
    ///
    /// Storage runs the other way, top row first, so that the file reads in the
    /// same order as the board is drawn and editing it by hand needs no mental
    /// arithmetic. This is the one place the two meet.
    pub const fn set(&mut self, x: usize, y: usize, byte: u8) -> bool {
        if x >= COLS || y >= ROWS || !printable(byte) {
            return false;
        }
        let row = ROWS - 1 - y;
        if self.0[row][x] == byte {
            return false;
        }
        self.0[row][x] = byte;
        true
    }

    /// Reads back what is stored, one string per row.
    #[must_use]
    pub fn rows(&self) -> Vec<String> {
        self.0
            .iter()
            .map(|row| row.iter().map(|&byte| char::from(byte)).collect())
            .collect()
    }

    /// The stored form: exactly [`ROWS`] lines of exactly [`COLS`] characters.
    #[must_use]
    pub fn render(&self) -> String {
        let mut text = self.rows().join("\n");
        text.push('\n');
        text
    }

    /// Reads the stored form back, forgivingly. A short line is padded and a
    /// long one is cut, so hand-editing the file cannot put the board into a
    /// shape the rest of the code does not expect.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut grid = Self::blank();
        for (y, line) in text.lines().take(ROWS).enumerate() {
            for (x, byte) in line.bytes().take(COLS).enumerate() {
                if printable(byte) {
                    grid.0[y][x] = byte;
                }
            }
        }
        grid
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::blank()
    }
}

/// Cells one address may set per [`WINDOW`]. A row's worth, so a word is quick
/// and repainting the board is not.
#[cfg(feature = "ssr")]
pub const BUDGET: u32 = 80;

/// Cells everyone together may write per [`WINDOW`].
///
/// The per-address budget is the fair share, and this is the ceiling. It exists
/// because the address is read from a header: behind Cloudflare that is
/// trustworthy, but anything reaching the origin directly could claim a new one
/// for every request and never meet a per-address limit at all.
#[cfg(feature = "ssr")]
pub const CEILING: u32 = 600;

#[cfg(feature = "ssr")]
pub const WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

/// How many addresses are tracked at once.
///
/// The key comes from a header, so an attacker choosing it must not be able to
/// choose how much is remembered. At the cap the oldest is dropped, which costs
/// that address its history rather than costing everyone the service.
#[cfg(feature = "ssr")]
const TRACKED: usize = 4096;

/// One address's writes inside the current window. Fixed size, unlike a list of
/// timestamps, so the cost of remembering someone does not depend on them.
#[cfg(feature = "ssr")]
#[derive(Clone, Copy)]
struct Window {
    start: std::time::Instant,
    cells: u32,
}

#[cfg(feature = "ssr")]
impl Window {
    const fn opened(now: std::time::Instant) -> Self {
        Self {
            start: now,
            cells: 0,
        }
    }

    /// Rolls over once the window has elapsed, so a count never carries across.
    fn current(self, now: std::time::Instant) -> Self {
        if now.duration_since(self.start) >= WINDOW {
            Self::opened(now)
        } else {
            self
        }
    }
}

#[cfg(feature = "ssr")]
struct Limiter {
    seen: std::collections::HashMap<String, Window>,
    everyone: Window,
}

#[cfg(feature = "ssr")]
impl Limiter {
    fn new() -> Self {
        Self {
            seen: std::collections::HashMap::new(),
            everyone: Window::opened(std::time::Instant::now()),
        }
    }

    /// Whether `who` may write `cells` more, counting them if so.
    ///
    /// All or nothing: a write that would cross either limit is refused whole
    /// rather than truncated, so nobody has to wonder which half landed.
    fn take(&mut self, who: &str, cells: u32) -> bool {
        let now = std::time::Instant::now();

        self.everyone = self.everyone.current(now);
        if self.everyone.cells.saturating_add(cells) > CEILING {
            return false;
        }

        // Only when it is worth doing, rather than scanning every address on
        // every write, which would make each request cost what the table has
        // grown to.
        if self.seen.len() >= TRACKED {
            self.seen
                .retain(|_, window| now.duration_since(window.start) < WINDOW);
            while self.seen.len() >= TRACKED {
                let Some(oldest) = self
                    .seen
                    .iter()
                    .min_by_key(|(_, window)| window.start)
                    .map(|(who, _)| who.clone())
                else {
                    break;
                };
                self.seen.remove(&oldest);
            }
        }

        let mine = self
            .seen
            .entry(who.to_owned())
            .or_insert_with(|| Window::opened(now))
            .current(now);

        if mine.cells.saturating_add(cells) > BUDGET {
            return false;
        }

        self.seen.insert(
            who.to_owned(),
            Window {
                start: mine.start,
                cells: mine.cells.saturating_add(cells),
            },
        );
        self.everyone.cells = self.everyone.cells.saturating_add(cells);
        true
    }
}

/// The board, who has been writing to it, and where it is kept.
#[cfg(feature = "ssr")]
pub struct State {
    grid: std::sync::RwLock<Grid>,
    limiter: std::sync::Mutex<Limiter>,
    path: std::path::PathBuf,
}

#[cfg(feature = "ssr")]
impl State {
    /// Loads the board from `path`, starting blank if it is not there yet.
    #[must_use]
    pub fn load(path: std::path::PathBuf) -> Self {
        let grid = std::fs::read_to_string(&path)
            .map_or_else(|_| Grid::blank(), |text| Grid::parse(&text));
        Self {
            grid: std::sync::RwLock::new(grid),
            limiter: std::sync::Mutex::new(Limiter::new()),
            path,
        }
    }

    #[must_use]
    pub fn render(&self) -> String {
        // A poisoned lock still holds a perfectly good grid: whatever panicked
        // happened elsewhere, and losing the board over it would be worse.
        self.grid
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .render()
    }

    pub fn allowed(&self, who: &str, cells: u32) -> bool {
        self.limiter
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take(who, cells)
    }

    /// Sets one cell. Returns the board's new form when it changed, which is
    /// what the caller persists, or `None` when nothing did.
    pub fn set(&self, x: usize, y: usize, byte: u8) -> Option<String> {
        let mut grid = self
            .grid
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        grid.set(x, y, byte).then(|| grid.render())
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Parses an `x y char` request, setting one cell.
///
/// The character is optional and everything after the coordinates is taken
/// literally, so `4 2 ` with a trailing space sets a space and `4 2` with
/// nothing after it clears the cell. That is the only way to reach a blank
/// through a command line that splits on whitespace.
///
/// Everything is validated here, so a handler only ever sees a write it can
/// carry out.
#[must_use]
pub fn parse_write(body: &str) -> Option<(usize, usize, u8)> {
    // Trailing newlines are the client's, not the writer's. A trailing space is
    // the writer's, and is the whole point, so it stays.
    let body = body.trim_end_matches(['\n', '\r']);
    let mut parts = body.splitn(3, ' ');

    let x: usize = parts.next()?.parse().ok()?;
    let y: usize = parts.next()?.parse().ok()?;

    let byte = match parts.next().map(str::as_bytes) {
        // Nothing after the coordinates clears the cell.
        None | Some(b"") => BLANK,
        Some(&[byte]) => byte,
        // Anything longer is a mistake worth reporting rather than truncating.
        Some(_) => return None,
    };

    (x < COLS && y < ROWS && printable(byte)).then_some((x, y, byte))
}

/// Fetches the board, or writes to it and fetches the result.
///
/// Returns the status alongside the body, because the server answers a refused
/// write with a plain sentence explaining it, and that sentence is the most
/// useful thing to show.
///
/// # Errors
///
/// Returns a message fit to print when the request cannot be made at all.
#[cfg(feature = "hydrate")]
#[expect(
    clippy::future_not_send,
    reason = "the browser is single threaded and nothing here crosses a thread"
)]
pub async fn fetch(write: Option<&str>) -> Result<(u16, String), String> {
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;

    fn failed<E>(what: &'static str) -> impl FnOnce(E) -> String {
        move |_| format!("wall: {what}")
    }

    let init = web_sys::RequestInit::new();
    if let Some(body) = write {
        init.set_method("POST");
        init.set_body(&JsValue::from_str(body));
    }

    let request = web_sys::Request::new_with_str_and_init("/api/wall", &init)
        .map_err(failed("could not build the request"))?;
    let response = JsFuture::from(leptos::prelude::window().fetch_with_request(&request))
        .await
        .map_err(failed("the board is unreachable"))?
        .dyn_into::<web_sys::Response>()
        .map_err(failed("the board answered with something unexpected"))?;

    let status = response.status();
    let text = JsFuture::from(response.text().map_err(failed("no body to read"))?)
        .await
        .map_err(failed("the board's answer was cut short"))?;

    text.as_string()
        .map(|body| (status, body))
        .ok_or_else(|| "wall: the board's answer was not text".to_owned())
}

/// The client half never runs on the server, but the command that calls it is
/// compiled into both. This exists so that it is.
///
/// # Errors
///
/// Always: there is no browser here to ask.
#[cfg(not(feature = "hydrate"))]
#[expect(clippy::unused_async, reason = "matches the client signature")]
pub async fn fetch(_write: Option<&str>) -> Result<(u16, String), String> {
    Err("wall: no browser to ask".to_owned())
}
