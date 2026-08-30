//! The graffiti board: a shared grid anyone can write to.
//!
//! Small on purpose. A write is one printable character at one coordinate, so
//! the worst anyone can do is spell something the next visitor writes over. It
//! is stored as plain text, so moderating it is editing a file.

/// Eighty by twenty-four, like a terminal. It does not fit a phone, so the
/// `wall` command puts it in a box that scrolls sideways.
pub const COLS: usize = 80;
pub const ROWS: usize = 24;

/// An unwritten cell.
pub const BLANK: u8 = b' ';

/// Printable ASCII only, checked at every edge, so nothing else ever reaches
/// the file or another visitor.
#[must_use]
pub const fn printable(byte: u8) -> bool {
    byte >= 0x20 && byte <= 0x7e
}

/// The board itself. Fixed size, whatever arrives.
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
    /// order the board is drawn. Here is where the two meet.
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

    /// What is stored, one string per row.
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

    /// The stored form back, forgivingly: a short line is padded and a long
    /// one is cut, so hand-editing cannot leave the board a shape the rest of
    /// the code does not expect.
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

/// Cells one address may set a minute: a row's worth.
#[cfg(feature = "ssr")]
pub const BUDGET: u32 = 80;

/// Cells everyone together may write a minute.
///
/// The address comes from a header. Behind Cloudflare that is trustworthy, but
/// anything reaching the origin directly could claim a new one per request and
/// never meet a per-address limit. This is the ceiling that stops it.
#[cfg(feature = "ssr")]
pub const CEILING: u32 = 600;

/// How many addresses to hold before sweeping the spent ones out.
///
/// The key comes from a header, so whoever chooses it must not also choose how
/// much is remembered.
#[cfg(feature = "ssr")]
const TRACKED: usize = 4096;

/// A quota of `per_minute` cells. Zero would be one a minute, but neither of
/// the two is zero.
#[cfg(feature = "ssr")]
fn quota(per_minute: u32) -> governor::Quota {
    governor::Quota::per_minute(
        std::num::NonZeroU32::new(per_minute).unwrap_or(std::num::NonZeroU32::MIN),
    )
}

/// The board, who has been writing to it, and where it is kept.
#[cfg(feature = "ssr")]
pub struct State {
    grid: std::sync::RwLock<Grid>,
    /// What one address may write, and what everyone together may.
    mine: governor::DefaultKeyedRateLimiter<String>,
    everyone: governor::DefaultDirectRateLimiter,
    path: std::path::PathBuf,
    /// When the file last matched this. A newer one means somebody edited it,
    /// so it is read back before the next answer.
    synced: std::sync::Mutex<Option<std::time::SystemTime>>,
}

#[cfg(feature = "ssr")]
impl State {
    /// Loads the board, starting blank if the file is not there yet.
    #[must_use]
    pub fn load(path: std::path::PathBuf) -> Self {
        let grid = std::fs::read_to_string(&path)
            .map_or_else(|_| Grid::blank(), |text| Grid::parse(&text));
        let state = Self {
            grid: std::sync::RwLock::new(grid),
            mine: governor::RateLimiter::keyed(quota(BUDGET)),
            everyone: governor::RateLimiter::direct(quota(CEILING)),
            synced: std::sync::Mutex::new(None),
            path,
        };
        state.saved();
        state
    }

    /// The file's modification time, or `None` if it cannot be read.
    fn touched(&self) -> Option<std::time::SystemTime> {
        std::fs::metadata(&self.path)
            .and_then(|meta| meta.modified())
            .ok()
    }

    /// Says the file now matches this. Call after writing it, or the next read
    /// sees a newer file and loads back what it just wrote.
    pub fn saved(&self) {
        *self
            .synced
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = self.touched();
    }

    /// Reads the file back if it has changed since this last wrote it.
    ///
    /// Editing it is how the board is moderated, so an edit has to land without
    /// a restart. Checked on read and on write rather than watched, so nothing
    /// has to be running in between.
    fn refresh(&self) {
        let Some(touched) = self.touched() else {
            return;
        };
        let mut synced = self
            .synced
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *synced == Some(touched) {
            return;
        }

        if let Ok(text) = std::fs::read_to_string(&self.path) {
            *self
                .grid
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Grid::parse(&text);
        }
        *synced = Some(touched);
    }

    #[must_use]
    pub fn render(&self) -> String {
        self.refresh();
        // A poisoned lock still holds a perfectly good grid: whatever panicked
        // did so elsewhere, and losing the board over it would be worse.
        self.grid
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .render()
    }

    /// Whether `who` may write one more cell, counting it if so.
    ///
    /// Their own budget first, then everyone's. A refused check costs nothing,
    /// so that order leaves the ceiling for people who have not used their own
    /// budget up, rather than letting one address spend everyone's on writes
    /// that were going to be refused anyway.
    ///
    /// Asking about an address is what starts remembering it, so the table is
    /// swept rather than held down by the ceiling.
    pub fn allowed(&self, who: &str) -> bool {
        // Only once it has grown, and it only drops addresses whose budget has
        // fully refilled, so this cannot forget anyone still being limited.
        if self.mine.len() >= TRACKED {
            self.mine.retain_recent();
            self.mine.shrink_to_fit();
        }

        self.mine.check_key(&who.to_owned()).is_ok() && self.everyone.check().is_ok()
    }

    /// Sets one cell. Returns the board to write out when it changed, and
    /// `None` when nothing did.
    pub fn set(&self, x: usize, y: usize, byte: u8) -> Option<String> {
        // So the write lands on what the file says now, not on a copy from
        // before somebody edited it.
        self.refresh();
        let mut grid = self
            .grid
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        grid.set(x, y, byte).then(|| grid.render())
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Writes the board out, creating the file if it is not there.
    ///
    /// Called once at startup, so a board that cannot be saved is an error in
    /// the journal rather than a surprise at the first write.
    ///
    /// # Errors
    ///
    /// Whatever stopped the write: usually the directory not being there, or
    /// the service not being allowed to write it.
    pub fn persist(&self) -> std::io::Result<()> {
        let written = std::fs::write(&self.path, self.render());
        self.saved();
        written
    }
}

/// Parses `x y char`.
///
/// The character is optional: `4 2` clears the cell. That is the only way to
/// ask for a blank through a line that was split on whitespace.
///
/// Checked here, so a handler only ever sees a write it can carry out.
#[must_use]
pub fn parse_write(body: &str) -> Option<(usize, usize, u8)> {
    // A trailing newline is the client's. A trailing space is the writer's,
    // and is the whole point, so it stays.
    let body = body.trim_end_matches(['\n', '\r']);
    let mut parts = body.splitn(3, ' ');

    let x: usize = parts.next()?.parse().ok()?;
    let y: usize = parts.next()?.parse().ok()?;

    let byte = match parts.next().map(str::as_bytes) {
        // Nothing after the coordinates clears the cell.
        None | Some(b"") => BLANK,
        Some(&[byte]) => byte,
        // Anything longer is worth reporting rather than truncating.
        Some(_) => return None,
    };

    (x < COLS && y < ROWS && printable(byte)).then_some((x, y, byte))
}

/// Reads the board, or writes one cell and reads back the result.
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

/// The server has no browser to ask. Here so the command still compiles.
///
/// # Errors
///
/// Always.
#[cfg(not(feature = "hydrate"))]
#[expect(clippy::unused_async, reason = "matches the client signature")]
pub async fn fetch(_write: Option<&str>) -> Result<(u16, String), String> {
    Err("wall: no browser to ask".to_owned())
}
