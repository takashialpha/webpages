//! A WASI preview1 shim, big enough for a terminal program.
//!
//! The browser runs the guest; this only hands it the imports it asks for.
//! Fetching and driving one is here, `host.rs` is the state a running guest
//! has, and `imports.rs` is the operating system it thinks it is talking to.
//!
//! A `wasm32-wasip1` binary that prints, reads, asks the time and exits needs
//! exactly seven WASI calls. Nothing else is stubbed: a missing import fails at
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

mod host;
mod imports;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::JsValue;

use host::Host;

use crate::screen::Screen;

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
    let imports = imports::build(&host);

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
