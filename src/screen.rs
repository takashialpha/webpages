//! The alternate screen: a grid of cells a program draws into.
//!
//! A terminal keeps two buffers. The normal one scrolls and is what the shell
//! prints into, which here is the scrollback and is left alone. The alternate
//! one is a fixed grid the size of the window, which a full screen program
//! takes over and which is thrown away when it exits, leaving the scrollback
//! exactly as it was. That is the same arrangement, and the reason `vim`
//! leaving does not eat your shell history.
//!
//! Colours are indices into the sixteen a console has, resolved by the
//! stylesheet, so a program follows whatever `theme` is set to without knowing
//! that themes exist.

/// The sixteen colours, in the order every terminal numbers them.
pub const COLORS: usize = 16;

/// Default foreground and background, as indices into that set.
pub const FG: u8 = 7;
pub const BG: u8 = 0;

/// One character and the colours it is drawn in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: u8,
    pub bg: u8,
}

impl Cell {
    pub const BLANK: Self = Self {
        ch: ' ',
        fg: FG,
        bg: BG,
    };
}

impl Default for Cell {
    fn default() -> Self {
        Self::BLANK
    }
}

/// A run of characters sharing one pair of colours, which is what a row is cut
/// into before it is drawn: one element per run rather than one per cell, so a
/// mostly empty row costs almost nothing.
#[derive(Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub fg: u8,
    pub bg: u8,
}

/// The grid itself.
pub struct Screen {
    cols: usize,
    rows: usize,
    cells: Vec<Cell>,
}

impl Screen {
    #[must_use]
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::BLANK; cols * rows],
        }
    }

    #[must_use]
    pub const fn cols(&self) -> usize {
        self.cols
    }

    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Resizes to a new shape, keeping whatever still fits. A program that
    /// redraws every frame will not notice; one that does not keeps its
    /// picture rather than having it blanked out from under it.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        if cols == self.cols && rows == self.rows {
            return;
        }
        let mut next = vec![Cell::BLANK; cols * rows];
        for y in 0..rows.min(self.rows) {
            for x in 0..cols.min(self.cols) {
                next[y * cols + x] = self.cells[y * self.cols + x];
            }
        }
        self.cols = cols;
        self.rows = rows;
        self.cells = next;
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::BLANK);
    }

    /// Writes one cell, ignoring anything off the grid. Out of range is not an
    /// error: a program that draws a box slightly too wide should lose the
    /// overhang, not stop.
    pub fn put(&mut self, x: usize, y: usize, ch: char, fg: u8, bg: u8) {
        if x >= self.cols || y >= self.rows {
            return;
        }
        self.cells[y * self.cols + x] = Cell {
            ch,
            fg: fg % COLORS_U8,
            bg: bg % COLORS_U8,
        };
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Cell {
        if x >= self.cols || y >= self.rows {
            return Cell::BLANK;
        }
        self.cells[y * self.cols + x]
    }

    /// Cuts one row into runs of matching colour.
    #[must_use]
    pub fn runs(&self, y: usize) -> Vec<Run> {
        if y >= self.rows {
            return Vec::new();
        }

        let mut runs: Vec<Run> = Vec::new();
        for cell in &self.cells[y * self.cols..(y + 1) * self.cols] {
            match runs.last_mut() {
                Some(run) if run.fg == cell.fg && run.bg == cell.bg => run.text.push(cell.ch),
                _ => runs.push(Run {
                    text: cell.ch.to_string(),
                    fg: cell.fg,
                    bg: cell.bg,
                }),
            }
        }
        runs
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "sixteen fits in a u8 by inspection"
)]
const COLORS_U8: u8 = COLORS as u8;
