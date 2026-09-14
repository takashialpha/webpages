//! What is mounted where.
//!
//! Four things and a fallback: the board, a sitemap, a health check, the one
//! page, and everything under the site root.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Uri, header};
use axum::middleware::from_fn;
use axum::response::Response;
use axum::routing::get;
use leptos::config::LeptosOptions;
use leptos_axum::render_app_to_stream;

use crate::app::shell;
use crate::wall;

use super::{board, logging};

pub fn router(options: LeptosOptions, wall: Arc<wall::State>) -> Router {
    // One page, so one URL. Built from `SITE_URL` rather than written out
    // again, and leaked because it never changes.
    let sitemap: &'static str = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>{}</loc></url></urlset>"#,
        crate::SITE_URL
    )
    .leak();

    Router::new()
        .route(
            "/sitemap.xml",
            get(move || async move {
                (
                    [
                        (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
                        (header::CACHE_CONTROL, "public, max-age=3600"),
                    ],
                    sitemap,
                )
            }),
        )
        .route("/api/wall", board::route(wall))
        .route(
            "/health",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    format!(
                        r#"{{"status":"ok","uptime_seconds":{},"build":"{}"}}"#,
                        crate::clock::uptime_secs(),
                        crate::BUILD
                    ),
                )
            }),
        )
        .route(
            "/",
            get(render_app_to_stream({
                let options = options.clone();
                move || shell(options.clone())
            })),
        )
        // Everything under the site root, and a 404 for anything else. The
        // pages the old site had are gone, so they land there.
        .fallback(elsewhere)
        .layer(from_fn(logging::log_request))
        .with_state(options)
}

/// Anything that is not one of the routes above.
///
/// A file under the site root is served as itself; everything else is the page
/// again, with a 404 status and the path that was asked for, which the view
/// names under the banner before putting the url itself right.
///
/// The handler is built per request because the path is what it carries, and
/// leptos hands the render its context through a closure that takes none.
async fn elsewhere(uri: Uri, state: State<LeptosOptions>, request: Request) -> Response {
    let asked = crate::missing_path(uri.path());
    leptos_axum::file_and_error_handler_with_context(
        move || leptos::prelude::provide_context(crate::Missing(asked.clone())),
        shell,
    )(uri, state, request)
    .await
}
