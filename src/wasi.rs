//! A WASI preview1 shim, big enough for a terminal program.
//!
//! The browser runs the guest; this only hands it the imports it asks for. A
//! `wasm32-wasip1` binary that prints, reads, asks the time and exits needs
//! exactly the seven below. Nothing else is stubbed: a missing import fails at
//! instantiation and names itself, which is where it should be noticed.
//!
//! WASI has no screen and no keyboard, so a program that draws uses `tty`:
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

/// WASI's wall clock. Everything else it can ask for only counts up, which is
/// what the page's own clock does.
const REALTIME: u32 = 0;

/// The host side of a running guest.
struct Host {
    /// Filled in after instantiation: memory is one of the guest's exports, so
    /// it does not exist yet when the imports are built.
    memory: Option<js_sys::WebAssembly::Memory>,
    /// The screen the guest draws on. Swapped in for a call and back out
    /// after, so the terminal keeps it.
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

    /// The guest's memory. Rebuilt each time, because growing it detaches the
    /// old buffer and a stale view reads as empty.
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

    /// A little-endian `u32`, as WASI lays out every number.
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

/// Fetches and instantiates a guest.
///
/// # Errors
///
/// A message to print: could not fetch, not wasm, or wants an import that is
/// not here.
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

/// The message out of a thrown JS value, which is what names a missing import.
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

/// The imports. Each closure is handed to JS, which holds it as long as the
/// instance does.
fn build_imports(host: &Rc<RefCell<Host>>) -> js_sys::Object {
    let wasi = js_sys::Object::new();
    let tty = js_sys::Object::new();

    // Gather the iovecs, and keep the bytes for the terminal to print.
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

    // Nothing to read, so a guest asking for a line gets end of file. Blocking
    // would be worse: nothing here could unblock it.
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

    // No environment. The two have to agree, or a guest sizes a buffer from
    // the first and overruns it.
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

    // The wall clock is the server's, like the rest of the site. Everything
    // else is how long the page has been open, which is monotonic and has more
    // than millisecond resolution, so a guest timing itself gets a real answer.
    // `precision` arrives as a BigInt, so it is taken untyped and ignored.
    let shared = Rc::clone(host);
    install(
        &wasi,
        "clock_time_get",
        &Closure::<dyn FnMut(u32, JsValue, u32) -> u32>::new(
            move |id: u32, _precision: JsValue, out: u32| {
                let nanos = if id == REALTIME {
                    crate::clock::now_millis().unsigned_abs() * 1_000_000
                } else {
                    open_for_nanos()
                };
                shared.borrow().write(out, &nanos.to_le_bytes());
                OK
            },
        )
        .into_js_value(),
    );

    // Not the cryptographic sort. A guest wants an unplanned pattern.
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

    // Recorded, not thrown. The guest treats this as never returning and hits
    // its own unreachable, and that trap ends the call. See `Guest::run`.
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

/// How long the page has been open, in nanoseconds.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "`performance.now()` counts up from zero, and no page is open for 500 years"
)]
fn open_for_nanos() -> u64 {
    (crate::clock::since_load_millis() * 1_000_000.0) as u64
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
    /// Whether it draws, rather than printing and exiting.
    #[must_use]
    pub const fn draws(&self) -> bool {
        self.frame.is_some()
    }

    /// Runs a program that prints and exits.
    ///
    /// A trap is the normal ending, since `proc_exit` never returns. The code
    /// recorded just before is what tells a clean exit from a crash.
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

    /// Lends the screen to the guest for one call and takes it back after.
    /// Swapped rather than shared, so the terminal owns it between frames.
    fn lending<T>(&self, screen: &mut Screen, call: impl FnOnce() -> T) -> T {
        std::mem::swap(screen, &mut self.host.borrow_mut().screen);
        let result = call();
        std::mem::swap(screen, &mut self.host.borrow_mut().screen);
        result
    }

    /// One frame. Returns whether the guest is finished.
    ///
    /// Not called `frame`: the trait the terminal drives this through has a
    /// method by that name, and an inherent one would quietly shadow it.
    pub fn advance(&mut self, screen: &mut Screen, elapsed: f64) -> bool {
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

    /// Runs `_start` with the screen lent out, so a guest can set itself up.
    pub fn begin(&mut self, screen: &mut Screen) {
        let Some(start) = self.start.clone() else {
            return;
        };
        self.lending(screen, || {
            let _ = start.call0(&JsValue::NULL);
        });
        self.host.borrow_mut().out.clear();
    }

    /// Queues a keypress, as the bytes a terminal would send.
    pub fn press(&mut self, key: &str) {
        let mut host = self.host.borrow_mut();
        let mut push = |byte: u8| host.keys.push_back(i32::from(byte));

        match key {
            "Enter" => push(b'\r'),
            "Backspace" => push(0x7f),
            "Tab" => push(b'\t'),
            "Escape" => push(0x1b),
            // The real escape sequences, so a guest parses what it would
            // parse in any other terminal.
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
