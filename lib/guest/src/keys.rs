//! Reading the keyboard, escape sequences included.

use crate::tty;

/// One keypress.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// An ordinary byte, which for a letter is that letter.
    Byte(u8),
    Up,
    Down,
    Left,
    Right,
}

/// Where we are in an escape sequence.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reading {
    Plain,
    Escaped,
    Bracket,
}

/// The keyboard, with the escape sequence an arrow arrives as folded back into
/// one key.
///
/// An arrow is `esc [ A`: three bytes, and they can arrive across three
/// frames. Reading them one at a time is how a program ends up steering on a
/// typed `A`, or doing whatever it binds `[` to every time anyone presses an
/// arrow.
pub struct Keys {
    reading: Reading,
}

impl Keys {
    pub const fn new() -> Self {
        Self {
            reading: Reading::Plain,
        }
    }

    /// The next key, or `None` when nothing is waiting.
    ///
    /// Not called `next`: this is not an iterator, and half a sequence leaves
    /// state behind for the frame that finishes it.
    pub fn read(&mut self) -> Option<Key> {
        loop {
            let byte = tty::pressed()?;
            self.reading = match (self.reading, byte) {
                // An escape always starts a new sequence, whatever was half
                // read. That is what a terminal does with it, and it is what
                // stops a stray one leaving the `[` of the next arrow to be
                // read as somebody pressing `[`.
                (_, 0x1b) => Reading::Escaped,
                (Reading::Escaped, b'[') => Reading::Bracket,
                // The letter after `esc [` is the arrow.
                (Reading::Bracket, letter) => {
                    self.reading = Reading::Plain;
                    return Some(match letter {
                        b'A' => Key::Up,
                        b'B' => Key::Down,
                        b'C' => Key::Right,
                        b'D' => Key::Left,
                        other => Key::Byte(other),
                    });
                }
                // Anything else, an escape that led nowhere included.
                (_, byte) => {
                    self.reading = Reading::Plain;
                    return Some(Key::Byte(byte));
                }
            };
        }
    }

    /// Forgets a half-read sequence. For a program starting again, so a key
    /// pressed before the restart cannot finish a sequence after it.
    pub const fn forget(&mut self) {
        self.reading = Reading::Plain;
    }
}

impl Default for Keys {
    fn default() -> Self {
        Self::new()
    }
}
