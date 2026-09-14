//! The line you are typing, and the lines you typed before.
//!
//! The text lives in a real `<input>`, which holds it and takes the keys but
//! does not draw it. What is drawn is a span over the top, so an echoed line
//! looks exactly like the line it was. That is why the caret column and the
//! first visible column are tracked here rather than left to the input: both
//! are in whole cells, and an input scrolls by pixels.
//!
//! Everything is a signal, so [`Editor`] is `Copy` and every handler can keep
//! one.

use leptos::html;
use leptos::prelude::*;

use super::fits;

#[derive(Clone, Copy)]
pub struct Editor {
    field: NodeRef<html::Input>,
    /// What is on the line right now.
    pub text: RwSignal<String>,
    /// Where the caret is, in cells. Fixed width font, so the column is just
    /// the offset: nothing to measure, and it stays right after an arrow key.
    pub column: RwSignal<usize>,
    /// The first column shown, once a line outgrows the field.
    pub scrolled: RwSignal<usize>,
    history: RwSignal<Vec<String>>,
    /// `None` means "editing a fresh line"; otherwise an index into `history`.
    recalled: RwSignal<Option<usize>>,
}

impl Editor {
    pub fn new(field: NodeRef<html::Input>) -> Self {
        Self {
            field,
            text: RwSignal::new(String::new()),
            column: RwSignal::new(0),
            scrolled: RwSignal::new(0),
            history: RwSignal::new(Vec::new()),
            recalled: RwSignal::new(None),
        }
    }

    /// Puts the keyboard back on the line.
    pub fn focus(self) {
        if let Some(field) = self.field.get() {
            let _ = field.focus();
        }
    }

    /// Puts the field back to an empty prompt with the keyboard on it, for the
    /// moment a program hands the screen back.
    ///
    /// A program is started by submitting a line, so the line is empty while one
    /// runs, and anything on it afterwards was put there behind the terminal's
    /// back. The value is written to the element as well as to the signal:
    /// writing the signal alone changes nothing when it already holds what the
    /// element should have, which is exactly the case being put right.
    ///
    /// Next frame, because the input cannot be focused until the scrollback it
    /// sits in is back on the screen.
    pub fn reset(self) {
        // Through `set`, so what follows is a fresh line rather than one still
        // holding a place in the history.
        self.set(String::new());
        self.scrolled.set(0);
        self.column.set(0);
        request_animation_frame(move || {
            if let Some(field) = self.field.get_untracked() {
                field.set_value("");
                let _ = field.set_selection_range(0, 0);
                let _ = field.focus();
            }
        });
    }

    /// Replaces the line, as recall and completion both do. Anything typed
    /// after this is a fresh line again.
    pub fn set(self, line: String) {
        self.recalled.set(None);
        self.text.set(line);
    }

    /// Keeps a line for `up` to find. Blank lines are not worth remembering.
    pub fn remember(self, line: &str) {
        if !line.trim().is_empty() {
            self.history.update(|entries| entries.push(line.to_owned()));
        }
        self.recalled.set(None);
    }

    /// Up walks back, down walks forward and off the end into a fresh empty
    /// line, which is what a shell does.
    pub fn recall(self, backwards: bool) {
        let entries = self.history.get();
        if entries.is_empty() {
            return;
        }
        let next = match (self.recalled.get(), backwards) {
            (None, true) => Some(entries.len().saturating_sub(1)),
            (None, false) => None,
            (Some(index), true) => Some(index.saturating_sub(1)),
            (Some(index), false) => (index + 1 < entries.len()).then_some(index + 1),
        };
        self.recalled.set(next);
        self.text.set(
            next.and_then(|index| entries.get(index).cloned())
                .unwrap_or_default(),
        );
    }

    /// Where the caret is, counted in characters.
    ///
    /// Characters, not bytes and not utf-16 code units. The browser counts in
    /// the second, Rust indexes strings by the first, and the two only agree on
    /// ascii: `€` is one code unit and three bytes. A caret read from the
    /// browser and used to cut a Rust string lands mid-character and panics,
    /// which on this build aborts and takes the terminal with it. A character
    /// is also what a column is, which is the other thing this number is for.
    ///
    /// Read from the input rather than from `column`, which only catches up
    /// next frame.
    pub fn caret(self) -> usize {
        let units = self
            .field
            .get_untracked()
            .and_then(|field| field.selection_start().ok().flatten())
            .map(|at| at as usize);

        self.text.with_untracked(|text| {
            let Some(units) = units else {
                return text.chars().count();
            };
            let mut counted = 0;
            for (column, ch) in text.chars().enumerate() {
                if counted >= units {
                    return column;
                }
                counted += ch.len_utf16();
            }
            text.chars().count()
        })
    }

    /// The column past the last character.
    pub fn end(self) -> usize {
        self.text.with_untracked(|text| text.chars().count())
    }

    /// Puts the caret on a column, back in the code units the browser wants.
    ///
    /// Next frame: changing the line rewrites `prop:value` and puts the caret
    /// at the end, so placing it first would be overwritten.
    pub fn put_caret(self, at: usize) {
        request_animation_frame(move || {
            if let Some(field) = self.field.get_untracked() {
                let units: usize = field.value().chars().take(at).map(char::len_utf16).sum();
                let units = u32::try_from(units).unwrap_or(u32::MAX);
                let _ = field.set_selection_range(units, units);
                self.column.set(at);
            }
        });
    }

    /// Cuts the line back to the caret, backwards or forwards, the way a shell
    /// does. By characters, so neither can land inside one.
    pub fn cut_to_start(self) {
        let at = self.caret();
        self.text
            .update(|text| *text = text.chars().skip(at).collect());
        self.put_caret(0);
    }

    pub fn cut_to_end(self) {
        let at = self.caret();
        self.text
            .update(|text| *text = text.chars().take(at).collect());
        self.put_caret(at);
    }

    /// Reads the caret back after anything that could have moved it, and
    /// scrolls the line if it would otherwise have left the field.
    ///
    /// Writes only on a change, since a held key comes through on every repeat.
    pub fn sync(self) {
        let Some(field) = self.field.get() else {
            return;
        };
        let at = self.caret();
        if self.column.get_untracked() != at {
            self.column.set(at);
        }

        // By whole columns: the input scrolls by pixels and stops mid
        // character, which a terminal never does.
        let cell = crate::viewport::cell_size().0;
        let visible = fits(f64::from(field.client_width()), cell);
        let mut offset = self.scrolled.get_untracked();
        if at < offset {
            offset = at;
        } else if at >= offset + visible {
            offset = at + 1 - visible;
        }
        if self.scrolled.get_untracked() != offset {
            self.scrolled.set(offset);
        }
    }
}
