//! Conway's life, as a guest program.
//!
//! Two cells to a character, stacked, using the half blocks the ROM font
//! carries. A character is twice as tall as it is wide, so splitting it in half
//! is both twice the resolution and square cells, which is what the patterns
//! are meant to look like.

#![deny(unsafe_code)]

use std::cell::RefCell;

mod tty {
    //! The screen and the keyboard, which WASI has no concept of.
    //!
    //! All the unsafe is here. Declaring an import and calling it are both
    //! unsafe, and wrapping them once keeps the rest of the program safe.
    #![expect(
        unsafe_code,
        reason = "a wasm import can only be declared and called unsafely"
    )]

    #[link(wasm_import_module = "tty")]
    unsafe extern "C" {
        fn cols() -> u32;
        fn rows() -> u32;
        fn put(x: u32, y: u32, ch: u32, fg: u32, bg: u32);
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
        // SAFETY: the host provides this. A module asking for an import it does
        // not provide fails to instantiate rather than linking to nothing.
        count(unsafe { cols() })
    }

    pub fn height() -> usize {
        // SAFETY: the host provides this. A module asking for an import it does
        // not provide fails to instantiate rather than linking to nothing.
        count(unsafe { rows() })
    }

    /// Draws one cell. Out of range is the host's problem, and it ignores it.
    pub fn draw(x: usize, y: usize, ch: char, fg: u8, bg: u8) {
        // SAFETY: the host provides this. A module asking for an import it does
        // not provide fails to instantiate rather than linking to nothing.
        unsafe {
            put(at(x), at(y), u32::from(ch), u32::from(fg), u32::from(bg));
        }
    }

    /// The next key as a byte, or `None` when nothing is waiting.
    pub fn pressed() -> Option<u8> {
        // SAFETY: the host provides this. A module asking for an import it does
        // not provide fails to instantiate rather than linking to nothing.
        match unsafe { key() } {
            -1 => None,
            byte => u8::try_from(byte).ok(),
        }
    }
}

/// Percent of cells alive in a fresh world.
const DENSITY: u8 = 28;

/// Most generations one frame will catch up on.
const CATCHUP: usize = 4;

/// Milliseconds a generation lasts, slowest to fastest. `[` and `]` walk the
/// list rather than scaling a number, so every speed is one worth watching.
const SPEEDS: [f32; 7] = [480.0, 240.0, 120.0, 60.0, 30.0, 16.0, 8.0];
const NORMAL: usize = 2;

/// The colour live cells are drawn in, and the two the status bar uses.
const LIVE: u8 = 10;
const BAR_FG: u8 = 0;
const BAR_BG: u8 = 7;

struct Life {
    cells: Vec<bool>,
    cols: usize,
    rows: usize,
    due: f32,
    paused: bool,
    speed: usize,
    generation: u64,
}

impl Life {
    const fn new() -> Self {
        Self {
            cells: Vec::new(),
            cols: 0,
            rows: 0,
            due: 0.0,
            paused: false,
            speed: NORMAL,
            generation: 0,
        }
    }

    fn scatter(&mut self) {
        let count = self.cols * self.rows;
        self.cells = (0..count).map(|_| fastrand::u8(..100) < DENSITY).collect();
        self.generation = 0;
    }

    /// The world is the screen less the status bar, at two cells per row.
    fn fit(&mut self) {
        self.cols = tty::width();
        self.rows = tty::height().saturating_sub(1) * 2;
        self.scatter();
    }

    fn fits(&self) -> bool {
        self.cols == tty::width() && self.rows == tty::height().saturating_sub(1) * 2
    }

    /// Neighbours, on a grid that wraps: a glider runs off one edge and back
    /// in the other, which is better to watch than one dying at a wall. The
    /// offsets are added rather than subtracted to keep the arithmetic
    /// unsigned.
    fn neighbours(&self, x: usize, y: usize) -> u8 {
        let mut count = 0;
        for dy in [self.rows - 1, 0, 1] {
            for dx in [self.cols - 1, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if self.cells[((y + dy) % self.rows) * self.cols + (x + dx) % self.cols] {
                    count += 1;
                }
            }
        }
        count
    }

    fn step(&mut self) {
        if self.cols == 0 || self.rows == 0 {
            return;
        }
        let mut next = vec![false; self.cells.len()];
        for y in 0..self.rows {
            for x in 0..self.cols {
                let here = self.cells[y * self.cols + x];
                next[y * self.cols + x] =
                    matches!((here, self.neighbours(x, y)), (true, 2 | 3) | (false, 3));
            }
        }
        self.cells = next;
        self.generation += 1;
    }

    fn draw(&self) {
        // Two world rows to a character row: the upper half block is the even
        // row and the lower half the odd one, so a full block, either half, or
        // nothing covers every case.
        for cy in 0..self.rows / 2 {
            for x in 0..self.cols {
                let top = self.cells[(cy * 2) * self.cols + x];
                let bottom = self.cells[(cy * 2 + 1) * self.cols + x];
                let ch = match (top, bottom) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                };
                tty::draw(x, cy, ch, LIVE, 0);
            }
        }
        self.status();
    }

    /// A status bar, the way a full screen program has one: what this is on
    /// the left, what the keys do on the right, reverse video across.
    fn status(&self) {
        let row = tty::height().saturating_sub(1);
        let width = tty::width();

        // Both halves close with a separator, so the space between reads as a
        // gap in one bar rather than two loose ends.
        let left: Vec<char> = format!(
            " life │ gen {} │ {}×{} │ {} │",
            compact(self.generation),
            self.cols,
            self.rows,
            if self.paused {
                "paused".to_owned()
            } else {
                format!("{:.0}ms", SPEEDS[self.speed])
            },
        )
        .chars()
        .collect();

        // Named by what the key would do next: space toggles, so it offers
        // whichever it is not.
        let toggle = if self.paused {
            "space: resume"
        } else {
            "space: pause"
        };
        // Least useful first, which is the order they are dropped in when the
        // screen is too narrow for all of them.
        let mut hints = vec!["r: seed", "[ ]: speed", toggle, "q: quit"];

        let right = loop {
            let right: Vec<char> = format!("│ {} ", hints.join(" │ ")).chars().collect();
            if hints.len() == 1 || left.len() + right.len() <= width {
                break right;
            }
            hints.remove(0);
        };
        // Even one may not fit, and half a word is worse than none.
        let start = width.saturating_sub(right.len());
        let room = left.len() <= start;

        for x in 0..width {
            let ch = if x < left.len() {
                left[x]
            } else if room && x >= start {
                right[x - start]
            } else {
                ' '
            };
            tty::draw(x, row, ch, BAR_FG, BAR_BG);
        }
    }

    /// Returns whether it is time to stop.
    fn keys(&mut self) -> bool {
        while let Some(byte) = tty::pressed() {
            match byte {
                b'q' | b'Q' => return true,
                b' ' => self.paused = !self.paused,
                b'r' | b'R' => self.scatter(),
                b'[' => self.speed = self.speed.saturating_sub(1),
                b']' => self.speed = (self.speed + 1).min(SPEEDS.len() - 1),
                _ => {}
            }
        }
        false
    }

    fn tick(&mut self, elapsed: f32) {
        if self.paused {
            return;
        }
        self.due += elapsed;
        // Bounded, not "until caught up": a backgrounded tab stops getting
        // frames, and coming back should not run every generation that happened
        // while nobody was looking.
        let tick = SPEEDS[self.speed];
        for _ in 0..CATCHUP {
            if self.due < tick {
                break;
            }
            self.due -= tick;
            self.step();
        }
        self.due = self.due.min(tick);
    }
}

/// `1234` stays itself, `12345` becomes `12.3k`. Left running overnight, a
/// counter should not push the rest of the bar off the screen.
fn compact(n: u64) -> String {
    const K: u64 = 1_000;
    const M: u64 = 1_000_000;
    const G: u64 = 1_000_000_000;
    match n {
        0..K => n.to_string(),
        K..M => format!("{}.{}k", n / K, n % K / 100),
        M..G => format!("{}.{}M", n / M, n % M / 100_000),
        _ => format!("{}.{}G", n / G, n % G / 100_000_000),
    }
}

thread_local! {
    /// The world, which outlives any one frame. A thread local rather than a
    /// `static mut`: wasm has one thread, and this needs no unsafe.
    static WORLD: RefCell<Life> = const { RefCell::new(Life::new()) };
}

/// Called once a frame by the terminal, which owns the loop. Non-zero quits.
///
/// The name has to survive mangling for the host to find it, and saying so is
/// itself unsafe. Nothing else here exports a symbol, so there is nothing for
/// it to collide with.
#[expect(unsafe_code, reason = "the host looks this up by name")]
#[unsafe(no_mangle)]
pub extern "C" fn frame(elapsed: f32) -> i32 {
    WORLD.with_borrow_mut(|world| {
        if world.keys() {
            return 1;
        }
        // A resize changes the world, so it starts again rather than pretend
        // the old one still fits.
        if !world.fits() {
            world.fit();
        }
        world.tick(elapsed);
        world.draw();
        0
    })
}

/// Run before the first frame, so the world is already there when it arrives.
fn main() {
    WORLD.with_borrow_mut(|world| {
        world.fit();
        world.draw();
    });
}
