//! The alternate screen: a grid a program draws into.
//!
//! A terminal keeps two buffers. The scrollback is the normal one and is left
//! alone; this is the other, thrown away when the program exits. It is why
//! leaving `vim` does not eat your shell history.
//!
//! Colours are indices into the sixteen a console has. The stylesheet resolves
//! them, so a program follows `theme` without knowing themes exist.

/// The sixteen colours a terminal has.
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

/// A run of characters sharing one pair of colours. Rows are drawn as runs,
/// not cells, so a mostly empty row costs almost nothing.
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

    /// Resizes, keeping whatever still fits.
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

    /// Writes one cell. Off the grid is ignored, not an error: a box drawn a
    /// little too wide should lose the overhang, not stop.
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
