//! The import object: the operating system a guest thinks it has.
//!
//! Seven WASI calls, which is all a terminal program reaches for, and a `tty`
//! module for the screen and keyboard WASI has no concept of. Nothing else is
//! stubbed: a missing import fails at instantiation and names itself, which is
//! where it should be noticed.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::JsValue;

use super::host::{EBADF, Host, OK, REALTIME, STDERR, STDIN, STDOUT};

/// One import, named and installed on a module object.
fn install(module: &js_sys::Object, name: &str, function: &JsValue) {
    let _ = js_sys::Reflect::set(module, &JsValue::from_str(name), function);
}

/// The whole import object. Each closure is handed to JS, which holds it as long as the
/// instance does.
pub fn build(host: &Rc<RefCell<Host>>) -> js_sys::Object {
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
                    since_load_nanos()
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

/// How long the page has been open, in nanoseconds. The monotonic clock a
/// guest gets, since a page is the longest thing a guest can outlive.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "`performance.now()` counts up from zero, and no page is open for 500 years"
)]
fn since_load_nanos() -> u64 {
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
