// Large static view trees produce deeply nested generic future types; the async
// SSR-resolve / hydrate paths exceed the default query depth of 128 in release.
#![recursion_limit = "256"]

pub mod app;
pub mod args;
pub mod clock;
pub mod commands;
pub mod fs;
pub mod seo;
pub mod shell;
pub mod terminal;
pub mod theme;
pub mod viewport;

/// Canonical origin of the deployed site. Used to build absolute URLs for the
/// canonical link, Open Graph tags, and the sitemap.
pub const SITE_URL: &str = "https://takashialpha.com";

/// The commit this binary was built from. CI exports `GITHUB_SHA`; a local
/// build has no commit to name and says so.
pub const BUILD: &str = match option_env!("GITHUB_SHA") {
    Some(sha) => sha,
    None => "dev",
};

/// The user a visitor is logged in as. The prompt and `whoami` both read this,
/// so they cannot disagree about who you are.
pub const USER: &str = "guest";

/// [`SITE_URL`] without its scheme, for the shell prompt. Derived rather than
/// written out a second time.
#[must_use]
pub fn site_host() -> &'static str {
    SITE_URL.trim_start_matches("https://")
}

/// The `uname -srm` line for the machine actually serving the site.
///
/// The server reads its own kernel from `/proc` and renders the result into
/// `<html data-uname>` as well as into the banner text. That attribute is how
/// the client gets the same string: hydration has to reproduce the banner
/// exactly, and a browser has no other way to know what the server runs.
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

/// `BUILD` shortened to the usual seven characters, when it is a full SHA.
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
