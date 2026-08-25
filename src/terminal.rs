//! The interactive terminal: scrollback, prompt, history, and completion.

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;

use crate::shell::{self, Line, Output, Session, Span};

/// One executed command and whatever it printed.
#[derive(Clone)]
struct Entry {
    /// The working directory at the time, so old lines keep their own prompt.
    prompt: String,
    input: String,
    output: Output,
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
        Output::Columns(ref lines) => view! {
            <div class="cols">
                {lines
                    .iter()
                    .map(|line| view! { <span class="line">{render_line(line)}</span> })
                    .collect_view()}
            </div>
        }
        .into_any(),
        Output::Clear | Output::Nothing => ().into_any(),
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

    let input_ref = NodeRef::<html::Input>::new();
    let bottom_ref = NodeRef::<html::Div>::new();

    let prompt = move || {
        format!(
            "{}@{}:{}$",
            crate::USER,
            crate::site_host(),
            session.get().prompt_path()
        )
    };

    let focus_input = move || {
        if let Some(element) = input_ref.get() {
            let _ = element.focus();
        }
    };

    // Read the caret back out of the input after anything that could move it.
    let sync_column = move || {
        if let Some(element) = input_ref.get() {
            if let Ok(Some(at)) = element.selection_start() {
                column.set(at as usize);
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

    // Keep the prompt in view as output accumulates, and when the on-screen
    // keyboard opens and shrinks the viewport out from under it.
    Effect::new(move |_| {
        scrollback.track();
        if let Some(bottom) = bottom_ref.get() {
            bottom.scroll_into_view();
        }
    });

    let submit = move || {
        let typed = input.get();
        let echoed = prompt();

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
            Output::Exit => {
                scrollback.update(|entries| {
                    entries.push(Entry {
                        prompt: echoed,
                        input: typed,
                        output: farewell(),
                    });
                });
                closed.set(true);
            }
            output => scrollback.update(|entries| {
                entries.push(Entry {
                    prompt: echoed,
                    input: typed,
                    output,
                });
            }),
        }
    };

    let complete = move || {
        let completion = shell::complete(&session.get(), &input.get());
        if !completion.candidates.is_empty() {
            let echoed = prompt();
            let typed = input.get();
            let listing = completion
                .candidates
                .iter()
                .map(|candidate| vec![Span::plain(candidate.clone())])
                .collect();
            scrollback.update(|entries| {
                entries.push(Entry {
                    prompt: echoed,
                    input: typed,
                    output: Output::Lines(listing),
                });
            });
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

    let on_keydown = move |event: KeyboardEvent| match event.key().as_str() {
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
    };

    view! {
        <main class="tty" on:pointerdown=move |_| focus_input()>
            <div class="screen" aria-live="polite">
                <div class:gone=move || !banner.get()>{children()}</div>
                {move || {
                    scrollback
                        .get()
                        .iter()
                        .map(|entry| {
                            view! {
                                <p class="line">
                                    <span class="prompt">{entry.prompt.clone()}</span>
                                    " "
                                    <span>{entry.input.clone()}</span>
                                </p>
                                {render_output(&entry.output)}
                            }
                        })
                        .collect_view()
                }}

                // A real input, sitting inline in the prompt line, rather than a
                // hidden one mirrored into a span. Mirroring would draw the
                // caret at the end of the text even after Left arrow, which
                // misreports where typing will land.
                <p class="line current">
                    <span class="prompt">{prompt}</span>
                    <input
                        class="stdin"
                        node_ref=input_ref
                        type="text"
                        autocapitalize="off"
                        autocomplete="off"
                        spellcheck="false"
                        aria-label="terminal input"
                        prop:value=move || input.get()
                        on:input:target=move |event| {
                            recalled.set(None);
                            input.set(event.target().value());
                        }
                        on:keydown=on_keydown
                    />
                </p>
                <div node_ref=bottom_ref></div>
            </div>

            // Pointerdown with the default prevented, so focus never leaves the
            // input and the on-screen keyboard stays up.
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
        </main>
    }
}
