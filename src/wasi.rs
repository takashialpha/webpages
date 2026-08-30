//! Running a guest program: a WASI preview1 shim, and the terminal it draws on.
//!
//! The browser has a WebAssembly engine, so nothing here interprets anything.
//! A guest is handed to that engine with an import object standing in for the
//! operating system it thinks it has.
//!
//! Only what a terminal program actually reaches for is implemented. A
//! `wasm32-wasip1` binary that prints, reads a line, asks the time, asks for
//! randomness and exits imports exactly seven functions, and those are the
//! seven below. Anything else is not stubbed because nothing asks for it: an
//! import a guest needs and this does not provide fails loudly at
//! instantiation, which is where a missing piece should be noticed.
//!
//! A second module, `tty`, carries what WASI has no concept of. There is no
//! way to address a screen or read a key in preview1, so a program that draws
//! imports these instead:
//!
//! ```text
//! tty::cols() -> u32          tty::rows() -> u32
//! tty::put(x, y, ch, fg, bg)  tty::clear()
//! tty::key() -> i32           // -1 when the queue is empty
//! ```

#![cfg(feature = "hydrate")]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::JsCast as _;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::JsValue;

use crate::screen::Screen;

/// Success, in WASI's numbering, and the one failure this shim reports.
const OK: u32 = 0;
const EBADF: u32 = 8;

/// The file descriptors a terminal program knows about.
const STDIN: u32 = 0;
const STDOUT: u32 = 1;
const STDERR: u32 = 2;

/// Everything the guest can reach, on this side of the wall.
struct Host {
    /// Filled in after instantiation: the guest's memory is one of its exports,
    /// so it does not exist yet when the imports are built.
    memory: Option<js_sys::WebAssembly::Memory>,
    /// The screen the guest draws on. Swapped in for the duration of a call and
    /// swapped back out after, so the terminal keeps ownership of it.
    screen: Screen,
    /// Keys waiting to be read, oldest first.
    keys: VecDeque<i32>,
    /// What the guest has written to stdout and stderr.
    out: Vec<u8>,
    /// What the guest passed to `proc_exit`, if it has.
    exit: Option<i32>,
}

impl Host {
    fn new() -> Self {
        Self {
            memory: None,
            screen: Screen::new(0, 0),
            keys: VecDeque::new(),
            out: Vec::new(),
            exit: None,
        }
    }

    /// The guest's memory as bytes. Rebuilt on each use rather than cached:
    /// growing the memory detaches the old buffer, and a stale view of it reads
    /// as empty.
    fn view(&self) -> Option<js_sys::Uint8Array> {
        self.memory
            .as_ref()
            .map(|memory| js_sys::Uint8Array::new(&memory.buffer()))
    }

    fn read(&self, ptr: u32, len: u32) -> Vec<u8> {
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

    fn write(&self, ptr: u32, bytes: &[u8]) {
        let Some(view) = self.view() else { return };
        let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        if u64::from(ptr) + u64::from(len) > u64::from(view.length()) {
            return;
        }
        view.subarray(ptr, ptr + len).copy_from(bytes);
    }

    /// A little-endian `u32`, which is how every number in a WASI struct is
    /// laid out.
    fn read_u32(&self, ptr: u32) -> u32 {
        let bytes = self.read(ptr, 4);
        u32::from_le_bytes([
            bytes.first().copied().unwrap_or(0),
            bytes.get(1).copied().unwrap_or(0),
            bytes.get(2).copied().unwrap_or(0),
            bytes.get(3).copied().unwrap_or(0),
        ])
    }

    fn write_u32(&self, ptr: u32, value: u32) {
        self.write(ptr, &value.to_le_bytes());
    }
}

/// A guest, instantiated and ready to be driven.
pub struct Guest {
    host: Rc<RefCell<Host>>,
    start: Option<js_sys::Function>,
    frame: Option<js_sys::Function>,
}

/// Builds the import object and hands it to the browser's engine.
///
/// # Errors
///
/// Returns a message fit to print when the guest cannot be fetched, does not
/// parse, or asks for something not provided here.
#[expect(
    clippy::future_not_send,
    reason = "the browser is single threaded and nothing here crosses a thread"
)]
pub async fn open(url: &str) -> Result<Guest, String> {
    use wasm_bindgen_futures::JsFuture;

    let host = Rc::new(RefCell::new(Host::new()));
    let imports = build_imports(&host);

    let response = JsFuture::from(leptos::prelude::window().fetch_with_str(url))
        .await
        .map_err(|_| format!("{url}: could not be fetched"))?
        .dyn_into::<web_sys::Response>()
        .map_err(|_| format!("{url}: unexpected answer"))?;
    if !response.ok() {
        return Err(format!("{url}: {}", response.status()));
    }

    let bytes = JsFuture::from(
        response
            .array_buffer()
            .map_err(|_| format!("{url}: no body"))?,
    )
    .await
    .map_err(|_| format!("{url}: body was cut short"))?;

    let result = JsFuture::from(js_sys::WebAssembly::instantiate_buffer(
        &js_sys::Uint8Array::new(&bytes).to_vec(),
        &imports,
    ))
    .await
    .map_err(|error| format!("{url}: {}", describe(&error)))?;

    let instance = js_sys::Reflect::get(&result, &JsValue::from_str("instance"))
        .map_err(|_| format!("{url}: no instance"))?;
    let exports = js_sys::Reflect::get(&instance, &JsValue::from_str("exports"))
        .map_err(|_| format!("{url}: no exports"))?;

    let memory = js_sys::Reflect::get(&exports, &JsValue::from_str("memory"))
        .ok()
        .and_then(|memory| memory.dyn_into::<js_sys::WebAssembly::Memory>().ok())
        .ok_or_else(|| format!("{url}: exports no memory"))?;
    host.borrow_mut().memory = Some(memory);

    let export = |name: &str| {
        js_sys::Reflect::get(&exports, &JsValue::from_str(name))
            .ok()
            .and_then(|value| value.dyn_into::<js_sys::Function>().ok())
    };

    let start = export("_start");
    let frame = export("frame");
    if start.is_none() && frame.is_none() {
        return Err(format!("{url}: exports neither `_start` nor `frame`"));
    }

    Ok(Guest { host, start, frame })
}

/// The message out of a thrown JS value, which is where instantiation failures
/// say which import was missing.
fn describe(error: &JsValue) -> String {
    js_sys::Reflect::get(error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "could not be instantiated".to_owned())
}

/// One import, named and installed on a module object.
fn install(module: &js_sys::Object, name: &str, function: &JsValue) {
    let _ = js_sys::Reflect::set(module, &JsValue::from_str(name), function);
}

/// The operating system the guest thinks it has.
///
/// Each closure is handed to JS, which keeps it alive for as long as the
/// instance that holds it, and lets it go when the instance does.
fn build_imports(host: &Rc<RefCell<Host>>) -> js_sys::Object {
    let wasi = js_sys::Object::new();
    let tty = js_sys::Object::new();

    // Writing: gather the iovecs and keep the bytes for the terminal to print.
    // Every descriptor that is not stdout or stderr is refused, which is what
    // a program checks when it wants to know whether it has a terminal.
    let shared = Rc::clone(host);
    install(
        &wasi,
        "fd_write",
        &Closure::<dyn FnMut(u32, u32, u32, u32) -> u32>::new(
            move |fd: u32, iovs: u32, count: u32, written: u32| {
                if fd != STDOUT && fd != STDERR {
                    return EBADF;
                }
                let mut host = shared.borrow_mut();
                let mut total = 0_u32;
                for index in 0..count {
                    // An iovec is a pointer and a length, four bytes each.
                    let entry = iovs + index * 8;
                    let ptr = host.read_u32(entry);
                    let len = host.read_u32(entry + 4);
                    let bytes = host.read(ptr, len);
                    total = total.saturating_add(len);
                    host.out.extend_from_slice(&bytes);
                }
                host.write_u32(written, total);
                OK
            },
        )
        .into_js_value(),
    );

    // Reading: nothing to read. A terminal program that asks for a line gets
    // end of file rather than blocking, because there is nowhere here for it
    // to block until.
    let shared = Rc::clone(host);
    install(
        &wasi,
        "fd_read",
        &Closure::<dyn FnMut(u32, u32, u32, u32) -> u32>::new(
            move |fd: u32, _iovs: u32, _count: u32, read: u32| {
                if fd != STDIN {
                    return EBADF;
                }
                shared.borrow().write_u32(read, 0);
                OK
            },
        )
        .into_js_value(),
    );

    // No environment, which is a perfectly ordinary thing for a process to
    // have. Both calls have to agree, or a guest reading them will walk off
    // the end of a buffer it sized from the first.
    let shared = Rc::clone(host);
    install(
        &wasi,
        "environ_sizes_get",
        &Closure::<dyn FnMut(u32, u32) -> u32>::new(move |count: u32, size: u32| {
            let host = shared.borrow();
            host.write_u32(count, 0);
            host.write_u32(size, 0);
            OK
        })
        .into_js_value(),
    );
    install(
        &wasi,
        "environ_get",
        &Closure::<dyn FnMut(u32, u32) -> u32>::new(|_environ: u32, _buf: u32| OK).into_js_value(),
    );

    // The clock the rest of the site uses, which is the server's rather than
    // the browser's. `precision` arrives as a BigInt and is ignored, so it is
    // taken untyped rather than coerced.
    let shared = Rc::clone(host);
    install(
        &wasi,
        "clock_time_get",
        &Closure::<dyn FnMut(u32, JsValue, u32) -> u32>::new(
            move |_id: u32, _precision: JsValue, out: u32| {
                let nanos = crate::clock::now_millis().unsigned_abs() * 1_000_000;
                shared.borrow().write(out, &nanos.to_le_bytes());
                OK
            },
        )
        .into_js_value(),
    );

    // Not the cryptographic sort. Nothing here keeps a secret, and a guest
    // asking for randomness wants an unplanned pattern, not entropy.
    let shared = Rc::clone(host);
    install(
        &wasi,
        "random_get",
        &Closure::<dyn FnMut(u32, u32) -> u32>::new(move |buf: u32, len: u32| {
            let bytes: Vec<u8> = (0..len).map(|_| scatter()).collect();
            shared.borrow().write(buf, &bytes);
            OK
        })
        .into_js_value(),
    );

    // Recorded rather than thrown. The guest treats this as never returning
    // and runs into its own unreachable, which traps, and the trap is what
    // ends the call: see `Guest::run`.
    let shared = Rc::clone(host);
    install(
        &wasi,
        "proc_exit",
        &Closure::<dyn FnMut(i32)>::new(move |code: i32| {
            shared.borrow_mut().exit = Some(code);
        })
        .into_js_value(),
    );

    let shared = Rc::clone(host);
    install(
        &tty,
        "cols",
        &Closure::<dyn FnMut() -> u32>::new(move || {
            u32::try_from(shared.borrow().screen.cols()).unwrap_or(0)
        })
        .into_js_value(),
    );
    let shared = Rc::clone(host);
    install(
        &tty,
        "rows",
        &Closure::<dyn FnMut() -> u32>::new(move || {
            u32::try_from(shared.borrow().screen.rows()).unwrap_or(0)
        })
        .into_js_value(),
    );
    let shared = Rc::clone(host);
    install(
        &tty,
        "put",
        &Closure::<dyn FnMut(u32, u32, u32, u32, u32)>::new(
            move |x: u32, y: u32, ch: u32, fg: u32, bg: u32| {
                let glyph = char::from_u32(ch).unwrap_or(' ');
                let colour = |value: u32| u8::try_from(value % 16).unwrap_or(0);
                shared.borrow_mut().screen.put(
                    x as usize,
                    y as usize,
                    glyph,
                    colour(fg),
                    colour(bg),
                );
            },
        )
        .into_js_value(),
    );
    let shared = Rc::clone(host);
    install(
        &tty,
        "clear",
        &Closure::<dyn FnMut()>::new(move || shared.borrow_mut().screen.clear()).into_js_value(),
    );
    let shared = Rc::clone(host);
    install(
        &tty,
        "key",
        &Closure::<dyn FnMut() -> i32>::new(move || {
            shared.borrow_mut().keys.pop_front().unwrap_or(-1)
        })
        .into_js_value(),
    );

    let imports = js_sys::Object::new();
    install(&imports, "wasi_snapshot_preview1", &wasi.into());
    install(&imports, "tty", &tty.into());
    imports
}

/// One unplanned byte.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "`random` is in [0, 1), so this is in 0..=255, and the cast saturates"
)]
fn scatter() -> u8 {
    (js_sys::Math::random() * 256.0) as u8
}

/// How a guest's turn ended.
pub enum Finished {
    /// It returned or exited. Whatever it printed comes with it.
    Exited { code: i32, output: String },
    /// It stopped in a way it did not choose.
    Trapped { output: String },
}

impl Guest {
    /// Whether this one draws, rather than printing and exiting.
    #[must_use]
    pub const fn draws(&self) -> bool {
        self.frame.is_some()
    }

    /// Runs a command program to completion.
    ///
    /// A trap is the ordinary ending: `proc_exit` is declared never to return,
    /// so a guest calling it runs into its own unreachable the moment the
    /// import hands control back. Having recorded the code first is what tells
    /// the two kinds of stop apart.
    pub fn run(&mut self) -> Finished {
        let outcome = self.start.as_ref().map(|start| start.call0(&JsValue::NULL));

        let mut host = self.host.borrow_mut();
        let output = String::from_utf8_lossy(&host.out).into_owned();
        host.out.clear();

        match (host.exit, outcome) {
            (Some(code), _) => Finished::Exited { code, output },
            (None, Some(Err(_))) => Finished::Trapped { output },
            (None, _) => Finished::Exited { code: 0, output },
        }
    }

    /// Lends the screen to the guest for the length of one call.
    ///
    /// Swapped rather than shared: the terminal owns the screen between
    /// frames, and the guest's `tty` imports reach for it through the host
    /// while a call is in flight.
    fn lending<T>(&self, screen: &mut Screen, call: impl FnOnce() -> T) -> T {
        std::mem::swap(screen, &mut self.host.borrow_mut().screen);
        let result = call();
        std::mem::swap(screen, &mut self.host.borrow_mut().screen);
        result
    }

    /// Draws one frame. Returns whether the guest is finished.
    pub fn frame(&mut self, screen: &mut Screen, elapsed: f64) -> bool {
        let Some(frame) = self.frame.clone() else {
            return true;
        };
        self.lending(screen, || {
            // Anything but zero means it is done, and so does a trap.
            frame
                .call1(&JsValue::NULL, &JsValue::from_f64(elapsed))
                .map_or(true, |value| value.as_f64().is_none_or(|code| code != 0.0))
        })
    }

    /// Runs `_start` once with the screen lent out, for a guest that sets
    /// itself up before its first frame.
    pub fn begin(&mut self, screen: &mut Screen) {
        let Some(start) = self.start.clone() else {
            return;
        };
        self.lending(screen, || {
            let _ = start.call0(&JsValue::NULL);
        });
        self.host.borrow_mut().out.clear();
    }

    /// Queues one keypress, as the bytes a terminal would have sent.
    pub fn press(&mut self, key: &str) {
        let mut host = self.host.borrow_mut();
        let mut push = |byte: u8| host.keys.push_back(i32::from(byte));

        match key {
            "Enter" => push(b'\r'),
            "Backspace" => push(0x7f),
            "Tab" => push(b'\t'),
            "Escape" => push(0x1b),
            // The escape sequences a terminal really sends, so a guest that
            // parses them is parsing the same thing it would anywhere else.
            "ArrowUp" | "ArrowDown" | "ArrowRight" | "ArrowLeft" => {
                push(0x1b);
                push(b'[');
                push(match key {
                    "ArrowUp" => b'A',
                    "ArrowDown" => b'B',
                    "ArrowRight" => b'C',
                    _ => b'D',
                });
            }
            _ => {
                let mut chars = key.chars();
                if let (Some(one), None) = (chars.next(), chars.next()) {
                    let mut bytes = [0; 4];
                    for byte in one.encode_utf8(&mut bytes).bytes() {
                        push(byte);
                    }
                }
            }
        }
    }
}
