//! The graffiti board: a shared grid anyone can write to.
//!
//! Small on purpose. A write is one printable character at one coordinate, so
//! the worst anyone can do is spell something the next visitor writes over. It
//! is stored as plain text, so moderating it is editing a file.
//!
//! Split three ways, because it runs in three places. What a board is lives
//! here and in [`Grid`], and holds on both sides. [`State`] is the server's
//! copy. [`fetch`] is how the browser asks about it.

mod client;
mod grid;
#[cfg(feature = "ssr")]
mod state;

pub use client::fetch;
pub use grid::Grid;
#[cfg(feature = "ssr")]
pub use state::{BUDGET, CEILING, State};

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
    byte == BLANK || byte.is_ascii_graphic()
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
