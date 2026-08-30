//! The alternate screen, and the loop that drives whatever is drawing on it.
//!
//! A program takes the whole terminal and gives it back exactly as it was. The
//! terminal owns the frame loop and calls in, because WebAssembly cannot
//! suspend a synchronous call: a program running its own loop would hold the
//! page still until it finished.
//!
//! Everything here is a signal or a stored value, so [`Alt`] is `Copy` and the
//! terminal's closures can each keep one.

use std::rc::Rc;

use leptos::prelude::*;

use crate::program::{Program, Step};
use crate::screen::{Run, Screen};

/// How many cells fit, and what to do once the screen is handed back. Held
/// rather than taken per call, so the frame loop and the mount agree.
type Measure = Rc<dyn Fn() -> (usize, usize)>;
type Focus = Rc<dyn Fn()>;

/// The frame loop, kept so it can schedule itself. Shared because it outlives
/// the call that set it up and has to be reachable from the frame it asked for.
type Tick = Rc<dyn Fn(f64)>;

#[derive(Clone, Copy)]
pub struct Alt {
    /// The running program and its screen. Local, because neither is `Send`
    /// and a browser has one thread anyway.
    running: StoredValue<Option<Box<dyn Program>>, LocalStorage>,
    screen: StoredValue<Screen, LocalStorage>,
    /// One signal per row, so a frame rewrites only the rows that changed.
    rows: RwSignal<Vec<RwSignal<Vec<Run>>>>,
    showing: RwSignal<bool>,
    tick: StoredValue<Option<Tick>, LocalStorage>,
    measure: StoredValue<Measure, LocalStorage>,
    focus: StoredValue<Focus, LocalStorage>,
}

impl Alt {
    pub fn new(measure: impl Fn() -> (usize, usize) + 'static, focus: impl Fn() + 'static) -> Self {
        let alt = Self {
            running: StoredValue::new_local(None),
            screen: StoredValue::new_local(Screen::new(0, 0)),
            rows: RwSignal::new(Vec::new()),
            showing: RwSignal::new(false),
            tick: StoredValue::new_local(None),
            measure: StoredValue::new_local(Rc::new(measure)),
            focus: StoredValue::new_local(Rc::new(focus)),
        };
        alt.tick
            .set_value(Some(Rc::new(move |last| alt.frame(last))));
        alt
    }

    /// Whether a program has the screen. Tracked, since the view swaps on it.
    pub fn showing(self) -> bool {
        self.showing.get()
    }

    /// The rows to draw, one signal each.
    pub fn rows(self) -> Vec<RwSignal<Vec<Run>>> {
        self.rows.get()
    }

    /// Hands the screen to a program ready to draw.
    pub fn mount(self, mut program: Box<dyn Program>) {
        let (cols, rows) = (self.measure.get_value())();
        self.screen.update_value(|screen| {
            screen.resize(cols, rows);
            screen.clear();
        });
        self.rows
            .set((0..rows).map(|_| RwSignal::new(Vec::new())).collect());
        self.screen.update_value(|screen| program.start(screen));
        self.running.set_value(Some(program));
        self.showing.set(true);
        // The input is the keyboard whether it can be seen or not, so it keeps
        // focus while the program has the screen.
        (self.focus.get_value())();
        self.repaint();
        self.step(0.0);
    }

    /// Takes the screen back, whether the program asked to go or was told to.
    pub fn leave(self) {
        self.running.set_value(None);
        self.showing.set(false);
        self.rows.set(Vec::new());
        (self.focus.get_value())();
    }

    /// One keypress for whatever is running.
    pub fn key(self, key: &str) {
        self.running.update_value(|program| {
            if let Some(program) = program.as_mut() {
                program.key(key);
            }
        });
    }

    /// Asks for the next frame, unless the loop has been torn down.
    fn step(self, last: f64) {
        let Some(tick) = self.tick.get_value() else {
            return;
        };
        request_animation_frame(move || tick(last));
    }

    /// One frame: resize to whatever the screen is now, let the program draw,
    /// paint what changed, and ask for another unless it is done.
    fn frame(self, last: f64) {
        if self.running.with_value(Option::is_none) {
            return;
        }

        let now = crate::clock::since_load_millis();
        let elapsed = if last <= 0.0 { 0.0 } else { now - last };

        let (cols, want) = (self.measure.get_value())();
        self.screen.update_value(|screen| screen.resize(cols, want));
        self.rows.update(|rows| {
            if rows.len() != want {
                rows.resize_with(want, || RwSignal::new(Vec::new()));
            }
        });

        // `try_` throughout: a stored value is gone once the terminal is, and a
        // frame already asked for can still arrive after that. Gone counts as
        // done.
        let done = self
            .running
            .try_update_value(|program| {
                program.as_mut().is_some_and(|program| {
                    self.screen
                        .try_update_value(|screen| {
                            matches!(program.frame(screen, elapsed), Step::Done)
                        })
                        .unwrap_or(true)
                })
            })
            .unwrap_or(true);

        self.repaint();

        if done {
            self.leave();
        } else {
            self.step(now);
        }
    }

    /// Rewrites only the rows whose contents actually changed.
    fn repaint(self) {
        self.screen.with_value(|screen| {
            self.rows.with_untracked(|rows| {
                for (y, row) in rows.iter().enumerate() {
                    let next = screen.runs(y);
                    if row.with_untracked(|current| *current != next) {
                        row.set(next);
                    }
                }
            });
        });
    }
}
