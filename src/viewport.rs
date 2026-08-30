//! Sizing the terminal to the part of the screen you can see.
//!
//! A keyboard shrinks the visual viewport, not the layout one. Chrome can be
//! told to keep them together with `interactive-widget=resizes-content`;
//! Safari ignores it. So `100dvh` and a fixed key row both measure a viewport
//! that runs on behind the keyboard.
//!
//! Measuring the visual viewport is what every browser agrees on. Its height
//! and offset go onto the document as custom properties, and the stylesheet
//! builds the terminal from those.

/// Writes the visual viewport onto the document and keeps it current.
///
/// `on_change` runs after each measurement, so the caller can put the prompt
/// back in view. The listeners are never removed, since the terminal lives as
/// long as the page does.
#[cfg(feature = "hydrate")]
pub fn track(on_change: impl Fn() + 'static) {
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen::closure::Closure;

    // Every browser this targets has one. Without it the stylesheet keeps its
    // `100dvh` fallback, which is what the page would have done anyway.
    let Some(viewport) = leptos::prelude::window().visual_viewport() else {
        return;
    };

    let measure = {
        let viewport = viewport.clone();
        move || {
            let Some(html) = leptos::prelude::document().document_element() else {
                return;
            };
            let style = html.unchecked_into::<web_sys::HtmlElement>().style();
            let _ = style.set_property("--screen-height", &format!("{}px", viewport.height()));
            // `position: fixed` resolves against the layout viewport, so the
            // offset is what pulls the terminal back over the visible part
            // when ios has scrolled the one inside the other.
            let _ = style.set_property("--screen-top", &format!("{}px", viewport.offset_top()));
            on_change();
        }
    };

    // Once up front: the first measurement is the one that matters, before any
    // keyboard has opened.
    measure();

    let listener = Closure::<dyn Fn()>::new(measure);
    let callback = listener.as_ref().unchecked_ref();
    // Resize is the keyboard opening and closing. Scroll is ios sliding the
    // visual viewport around inside the layout one, which moves the terminal
    // without resizing it.
    let _ = viewport.add_event_listener_with_callback("resize", callback);
    let _ = viewport.add_event_listener_with_callback("scroll", callback);
    listener.forget();
}

/// The server has no viewport to measure. This exists so the module compiles
/// into the server build alongside the rest.
#[cfg(not(feature = "hydrate"))]
pub fn track(_on_change: impl Fn() + 'static) {}

/// How wide and tall one cell is, in pixels. Measured, because the stylesheet
/// switches size at a breakpoint.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn cell_size() -> (f64, f64) {
    use wasm_bindgen::JsCast as _;

    // A run of them rather than one, so the width divides out whatever
    // rounding the browser does on a single glyph.
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
