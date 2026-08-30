//! The board itself, and the two orders it is kept in.

use super::{BLANK, COLS, ROWS, printable};

/// The board itself. Fixed size, whatever arrives.
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

    /// The stored form: exactly [`ROWS`] lines of exactly [`COLS`] characters.
    #[must_use]
    pub fn render(&self) -> String {
        let mut text = String::with_capacity(ROWS * (COLS + 1));
        for row in &self.0 {
            text.extend(row.iter().map(|&byte| char::from(byte)));
            text.push('\n');
        }
        text
    }

    /// Reads the stored form back, forgivingly: a short line is padded and a
    /// long one is cut, so hand-editing cannot leave the board a shape the rest
    /// of the code does not expect.
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
