//! Turning what a command said into markup.
//!
//! No state and no signals: everything here is a value in and a view out.

use leptos::prelude::*;

use crate::shell::{Line, Output, Span};

/// The prompt, built the same way everywhere it appears.
///
/// Split into elements so no text node looks like an email address, which
/// Cloudflare rewrites. Both prompts have to split the same way: each inline
/// box rounds on its own, so the same text in one box and in four lands a
/// sixteenth of a pixel apart, and the line shifts when you press Enter.
pub fn prompt_view(tail: impl IntoView + 'static) -> impl IntoView {
    view! {
        <span class="prompt">
            <span>{crate::USER}</span>
            "@"
            <span>{crate::site_host()}</span>
            {tail}
        </span>
    }
}

/// What `exit` prints before the session goes quiet. The personal line is in
/// `content/logout.txt`, so rewording it is not a code change.
pub fn farewell() -> Output {
    const GOODBYE: &str = include_str!("../../content/logout.txt");

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

pub fn render_output(output: &Output) -> AnyView {
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
