//! The interactive terminal.
//!
//! This is the component and the keyboard. What it is made of sits beside it:
//! `render.rs` turns output into markup, `editor.rs` is the line you are
//! typing, and `alt.rs` is the screen a program takes and the loop that drives
//! it. All three are `Copy` handles or plain functions, so the handlers here
//! can hold them without ceremony.

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;

mod alt;
mod editor;
mod render;

use alt::Alt;
use editor::Editor;
use render::{farewell, prompt_view, render_output};

use crate::program;
use crate::shell::{self, Output, Session, Sink, Span};

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

#[component]
pub fn Terminal(children: Children) -> impl IntoView {
    let session = RwSignal::new(Session::new());
    let scrollback = RwSignal::new(Vec::<Entry>::new());
    // `clear` takes the banner with it, the way clearing a real screen would.
    let banner = RwSignal::new(true);
    // Set by `exit`. The prompt goes away and nothing else is read.
    let closed = RwSignal::new(false);

    let input_ref = NodeRef::<html::Input>::new();
    let screen_ref = NodeRef::<html::Div>::new();
    let tty_ref = NodeRef::<html::Main>::new();

    // The line being typed, the caret in it, and the lines before it. See
    // editor.rs.
    let line = Editor::new(input_ref);

    // Only the path moves, so this is all that is ever rebuilt and all an
    // echoed line has to remember.
    let prompt_tail = move || format!(":{}$", session.get().prompt_path());

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

    // A program takes the whole screen; this is the screen it takes and the
    // loop that drives it. See alt.rs.
    let alt = Alt::new(measure, move || line.focus());

    // Focus on mount, so you can type without clicking first. On touch this
    // only arms the input: the keyboard still needs a tap, which no browser
    // will skip.
    Effect::new(move |_| line.focus());

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
        alt.showing();
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

    // Fetched when it is run, so none of it is in the bundle until then. The
    // entry waits in the scrollback, and says why if it never arrives.
    let launch = move |listing: &'static program::Listing, sink: Sink| {
        leptos::task::spawn_local(async move {
            match program::open_guest(&program::url(listing)).await {
                // It draws, so it takes the screen.
                Ok(program::Opened::Draws(program)) => {
                    sink.set(Output::Nothing);
                    alt.mount(program);
                }
                // It printed and exited, so the entry keeps what it said.
                Ok(program::Opened::Printed(output)) => sink.set(output),
                Err(problem) => sink.set(shell::error(problem)),
            }
        });
    };

    let submit = move || {
        let typed = line.text.get();
        let echoed = prompt_tail();

        let mut current = session.get();
        let output = shell::run(&mut current, &typed);
        session.set(current);

        line.remember(&typed);
        line.set(String::new());

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
        let completion = shell::complete(&session.get(), &line.text.get());
        if !completion.candidates.is_empty() {
            let echoed = prompt_tail();
            let typed = line.text.get();
            let listing = completion
                .candidates
                .iter()
                .map(|candidate| vec![Span::plain(candidate.clone())])
                .collect();
            record(echoed, typed, Output::Lines(listing));
        }
        line.set(completion.line);
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
        if alt.showing() {
            event.prevent_default();
            let key = event.key();
            if key == "q" || (event.ctrl_key() && key.eq_ignore_ascii_case("c")) {
                alt.leave();
            } else {
                alt.key(&key);
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
                        format!("{}^C", line.text.get_untracked()),
                        Output::Nothing,
                    );
                    line.set(String::new());
                    line.put_caret(0);
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
                'd' if line.text.get_untracked().is_empty() => {
                    line.set("exit".to_owned());
                    submit();
                    true
                }
                'a' => {
                    line.put_caret(0);
                    true
                }
                'e' => {
                    line.put_caret(line.end());
                    true
                }
                // Kill to the start of the line, and to the end of it.
                'u' => {
                    line.cut_to_start();
                    true
                }
                'k' => {
                    line.cut_to_end();
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
                line.recall(true);
            }
            "ArrowDown" => {
                event.prevent_default();
                line.recall(false);
            }
            _ => {}
        }

        // Moving the caret is this event's default action, which happens after
        // the handler returns, so where it lands can only be read next frame.
        // Not on keyup: a held key repeats keydown and sends no keyup until
        // release, which left the drawn cursor behind for as long as it was
        // down.
        request_animation_frame(move || line.sync());
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
                line.focus();
            }
        >
            <Show when=move || alt.showing()>
                <div class="alt" aria-hidden="true">
                    {move || {
                        alt.rows()
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
                class:stashed=move || alt.showing()
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
                                prop:value=move || line.text.get()
                                // The one place a click behaves normally, so
                                // you can put the caret where you want it.
                                on:pointerdown=|event| event.stop_propagation()
                                on:input:target=move |event| {
                                    line.set(event.target().value());
                                    line.sync();
                                }
                                on:click=move |_| line.sync()
                                on:select=move |_| line.sync()
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
                                    format!("calc({} * var(--cell) * -1)", line.scrolled.get())
                                }
                            >
                                {move || line.text.get()}
                            </span>
                            // The block cursor, at the real caret column. The
                            // native one is hidden in CSS.
                            <span
                                class="cursor"
                                aria-hidden="true"
                                style:left=move || {
                                    format!(
                                        "calc({} * var(--cell))",
                                        line.column.get().saturating_sub(line.scrolled.get()),
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
            <Show when=move || !closed.get() && !alt.showing()>
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
                            line.recall(true);
                        }
                    >
                        "up"
                    </button>
                    <button
                        class="key"
                        type="button"
                        on:pointerdown=move |event| {
                            event.prevent_default();
                            line.recall(false);
                        }
                    >
                        "down"
                    </button>
                </div>
            </Show>
        </main>
    }
}
