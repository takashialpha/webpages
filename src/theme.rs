//! Palettes, switched by the `theme` command.
//!
//! A palette is nothing but a `data-theme` value on the document; the CSS holds
//! the actual colors. That keeps switching to one attribute write and means a
//! new palette is a block of custom properties, not code.
//!
//! Nothing is stored, so a palette lasts for the visit and the next one starts
//! at the default. A tty does not remember you either.

pub struct Palette {
    pub name: &'static str,
    pub summary: &'static str,
}

pub const PALETTES: &[Palette] = &[
    Palette {
        name: "vga",
        summary: "the classic 16-color console",
    },
    Palette {
        name: "mocha",
        summary: "catppuccin mocha",
    },
    Palette {
        name: "gruvbox",
        summary: "gruvbox dark",
    },
];

/// The palette used before anyone chooses one.
pub const DEFAULT: &str = "vga";

#[must_use]
fn known(name: &str) -> Option<&'static Palette> {
    PALETTES.iter().find(|palette| palette.name == name)
}

/// Switches palette, returning `None` if there is no palette by that name.
#[cfg(feature = "hydrate")]
pub fn apply(name: &str) -> Option<&'static Palette> {
    let palette = known(name)?;

    if let Some(html) = leptos::prelude::document().document_element() {
        let _ = html.set_attribute("data-theme", palette.name);
    }
    Some(palette)
}

/// The palette in effect.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn current() -> String {
    leptos::prelude::document()
        .document_element()
        .and_then(|html| html.get_attribute("data-theme"))
        .unwrap_or_else(|| DEFAULT.to_owned())
}

/// The server has no browser to read from, and never runs commands. These exist
/// so the module compiles into the server build alongside the rest.
#[cfg(not(feature = "hydrate"))]
pub fn apply(name: &str) -> Option<&'static Palette> {
    known(name)
}

#[cfg(not(feature = "hydrate"))]
#[must_use]
pub fn current() -> String {
    DEFAULT.to_owned()
}
