//! The host side of a running guest: its memory, its screen, its keyboard, and
//! whatever it has printed.
//!
//! Every import in `imports.rs` works through this, so reaching into the
//! guest's memory happens in one place and is bounds-checked there.

use std::collections::VecDeque;

use crate::screen::Screen;

/// Success, in WASI's numbering, and the one failure this shim reports.
pub const OK: u32 = 0;
pub const EBADF: u32 = 8;

/// The file descriptors a terminal program knows about.
pub const STDIN: u32 = 0;
pub const STDOUT: u32 = 1;
pub const STDERR: u32 = 2;

/// WASI's wall clock. Everything else it can ask for only counts up, which is
/// what the page's own clock does.
pub const REALTIME: u32 = 0;

/// The host side of a running guest.
pub struct Host {
    /// Filled in after instantiation: memory is one of the guest's exports, so
    /// it does not exist yet when the imports are built.
    pub memory: Option<js_sys::WebAssembly::Memory>,
    /// The screen the guest draws on. Swapped in for a call and back out
    /// after, so the terminal keeps it.
    pub screen: Screen,
    /// Keys waiting to be read, oldest first.
    pub keys: VecDeque<i32>,
    /// What the guest has written to stdout and stderr.
    pub out: Vec<u8>,
    /// What the guest passed to `proc_exit`, if it has.
    pub exit: Option<i32>,
}

impl Host {
    pub fn new() -> Self {
        Self {
            memory: None,
            screen: Screen::new(0, 0),
            keys: VecDeque::new(),
            out: Vec::new(),
            exit: None,
        }
    }

    /// The guest's memory. Rebuilt each time, because growing it detaches the
    /// old buffer and a stale view reads as empty.
    pub fn view(&self) -> Option<js_sys::Uint8Array> {
        self.memory
            .as_ref()
            .map(|memory| js_sys::Uint8Array::new(&memory.buffer()))
    }

    pub fn read(&self, ptr: u32, len: u32) -> Vec<u8> {
        let Some(view) = self.view() else {
            return Vec::new();
        };
        if u64::from(ptr) + u64::from(len) > u64::from(view.length()) {
            return Vec::new();
        }
        let mut bytes = vec![0; len as usize];
        view.subarray(ptr, ptr + len).copy_to(&mut bytes);
        bytes
    }

    pub fn write(&self, ptr: u32, bytes: &[u8]) {
        let Some(view) = self.view() else { return };
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        if u64::from(ptr) + u64::from(len) > u64::from(view.length()) {
            return;
        }
        view.subarray(ptr, ptr + len).copy_from(bytes);
    }

    /// A little-endian `u32`, as WASI lays out every number.
    pub fn read_u32(&self, ptr: u32) -> u32 {
        let bytes = self.read(ptr, 4);
        u32::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            bytes.get(1).copied().unwrap_or(0),
            bytes.get(2).copied().unwrap_or(0),
            bytes.get(3).copied().unwrap_or(0),
        ])
    }

    pub fn write_u32(&self, ptr: u32, value: u32) {
        self.write(ptr, &value.to_le_bytes());
    }
}
