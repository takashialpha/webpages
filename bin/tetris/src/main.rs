//! Tetris.
//!
//! Two cells to a character, using half blocks, so a block is square rather
//! than twice as tall as it is wide. The well is a fixed ten by twenty, so
//! unlike the others this one is centred on whatever screen it gets rather
//! than sized to it.
//!
//! What it does, it does the way the guideline says, because a tetris that is
//! nearly right plays wrong: pieces come out of a shuffled bag, turning is
//! super rotation with its wall kicks, a piece can be held, a piece resting on
//! the stack has half a second before it locks, and gravity follows the
//! guideline's own curve.

#![deny(unsafe_code)]

use std::cell::RefCell;

use guest::{Key, Keys, half, line, notice, status, tty};

/// The well, in blocks.
const COLS: usize = 10;
const ROWS: usize = 20;

/// The boxes beside it. Four blocks across, since that is the widest piece, and
/// two block rows, since every piece spawns lying flat; the queue holds three of
/// those, two block rows apart so that three pieces read as three rather than
/// as one tall shape. Two rather than one because two block rows are one
/// character at the small scale, which keeps every piece of the queue on a whole
/// character either way. `GAP` is the space between the well and the boxes.
const SIDE: usize = 4;
const PIECE_ROWS: usize = 2;
const QUEUED: usize = 3;
const BETWEEN: usize = 2;
const GAP: usize = 1;

/// How many block rows the queue takes: the pieces, and the air between them.
const QUEUE_ROWS: usize = QUEUED * PIECE_ROWS + (QUEUED - 1) * BETWEEN;

/// A tetromino: the four cells it spawns with, the box it turns inside, and its
/// colour.
///
/// The box is the whole of super rotation. The three-wide pieces turn in a 3x3,
/// `I` turns in a 4x4 and `O` in a 2x2 that never moves, and it is those boxes
/// the wall kicks are written against.
struct Shape {
    /// Bit `size * size - 1` is the top left corner and the bits run along each
    /// row, so a shape can be written out looking like itself.
    spawn: u16,
    size: usize,
    /// One each, and the seven a tetris has always used. Never zero: zero is an
    /// empty cell.
    colour: u8,
}

/// The seven, in the order the bag is filled with them.
const SHAPES: [Shape; 7] = [
    Shape {
        spawn: 0b0000_1111_0000_0000,
        size: 4,
        colour: 14,
    },
    Shape {
        spawn: 0b11_11,
        size: 2,
        colour: 11,
    },
    Shape {
        spawn: 0b010_111_000,
        size: 3,
        colour: 13,
    },
    Shape {
        spawn: 0b011_110_000,
        size: 3,
        colour: 10,
    },
    Shape {
        spawn: 0b110_011_000,
        size: 3,
        colour: 9,
    },
    Shape {
        spawn: 0b100_111_000,
        size: 3,
        colour: 12,
    },
    Shape {
        spawn: 0b001_111_000,
        size: 3,
        colour: 3,
    },
];

/// The two that kick differently from the other five, so which table a piece
/// uses is which of the three sorts it is.
const I: usize = 0;
const O: usize = 1;

/// Where a turn is tried when the piece does not fit where the turn left it.
///
/// Super rotation's wall kick data, indexed by [`kicking`]: the turn being left,
/// and which way round it is going. Five tries each, the first of which is no
/// move at all, and a piece that fits none of them does not turn. This is what
/// lets a piece climb a wall or spin into a gap it is standing in.
///
/// `x` is across and `y` is *up*, the way the tables are written. [`Piece::nudged`]
/// is where that is turned back into a screen where `y` counts down.
type Kicks = [[(isize, isize); 5]; 8];

const JLSTZ_KICKS: Kicks = [
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)], // spawn, anticlockwise
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)], // spawn, clockwise
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],  // right, anticlockwise
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],  // right, clockwise
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)], // half, anticlockwise
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)], // half, clockwise
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)], // left, anticlockwise
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)], // left, clockwise
];

const I_KICKS: Kicks = [
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)], // spawn, anticlockwise
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)], // spawn, clockwise
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)], // right, anticlockwise
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)], // right, clockwise
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)], // half, anticlockwise
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)], // half, clockwise
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)], // left, anticlockwise
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)], // left, clockwise
];

/// Which row of a kick table a turn is tried from.
const fn kicking(turn: usize, clockwise: bool) -> usize {
    turn * 2 + if clockwise { 1 } else { 0 }
}

/// Milliseconds a row takes to fall, by level, which is the guideline's curve:
/// `(0.8 - level * 0.007)^level` seconds, counting the first level as zero. Past
/// the end it stays where it is, which is already faster than a frame.
const DROPS: [f32; 15] = [
    1000.0, 793.0, 618.0, 473.0, 355.0, 262.0, 190.0, 135.0, 94.0, 64.0, 43.0, 28.0, 18.0, 11.0,
    7.0,
];

/// Lines to the next level.
const PER_LEVEL: u32 = 10;

/// What one, two, three and four rows at once are worth, before the level
/// multiplies it. Four is worth far more than four ones, which is the whole
/// reason to stack rather than clear as you go.
const CLEARS: [u32; 4] = [100, 300, 500, 800];

/// A cell of dropping it yourself, softly and then all the way. Dropping is
/// scored at all so that knowing where a piece goes is worth something.
const SOFT: u32 = 1;
const HARD: u32 = 2;

/// How long a piece rests on the stack before it locks, and how many moves may
/// put that off. Both the guideline's: without the delay a piece cannot be slid
/// under an overhang at speed, and without the limit it can be spun in place
/// forever.
const LOCK: f32 = 500.0;
const RESETS: u32 = 15;

/// Most rows one frame will drop through, so a backgrounded tab does not come
/// back and bury the well all at once.
const CATCHUP: usize = 4;

/// A key, and what it does.
type Keying = (&'static str, &'static str);

/// What the keys do, in each of the three states the well can be in.
///
/// The one list of them: the panel beside the well and the hints in the bar are
/// both built from this. Two lists of what a key does is two places for a key to
/// go missing from, and this one went missing from one of them.
///
/// Least useful first, since that is the order the bar drops hints in when the
/// screen is too narrow to hold them all. The panel reads the list the other way
/// round, so it opens with how to leave and how to move.
const PLAYING: &[Keying] = &[
    ("arrows", "or wasd"),
    ("z", "turn back"),
    ("s", "drop one"),
    ("r", "restart"),
    ("p", "pause"),
    ("c", "hold"),
    ("w x", "turn"),
    ("space", "drop"),
    ("a d", "move"),
    ("q", "quit"),
];

const PAUSED: &[Keying] = &[("r", "restart"), ("p", "resume"), ("q", "quit")];

const OVER: &[Keying] = &[("r", "again"), ("space", "again"), ("q", "quit")];

/// Where the piece would land, the lines around everything, and a stack that is
/// out of play. One grey between them: none of it is anything you can move.
const GHOST: u8 = 8;
const FRAME: u8 = 8;
const SPENT: u8 = 8;

#[derive(Clone, Copy)]
struct Piece {
    kind: usize,
    /// Quarter turns clockwise from the way it spawns.
    turn: usize,
    /// The top left of its box, which may sit off the left of the well.
    x: isize,
    y: isize,
}

impl Piece {
    /// One of the seven where it spawns: its box at the top, in the middle, with
    /// an odd column over to the left, which is where the guideline starts them.
    fn new(kind: usize) -> Self {
        let room = COLS.saturating_sub(Self::shape(kind).size) / 2;
        Self {
            kind,
            turn: 0,
            x: isize::try_from(room).unwrap_or(0),
            y: 0,
        }
    }

    const fn shape(kind: usize) -> &'static Shape {
        &SHAPES[kind]
    }

    /// The four cells it fills, as offsets inside its box.
    ///
    /// Turned rather than tabulated: a quarter turn clockwise in a box `n` across
    /// sends `(x, y)` to `(n - 1 - y, x)`, and this is that, `turn` times. Four
    /// turns of seven shapes written out by hand is twenty-eight chances to write
    /// one down wrong, and the box is what the kicks are measured against anyway.
    fn cells(self) -> [(isize, isize); 4] {
        let shape = Self::shape(self.kind);
        let size = isize::try_from(shape.size).unwrap_or(0);
        let mut cells = [(0, 0); 4];
        let mut found = 0;

        for index in 0..shape.size * shape.size {
            if shape.spawn & (1_u16 << (shape.size * shape.size - 1 - index)) == 0 {
                continue;
            }
            let mut x = isize::try_from(index % shape.size).unwrap_or(0);
            let mut y = isize::try_from(index / shape.size).unwrap_or(0);
            for _ in 0..self.turn {
                (x, y) = (size - 1 - y, x);
            }
            // A tetromino is four cells by definition, so this cannot run off
            // the end; a shape written down wrong loses a cell rather than
            // taking the program with it.
            if let Some(cell) = cells.get_mut(found) {
                *cell = (x, y);
            }
            found += 1;
        }
        cells
    }

    const fn colour(self) -> u8 {
        Self::shape(self.kind).colour
    }

    /// The same piece somewhere else, which is how every move is tried before it
    /// is taken.
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

    /// A quarter turn round, without yet asking whether it fits.
    const fn turned(self, clockwise: bool) -> Self {
        Self {
            turn: (self.turn + if clockwise { 1 } else { 3 }) % 4,
            ..self
        }
    }

    /// One wall kick applied. The tables count `y` upwards and the screen counts
    /// it down, which is the whole of the difference.
    const fn nudged(self, (dx, dy): (isize, isize)) -> Self {
        Self {
            x: self.x + dx,
            y: self.y - dy,
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

/// The bag the pieces come out of: all seven shuffled, dealt one at a time, and
/// shuffled again once it is empty.
///
/// Not seven independent draws. Chance alone will keep an `I` from you for a
/// dozen pieces and then hand you three, which is not the game anybody means to
/// play. Every tetris for twenty years has dealt from a bag.
struct Bag {
    order: [usize; SHAPES.len()],
    dealt: usize,
}

impl Bag {
    /// Empty, so the first deal fills it.
    const fn new() -> Self {
        Self {
            order: [0; SHAPES.len()],
            dealt: SHAPES.len(),
        }
    }

    fn deal(&mut self) -> usize {
        if self.dealt >= self.order.len() {
            for (kind, slot) in self.order.iter_mut().enumerate() {
                *slot = kind;
            }
            fastrand::shuffle(&mut self.order);
            self.dealt = 0;
        }
        let kind = self.order[self.dealt];
        self.dealt += 1;
        kind
    }
}

struct Game {
    well: [[u8; COLS]; ROWS],
    piece: Piece,
    bag: Bag,
    /// What is coming, oldest first.
    queue: [usize; QUEUED],
    /// What is being put by, and whether this piece has already swapped with it.
    /// One swap a piece: a piece comes back at the top in its first turn, so two
    /// would be a way to stall for as long as you liked.
    held: Option<usize>,
    swapped: bool,
    /// Milliseconds owed to gravity, and milliseconds this piece has spent
    /// resting on the stack, with how many times a move has put its lock off.
    due: f32,
    rest: f32,
    resets: u32,
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
            // Replaced before it is ever drawn: `restart` runs before the first
            // frame. A `const` one cannot come from `Piece::new`.
            piece: Piece {
                kind: 0,
                turn: 0,
                x: 0,
                y: 0,
            },
            bag: Bag::new(),
            queue: [0; QUEUED],
            held: None,
            swapped: false,
            due: 0.0,
            rest: 0.0,
            resets: 0,
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
        self.held = None;
        self.score = 0;
        self.lines = 0;
        self.level = 0;
        self.paused = false;
        self.over = false;
        self.keys.forget();

        // Dealt before the first piece is taken, so the queue is full from the
        // first frame rather than filling up over the first three pieces.
        let mut bag = Bag::new();
        self.queue = [(); QUEUED].map(|()| bag.deal());
        self.bag = bag;
        self.spawn();
    }

    /// Whether a piece could sit exactly there: on the well, and on nothing.
    fn free(&self, piece: Piece) -> bool {
        piece
            .cells()
            .into_iter()
            .all(|(dx, dy)| spot(piece, dx, dy).is_some_and(|(x, y)| self.well[y][x] == 0))
    }

    /// Whether the piece is standing on something rather than falling.
    fn resting(&self) -> bool {
        !self.free(self.piece.below())
    }

    /// Takes the next piece out of the queue and tops the queue up.
    fn spawn(&mut self) {
        let kind = self.take();
        self.bring(kind);
        self.swapped = false;
    }

    /// The next kind, with another dealt into the back of the queue.
    fn take(&mut self) -> usize {
        let kind = self.queue[0];
        self.queue.rotate_left(1);
        if let Some(last) = self.queue.last_mut() {
            *last = self.bag.deal();
        }
        kind
    }

    /// Puts a piece at the top and starts its clocks. The game ends here, when
    /// there is no longer room to put one.
    fn bring(&mut self, kind: usize) {
        self.piece = Piece::new(kind);
        self.due = 0.0;
        self.rest = 0.0;
        self.resets = 0;
        self.over = !self.free(self.piece);
    }

    /// Swaps the falling piece with the held one, or puts it by and takes the
    /// next.
    fn hold(&mut self) {
        if self.swapped {
            return;
        }
        let swap = self.held;
        self.held = Some(self.piece.kind);
        let kind = swap.unwrap_or_else(|| self.take());
        self.bring(kind);
        self.swapped = true;
    }

    fn shift(&mut self, dx: isize) {
        let moved = self.piece.beside(dx);
        if self.free(moved) {
            self.piece = moved;
            self.moved();
        }
    }

    /// Turns, kicking out of whatever is in the way if one of the five tries
    /// fits.
    fn rotate(&mut self, clockwise: bool) {
        // `O` is the same four cells whichever way round it is, and has no kick
        // table because it never needs one.
        if self.piece.kind == O {
            return;
        }

        let turned = self.piece.turned(clockwise);
        let kicks = if self.piece.kind == I {
            &I_KICKS
        } else {
            &JLSTZ_KICKS
        };
        for kick in kicks[kicking(self.piece.turn, clockwise)] {
            let tried = turned.nudged(kick);
            if self.free(tried) {
                self.piece = tried;
                self.moved();
                return;
            }
        }
    }

    /// A move that landed. Moving a piece that is already resting puts its lock
    /// off, which is what lets one be slid along the floor at a speed where it
    /// would otherwise stick where it fell. Fifteen times, and then the clock
    /// runs however much it is moved.
    fn moved(&mut self) {
        if self.resets < RESETS && self.resting() {
            self.resets += 1;
            self.rest = 0.0;
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
            self.score += SOFT;
        }
    }

    /// Straight to the bottom, where it locks at once: a piece dropped on
    /// purpose is not asking for the resting delay.
    fn slam(&mut self) {
        let landing = self.ghost();
        self.score += u32::try_from(landing.y - self.piece.y).unwrap_or(0) * HARD;
        self.piece = landing;
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
        self.score +=
            CLEARS.get(cleared.saturating_sub(1)).copied().unwrap_or(0) * (self.level + 1);
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
                break;
            }
        }
        self.due = self.due.min(pace);

        // Standing on the stack rather than falling: it has the delay above
        // before it locks, and a move can put that off.
        if self.resting() {
            self.rest += elapsed;
            if self.rest >= LOCK {
                self.settle();
            }
        } else {
            self.rest = 0.0;
            self.resets = 0;
        }
    }

    /// The keys that move the piece, which do nothing while paused or after the
    /// end. An arrow and the letter under it are the same key here, and `z` and
    /// `x` turn the two ways round an arcade cabinet does.
    fn steer(&mut self, key: Key) {
        if self.paused || self.over {
            return;
        }
        match key {
            Key::Up | Key::Byte(b'w' | b'W' | b'x' | b'X') => self.rotate(true),
            Key::Byte(b'z' | b'Z') => self.rotate(false),
            Key::Down | Key::Byte(b's' | b'S') => self.soft_drop(),
            Key::Left | Key::Byte(b'a' | b'A') => self.shift(-1),
            Key::Right | Key::Byte(b'd' | b'D') => self.shift(1),
            Key::Byte(b' ') => self.slam(),
            Key::Byte(b'c' | b'C') => self.hold(),
            Key::Byte(_) => {}
        }
    }

    /// Returns whether it is time to stop.
    fn input(&mut self) -> bool {
        while let Some(key) = self.keys.read() {
            match key {
                // The terminal answers `q` itself, before a program is handed
                // the key, so that nothing running in it can trap you. This is
                // here because a program that offers `q: quit` should mean it
                // wherever it runs.
                Key::Byte(b'q' | b'Q') => return true,
                Key::Byte(b'r' | b'R') => self.restart(),
                Key::Byte(b'p' | b'P') if !self.over => self.paused = !self.paused,
                Key::Byte(b' ') if self.over => self.restart(),
                other => self.steer(other),
            }
        }
        false
    }

    /// What the well is doing, when it is not simply running. The one place the
    /// two stopped states are named, so the band across the well and the bar
    /// under it cannot disagree about which one it is in.
    const fn state(&self) -> Option<&'static str> {
        if self.over {
            Some("game over")
        } else if self.paused {
            Some("paused")
        } else {
            None
        }
    }

    /// The well as it should be drawn: the ghost and the falling piece over it
    /// while it is running, grey once it is over, and nothing at all while it is
    /// paused.
    fn occupancy(&self) -> [[u8; COLS]; ROWS] {
        // A pause takes the stack away, the way a tetris that would rather not
        // be studied with the clock stopped does.
        if self.paused {
            return [[0; COLS]; ROWS];
        }

        let mut cells = self.well;
        if self.over {
            for row in &mut cells {
                for cell in row.iter_mut().filter(|cell| **cell != 0) {
                    *cell = SPENT;
                }
            }
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
            line(0, 1, "q: quit", SPENT);
            return;
        };

        let left = (width - needs_w(scale)) / 2;
        let top = (height - needs_h(scale)) / 2;
        self.well_box(left, top, scale);

        // Both boxes in one column beside the well, which is what keeps the
        // whole thing as wide as a well and a box however many pieces the queue
        // shows.
        let side = left + COLS * scale + 2 + GAP;
        self.hold_box(side, top, scale);
        self.next_box(side, top + box_height(PIECE_ROWS, scale) + GAP, scale);

        // What the keys do, in the margin to the left of the well, where the
        // screen is wide enough to have a margin. One column in from the edge.
        let (keys, what) = Self::columns();
        let panel = keys + 2 + what;
        if left > panel + GAP {
            let rows = self.keying().len();
            let down = box_height(ROWS, scale).saturating_sub(rows) / 2;
            self.panel(left - GAP - panel, top + down);
        }

        self.bar();
    }

    fn well_box(&self, left: usize, top: usize, scale: usize) {
        let cells = self.occupancy();
        let inner = COLS * scale;
        let height = box_height(ROWS, scale);
        outline(left, top, inner, height, "");
        blocks(left + 1, top + 1, scale, (COLS, ROWS), |x, y| cells[y][x]);

        // Over the picture, and only once there is no picture left to interrupt.
        // A stopped well and a running one are otherwise the same well, which is
        // the thing that has to be obvious without reading anything.
        if let Some(word) = self.state() {
            notice(left + 1, top + height / 2, inner, word);
        }
    }

    /// What is being put by, greyed out once this piece has spent its swap,
    /// since it is not a piece you can take back until the next one.
    fn hold_box(&self, left: usize, top: usize, scale: usize) {
        outline(
            left,
            top,
            SIDE * scale,
            box_height(PIECE_ROWS, scale),
            "hold",
        );
        let shown = self.held.map_or([[0; SIDE]; PIECE_ROWS], |kind| {
            let colour = if self.swapped {
                SPENT
            } else {
                Piece::new(kind).colour()
            };
            framed(kind, colour)
        });
        blocks(left + 1, top + 1, scale, (SIDE, PIECE_ROWS), |x, y| {
            shown[y][x]
        });
    }

    /// What is coming, oldest at the top.
    fn next_box(&self, left: usize, top: usize, scale: usize) {
        outline(
            left,
            top,
            SIDE * scale,
            box_height(QUEUE_ROWS, scale),
            "next",
        );

        let mut shown = [[0_u8; SIDE]; QUEUE_ROWS];
        for (slot, &kind) in self.queue.iter().enumerate() {
            for (row, cells) in framed(kind, Piece::new(kind).colour())
                .into_iter()
                .enumerate()
            {
                if let Some(into) = shown.get_mut(slot * (PIECE_ROWS + BETWEEN) + row) {
                    *into = cells;
                }
            }
        }
        blocks(left + 1, top + 1, scale, (SIDE, QUEUE_ROWS), |x, y| {
            shown[y][x]
        });
    }

    /// The keys that do something in the state the game is in. A hint for a key
    /// that is being ignored is worse than no hint.
    const fn keying(&self) -> &'static [Keying] {
        if self.over {
            OVER
        } else if self.paused {
            PAUSED
        } else {
            PLAYING
        }
    }

    fn bar(&self) {
        let hints: Vec<String> = self
            .keying()
            .iter()
            .map(|(key, what)| format!("{key}: {what}"))
            .collect();
        let hints: Vec<&str> = hints.iter().map(String::as_str).collect();

        status(
            &format!(
                " tetris │ {} │ lines {} │ level {} │{}",
                self.score,
                self.lines,
                self.level + 1,
                self.state()
                    .map_or_else(String::new, |word| format!(" {word} │")),
            ),
            &hints,
        );
    }

    /// The keys down the margin beside the well, most useful first.
    ///
    /// Only where there is a margin to put them in. The well is the one fixed
    /// thing on the screen, so a desktop has room either side of it and a phone
    /// has none; the bar's hints are what a phone gets, which is why they are
    /// the same list. The well does not move to make room, so nothing on the
    /// screen shifts because a panel appeared next to it.
    fn panel(&self, left: usize, top: usize) {
        let (keys, _) = Self::columns();
        for (row, (key, what)) in self.keying().iter().rev().enumerate() {
            line(left, top + row, key, 7);
            line(left + keys + 2, top + row, what, SPENT);
        }
    }

    /// The panel's two columns, measured on the keys of a game in play, which is
    /// the longest the list gets. Measured rather than written down so that a key
    /// with a longer name cannot quietly overlap the column beside it, and taken
    /// from the one state so the panel does not change width when the game stops.
    fn columns() -> (usize, usize) {
        let keys = PLAYING
            .iter()
            .map(|(key, _)| key.chars().count())
            .max()
            .unwrap_or(0);
        let what = PLAYING
            .iter()
            .map(|(_, what)| what.chars().count())
            .max()
            .unwrap_or(0);
        (keys, what)
    }
}

/// One piece centred in a grid of its own, rather than drawn where the well
/// would put it. They do not all start in the same corner of their own box, and
/// one piece hanging off the edge of the box while the next sits in the middle
/// of it looks broken.
fn framed(kind: usize, colour: u8) -> [[u8; SIDE]; PIECE_ROWS] {
    let cells = Piece::new(kind).cells();
    let across = || cells.iter().map(|&(col, _)| col);
    let down = || cells.iter().map(|&(_, row)| row);
    let (left, top) = (across().min().unwrap_or(0), down().min().unwrap_or(0));
    let width = across().max().unwrap_or(0) - left + 1;
    let height = down().max().unwrap_or(0) - top + 1;
    let (dx, dy) = (
        centred(width, SIDE) - left,
        centred(height, PIECE_ROWS) - top,
    );

    let mut shown = [[0_u8; SIDE]; PIECE_ROWS];
    for (col, row) in cells {
        let (Ok(x), Ok(y)) = (usize::try_from(col + dx), usize::try_from(row + dy)) else {
            continue;
        };
        if let Some(cell) = shown.get_mut(y).and_then(|shown| shown.get_mut(x)) {
            *cell = colour;
        }
    }
    shown
}

/// How far to shift a run of `size` blocks so it sits in the middle of `room`.
/// An odd block over goes on the right, which is where the eye misses it.
fn centred(size: isize, room: usize) -> isize {
    (isize::try_from(room).unwrap_or(size) - size) / 2
}

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

/// The well and the column of boxes beside it, each with a border.
const fn needs_w(scale: usize) -> usize {
    (COLS * scale + 2) + GAP + (SIDE * scale + 2)
}

/// The well, its border, and the status bar under it. The two boxes stacked
/// beside it come to less than the well at either scale, so the well is the
/// thing that has to fit.
const fn needs_h(scale: usize) -> usize {
    box_height(ROWS, scale) + 1
}

/// A box holding `rows` block rows: half a character each, and a border above
/// and below.
const fn box_height(rows: usize, scale: usize) -> usize {
    rows * scale / 2 + 2
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

/// A box `inner` characters wide inside its borders and `height` tall including
/// them, with its name in the top rule when there is room for the name.
fn outline(left: usize, top: usize, inner: usize, height: usize, title: &str) {
    let rule = "─".repeat(inner);
    let named = title.chars().count();
    let head = if named <= inner {
        let pad = inner - named;
        format!(
            "{}{title}{}",
            "─".repeat(pad / 2),
            "─".repeat(pad - pad / 2)
        )
    } else {
        rule.clone()
    };

    line(left, top, &format!("┌{head}┐"), FRAME);
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
