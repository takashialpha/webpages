//! Sizing the terminal to the part of the screen you can see.
//!
//! A keyboard shrinks the visual viewport, not the layout one. Chrome can be
//! told to keep them together with `interactive-widget=resizes-content`; Safari
//! ignores it, so `100dvh` and a fixed key row both measure a viewport that
//! carries on behind the keyboard.
//!
//! Measuring the visual viewport is the one thing every browser agrees on. Its
//! height and offset go onto the document as custom properties, and the
//! stylesheet builds the terminal out of those.

/// Writes the visual viewport onto the document and keeps it up to date.
///
/// `on_change` runs after each measurement, so the caller can put the prompt
/// back in view. Nothing is ever unlistened: the terminal lives as long as the
/// page.
#[cfg(feature = "hydrate")]
pub fn track(on_change: impl Fn() + 'static) {
    use std::rc::Rc;
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::prelude::JsValue;

    // Every browser this targets has one. Without it the stylesheet keeps its
    // `100dvh` fallback, which is what the page would have done anyway.

    let Some(viewport) = leptos::prelude::window().visual_viewport() else {
        return;
    };

    let measure = Rc::new({
        let viewport = viewport.clone();
        move || {
            let Some(html) = leptos::prelude::document().document_element() else {
                return;
            };
            let style = html.unchecked_into::<web_sys::HtmlElement>().style();
            let _ = style.set_property("--screen-height", &format!("{}px", viewport.height()));
            // `position: fixed` resolves against the layout viewport, so the
            // offset is what pulls the terminal back over the visible part when
            // ios has scrolled the one inside the other.
            let _ = style.set_property("--screen-top", &format!("{}px", viewport.offset_top()));
            on_change();
        }
    });

    // Once up front, before any keyboard has opened.
    measure();

    // Resize is the keyboard opening and closing, and it is also the only
    // thing that crosses the stylesheet's breakpoint, so it is where the
    // remembered cell size stops being true.
    let resized = {
        let measure = Rc::clone(&measure);
        Closure::<dyn Fn()>::new(move || {
            forget_cell_size();
            measure();
        })
    };
    // Scroll is ios sliding the visual viewport around inside the layout one,
    // which moves the terminal without resizing it.
    let scrolled = {
        let measure = Rc::clone(&measure);
        Closure::<dyn Fn()>::new(move || measure())
    };
    let _ = viewport.add_event_listener_with_callback("resize", resized.as_ref().unchecked_ref());
    let _ = viewport.add_event_listener_with_callback("scroll", scrolled.as_ref().unchecked_ref());
    resized.forget();
    scrolled.forget();

    // The console font swaps in after the first paint, so a cell measured
    // before it lands is the fallback's, not its.
    if let Ok(ready) = leptos::prelude::document().fonts().ready() {
        let arrived = Closure::<dyn FnMut(JsValue)>::new(|_| forget_cell_size());
        let _ = ready.then(&arrived);
        arrived.forget();
    }
}

/// The server has no viewport. Here so the module still compiles into it.
#[cfg(not(feature = "hydrate"))]
pub fn track(_on_change: impl Fn() + 'static) {}

#[cfg(feature = "hydrate")]
thread_local! {
    /// The last cell measured. Probing for one costs a DOM node and a forced
    /// layout, and the frame loop asks once a frame, so the answer is kept
    /// until something that could change it happens: a resize, or the real
    /// font arriving. Both are handled in [`track`].
    static CELL: std::cell::Cell<Option<(f64, f64)>> = const { std::cell::Cell::new(None) };
}

/// Throws the remembered cell away, so the next ask measures again.
#[cfg(feature = "hydrate")]
fn forget_cell_size() {
    CELL.with(|remembered| remembered.set(None));
}

/// How wide and tall one cell is, in pixels. Measured rather than assumed,
/// because the stylesheet switches size at a breakpoint.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn cell_size() -> (f64, f64) {
    if let Some(remembered) = CELL.with(std::cell::Cell::get) {
        return remembered;
    }
    let cell = measure_cell();
    // A zero means the page was not ready to be measured, which is not an
    // answer worth keeping.
    if cell.0 > 0.0 && cell.1 > 0.0 {
        CELL.with(|remembered| remembered.set(Some(cell)));
    }
    cell
}

/// Puts a run of characters on the page and reads back how much room it took.
#[cfg(feature = "hydrate")]
fn measure_cell() -> (f64, f64) {
    use wasm_bindgen::JsCast as _;

    // A run rather than one glyph, so the width divides out whatever rounding
    // the browser does.
    const RUN: usize = 100;
    #[expect(
        clippy::cast_precision_loss,
        reason = "one hundred is exact in a double"
    )]
    const RUN_F: f64 = RUN as f64;

    let document = leptos::prelude::document();
    let Ok(probe) = document.create_element("span") else {
        return (0.0, 0.0);
    };
    let Some(body) = document.body() else {
        return (0.0, 0.0);
    };

    probe.set_text_content(Some(&"x".repeat(RUN)));
    let _ = probe.set_attribute(
        "style",
        "position:absolute;visibility:hidden;white-space:pre;top:0;left:0",
    );
    if body.append_child(&probe).is_err() {
        return (0.0, 0.0);
    }

    let size = probe.unchecked_ref::<web_sys::HtmlElement>();
    let cell = (
        f64::from(size.offset_width()) / RUN_F,
        f64::from(size.offset_height()),
    );
    let _ = body.remove_child(&probe);
    cell
}

/// The server draws no cells.
#[cfg(not(feature = "hydrate"))]
#[must_use]
pub const fn cell_size() -> (f64, f64) {
    (0.0, 0.0)
}
