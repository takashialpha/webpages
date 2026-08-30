//! The interactive terminal: scrollback, prompt, history, and completion.

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;

use crate::program::{self, Program, Step};
use crate::screen::{Run, Screen};
use crate::shell::{self, Line, Output, Session, Sink, Span};

/// One executed command and whatever it printed.
#[derive(Clone)]
struct Entry {
    /// The prompt's tail at the time, so an old line keeps its directory.
    /// [`prompt_view`] rebuilds the rest around it.
    tail: String,
    input: String,
    /// A signal, because a command may answer later.
    output: RwSignal<Output>,
}

/// The prompt, built the same way everywhere it appears.
///
/// Split into elements so no text node looks like an email address, which
/// Cloudflare rewrites. Both prompts have to split the same way: each inline
/// box rounds on its own, so the same text in one box and in four lands a
/// sixteenth of a pixel apart, and the line shifts when you press Enter.
fn prompt_view(tail: impl IntoView + 'static) -> impl IntoView {
    view! {
        <span class="prompt">
            <span>{crate::USER}</span>
            "@"
            <span>{crate::site_host()}</span>
            {tail}
        </span>
    }
}

/// Whether anything is selected. `ctrl-c` copies if so, interrupts if not.
#[cfg(feature = "hydrate")]
fn selecting() -> bool {
    window()
        .get_selection()
        .ok()
        .flatten()
        .and_then(|selection| selection.to_string().as_string())
        .is_some_and(|text| !text.is_empty())
}

/// The server handles no keystrokes. Here so the module compiles there.
#[cfg(not(feature = "hydrate"))]
const fn selecting() -> bool {
    false
}

/// The frame loop, kept so it can schedule itself. Shared because it outlives
/// the call that set it up and has to be reachable from the frame it asked for.
type Tick = std::rc::Rc<dyn Fn(f64)>;

/// How many whole cells fit into a span of pixels.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped into 1..=4096 before the cast"
)]
fn fits(span: f64, cell: f64) -> usize {
    if cell <= 0.0 {
        return 1;
    }
    (span / cell).floor().clamp(1.0, 4096.0) as usize
}

/// What `exit` prints before the session goes quiet. The personal line is in
/// `content/logout.txt`, so rewording it is not a code change.
fn farewell() -> Output {
    const GOODBYE: &str = include_str!("../content/logout.txt");

    Output::Lines(vec![
        vec![Span::plain("logout")],
        vec![Span::plain(GOODBYE.trim_end())],
        vec![Span::new(
            format!("Connection to {} closed.", crate::site_host()),
            "dim",
        )],
    ])
}

/// One line of output. Link spans become real anchors, the one thing here you
/// click rather than type.
fn render_line(line: &Line) -> AnyView {
    line.iter()
        .map(|span| {
            if span.class == "link" {
                view! {
                    <a href=span.text.clone() target="_blank" rel="noreferrer">
                        {span.text.clone()}
                    </a>
                }
                .into_any()
            } else {
                view! { <span class=span.class>{span.text.clone()}</span> }.into_any()
            }
        })
        .collect_view()
        .into_any()
}

fn render_output(output: &Output) -> AnyView {
    match *output {
        Output::Lines(ref lines) => lines
            .iter()
            .map(|line| view! { <p class="line">{render_line(line)}</p> })
            .collect_view()
            .into_any(),
        Output::Wide(ref lines) => view! {
            <div class="wide">
                {lines
                    .iter()
                    .map(|line| view! { <p class="line">{render_line(line)}</p> })
                    .collect_view()}
            </div>
        }
        .into_any(),
        Output::Columns(ref lines) => view! {
            <div class="cols">
                {lines
                    .iter()
                    .map(|line| view! { <span class="line">{render_line(line)}</span> })
                    .collect_view()}
            </div>
        }
        .into_any(),
        // Pending never gets here: the entry holds `Nothing` until its task
        // answers, and the task writes one of the arms above.
        Output::Pending(_) | Output::Run(_) | Output::Clear | Output::Exit | Output::Nothing => {
            ().into_any()
        }
    }
}

#[component]
pub fn Terminal(children: Children) -> impl IntoView {
    let session = RwSignal::new(Session::new());
    let scrollback = RwSignal::new(Vec::<Entry>::new());
    let input = RwSignal::new(String::new());
    let history = RwSignal::new(Vec::<String>::new());
    // `None` means "editing a fresh line"; otherwise an index into `history`.
    let recalled = RwSignal::new(Option::<usize>::None);
    // `clear` takes the banner with it, the way clearing a real screen would.
    let banner = RwSignal::new(true);
    // Set by `exit`. The prompt goes away and nothing else is read.
    let closed = RwSignal::new(false);
    // Where the caret is, in cells. Fixed width font, so the column is just
    // the offset: nothing to measure, and it stays right after an arrow key.
    let column = RwSignal::new(0_usize);
    // The first column shown, once a line outgrows the field. Whole columns:
    // an input scrolls by pixels and stops mid-character, which a terminal
    // never does. Its own text is not drawn, so only this matters.
    let scrolled = RwSignal::new(0_usize);

    let input_ref = NodeRef::<html::Input>::new();
    let screen_ref = NodeRef::<html::Div>::new();
    let tty_ref = NodeRef::<html::Main>::new();

    // The running program and its screen. Local, because neither is `Send` and
    // a browser has one thread anyway.
    let running = StoredValue::new_local(None::<Box<dyn Program>>);
    let screen = StoredValue::new_local(Screen::new(0, 0));
    // One signal per row, so a frame rewrites only the rows that changed.
    let rows = RwSignal::new(Vec::<RwSignal<Vec<Run>>>::new());
    let alt = RwSignal::new(false);

    // Only the path moves, so this is all that is ever rebuilt and all an
    // echoed line has to remember.
    let prompt_tail = move || format!(":{}$", session.get().prompt_path());

    let focus_input = move || {
        if let Some(element) = input_ref.get() {
            let _ = element.focus();
        }
    };

    // Read the caret back after anything that could have moved it. Writes only
    // on a change, since a held key comes through on every repeat.
    let sync_column = move || {
        if let Some(element) = input_ref.get()
            && let Ok(Some(at)) = element.selection_start()
        {
            let at = at as usize;
            if column.get_untracked() != at {
                column.set(at);
            }
            // Only when the caret would leave the field, and then by whole
            // columns.
            let cell = crate::viewport::cell_size().0;
            let visible = fits(f64::from(element.client_width()), cell);
            let mut offset = scrolled.get_untracked();
            if at < offset {
                offset = at;
            } else if at >= offset + visible {
                offset = at + 1 - visible;
            }
            if scrolled.get_untracked() != offset {
                scrolled.set(offset);
            }
        }
    };

    // Focus on mount, so you can type without clicking first. On touch this
    // only arms the input: the keyboard still needs a tap, which no browser
    // will skip.
    Effect::new(move |_| focus_input());

    // A Safari attribute with no typed setter in leptos. Without it, ios
    // rewrites commands into prose as you type.
    Effect::new(move |_| {
        if let Some(element) = input_ref.get() {
            let _ = element.set_attribute("autocorrect", "off");
        }
    });

    // Next frame, not this one: what was just printed has to be laid out before
    // the browser can be told where the bottom is.
    //
    // The scroller's own `scrollTop`, rather than scrolling an element into
    // view: that walks every scrollable ancestor on the way up, and ios will
    // scroll the document itself even with `overflow: hidden` on the body,
    // which is exactly what drags the fixed terminal out of place.
    let scroll_to_prompt = move || {
        request_animation_frame(move || {
            if let Some(screen) = screen_ref.get_untracked() {
                screen.set_scroll_top(screen.scroll_height());
            }
        });
    };

    // Keep the prompt in view as output accumulates, and again when a program
    // hands the screen back: the scrollback is `overflow: hidden` while it sits
    // out of sight, which pins it to the top and loses where it was.
    Effect::new(move |_| {
        scrollback.track();
        alt.track();
        scroll_to_prompt();
    });

    // And keep it in view when the screen itself changes size, which is what
    // an on-screen keyboard opening looks like from here. Measuring is also
    // what gives the terminal its height, so this runs on mount either way.
    Effect::new(move |_| crate::viewport::track(scroll_to_prompt));

    // The only way into the scrollback. A settled output goes straight in; a
    // pending one leaves the entry blank and starts the task that fills it,
    // after the entry is in place so a task that answers at once has somewhere
    // to write.
    let record = move |tail: String, input: String, output: Output| {
        let (initial, task) = match output {
            Output::Pending(task) => (Output::Nothing, Some(task)),
            settled => (settled, None),
        };

        let cell = RwSignal::new(initial);
        scrollback.update(|entries| {
            entries.push(Entry {
                tail,
                input,
                output: cell,
            });
        });

        if let Some(task) = task {
            // A late answer touches this cell and not the scrollback, so the
            // effect watching the scrollback would not fire. This one does.
            Effect::new(move |_| {
                cell.track();
                scroll_to_prompt();
            });
            task(Sink::new(cell));
        }
    };

    // How many cells fit, measured rather than assumed.
    let measure = move || {
        // The terminal, not the scrollback inside it: that is out of sight
        // while a program runs. The alternate screen fills this box exactly.
        let Some(host) = tty_ref.get_untracked() else {
            return (80, 24);
        };
        let width = f64::from(host.client_width());
        let height = f64::from(host.client_height());
        // The font is 8 by 16 and the stylesheet only ever doubles it, so a
        // cell is half as wide as it is tall. Measuring is what keeps that true
        // at either breakpoint.
        let cell = crate::viewport::cell_size();
        if cell.0 <= 0.0 || cell.1 <= 0.0 {
            return (80, 24);
        }
        (fits(width, cell.0), fits(height, cell.1))
    };

    /// Repaints only the rows that changed.
    #[expect(
        clippy::items_after_statements,
        reason = "it belongs beside the loop that calls it"
    )]
    fn repaint(screen: &Screen, rows: &[RwSignal<Vec<Run>>]) {
        for (y, row) in rows.iter().enumerate() {
            let next = screen.runs(y);
            if row.with_untracked(|current| *current != next) {
                row.set(next);
            }
        }
    }

    let leave = move || {
        running.set_value(None);
        alt.set(false);
        rows.set(Vec::new());
        focus_input();
    };

    // Owns the clock, calls the program once a frame, and stops the moment it
    // says it is done or is taken away underneath it.
    let tick: StoredValue<Option<Tick>, LocalStorage> = StoredValue::new_local(None);
    let step = move |last: f64| {
        let Some(run) = tick.get_value() else { return };
        request_animation_frame(move || run(last));
    };

    tick.set_value(Some(std::rc::Rc::new(move |last: f64| {
        if running.with_value(Option::is_none) {
            return;
        }

        let now = crate::clock::since_load_millis();
        let elapsed = if last <= 0.0 { 0.0 } else { now - last };

        let (cols, want_rows) = measure();
        screen.update_value(|screen| screen.resize(cols, want_rows));
        rows.update(|rows| {
            if rows.len() != want_rows {
                rows.resize_with(want_rows, || RwSignal::new(Vec::new()));
            }
        });

        let done = running
            .try_update_value(|program| {
                program.as_mut().is_some_and(|program| {
                    screen
                        .try_update_value(|screen| {
                            matches!(program.frame(screen, elapsed), Step::Done)
                        })
                        .unwrap_or(true)
                })
            })
            .unwrap_or(true);

        screen.with_value(|screen| rows.with_untracked(|rows| repaint(screen, rows)));

        if done {
            leave();
        } else {
            step(now);
        }
    })));

    // Hands the screen to a program ready to draw.
    let mount = move |mut program: Box<dyn Program>| {
        let (cols, want_rows) = measure();
        screen.update_value(|screen| {
            screen.resize(cols, want_rows);
            screen.clear();
        });
        rows.set((0..want_rows).map(|_| RwSignal::new(Vec::new())).collect());
        screen.update_value(|screen| program.start(screen));
        running.set_value(Some(program));
        alt.set(true);
        // The input is the keyboard whether it can be seen or not, so it keeps
        // focus while the program has the screen.
        focus_input();
        screen.with_value(|screen| rows.with_untracked(|rows| repaint(screen, rows)));
        step(0.0);
    };

    // Fetched when it is run, so none of it is in the bundle until then. The
    // entry waits in the scrollback, and says why if it never arrives.
    let launch = move |listing: &'static program::Listing, sink: Sink| {
        leptos::task::spawn_local(async move {
            match program::open_guest(listing.url).await {
                // It draws, so it takes the screen.
                Ok(program::Opened::Draws(program)) => {
                    sink.set(Output::Nothing);
                    mount(program);
                }
                // It printed and exited, so the entry keeps what it said.
                Ok(program::Opened::Printed(output)) => sink.set(output),
                Err(problem) => sink.set(shell::error(problem)),
            }
        });
    };

    let submit = move || {
        let typed = input.get();
        let echoed = prompt_tail();

        let mut current = session.get();
        let output = shell::run(&mut current, &typed);
        session.set(current);

        if !typed.trim().is_empty() {
            history.update(|entries| entries.push(typed.clone()));
        }
        recalled.set(None);
        input.set(String::new());

        match output {
            Output::Clear => {
                scrollback.set(Vec::new());
                banner.set(false);
            }
            Output::Run(listing) => {
                // Deferred, because a guest has to be fetched before it can
                // draw. One path rather than two.
                record(
                    echoed,
                    typed,
                    shell::pending(move |sink| launch(listing, sink)),
                );
            }
            Output::Exit => {
                record(echoed, typed, farewell());
                closed.set(true);
            }
            output => record(echoed, typed, output),
        }
    };

    let complete = move || {
        let completion = shell::complete(&session.get(), &input.get());
        if !completion.candidates.is_empty() {
            let echoed = prompt_tail();
            let typed = input.get();
            let listing = completion
                .candidates
                .iter()
                .map(|candidate| vec![Span::plain(candidate.clone())])
                .collect();
            record(echoed, typed, Output::Lines(listing));
        }
        input.set(completion.line);
    };

    // Up walks back, down walks forward and off the end into a fresh empty
    // line, which is what a shell does.
    let recall = move |backwards: bool| {
        let entries = history.get();
        if entries.is_empty() {
            return;
        }
        let next = match (recalled.get(), backwards) {
            (None, true) => Some(entries.len().saturating_sub(1)),
            (None, false) => None,
            (Some(index), true) => Some(index.saturating_sub(1)),
            (Some(index), false) => {
                if index + 1 < entries.len() {
                    Some(index + 1)
                } else {
                    None
                }
            }
        };
        recalled.set(next);
        input.set(
            next.and_then(|index| entries.get(index).cloned())
                .unwrap_or_default(),
        );
    };

    // Read from the input, not from `column`, which only catches up next frame.
    let caret = move || {
        input_ref
            .get_untracked()
            .and_then(|element| element.selection_start().ok().flatten())
            .map_or_else(|| input.get_untracked().len(), |at| at as usize)
    };

    // Next frame: changing the line rewrites `prop:value` and puts the caret
    // at the end, so placing it first would be overwritten.
    let put_caret = move |at: usize| {
        request_animation_frame(move || {
            if let Some(element) = input_ref.get_untracked() {
                let at = u32::try_from(at.min(element.value().len())).unwrap_or(u32::MAX);
                let _ = element.set_selection_range(at, at);
                column.set(at as usize);
            }
        });
    };

    /// The letter of a plain one-character key, so `ctrl-a` is told from
    /// `ctrl-ArrowLeft`.
    #[expect(
        clippy::items_after_statements,
        reason = "it belongs beside the handler that calls it"
    )]
    fn shortcut(key: &str) -> Option<char> {
        let mut letters = key.chars();
        letters.next().filter(|_| letters.next().is_none())
    }

    let on_keydown = move |event: KeyboardEvent| {
        // A running program owns the keyboard, except for ctrl-c, the way it
        // is anywhere: it is how you leave something that stopped listening.
        if alt.get_untracked() {
            event.prevent_default();
            let key = event.key();
            if key == "q" || (event.ctrl_key() && key.eq_ignore_ascii_case("c")) {
                leave();
            } else {
                running.update_value(|program| {
                    if let Some(program) = program.as_mut() {
                        program.key(&key);
                    }
                });
            }
            return;
        }

        // The readline bindings a shell answers to. `ctrl-w` and `ctrl-n` are
        // missing on purpose: browsers keep them for closing the tab and
        // opening a window and will not hand them over, so binding them would
        // delete a word and close the tab.
        if event.ctrl_key()
            && let Some(letter) = shortcut(&event.key().to_lowercase())
        {
            let handled = match letter {
                // A selection means copy, the way any terminal emulator hands
                // ctrl-c back for one.
                'c' if !selecting() => {
                    record(
                        prompt_tail(),
                        format!("{}^C", input.get_untracked()),
                        Output::Nothing,
                    );
                    recalled.set(None);
                    input.set(String::new());
                    put_caret(0);
                    true
                }
                // Clears around whatever is half typed, which stays put.
                'l' => {
                    scrollback.set(Vec::new());
                    banner.set(false);
                    true
                }
                // End of input closes the session, but only on an empty line,
                // or it would throw away what you were writing.
                'd' if input.get_untracked().is_empty() => {
                    input.set("exit".to_owned());
                    submit();
                    true
                }
                'a' => {
                    put_caret(0);
                    true
                }
                'e' => {
                    put_caret(input.get_untracked().len());
                    true
                }
                // Kill to the start of the line, and to the end of it.
                'u' => {
                    let at = caret();
                    input.update(|line| line.drain(..at.min(line.len())).for_each(drop));
                    put_caret(0);
                    true
                }
                'k' => {
                    let at = caret();
                    input.update(|line| line.truncate(at.min(line.len())));
                    put_caret(at);
                    true
                }
                _ => false,
            };

            if handled {
                event.prevent_default();
                return;
            }
        }

        match event.key().as_str() {
            "Enter" => {
                event.prevent_default();
                submit();
            }
            "Tab" => {
                event.prevent_default();
                complete();
            }
            "ArrowUp" => {
                event.prevent_default();
                recall(true);
            }
            "ArrowDown" => {
                event.prevent_default();
                recall(false);
            }
            _ => {}
        }

        // Moving the caret is this event's default action, which happens after
        // the handler returns, so where it lands can only be read next frame.
        // Not on keyup: a held key repeats keydown and sends no keyup until
        // release, which left the drawn cursor behind for as long as it was
        // down.
        request_animation_frame(sync_column);
    };

    view! {
        // Anywhere but a link or the prompt hands the keyboard back, so the
        // shell never quietly stops listening. There is no mouse on a tty, so a
        // click has nothing else to do.
        //
        // Preventing the default on pointerdown stops focus moving and stops a
        // selection starting, but a click still fires, so links keep working
        // without opting out.
        <main
            node_ref=tty_ref
            class="tty"
            on:pointerdown=move |event| {
                event.prevent_default();
                focus_input();
            }
        >
            <Show when=move || alt.get()>
                <div class="alt" aria-hidden="true">
                    {move || {
                        rows.get()
                            .into_iter()
                            .map(|row| {
                                view! {
                                    <p class="row">
                                        {move || {
                                            row.get()
                                                .into_iter()
                                                .map(|run| {
                                                    view! {
                                                        <span class=format!(
                                                            "f{} b{}",
                                                            run.fg,
                                                            run.bg,
                                                        )>{run.text}</span>
                                                    }
                                                })
                                                .collect_view()
                                        }}
                                    </p>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </Show>

            <div
                node_ref=screen_ref
                class="screen"
                class:stashed=move || alt.get()
                aria-live="polite"
            >
                <div class:gone=move || !banner.get()>{children()}</div>
                {move || {
                    scrollback
                        .get()
                        .iter()
                        .map(|entry| {
                            view! {
                                <p class="line">
                                    {prompt_view(entry.tail.clone())}
                                    " "
                                    // The colour it was while you were typing
                                    // it: running a line should not change how
                                    // it looks.
                                    <span class="typed">{entry.input.clone()}</span>
                                </p>
                                {
                                    let cell = entry.output;
                                    move || cell.with(render_output)
                                }
                            }
                        })
                        .collect_view()
                }}

                // A real input, inline in the prompt line, rather than a
                // hidden one mirrored into a span. Mirroring draws the caret at
                // the end of the text even after Left, which lies about where
                // typing will land.
                <Show when=move || !closed.get()>
                    <p class="line current">
                        // Split so no one text node is shaped like an email
                        // address. Cloudflare rewrites any that is into a link
                        // it decodes on load, which left this prompt as two
                        // nodes where the server sent one. Hydration bound to
                        // the first, and every `cd` wrote there while the stale
                        // tail sat beside it: `guest@host:~/projects$:~$`.
                        {prompt_view(prompt_tail)}
                        <span class="field">
                            <input
                                class="stdin"
                                node_ref=input_ref
                                type="text"
                                autocapitalize="off"
                                autocomplete="off"
                                spellcheck="false"
                                // Labels the phone's return key "go", since
                                // that is what it does.
                                enterkeyhint="go"
                                aria-label="terminal input"
                                prop:value=move || input.get()
                                // The one place a click behaves normally, so
                                // you can put the caret where you want it.
                                on:pointerdown=|event| event.stop_propagation()
                                on:input:target=move |event| {
                                    recalled.set(None);
                                    input.set(event.target().value());
                                    sync_column();
                                }
                                on:click=move |_| sync_column()
                                on:select=move |_| sync_column()
                                on:keydown=on_keydown
                            />
                            // The typed text, drawn over the input rather than
                            // by it. An input lays its text out slightly
                            // differently from a span, so a line used to change
                            // appearance the moment it was echoed into the
                            // scrollback as one. Both are spans now.
                            //
                            // After the input, so the selection the input draws
                            // shows behind these glyphs rather than instead of
                            // them.
                            <span
                                class="typed"
                                aria-hidden="true"
                                // `left`, not a transform: a transform puts
                                // the text on its own layer, and a layer
                                // rasterises glyphs on a different pixel grid
                                // from the plain span this becomes when echoed.
                                style:left=move || {
                                    format!("calc({} * var(--cell) * -1)", scrolled.get())
                                }
                            >
                                {move || input.get()}
                            </span>
                            // The block cursor, at the real caret column. The
                            // native one is hidden in CSS.
                            <span
                                class="cursor"
                                aria-hidden="true"
                                style:left=move || {
                                    format!(
                                        "calc({} * var(--cell))",
                                        column.get().saturating_sub(scrolled.get()),
                                    )
                                }
                            >
                                "_"
                            </span>
                        </span>
                    </p>
                </Show>
            </div>

            // Pointerdown with the default prevented, so focus never leaves
            // the input and the keyboard stays up.
            <Show when=move || !closed.get() && !alt.get()>
                <div class="keys">
                    <button
                        class="key"
                        type="button"
                        on:pointerdown=move |event| {
                            event.prevent_default();
                            complete();
                        }
                    >
                        "tab"
                    </button>
                    <button
                        class="key"
                        type="button"
                        on:pointerdown=move |event| {
                            event.prevent_default();
                            recall(true);
                        }
                    >
                        "up"
                    </button>
                    <button
                        class="key"
                        type="button"
                        on:pointerdown=move |event| {
                            event.prevent_default();
                            recall(false);
                        }
                    >
                        "down"
                    </button>
                </div>
            </Show>
        </main>
    }
}
