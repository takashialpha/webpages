//! Tetris.
//!
//! Two cells to a character, using half blocks, so a block is square rather
//! than twice as tall as it is wide. The well is a fixed ten by twenty, so
//! unlike the others this one is centred on whatever screen it gets rather
//! than sized to it.

#![deny(unsafe_code)]

use std::cell::RefCell;

use guest::{Key, Keys, half, line, status, tty};

/// The well, in blocks.
const COLS: usize = 10;
const ROWS: usize = 20;

/// The box the next piece sits in, and the gap before it. Two rows is enough
/// because every piece lies flat in its first turn.
const NEXT_COLS: usize = 4;
const NEXT_ROWS: usize = 2;
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
    (COLS * scale + 2) + GAP + (NEXT_COLS * scale + 2)
}

/// The well, its border, and the status bar under it.
const fn needs_h(scale: usize) -> usize {
    ROWS * scale / 2 + 2 + 1
}

/// The seven pieces, each as its four turns in a 4x4 box. Bit 15 is the top
/// left corner and the bits run along each row.
///
/// The first of each is how it spawns, and all seven lie flat: two rows at
/// most. J and L start a turn along from where they are usually written,
/// because that list stands them on end, which is neither how tetris drops
/// them nor something the preview box has room for.
const PIECES: [[u16; 4]; 7] = [
    [0x0F00, 0x2222, 0x00F0, 0x4444], // I
    [0xCC00, 0xCC00, 0xCC00, 0xCC00], // O
    [0x0E40, 0x4C40, 0x4E00, 0x4640], // T
    [0x06C0, 0x8C40, 0x6C00, 0x4620], // S
    [0x0C60, 0x4C80, 0xC600, 0x2640], // Z
    [0x8E00, 0xC880, 0x0E20, 0x44C0], // J
    [0x2E00, 0x4460, 0x0E80, 0xC440], // L
];

/// One colour each, in the order above. Never zero: zero is an empty cell.
const COLOURS: [u8; 7] = [14, 11, 13, 10, 9, 12, 3];

/// Where the piece would land, and the lines around everything.
const GHOST: u8 = 8;
const FRAME: u8 = 8;

/// Milliseconds a row takes to fall, by level. Past the end it stays here.
const DROPS: [f32; 10] = [
    800.0, 700.0, 600.0, 500.0, 400.0, 300.0, 220.0, 160.0, 120.0, 90.0,
];

/// Lines to the next level.
const PER_LEVEL: u32 = 10;

/// Most rows one frame will drop through, so a backgrounded tab does not come
/// back and bury the well all at once.
const CATCHUP: usize = 4;

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
    keys: Keys,
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
            keys: Keys::new(),
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
        self.keys.forget();
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
    /// the end. An arrow and the letter under it are the same key here.
    fn steer(&mut self, key: Key) {
        if self.paused || self.over {
            return;
        }
        match key {
            Key::Up | Key::Byte(b'w' | b'W') => self.rotate(),
            Key::Down | Key::Byte(b's' | b'S') => self.soft_drop(),
            Key::Left | Key::Byte(b'a' | b'A') => self.shift(-1),
            Key::Right | Key::Byte(b'd' | b'D') => self.shift(1),
            Key::Byte(b' ') => self.slam(),
            Key::Byte(_) => {}
        }
    }

    /// Returns whether it is time to stop.
    fn input(&mut self) -> bool {
        while let Some(key) = self.keys.read() {
            match key {
                Key::Byte(b'q' | b'Q') => return true,
                Key::Byte(b'r' | b'R') => self.restart(),
                Key::Byte(b'p' | b'P') if !self.over => self.paused = !self.paused,
                Key::Byte(b' ') if self.over => self.restart(),
                other => self.steer(other),
            }
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

        // Least useful first, since that is the order they are dropped in on a
        // narrow screen.
        let hints: &[&str] = if self.over {
            &["space: again", "q: quit"]
        } else {
            &["p: pause", "space: drop", "arrows or wasd", "q: quit"]
        };
        let state = if self.over {
            " over │"
        } else if self.paused {
            " paused │"
        } else {
            ""
        };
        status(
            &format!(
                " tetris │ {} │ lines {} │ level {} │{state}",
                self.score,
                self.lines,
                self.level + 1,
            ),
            hints,
        );
    }

    fn well_box(&self, left: usize, top: usize, scale: usize) {
        let cells = self.occupancy();
        outline(left, top, COLS * scale, ROWS * scale / 2 + 2);
        blocks(left + 1, top + 1, scale, (COLS, ROWS), |x, y| cells[y][x]);
    }

    fn next_box(&self, left: usize, top: usize, scale: usize) {
        outline(left, top, NEXT_COLS * scale, NEXT_ROWS * scale / 2 + 2);
        let shown = self.preview();
        blocks(left + 1, top + 1, scale, (NEXT_COLS, NEXT_ROWS), |x, y| {
            shown[y][x]
        });
    }

    /// The next piece as its own little grid.
    ///
    /// Centred, rather than drawn where the well would put it. They all lie
    /// flat, but not all on the same row or in the same columns, and a piece
    /// hanging off one corner of the box while the next sits in the middle
    /// looks broken.
    fn preview(&self) -> [[u8; NEXT_COLS]; NEXT_ROWS] {
        let piece = Piece::new(self.next);
        let cells: Vec<(isize, isize)> = piece.cells().collect();

        let across = || cells.iter().map(|&(col, _)| col);
        let down = || cells.iter().map(|&(_, row)| row);
        let (left, top) = (across().min().unwrap_or(0), down().min().unwrap_or(0));
        let width = across().max().unwrap_or(0) - left + 1;
        let height = down().max().unwrap_or(0) - top + 1;
        let (dx, dy) = (
            centred(width, NEXT_COLS) - left,
            centred(height, NEXT_ROWS) - top,
        );

        let mut shown = [[0_u8; NEXT_COLS]; NEXT_ROWS];
        for (col, row) in cells {
            let (Ok(x), Ok(y)) = (usize::try_from(col + dx), usize::try_from(row + dy)) else {
                continue;
            };
            if let Some(cell) = shown.get_mut(y).and_then(|shown| shown.get_mut(x)) {
                *cell = piece.colour();
            }
        }
        shown
    }
}

/// How far to shift a run of `size` blocks so it sits in the middle of `room`.
/// An odd block over goes on the right, which is where the eye misses it.
fn centred(size: isize, room: usize) -> isize {
    (isize::try_from(room).unwrap_or(size) - size) / 2
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
    // Zero is an empty cell here; the painter wants nothing at all.
    let colour = |x, y| match at(x, y) {
        0 => None,
        filled => Some(filled),
    };
    for cy in 0..rows * scale / 2 {
        for hx in 0..cols * scale {
            let upper = colour(hx / scale, cy * 2 / scale);
            let lower = colour(hx / scale, (cy * 2 + 1) / scale);
            half(left + hx, top + cy, upper, lower);
        }
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
        if game.input() {
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
