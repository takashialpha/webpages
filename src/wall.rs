//! The graffiti board: a shared grid anyone can write to.
//!
//! Bounded on purpose. A write is one printable character at one coordinate,
//! so the worst anyone can do is spell something the next visitor writes over.
//! It is stored as plain text, so moderating it is editing a file.

/// Eighty by twenty-four, like a terminal. It does not fit a phone, so the
/// `wall` command puts it in a box that scrolls sideways.
pub const COLS: usize = 80;
pub const ROWS: usize = 24;

/// An unwritten cell.
pub const BLANK: u8 = b' ';

/// Printable ASCII only, checked at every edge, so a control character never
/// reaches the file or anyone else.
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

    /// Sets one cell. `0,0` is the bottom left and `y` counts up, as drawn.
    ///
    /// Storage runs the other way, top row first, so the file reads in the
    /// order the board is drawn. This is where the two meet.
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

/// Cells one address may set per [`WINDOW`]: a row's worth.
#[cfg(feature = "ssr")]
pub const BUDGET: u32 = 80;

/// Cells everyone together may write per [`WINDOW`].
///
/// The address comes from a header. Behind Cloudflare that is trustworthy, but
/// anything reaching the origin directly could claim a new one per request and
/// never meet a per-address limit. This is the ceiling that stops it.
#[cfg(feature = "ssr")]
pub const CEILING: u32 = 600;

#[cfg(feature = "ssr")]
pub const WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

/// How many addresses are remembered at once.
///
/// The key comes from a header, so whoever chooses it must not also choose how
/// much is remembered. Past the cap the oldest is dropped.
#[cfg(feature = "ssr")]
const TRACKED: usize = 4096;

/// One address's writes in the current window. Fixed size, so remembering
/// someone costs the same however much they write.
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

    /// Rolls over once the window has elapsed.
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
    /// All or nothing, so nobody has to wonder which half of a write landed.
    fn take(&mut self, who: &str, cells: u32) -> bool {
        let now = std::time::Instant::now();

        self.everyone = self.everyone.current(now);
        if self.everyone.cells.saturating_add(cells) > CEILING {
            return false;
        }

        // Only at the cap. Scanning on every write would make each one cost
        // whatever the table had grown to.
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

    /// Writes the board where it will be kept, creating it if it is not there.
    ///
    /// Called once at startup so a board that cannot be saved is an error in
    /// the journal, not a surprise at the first write.
    ///
    /// # Errors
    ///
    /// Whatever stopped the write: usually the directory not existing, or the
    /// service not being allowed to write it.
    pub fn persist(&self) -> std::io::Result<()> {
        std::fs::write(&self.path, self.render())
    }
}

/// Parses `x y char`.
///
/// The character is optional: `4 2` clears the cell, and `4 2 ` with a
/// trailing space sets one. That is the only way to reach a blank through a
/// line split on whitespace.
///
/// Validated here, so a handler only sees writes it can carry out.
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
/// Returns the status with the body: the server explains a refusal in a
/// sentence, and that sentence is what to show.
///
/// # Errors
///
/// A message to print, if the request cannot be made.
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

/// The server has no browser to ask. This exists so the command compiles there.
///
/// # Errors
///
/// Always.
#[cfg(not(feature = "hydrate"))]
#[expect(clippy::unused_async, reason = "matches the client signature")]
pub async fn fetch(_write: Option<&str>) -> Result<(u16, String), String> {
    Err("wall: no browser to ask".to_owned())
}
