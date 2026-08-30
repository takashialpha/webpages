//! Asking the server about the board. The `wall` and `cat` commands both come
//! through here.

/// Reads the board, or writes one cell and reads back the result.
///
/// Returns the status with the body: the server explains a refusal in a
/// sentence, and that sentence is what to show.
///
/// # Errors
///
/// A message to print, if the request cannot be made.
#[cfg(feature = "hydrate")]
#[expect(
    clippy::future_not_send,
    reason = "the browser is single threaded and nothing here crosses a thread"
)]
pub async fn fetch(write: Option<&str>) -> Result<(u16, String), String> {
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;

    fn failed<E>(what: &'static str) -> impl FnOnce(E) -> String {
        move |_| format!("wall: {what}")
    }

    let init = web_sys::RequestInit::new();
    if let Some(body) = write {
        init.set_method("POST");
        init.set_body(&JsValue::from_str(body));
    }

    let request = web_sys::Request::new_with_str_and_init("/api/wall", &init)
        .map_err(failed("could not build the request"))?;
    let response = JsFuture::from(leptos::prelude::window().fetch_with_request(&request))
        .await
        .map_err(failed("the board is unreachable"))?
        .dyn_into::<web_sys::Response>()
        .map_err(failed("the board answered with something unexpected"))?;

    let status = response.status();
    let text = JsFuture::from(response.text().map_err(failed("no body to read"))?)
        .await
        .map_err(failed("the board's answer was cut short"))?;

    text.as_string()
        .map(|body| (status, body))
        .ok_or_else(|| "wall: the board's answer was not text".to_owned())
}

/// The server has no browser to ask. Here so the command still compiles.
///
/// # Errors
///
/// Always.
#[cfg(not(feature = "hydrate"))]
pub async fn fetch(_write: Option<&str>) -> Result<(u16, String), String> {
    Err("wall: no browser to ask".to_owned())
}
