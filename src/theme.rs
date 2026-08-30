//! Palettes, switched by the `theme` command.
//!
//! A palette is a `data-theme` value and nothing else; the CSS holds the
//! colours. So switching is one attribute write, and a new palette is a block
//! of custom properties rather than code.
//!
//! Nothing is stored, so a palette lasts the visit. A tty does not remember you
//! either.

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

/// Switches palette, or `None` if there is no palette by that name.
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

/// The server has no browser to read and never runs commands. These are here
/// so the module still compiles into it.
#[cfg(not(feature = "hydrate"))]
pub fn apply(name: &str) -> Option<&'static Palette> {
    known(name)
}

#[cfg(not(feature = "hydrate"))]
#[must_use]
pub fn current() -> String {
    DEFAULT.to_owned()
}
