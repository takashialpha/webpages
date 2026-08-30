//! Snake.
//!
//! Two cells to a character, using half blocks, so the field is square rather
//! than twice as tall as it is wide. `0,0` is the top left and `y` counts down,
//! the way the screen is drawn.

#![deny(unsafe_code)]

use std::cell::RefCell;
use std::collections::VecDeque;

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

/// Milliseconds a step lasts, before and after growing.
const START: f32 = 130.0;
const FASTEST: f32 = 55.0;

const SNAKE: u8 = 10;
const HEAD: u8 = 15;
const FOOD: u8 = 11;
const DEAD: u8 = 9;
const FRAME: u8 = 8;
const BAR_FG: u8 = 0;
const BAR_BG: u8 = 7;

/// Rows the frame and the status bar take, leaving the rest to play in.
const CHROME: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Point {
    x: usize,
    y: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Way {
    Up,
    Down,
    Left,
    Right,
}

impl Way {
    /// Whether turning this way would fold the snake back into itself.
    const fn reverses(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Up, Self::Down)
                | (Self::Down, Self::Up)
                | (Self::Left, Self::Right)
                | (Self::Right, Self::Left)
        )
    }
}

/// Where we are in an escape sequence. Arrows arrive as `esc [ A`, and without
/// tracking that, a typed `A` would steer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reading {
    Plain,
    Escaped,
    Bracket,
}

struct Game {
    snake: VecDeque<Point>,
    heading: Way,
    /// Read at the moment of a step, so two turns in one frame cannot fold the
    /// snake back on itself.
    turning: Way,
    food: Point,
    cols: usize,
    rows: usize,
    due: f32,
    score: u32,
    best: u32,
    dead: bool,
    reading: Reading,
}

impl Game {
    const fn new() -> Self {
        Self {
            snake: VecDeque::new(),
            heading: Way::Right,
            turning: Way::Right,
            food: Point { x: 0, y: 0 },
            cols: 0,
            rows: 0,
            due: 0.0,
            score: 0,
            best: 0,
            dead: false,
            reading: Reading::Plain,
        }
    }

    /// The field inside the frame, two game rows to a character row.
    fn shape() -> (usize, usize) {
        (
            tty::width().saturating_sub(2),
            tty::height().saturating_sub(CHROME) * 2,
        )
    }

    fn fits(&self) -> bool {
        Self::shape() == (self.cols, self.rows)
    }

    fn restart(&mut self) {
        (self.cols, self.rows) = Self::shape();
        self.snake.clear();
        self.reading = Reading::Plain;
        if self.cols < 8 || self.rows < 8 {
            return;
        }
        let middle = Point {
            x: self.cols / 2,
            y: self.rows / 2,
        };
        for back in 0..3 {
            self.snake.push_back(Point {
                x: middle.x - back,
                y: middle.y,
            });
        }
        self.heading = Way::Right;
        self.turning = Way::Right;
        self.score = 0;
        self.dead = false;
        self.due = 0.0;
        self.drop_food();
    }

    fn drop_food(&mut self) {
        // A few tries, then the first free cell: landing on one at random gets
        // unlikely once the snake fills the board.
        for _ in 0..64 {
            let spot = Point {
                x: fastrand::usize(..self.cols),
                y: fastrand::usize(..self.rows),
            };
            if !self.snake.contains(&spot) {
                self.food = spot;
                return;
            }
        }
        for y in 0..self.rows {
            for x in 0..self.cols {
                let spot = Point { x, y };
                if !self.snake.contains(&spot) {
                    self.food = spot;
                    return;
                }
            }
        }
    }

    /// One cell along, or `None` at a wall.
    fn ahead(&self) -> Option<Point> {
        let head = *self.snake.front()?;
        let (x, y) = match self.turning {
            Way::Up => (head.x, head.y.checked_sub(1)?),
            Way::Down => (head.x, head.y.checked_add(1).filter(|y| *y < self.rows)?),
            Way::Left => (head.x.checked_sub(1)?, head.y),
            Way::Right => (head.x.checked_add(1).filter(|x| *x < self.cols)?, head.y),
        };
        Some(Point { x, y })
    }

    fn step(&mut self) {
        if self.dead {
            return;
        }
        self.heading = self.turning;
        let Some(next) = self.ahead() else {
            self.dead = true;
            return;
        };

        // The tail moves out as the head moves in, so the cell it leaves is
        // free. Unless there is food ahead, when it stays put.
        let eating = next == self.food;
        let body = self.snake.iter().rev().skip(usize::from(!eating));
        if body.clone().any(|part| *part == next) {
            self.dead = true;
            return;
        }

        self.snake.push_front(next);
        if eating {
            self.score += 1;
            self.best = self.best.max(self.score);
            self.drop_food();
        } else {
            self.snake.pop_back();
        }
    }

    /// Faster as it grows, down to a floor.
    fn pace(&self) -> f32 {
        let eaten = f32::from(u16::try_from(self.score.min(40)).unwrap_or(0));
        (START - eaten * 2.0).max(FASTEST)
    }

    /// Returns whether it is time to stop.
    fn keys(&mut self) -> bool {
        while let Some(byte) = tty::pressed() {
            self.reading = match (self.reading, byte) {
                (Reading::Plain, 0x1b) => Reading::Escaped,
                (Reading::Escaped, b'[') => Reading::Bracket,
                // The letter after `esc [` is the arrow.
                (Reading::Bracket, letter) => {
                    match letter {
                        b'A' => self.turn(Way::Up),
                        b'B' => self.turn(Way::Down),
                        b'C' => self.turn(Way::Right),
                        b'D' => self.turn(Way::Left),
                        _ => {}
                    }
                    Reading::Plain
                }
                (_, letter) => {
                    match letter {
                        b'q' | b'Q' => return true,
                        b'r' | b'R' => self.restart(),
                        b' ' if self.dead => self.restart(),
                        b'w' | b'W' => self.turn(Way::Up),
                        b's' | b'S' => self.turn(Way::Down),
                        b'a' | b'A' => self.turn(Way::Left),
                        b'd' | b'D' => self.turn(Way::Right),
                        _ => {}
                    }
                    Reading::Plain
                }
            };
        }
        false
    }

    const fn turn(&mut self, way: Way) {
        if !way.reverses(self.heading) {
            self.turning = way;
        }
    }

    /// What is in each cell, laid out once so drawing does not search the
    /// snake for every one.
    fn occupancy(&self) -> Vec<Option<u8>> {
        let mut cells = vec![None; self.cols * self.rows];
        let mut mark = |at: Point, colour| {
            if at.x < self.cols && at.y < self.rows {
                cells[at.y * self.cols + at.x] = Some(colour);
            }
        };

        mark(self.food, FOOD);
        for part in self.snake.iter().skip(1) {
            mark(*part, SNAKE);
        }
        if let Some(head) = self.snake.front() {
            mark(*head, if self.dead { DEAD } else { HEAD });
        }
        cells
    }

    fn draw(&self) {
        let cells = self.occupancy();
        let at = |x: usize, y: usize| cells[y * self.cols + x];

        // So the wall you can die against is one you can see.
        let rule = "─".repeat(self.cols);
        let bottom = tty::height().saturating_sub(2);
        line(0, &format!("┌{rule}┐"), FRAME);
        line(bottom, &format!("└{rule}┘"), FRAME);

        for cy in 0..self.rows / 2 {
            let row = cy + 1;
            tty::draw(0, row, '│', FRAME, 0);
            for x in 0..self.cols {
                // The upper half is the smaller `y`, since `y` counts down.
                let (upper, lower) = (at(x, cy * 2), at(x, cy * 2 + 1));
                match (upper, lower) {
                    (None, None) => tty::draw(x + 1, row, ' ', 0, 0),
                    (Some(colour), None) => tty::draw(x + 1, row, '▀', colour, 0),
                    (None, Some(colour)) => tty::draw(x + 1, row, '▄', colour, 0),
                    // Two colours in one character: the lower half becomes the
                    // background, so both show.
                    (Some(up), Some(down)) => tty::draw(x + 1, row, '▀', up, down),
                }
            }
            tty::draw(self.cols + 1, row, '│', FRAME, 0);
        }
        self.status();
    }

    fn status(&self) {
        let row = tty::height().saturating_sub(1);
        let width = tty::width();

        let left: Vec<char> = format!(
            " snake │ {} │ best {} │{}",
            self.score,
            self.best,
            if self.dead { " dead │" } else { "" },
        )
        .chars()
        .collect();

        let mut hints = if self.dead {
            vec!["space: again", "q: quit"]
        } else {
            vec!["r: restart", "arrows or wasd", "q: quit"]
        };
        let right = loop {
            let right: Vec<char> = format!("│ {} ", hints.join(" │ ")).chars().collect();
            if hints.len() == 1 || left.len() + right.len() <= width {
                break right;
            }
            hints.remove(0);
        };

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

/// One row of text, from the left edge.
fn line(row: usize, text: &str, colour: u8) {
    for (x, ch) in text.chars().enumerate() {
        tty::draw(x, row, ch, colour, 0);
    }
}

thread_local! {
    static GAME: RefCell<Game> = const { RefCell::new(Game::new()) };
}

/// Called once a frame by the terminal, which owns the loop. Non-zero quits.
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
        if !game.fits() {
            game.restart();
        }
        if game.snake.is_empty() {
            return 0;
        }

        game.due += elapsed;
        let pace = game.pace();
        // Bounded, so a backgrounded tab does not come back and run every
        // step it missed at once.
        for _ in 0..4 {
            if game.due < pace {
                break;
            }
            game.due -= pace;
            game.step();
        }
        game.due = game.due.min(pace);

        game.draw();
        0
    })
}

/// Run before the first frame, so the board is already there when it arrives.
fn main() {
    GAME.with_borrow_mut(|game| {
        game.restart();
        game.draw();
    });
}
