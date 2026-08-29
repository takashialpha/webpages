//! Sizing the terminal to the part of the screen you can actually see.
//!
//! An on-screen keyboard does not shrink the layout viewport. It shrinks the
//! visual viewport, which is the window onto it, and only some browsers can be
//! asked to keep the two in step: `interactive-widget=resizes-content` in the
//! viewport meta does it for chrome, and safari ignores it. So the layout
//! cannot be pinned to `100dvh` and a key row cannot be `position: fixed`,
//! because on ios both of those measure a viewport that runs on behind the
//! keyboard: the row lands under it, the prompt scrolls to a bottom that is not
//! the visible bottom, and scrolling detaches the row until the gesture ends.
//!
//! Measuring the visual viewport directly is the one answer every browser
//! agrees on. Its height and its offset from the layout viewport go onto the
//! document as custom properties, and the stylesheet builds the terminal out of
//! those instead.

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
