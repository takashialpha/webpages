//! The chrome the programs draw the same way.

use crate::tty;

/// Reverse video, the way a full screen program marks the row that is not part
/// of the picture.
const BAR_FG: u8 = 0;
const BAR_BG: u8 = 7;

/// One character holding two stacked cells, using the half blocks the ROM font
/// carries.
///
/// A character is twice as tall as it is wide, so half of one is square, which
/// is what a grid of anything is supposed to look like. `None` is an empty
/// half.
pub fn half(x: usize, y: usize, upper: Option<u8>, lower: Option<u8>) {
    match (upper, lower) {
        (None, None) => tty::draw(x, y, ' ', 0, 0),
        (Some(up), None) => tty::draw(x, y, '▀', up, 0),
        (None, Some(down)) => tty::draw(x, y, '▄', down, 0),
        // One colour fills the character, rather than being drawn as a half
        // block over a background of itself.
        (Some(up), Some(down)) if up == down => tty::draw(x, y, '█', up, 0),
        // Two colours in one character: the lower half becomes the background,
        // so both show.
        (Some(up), Some(down)) => tty::draw(x, y, '▀', up, down),
    }
}

/// A word across the middle of a field, in the same reverse video as the status
/// bar, for a program that has stopped without leaving the screen.
///
/// Reverse video because it is the one thing on the screen that is not part of
/// the picture, which is what the bar uses it for. A stopped field and a running
/// one have to be told apart without reading anything, so this is a band across
/// the picture rather than a word tucked into the corner of it.
///
/// Centred in `width` and cut to it, since a field can be narrower than a word.
pub fn notice(x: usize, y: usize, width: usize, text: &str) {
    let text: Vec<char> = text.chars().collect();
    let start = x + width.saturating_sub(text.len()) / 2;
    for at in x..x + width {
        // Before the text starts, `at - start` wraps to something huge, which
        // is as absent as anything past the end.
        let ch = text.get(at.wrapping_sub(start)).copied().unwrap_or(' ');
        tty::draw(at, y, ch, BAR_FG, BAR_BG);
    }
}

/// A run of text, left to right from where it starts.
pub fn line(x: usize, y: usize, text: &str, colour: u8) {
    for (offset, ch) in text.chars().enumerate() {
        tty::draw(x + offset, y, ch, colour, 0);
    }
}

/// The status bar across the bottom row: what the program is and what it is
/// doing on the left, what the keys do on the right.
///
/// Both halves close with a separator, so the space between them reads as a gap
/// in one bar rather than as two loose ends. Hints are dropped from the front
/// when the screen is too narrow to hold them all, so put the least useful
/// first.
pub fn status(left: &str, hints: &[&str]) {
    let row = tty::height().saturating_sub(1);
    let width = tty::width();
    let left: Vec<char> = left.chars().collect();

    let mut hints = hints;
    let right = loop {
        let right: Vec<char> = format!("│ {} ", hints.join(" │ ")).chars().collect();
        if hints.len() <= 1 || left.len() + right.len() <= width {
            break right;
        }
        hints = &hints[1..];
    };

    // Even one hint may not fit a very narrow screen, and half a word is worse
    // than none.
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
