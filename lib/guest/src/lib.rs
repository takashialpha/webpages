//! What a program needs from the terminal running it.
//!
//! Not under `bin/`, which holds programs, and this is not one. It is the part
//! four of them had a copy of each: the `tty` imports, the escape sequence an
//! arrow arrives as, and the chrome they all draw the same way. Four copies of
//! anything drift, and these had started to.

#![deny(unsafe_code)]

pub mod tty;

mod keys;
mod paint;

pub use keys::{Key, Keys};
pub use paint::{half, line, status};
