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
    /// The prompt's tail at the time, so old lines keep their own working
    /// directory. Only the tail: the rest of the prompt is fixed, and is
    /// rebuilt around this by [`prompt_view`].
    tail: String,
    input: String,
    /// A signal rather than a value, because a command is allowed to answer
    /// later. Anything settled writes it once and never touches it again.
    output: RwSignal<Output>,
}

/// The prompt, built the same way everywhere it appears.
///
/// The user and the host are separate elements so that no single text node
/// holds anything shaped like an email address, which Cloudflare would rewrite
/// (see the note in the terminal's view). That split has to be identical in
/// the live prompt and in the echoed one: each inline box is shaped and
/// rounded on its own, so the same characters in one box and in four do not
/// land on quite the same pixels, and the line visibly shifts the moment you
/// press Enter.
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

/// Whether anything on the page is selected.
///
/// `ctrl-c` means copy when something is and interrupt when nothing is, which
/// is the same split a terminal emulator makes.
#[cfg(feature = "hydrate")]
fn selecting() -> bool {
    window()
        .get_selection()
        .ok()
        .flatten()
        .and_then(|selection| selection.to_string().as_string())
        .is_some_and(|text| !text.is_empty())
}

/// The server has no selection to read, and never handles a keystroke. This
/// exists so the module compiles into its build alongside the rest.
#[cfg(not(feature = "hydrate"))]
const fn selecting() -> bool {
    false
}

/// The frame loop, stored so it can schedule itself.
///
/// Shared rather than owned because it has to outlive the call that set it up
/// and be reachable from the frame it asks for, which is the one thing a plain
/// closure cannot do for itself.
type Tick = std::rc::Rc<dyn Fn(f64)>;

/// How many whole cells fit into a span of pixels.
///
/// Clamped before it is counted: a zero cell would divide by nothing, and no
/// screen is four thousand characters across, so the conversion cannot lose
/// anything the caller would have wanted.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 1..=4096 on the line above"
)]
fn fits(span: f64, cell: f64) -> usize {
    if cell <= 0.0 {
        return 1;
    }
    (span / cell).floor().clamp(1.0, 4096.0) as usize
}

/// What `exit` prints before the session goes inert. The personal line lives in
/// `content/logout.txt` so it can be reworded without touching code.
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

/// Paints one line of output. Link spans become real anchors, which is the one
/// thing on this page you click rather than type.
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
        // Pending never reaches here: the entry holds `Nothing` until its task
        // answers, and what the task writes is one of the arms above.
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
    // Where the caret sits, in cells. The font is fixed width, so the column is
    // the offset: no measuring, and it stays right after an arrow key.
    let column = RwSignal::new(0_usize);
    // The first column shown, for a line too long for the field. Kept here in
    // whole columns rather than read back from the input: the input scrolls by
    // pixels and will happily stop half way through a character, which a
    // terminal never does. Its own text is not drawn, so only this matters.
    let scrolled = RwSignal::new(0_usize);

    let input_ref = NodeRef::<html::Input>::new();
    let bottom_ref = NodeRef::<html::Div>::new();
    let tty_ref = NodeRef::<html::Main>::new();

    // The running program and the screen it draws into. Local storage because
    // neither is `Send`, and neither has any reason to be: both live and die
    // on the one thread the browser gives us.
    let running = StoredValue::new_local(None::<Box<dyn Program>>);
    let screen = StoredValue::new_local(Screen::new(0, 0));
    // One signal per row, so a frame only rewrites the rows that changed
    // rather than the whole grid.
    let rows = RwSignal::new(Vec::<RwSignal<Vec<Run>>>::new());
    let alt = RwSignal::new(false);

    // Only the path moves. The rest of the prompt is fixed markup, so this is
    // all that is ever rebuilt, and all an echoed line has to remember.
    let prompt_tail = move || format!(":{}$", session.get().prompt_path());

    let focus_input = move || {
        if let Some(element) = input_ref.get() {
            let _ = element.focus();
        }
    };

    // Read the caret back out of the input after anything that could move it.
    // Only writes when it actually moved, since a held key syncs on every
    // repeat and an unconditional set would redraw the cursor each time.
    let sync_column = move || {
        if let Some(element) = input_ref.get()
            && let Ok(Some(at)) = element.selection_start()
        {
            let at = at as usize;
            if column.get_untracked() != at {
                column.set(at);
            }
            // Scroll only when the caret would otherwise leave the field, and
            // then by whole columns.
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

    // Focus as soon as the terminal mounts, so a visitor can type immediately
    // instead of having to click first. On touch this only arms the input; the
    // keyboard still needs a tap, which no browser will skip.
    Effect::new(move |_| focus_input());

    // `autocorrect` is a Safari extension with no typed setter in Leptos, and
    // without it iOS rewrites commands into prose as you type them.
    Effect::new(move |_| {
        if let Some(element) = input_ref.get() {
            let _ = element.set_attribute("autocorrect", "off");
        }
    });

    // On the next frame, not this one: whatever was just printed has to be laid
    // out before the browser can be told where the bottom now is. Chrome hides
    // the difference by scrolling the focused input back into view by itself,
    // which is why this only shows up elsewhere, as a prompt left below the
    // bottom of a full screen.
    let scroll_to_prompt = move || {
        request_animation_frame(move || {
            if let Some(bottom) = bottom_ref.get_untracked() {
                bottom.scroll_into_view();
            }
        });
    };

    // Keep the prompt in view as output accumulates.
    Effect::new(move |_| {
        scrollback.track();
        scroll_to_prompt();
    });

    // And keep it in view when the screen itself changes size, which is what
    // an on-screen keyboard opening looks like from here. Measuring is also
    // what gives the terminal its height, so this runs on mount either way.
    Effect::new(move |_| crate::viewport::track(scroll_to_prompt));

    // The only way an entry reaches the scrollback. A settled output goes
    // straight in; a pending one leaves the entry blank and starts the task
    // that fills it, which has to happen after the entry is in place so a task
    // resolving immediately still has somewhere to write.
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
            // A late answer changes this one cell and not the scrollback, so
            // the effect watching the scrollback has no reason to fire. This
            // gives it one.
            Effect::new(move |_| {
                cell.track();
                scroll_to_prompt();
            });
            task(Sink::new(cell));
        }
    };

    // How many cells fit, measured from the terminal itself rather than
    // assumed: the cell is fixed width, so one probe gives both dimensions.
    let measure = move || {
        // The terminal itself, not the scrollback inside it: the scrollback is
        // hidden while a program is running, and a hidden element measures
        // zero. The alternate screen fills this box exactly.
        let Some(host) = tty_ref.get_untracked() else {
            return (80, 24);
        };
        let width = f64::from(host.client_width());
        let height = f64::from(host.client_height());
        // The font is 8 by 16 at its base size and the stylesheet doubles it,
        // so a cell is always half as wide as it is tall. Reading the computed
        // size back is what keeps this true at either breakpoint.
        let cell = crate::viewport::cell_size();
        if cell.0 <= 0.0 || cell.1 <= 0.0 {
            return (80, 24);
        }
        (fits(width, cell.0), fits(height, cell.1))
    };

    /// Repaints only the rows whose contents actually changed.
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

    // The frame loop. It owns the clock, calls the program once per frame, and
    // stops the moment the program says it is done or is torn down under it.
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

    // Hands the screen to a program that is ready to draw.
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
        // The input is the keyboard whether or not it can be seen, so it keeps
        // focus while the program has the screen.
        focus_input();
        screen.with_value(|screen| rows.with_untracked(|rows| repaint(screen, rows)));
        step(0.0);
    };

    // Fetched when it is run, so none of it is in the bundle until then. The
    // entry sits in the scrollback meanwhile, and carries the reason if it
    // never arrives.
    let launch = move |listing: &'static program::Listing, sink: Sink| {
        leptos::task::spawn_local(async move {
            match program::open_guest(listing.url).await {
                // It draws, so it takes the screen.
                Ok(program::Opened::Draws(program)) => {
                    sink.set(Output::Nothing);
                    mount(program);
                }
                // It printed and exited, so the entry keeps what it said and
                // the prompt comes straight back.
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
                // draw, and a native one resolves the same way for one path
                // rather than two.
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

    // Up walks back through history, down walks forward and off the end into a
    // fresh empty line, which is what a shell does.
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

    // Where the caret is right now, read from the input rather than from the
    // `column` signal, which only catches up on the next frame.
    let caret = move || {
        input_ref
            .get_untracked()
            .and_then(|element| element.selection_start().ok().flatten())
            .map_or_else(|| input.get_untracked().len(), |at| at as usize)
    };

    // On the next frame, for the same reason the cursor syncs there: changing
    // the line writes `prop:value` back to the input, and the browser puts the
    // caret at the end when it does. Placing it before that write would just be
    // overwritten.
    let put_caret = move |at: usize| {
        request_animation_frame(move || {
            if let Some(element) = input_ref.get_untracked() {
                let at = u32::try_from(at.min(element.value().len())).unwrap_or(u32::MAX);
                let _ = element.set_selection_range(at, at);
                column.set(at as usize);
            }
        });
    };

    /// The readline bindings a shell answers to. `ctrl-w` and `ctrl-n` are
    /// missing on purpose: browsers reserve them for closing the tab and
    /// opening a window, and will not let the page have them, so binding them
    /// would delete a word and close the tab.
    #[expect(
        clippy::items_after_statements,
        reason = "the table belongs beside the handler that reads it"
    )]
    fn shortcut(key: &str) -> Option<char> {
        key.chars().next().filter(|_| key.len() == 1)
    }

    let on_keydown = move |event: KeyboardEvent| {
        // A running program owns the keyboard. Ctrl-C is the exception, the
        // way it is in any terminal: it is how you get out of something that
        // has stopped listening.
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

        if event.ctrl_key()
            && let Some(letter) = shortcut(&event.key().to_lowercase())
        {
            let handled = match letter {
                // Copying wins when there is something selected, the way a
                // terminal emulator hands ctrl-c back for a selection.
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
                // Clears around whatever is half typed, which stays.
                'l' => {
                    scrollback.set(Vec::new());
                    banner.set(false);
                    true
                }
                // End of input closes the session, but only on an empty line;
                // otherwise it would throw away what you were writing.
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

        // Moving the caret is this event's default action, which the browser
        // performs after the handler returns, so the column it lands on can
        // only be read on the next frame. Holding a key repeats keydown and
        // sends no keyup until release, which is why reading it on keyup alone
        // left the drawn cursor behind for as long as the key was down.
        request_animation_frame(sync_column);
    };

    view! {
        // Anywhere that is not a link or the prompt itself gives the keyboard
        // straight back, so the shell never silently stops listening. There is
        // no mouse on a tty, so there is nothing else for a click to do.
        //
        // Preventing the default on pointerdown stops the focus moving and the
        // selection starting, but a click still fires, so links keep working
        // without needing to opt out of this.
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

            <div class="screen" class:stashed=move || alt.get() aria-live="polite">
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
                                    // The same colour it was while it was
                                    // being typed: what you wrote should not
                                    // change appearance the moment you run it.
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

                // A real input, sitting inline in the prompt line, rather than a
                // hidden one mirrored into a span. Mirroring would draw the
                // caret at the end of the text even after Left arrow, which
                // misreports where typing will land.
                <Show when=move || !closed.get()>
                    <p class="line current">
                        // Split so that no single text node holds anything
                        // shaped like an email address. Cloudflare rewrites
                        // any that does into a link it decodes on load, which
                        // left this prompt as two nodes where the server sent
                        // one. Hydration bound to the first, and every `cd`
                        // wrote the new prompt there while the stale tail sat
                        // beside it: `guest@host:~/projects$:~$`.
                        {prompt_view(prompt_tail)}
                        <span class="field">
                            <input
                                class="stdin"
                                node_ref=input_ref
                                type="text"
                                autocapitalize="off"
                                autocomplete="off"
                                spellcheck="false"
                                // Labels the phone's return key "go" rather
                                // than "return", since it runs the command.
                                enterkeyhint="go"
                                aria-label="terminal input"
                                prop:value=move || input.get()
                                // The prompt is the one place a click should
                                // behave normally, so you can put the caret
                                // where you want it.
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
                            // by it. An input renders its text in its own way,
                            // which is not quite how a span renders the same
                            // characters, so a line changed appearance the
                            // moment it was echoed into the scrollback as one.
                            // Both are spans now, and cannot differ.
                            //
                            // Painted after the input, so the selection the
                            // input draws shows behind these glyphs rather
                            // than instead of them. It scrolls with the input
                            // too: a line longer than the field scrolls rather
                            // than wrapping.
                            <span
                                class="typed"
                                aria-hidden="true"
                                // Offset with `left`, not a transform: a
                                // transform puts the text on its own layer,
                                // and a layer rasterises glyphs on a different
                                // pixel grid from the plain span this becomes
                                // when the line is echoed.
                                style:left=move || {
                                    format!("calc({} * var(--cell) * -1)", scrolled.get())
                                }
                            >
                                {move || input.get()}
                            </span>
                            // The block cursor, drawn at the real caret column.
                            // The native caret is hidden in CSS.
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
                <div node_ref=bottom_ref></div>
            </div>

            // Pointerdown with the default prevented, so focus never leaves the
            // input and the on-screen keyboard stays up.
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
