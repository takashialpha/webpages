//! The screen and the keyboard, which WASI has no concept of.
//!
//! All the unsafe in every program is here. Declaring a wasm import and
//! calling it are both unsafe, and wrapping them once is what keeps the
//! programs themselves ordinary safe Rust.
#![expect(
    unsafe_code,
    reason = "a wasm import can only be declared and called unsafely"
)]

#[link(wasm_import_module = "tty")]
unsafe extern "C" {
    fn cols() -> u32;
    fn rows() -> u32;
    fn put(x: u32, y: u32, ch: u32, fg: u32, bg: u32);
    fn clear();
    fn key() -> i32;
}

/// A count of cells, which is always small enough to say exactly.
fn count(value: u32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// A cell index, back the other way. Out of range saturates, and the host
/// ignores anything off the screen.
fn at(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// How many cells fit across.
pub fn width() -> usize {
    // SAFETY: the host provides this. A module asking for an import it does
    // not provide fails to instantiate rather than linking to nothing.
    count(unsafe { cols() })
}

/// How many cells fit down.
pub fn height() -> usize {
    // SAFETY: as above.
    count(unsafe { rows() })
}

/// Blanks the whole screen. For a program that does not repaint every cell of
/// every frame, so what it drew last frame does not stay behind.
pub fn wipe() {
    // SAFETY: as above.
    unsafe { clear() };
}

/// Draws one cell. Off-screen coordinates are the host's problem, and it
/// ignores them.
pub fn draw(x: usize, y: usize, ch: char, fg: u8, bg: u8) {
    // SAFETY: as above.
    unsafe { put(at(x), at(y), u32::from(ch), u32::from(fg), u32::from(bg)) };
}

/// The next byte the keyboard sent, or `None` when nothing is waiting.
///
/// Raw. [`crate::Keys`] is what a program should read, since it puts the
/// escape sequences back together.
pub fn pressed() -> Option<u8> {
    // SAFETY: as above.
    match unsafe { key() } {
        -1 => None,
        byte => u8::try_from(byte).ok(),
    }
}
