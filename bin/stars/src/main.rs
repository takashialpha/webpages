//! A starfield, drifting.
//!
//! It exports `frame`, so the terminal calls it once a frame and it draws
//! through the `tty` imports rather than printing. WebAssembly cannot suspend a
//! synchronous call, so a program running its own loop would hold the page
//! still until it finished. A frame at a time is what lets it be watched.

#![deny(unsafe_code)]

use std::cell::RefCell;

use guest::{Key, Keys, tty};

const STARS: usize = 240;

/// A count of cells as a distance. Exact, since a screen is never more than a
/// few thousand cells across.
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

/// A star somewhere on the screen, at one of the three depths.
fn scatter(width: f32, height: f32) -> Star {
    Star {
        x: fastrand::f32() * width,
        y: fastrand::f32() * height,
        depth: fastrand::usize(..SHADES.len()),
    }
}

struct Field {
    stars: Vec<Star>,
    paused: bool,
    keys: Keys,
}

impl Field {
    const fn new() -> Self {
        Self {
            stars: Vec::new(),
            paused: false,
            keys: Keys::new(),
        }
    }

    /// Returns whether it is time to stop.
    fn input(&mut self) -> bool {
        while let Some(key) = self.keys.read() {
            match key {
                Key::Byte(b'q' | b'Q') => return true,
                Key::Byte(b' ') => self.paused = !self.paused,
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
            self.stars.push(scatter(width, height));
        }

        tty::wipe();
        // Seconds, so the speeds above read as columns per second.
        let step = elapsed / 1000.0;

        let paused = self.paused;
        for star in &mut self.stars {
            let (glyph, colour, speed) = SHADES[star.depth];
            if !paused {
                star.x = speed.mul_add(-step, star.x);
                // Off the left edge, so it comes back on the right as a new
                // star at a new depth.
                if star.x < 0.0 {
                    *star = Star {
                        x: width - 1.0,
                        ..scatter(width, height)
                    };
                }
            }
            if star.y < height {
                tty::draw(cell(star.x), cell(star.y), glyph, colour, 0);
            }
        }
    }
}

thread_local! {
    /// The field, which outlives any one frame. A thread local rather than a
    /// `static mut`: wasm has one thread, and this needs no unsafe.
    static FIELD: RefCell<Field> = const { RefCell::new(Field::new()) };
}

/// Called once a frame by the terminal, which owns the loop. Non-zero quits.
///
/// The name has to survive mangling for the host to find it, and saying so is
/// itself unsafe. Nothing else here exports a symbol, so there is nothing for
/// it to collide with.
#[expect(unsafe_code, reason = "the host looks this up by name")]
#[unsafe(no_mangle)]
pub extern "C" fn frame(elapsed: f32) -> i32 {
    FIELD.with_borrow_mut(|field| {
        if field.input() {
            return 1;
        }
        field.draw(elapsed);
        0
    })
}

/// Run before the first frame, so the field is already there when it arrives.
fn main() {
    FIELD.with_borrow_mut(|field| field.draw(0.0));
}
