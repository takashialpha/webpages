//! Tetris.
//!
//! Two cells to a character, using half blocks, so a block is square rather
//! than twice as tall as it is wide. The well is a fixed ten by twenty, so
//! unlike the others this one is centred on whatever screen it gets rather
//! than sized to it.

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
        fn clear();
        fn key() -> i32;
    }

    fn count(value: u32) -> usize {
        usize::try_from(value).unwrap_or(0)
    }

    fn at(value: usize) -> u32 {
        u32::try_from(value).unwrap_or(u32::MAX)
    }

    pub fn width() -> usize {
        // SAFETY: the host provides this. A module asking for an import it does
        // not provide fails to instantiate rather than linking to nothing.
        count(unsafe { cols() })
    }

    pub fn height() -> usize {
        // SAFETY: as above.
        count(unsafe { rows() })
    }

    /// Wipes the screen. The well moves when the window resizes, so every
    /// frame starts from nothing rather than leaving the old one behind.
    pub fn wipe() {
        // SAFETY: as above.
        unsafe { clear() };
    }

    pub fn draw(x: usize, y: usize, ch: char, fg: u8, bg: u8) {
        // SAFETY: as above. Off-screen coordinates are ignored by the host.
        unsafe { put(at(x), at(y), u32::from(ch), u32::from(fg), u32::from(bg)) };
    }

    /// The next key as a byte, or `None` when nothing is waiting.
    pub fn pressed() -> Option<u8> {
        // SAFETY: as above.
        match unsafe { key() } {
            -1 => None,
            byte => u8::try_from(byte).ok(),
        }
    }
}

/// The well, in blocks.
const COLS: usize = 10;
const ROWS: usize = 20;

/// The next piece sits in a box of its own, this many blocks square, with a
/// gap before it.
const NEXT: usize = 4;
const GAP: usize = 1;

/// How many characters across a block is drawn, and how many half rows down.
/// The same number both ways is what keeps it square, since a character cell
/// is exactly twice as tall as it is wide. Only one or two are whole: three
/// would be a block and a half tall.
///
/// The largest that fits, or `None` when even the small one does not.
fn scale(width: usize, height: usize) -> Option<usize> {
    [2, 1]
        .into_iter()
        .find(|&scale| width >= needs_w(scale) && height >= needs_h(scale))
}

/// The well and the next box side by side, each with a border.
const fn needs_w(scale: usize) -> usize {
    (COLS * scale + 2) + GAP + (NEXT * scale + 2)
}

/// The well, its border, and the status bar under it.
const fn needs_h(scale: usize) -> usize {
    ROWS * scale / 2 + 2 + 1
}

/// The seven pieces, each as its four turns in a 4x4 box. Bit 15 is the top
/// left corner and the bits run along each row.
const PIECES: [[u16; 4]; 7] = [
    [0x0F00, 0x2222, 0x00F0, 0x4444], // I
    [0xCC00, 0xCC00, 0xCC00, 0xCC00], // O
    [0x0E40, 0x4C40, 0x4E00, 0x4640], // T
    [0x06C0, 0x8C40, 0x6C00, 0x4620], // S
    [0x0C60, 0x4C80, 0xC600, 0x2640], // Z
    [0x44C0, 0x8E00, 0xC880, 0x0E20], // J
    [0x4460, 0x0E80, 0xC440, 0x2E00], // L
];

/// One colour each, in the order above. Never zero: zero is an empty cell.
const COLOURS: [u8; 7] = [14, 11, 13, 10, 9, 12, 3];

/// Where the piece would land, and the lines around everything.
const GHOST: u8 = 8;
const FRAME: u8 = 8;
const BAR_FG: u8 = 0;
const BAR_BG: u8 = 7;

/// Milliseconds a row takes to fall, by level. Past the end it stays here.
const DROPS: [f32; 10] = [
    800.0, 700.0, 600.0, 500.0, 400.0, 300.0, 220.0, 160.0, 120.0, 90.0,
];

/// Lines to the next level.
const PER_LEVEL: u32 = 10;

/// Most rows one frame will drop through, so a backgrounded tab does not come
/// back and bury the well all at once.
const CATCHUP: usize = 4;

/// Where we are in an escape sequence. Arrow keys arrive as `esc [ A`, and
/// without tracking that a literal `A` would move the piece.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reading {
    Plain,
    Escaped,
    Bracket,
}

/// Where a new piece appears: its box at the top, roughly in the middle.
const SPAWN_X: isize = 3;

#[derive(Clone, Copy)]
struct Piece {
    kind: usize,
    turn: usize,
    /// The top left of its 4x4 box, which may sit off the left of the well.
    x: isize,
    y: isize,
}

impl Piece {
    /// One of the seven, in its first turn, at the corner.
    const fn new(kind: usize) -> Self {
        Self {
            kind,
            turn: 0,
            x: 0,
            y: 0,
        }
    }

    /// The cells it fills, as offsets inside its box.
    fn cells(self) -> impl Iterator<Item = (isize, isize)> {
        let mask = PIECES[self.kind][self.turn];
        (0..4_isize)
            .flat_map(|row| (0..4_isize).map(move |col| (col, row)))
            .filter(move |&(col, row)| mask & (1_u16 << (15 - (row * 4 + col))) != 0)
    }

    const fn colour(self) -> u8 {
        COLOURS[self.kind]
    }

    /// The same piece somewhere else, which is how every move is tried before
    /// it is taken.
    const fn beside(self, dx: isize) -> Self {
        Self {
            x: self.x + dx,
            ..self
        }
    }

    const fn below(self) -> Self {
        Self {
            y: self.y + 1,
            ..self
        }
    }

    const fn turned(self) -> Self {
        Self {
            turn: (self.turn + 1) % 4,
            ..self
        }
    }
}

/// A cell of a piece as a place in the well, or `None` if it is off it.
fn spot(piece: Piece, dx: isize, dy: isize) -> Option<(usize, usize)> {
    let x = usize::try_from(piece.x + dx).ok()?;
    let y = usize::try_from(piece.y + dy).ok()?;
    (x < COLS && y < ROWS).then_some((x, y))
}

struct Game {
    well: [[u8; COLS]; ROWS],
    piece: Piece,
    next: usize,
    due: f32,
    score: u32,
    lines: u32,
    level: u32,
    paused: bool,
    over: bool,
    reading: Reading,
}

impl Game {
    const fn new() -> Self {
        Self {
            well: [[0; COLS]; ROWS],
            piece: Piece::new(0),
            next: 0,
            due: 0.0,
            score: 0,
            lines: 0,
            level: 0,
            paused: false,
            over: true,
            reading: Reading::Plain,
        }
    }

    fn restart(&mut self) {
        self.well = [[0; COLS]; ROWS];
        self.due = 0.0;
        self.score = 0;
        self.lines = 0;
        self.level = 0;
        self.paused = false;
        self.over = false;
        self.reading = Reading::Plain;
        self.next = fastrand::usize(..PIECES.len());
        self.spawn();
    }

    /// Whether a piece could sit exactly there: on the well, and on nothing.
    fn free(&self, piece: Piece) -> bool {
        piece
            .cells()
            .all(|(dx, dy)| spot(piece, dx, dy).is_some_and(|(x, y)| self.well[y][x] == 0))
    }

    /// Takes the next piece, draws another, and ends the game if there is no
    /// room left to put one.
    fn spawn(&mut self) {
        self.piece = Piece::new(self.next).beside(SPAWN_X);
        self.next = fastrand::usize(..PIECES.len());
        self.over = !self.free(self.piece);
    }

    fn shift(&mut self, dx: isize) {
        let moved = self.piece.beside(dx);
        if self.free(moved) {
            self.piece = moved;
        }
    }

    /// Turns clockwise, nudging off a wall when that is all that is in the way.
    fn rotate(&mut self) {
        let turned = self.piece.turned();
        for nudge in [0, -1, 1, -2, 2] {
            let tried = turned.beside(nudge);
            if self.free(tried) {
                self.piece = tried;
                return;
            }
        }
    }

    /// One row down. Returns whether there was room.
    fn sink(&mut self) -> bool {
        let down = self.piece.below();
        let room = self.free(down);
        if room {
            self.piece = down;
        }
        room
    }

    /// One row down for free, and the fall clock starts again from there.
    fn soft_drop(&mut self) {
        if self.sink() {
            self.due = 0.0;
        }
    }

    /// Straight to the bottom, and it stays there.
    fn slam(&mut self) {
        self.piece = self.ghost();
        self.settle();
    }

    /// Where the piece would land if nothing else were pressed.
    fn ghost(&self) -> Piece {
        let mut ghost = self.piece;
        while self.free(ghost.below()) {
            ghost = ghost.below();
        }
        ghost
    }

    /// Leaves the piece where it is and brings on the next one.
    fn settle(&mut self) {
        let colour = self.piece.colour();
        for (dx, dy) in self.piece.cells() {
            if let Some((x, y)) = spot(self.piece, dx, dy) {
                self.well[y][x] = colour;
            }
        }
        self.sweep();
        self.spawn();
        self.due = 0.0;
    }

    /// Takes out the full rows and drops everything above them.
    fn sweep(&mut self) {
        let kept: Vec<[u8; COLS]> = self
            .well
            .iter()
            .copied()
            .filter(|row| row.contains(&0))
            .collect();
        let cleared = ROWS - kept.len();
        if cleared == 0 {
            return;
        }

        // What survived falls to the bottom; the blank rows take the top.
        self.well = [[0; COLS]; ROWS];
        for (row, kept) in self.well[cleared..].iter_mut().zip(kept) {
            *row = kept;
        }

        self.lines += u32::try_from(cleared).unwrap_or(0);
        self.level = self.lines / PER_LEVEL;
        // Four at once is worth far more than four one at a time, which is the
        // whole reason to stack rather than clear as you go.
        self.score += match cleared {
            1 => 100,
            2 => 300,
            3 => 500,
            _ => 800,
        } * (self.level + 1);
    }

    fn pace(&self) -> f32 {
        let level = usize::try_from(self.level).unwrap_or(0);
        DROPS[level.min(DROPS.len() - 1)]
    }

    fn tick(&mut self, elapsed: f32) {
        if self.paused || self.over {
            return;
        }
        self.due += elapsed;
        let pace = self.pace();
        for _ in 0..CATCHUP {
            if self.due < pace {
                break;
            }
            self.due -= pace;
            if !self.sink() {
                self.settle();
            }
        }
        self.due = self.due.min(pace);
    }

    /// The keys that move the piece, which do nothing while paused or after
    /// the end. Arrows are turned into the letters they mean before they get
    /// here, so there is one place that decides what steering is.
    fn steer(&mut self, byte: u8) {
        if self.paused || self.over {
            return;
        }
        match byte {
            b'w' | b'W' => self.rotate(),
            b's' | b'S' => self.soft_drop(),
            b'a' | b'A' => self.shift(-1),
            b'd' | b'D' => self.shift(1),
            b' ' => self.slam(),
            _ => {}
        }
    }

    /// Returns whether it is time to stop.
    fn keys(&mut self) -> bool {
        while let Some(byte) = tty::pressed() {
            self.reading = match (self.reading, byte) {
                (Reading::Plain, 0x1b) => Reading::Escaped,
                (Reading::Escaped, b'[') => Reading::Bracket,
                // The letter after `esc [` is the arrow.
                (Reading::Bracket, letter) => {
                    self.steer(match letter {
                        b'A' => b'w',
                        b'B' => b's',
                        b'C' => b'd',
                        b'D' => b'a',
                        other => other,
                    });
                    Reading::Plain
                }
                (_, letter) => {
                    match letter {
                        b'q' | b'Q' => return true,
                        b'r' | b'R' => self.restart(),
                        b'p' | b'P' if !self.over => self.paused = !self.paused,
                        b' ' if self.over => self.restart(),
                        other => self.steer(other),
                    }
                    Reading::Plain
                }
            };
        }
        false
    }

    /// The well with the ghost and the falling piece painted over it.
    fn occupancy(&self) -> [[u8; COLS]; ROWS] {
        let mut cells = self.well;
        if self.over {
            return cells;
        }

        let ghost = self.ghost();
        for (dx, dy) in ghost.cells() {
            if let Some((x, y)) = spot(ghost, dx, dy) {
                cells[y][x] = GHOST;
            }
        }
        for (dx, dy) in self.piece.cells() {
            if let Some((x, y)) = spot(self.piece, dx, dy) {
                cells[y][x] = self.piece.colour();
            }
        }
        cells
    }

    fn draw(&self) {
        tty::wipe();
        let (width, height) = (tty::width(), tty::height());
        let Some(scale) = scale(width, height) else {
            line(
                0,
                0,
                &format!("tetris wants {}x{}", needs_w(1), needs_h(1)),
                9,
            );
            line(0, 1, "q: quit", GHOST);
            return;
        };

        let left = (width - needs_w(scale)) / 2;
        let top = (height - needs_h(scale)) / 2;
        self.well_box(left, top, scale);
        self.next_box(left + COLS * scale + 2 + GAP, top, scale);
        self.status(width, height);
    }

    fn well_box(&self, left: usize, top: usize, scale: usize) {
        let cells = self.occupancy();
        outline(left, top, COLS * scale, ROWS * scale / 2 + 2);
        blocks(left + 1, top + 1, scale, (COLS, ROWS), |x, y| cells[y][x]);
    }

    fn next_box(&self, left: usize, top: usize, scale: usize) {
        outline(left, top, NEXT * scale, NEXT * scale / 2 + 2);

        // At the corner rather than where it will spawn, so every cell of it
        // lands inside this box.
        let piece = Piece::new(self.next);
        let mut cells = [[0_u8; NEXT]; NEXT];
        for (dx, dy) in piece.cells() {
            if let Some((x, y)) = spot(piece, dx, dy) {
                cells[y][x] = piece.colour();
            }
        }
        blocks(left + 1, top + 1, scale, (NEXT, NEXT), |x, y| cells[y][x]);
    }

    fn status(&self, width: usize, height: usize) {
        let row = height.saturating_sub(1);

        // Both halves close with a separator, so the space between them reads
        // as a gap in one bar rather than as two loose ends.
        let state = if self.over {
            " over │"
        } else if self.paused {
            " paused │"
        } else {
            ""
        };
        let left: Vec<char> = format!(
            " tetris │ {} │ lines {} │ level {} │{state}",
            self.score,
            self.lines,
            self.level + 1,
        )
        .chars()
        .collect();

        // Least useful first, because that is the order they are dropped in
        // when the screen is too narrow to hold them all.
        let mut hints = if self.over {
            vec!["space: again", "q: quit"]
        } else {
            vec!["p: pause", "space: drop", "arrows or wasd", "q: quit"]
        };
        let right = loop {
            let right: Vec<char> = format!("│ {} ", hints.join(" │ ")).chars().collect();
            if hints.len() == 1 || left.len() + right.len() <= width {
                break right;
            }
            hints.remove(0);
        };

        // Even one hint may not fit a very narrow screen, and half a word is
        // worse than none.
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
}

/// Paints a grid of blocks. Every screen half-cell asks which block it falls
/// inside, so one path draws whatever the scale is.
fn blocks(
    left: usize,
    top: usize,
    scale: usize,
    (cols, rows): (usize, usize),
    at: impl Fn(usize, usize) -> u8,
) {
    for cy in 0..rows * scale / 2 {
        for hx in 0..cols * scale {
            let upper = at(hx / scale, cy * 2 / scale);
            let lower = at(hx / scale, (cy * 2 + 1) / scale);
            half(left + hx, top + cy, upper, lower);
        }
    }
}

/// One character holding two stacked half blocks. Zero is an empty one.
fn half(x: usize, y: usize, upper: u8, lower: u8) {
    match (upper, lower) {
        (0, 0) => tty::draw(x, y, ' ', 0, 0),
        (up, 0) => tty::draw(x, y, '▀', up, 0),
        (0, down) => tty::draw(x, y, '▄', down, 0),
        // Two colours in one character: the lower half becomes the background,
        // so both still show.
        (up, down) => tty::draw(x, y, '▀', up, down),
    }
}

/// A box `inner` characters wide inside its borders and `height` tall
/// including them.
fn outline(left: usize, top: usize, inner: usize, height: usize) {
    let rule = "─".repeat(inner);
    line(left, top, &format!("┌{rule}┐"), FRAME);
    line(left, top + height - 1, &format!("└{rule}┘"), FRAME);
    for row in top + 1..top + height - 1 {
        tty::draw(left, row, '│', FRAME, 0);
        tty::draw(left + inner + 1, row, '│', FRAME, 0);
    }
}

/// One run of text, left to right from where it starts.
fn line(x: usize, y: usize, text: &str, colour: u8) {
    for (offset, ch) in text.chars().enumerate() {
        tty::draw(x + offset, y, ch, colour, 0);
    }
}

thread_local! {
    static GAME: RefCell<Game> = const { RefCell::new(Game::new()) };
}

/// Called once per animation frame. Non-zero quits.
///
/// The name has to survive mangling for the host to find it, and saying so is
/// itself unsafe.
#[expect(unsafe_code, reason = "the host looks this up by name")]
#[unsafe(no_mangle)]
pub extern "C" fn frame(elapsed: f32) -> i32 {
    GAME.with_borrow_mut(|game| {
        if game.keys() {
            return 1;
        }
        game.tick(elapsed);
        game.draw();
        0
    })
}

/// Run before the first frame, so the well is already there when it arrives.
fn main() {
    GAME.with_borrow_mut(|game| {
        game.restart();
        game.draw();
    });
}
