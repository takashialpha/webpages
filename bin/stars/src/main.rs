//! A starfield, drifting.
//!
//! The other shape a guest can take: it exports `frame`, so the terminal calls
//! it once per animation frame and it draws through the `tty` imports rather
//! than printing. WebAssembly cannot suspend a synchronous call, so a program
//! that ran its own loop would hold the page still until it finished; taking a
//! frame at a time is what lets it be watched.

#![deny(unsafe_code)]

use std::cell::RefCell;

mod tty {
    //! The screen and the keyboard, which WASI has no concept of.
    //!
    //! Everything unsafe in this program is here: declaring an import and
    //! calling it are both unsafe, and there is no way to reach the host
    //! without doing so. Wrapping them once means the rest is ordinary safe
    //! Rust rather than unsafe scattered through the drawing code.
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

    pub fn width() -> usize {
        // SAFETY: the host provides this import, and a module asking for one it
        // does not provide fails to instantiate rather than linking to nothing.
        count(unsafe { cols() })
    }

    pub fn height() -> usize {
        // SAFETY: the host provides this import, and a module asking for one it
        // does not provide fails to instantiate rather than linking to nothing.
        count(unsafe { rows() })
    }

    pub fn wipe() {
        // SAFETY: the host provides this import, and a module asking for one it
        // does not provide fails to instantiate rather than linking to nothing.
        unsafe { clear() };
    }

    /// Draws one cell. Out of range is the host's problem, and it ignores it.
    pub fn draw(x: usize, y: usize, ch: char, fg: u8) {
        // SAFETY: the host provides this import, and a module asking for one it
        // does not provide fails to instantiate rather than linking to nothing.
        unsafe { put(at(x), at(y), u32::from(ch), u32::from(fg), 0) };
    }

    /// The next key as a byte, or `None` when nothing is waiting.
    pub fn pressed() -> Option<u8> {
        // SAFETY: the host provides this import, and a module asking for one it
        // does not provide fails to instantiate rather than linking to nothing.
        match unsafe { key() } {
            -1 => None,
            byte => u8::try_from(byte).ok(),
        }
    }
}

const STARS: usize = 240;

/// A count of cells as a distance. Exact: a screen is never more than a few
/// thousand cells across, which a float says precisely.
fn span(cells: usize) -> f32 {
    f32::from(u16::try_from(cells).unwrap_or(u16::MAX))
}

/// A distance back to a cell index.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a coordinate is small and positive, and the cast saturates"
)]
const fn cell(distance: f32) -> usize {
    distance as usize
}

/// Nearer stars are brighter and move faster, which is the whole trick.
const SHADES: [(char, u8, f32); 3] = [('.', 8, 4.0), ('+', 7, 9.0), ('*', 15, 18.0)];

struct Star {
    x: f32,
    y: f32,
    depth: usize,
}

struct Field {
    stars: Vec<Star>,
    paused: bool,
    seed: u32,
}

impl Field {
    const fn new() -> Self {
        Self {
            stars: Vec::new(),
            paused: false,
            seed: 0x2545_f491,
        }
    }

    /// xorshift: an unplanned pattern is all this needs.
    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        f32::from(u16::try_from(self.seed % 10_000).unwrap_or(0)) / 10_000.0
    }

    fn scatter(&mut self, width: f32, height: f32) -> Star {
        Star {
            x: self.random() * width,
            y: self.random() * height,
            depth: cell(self.random() * 3.0) % SHADES.len(),
        }
    }

    /// Returns whether it is time to stop.
    fn keys(&mut self) -> bool {
        while let Some(byte) = tty::pressed() {
            match byte {
                b'q' | b'Q' => return true,
                b' ' => self.paused = !self.paused,
                _ => {}
            }
        }
        false
    }

    fn draw(&mut self, elapsed: f32) {
        let (width, height) = (span(tty::width()), span(tty::height()));
        if width < 1.0 || height < 1.0 {
            return;
        }

        while self.stars.len() < STARS {
            let star = self.scatter(width, height);
            self.stars.push(star);
        }

        tty::wipe();
        // Seconds, so the speeds above read as columns per second.
        let step = elapsed / 1000.0;

        for index in 0..self.stars.len() {
            let (glyph, colour, speed) = SHADES[self.stars[index].depth];
            if !self.paused {
                self.stars[index].x = speed.mul_add(-step, self.stars[index].x);
                if self.stars[index].x < 0.0 {
                    let mut fresh = self.scatter(width, height);
                    fresh.x = width - 1.0;
                    self.stars[index] = fresh;
                }
            }
            let star = &self.stars[index];
            if star.y < height {
                tty::draw(cell(star.x), cell(star.y), glyph, colour);
            }
        }
    }
}

thread_local! {
    /// The field, which outlives any one frame. A thread local rather than a
    /// `static mut`: wasm is single threaded, and this needs no unsafe.
    static FIELD: RefCell<Field> = const { RefCell::new(Field::new()) };
}

/// Called once per animation frame by the terminal, which owns the loop.
/// Returns non-zero to quit.
///
/// The name has to survive mangling for the host to find it, and saying so is
/// itself unsafe: nothing else in this program exports a symbol, so there is
/// nothing for it to collide with.
#[expect(unsafe_code, reason = "the host looks this up by name")]
#[unsafe(no_mangle)]
pub extern "C" fn frame(elapsed: f32) -> i32 {
    FIELD.with_borrow_mut(|field| {
        if field.keys() {
            return 1;
        }
        field.draw(elapsed);
        0
    })
}

/// The low bits of a wide number, without a cast that could lose more than it
/// means to.
fn fold(wide: u128) -> u32 {
    u32::try_from(wide & u128::from(u32::MAX)).unwrap_or(1)
}

/// Run before the first frame, and where the seed comes from.
fn main() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        // Only the low bits matter: this is a seed, not a time, so the wide
        // value is folded down rather than cast.
        .map_or(1, |since| fold(since.as_nanos()));
    FIELD.with_borrow_mut(|field| field.seed ^= now | 1);
}
