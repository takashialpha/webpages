// Large static view trees produce deeply nested generic future types; the async
// SSR-resolve / hydrate paths exceed the default query depth of 128 in release.
#![recursion_limit = "256"]

pub mod app;
pub mod components;
pub mod pages;

/// Canonical origin of the deployed site. Used to build absolute URLs for
/// canonical links, Open Graph tags, and the sitemap.
pub const SITE_URL: &str = "https://takashialpha.com";

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
