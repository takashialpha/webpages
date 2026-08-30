//! The site: the shell in the browser, the server behind it, and the few
//! things both halves have to agree on.
//!
//! Built twice from one crate. `ssr` is the server binary; `hydrate` is the
//! wasm bundle. A module that exists on both sides says so, usually by having
//! two versions of the same function with the same signature.

// A big static view tree becomes a deeply nested generic future, and resolving
// or hydrating one goes past the default depth of 128.
#![recursion_limit = "256"]
// Nothing in the site needs unsafe, so nothing may bring it back.
#![forbid(unsafe_code)]

pub mod app;
pub mod args;
pub mod clock;
pub mod commands;
pub mod fs;
pub mod program;
pub mod screen;
pub mod seo;
#[cfg(feature = "ssr")]
pub mod server;
pub mod shell;
pub mod terminal;
pub mod theme;
pub mod viewport;
pub mod wall;
pub mod wasi;

/// Where the site lives. The one place the domain is written: the canonical
/// link, the Open Graph tags, the sitemap and the prompt all come from here.
pub const SITE_URL: &str = "https://takashialpha.com";

/// The commit this was built from. CI sets `GITHUB_SHA`; a local build has no
/// commit to name and says so.
pub const BUILD: &str = match option_env!("GITHUB_SHA") {
    Some(sha) => sha,
    None => "dev",
};

/// Who a visitor is logged in as. The prompt and `whoami` both read this, so
/// they cannot disagree.
pub const USER: &str = "guest";

/// [`SITE_URL`] without its scheme, for the prompt.
#[must_use]
pub fn site_host() -> &'static str {
    SITE_URL.trim_start_matches("https://")
}

/// The `uname -srm` line for the machine actually serving the site.
///
/// The server reads its kernel from `/proc` and puts the answer in the banner
/// and in `<html data-uname>`. The attribute is how the browser gets the same
/// string: hydration has to rebuild the banner exactly, and there is no other
/// way for it to know what the server runs.
#[cfg(not(feature = "hydrate"))]
#[must_use]
pub fn uname() -> String {
    use std::sync::OnceLock;

    static UNAME: OnceLock<String> = OnceLock::new();
    UNAME
        .get_or_init(|| {
            let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").map_or_else(
                |_| "unknown".to_owned(),
                |release| release.trim().to_owned(),
            );
            format!("Linux takashialpha {release} {}", std::env::consts::ARCH)
        })
        .clone()
}

/// Reads back what the server wrote, so the banner hydrates to identical markup.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn uname() -> String {
    leptos::prelude::document()
        .document_element()
        .and_then(|html| html.get_attribute("data-uname"))
        .unwrap_or_else(|| "Linux takashialpha unknown".to_owned())
}

/// [`BUILD`] cut to the usual seven characters, when it is a full sha.
#[must_use]
pub fn short_build() -> &'static str {
    BUILD.get(..7).unwrap_or(BUILD)
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
