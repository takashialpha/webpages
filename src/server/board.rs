//! `GET` and `POST /api/wall`: the one route that is not a page.

use std::sync::Arc;

use axum::http::{HeaderMap, HeaderName, StatusCode, header};
use axum::response::{IntoResponse as _, Response};
use axum::routing::{MethodRouter, get};

use crate::wall;

/// The board changes constantly and is read by a command, not a cache, so
/// nothing along the way may hold on to it.
const HEADERS: [(HeaderName, &str); 2] = [
    (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
    (header::CACHE_CONTROL, "no-store"),
];

/// Reading it and writing to it, both answering with the board as it now
/// stands, so a writer sees the result of what they did.
pub fn route<S: Clone + Send + Sync + 'static>(wall: Arc<wall::State>) -> MethodRouter<S> {
    get({
        let wall = Arc::clone(&wall);
        move || {
            let wall = Arc::clone(&wall);
            async move { (HEADERS, wall.render()) }
        }
    })
    .post({
        move |headers: HeaderMap, body: String| {
            let wall = Arc::clone(&wall);
            async move { write(&wall, &headers, &body).await }
        }
    })
}

/// Whether a write came from somewhere it should have.
///
/// A `text/plain` POST is a CORS simple request, so any page anywhere can send
/// one. Never seeing the answer is no obstacle to somebody who only wants to
/// draw on the board, and it would be the visitor's address that got charged
/// for it. `Sec-Fetch-Site` is set by the browser and a page cannot forge it,
/// so cross-site means a page that is not ours.
///
/// Absent means it did not come from a browser at all, which is `curl` acting
/// for whoever ran it. That is allowed: a shell is a fine way to write to a
/// wall, and it is the visitor's own address either way.
fn same_site(headers: &HeaderMap) -> bool {
    headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_none_or(|site| site == "same-origin" || site == "none")
}

/// Who is writing, for the rate limit.
///
/// Cloudflare sets `CF-Connecting-IP` and strips whatever the client sent, so
/// this is trustworthy behind the proxy and nowhere else. Anything reaching the
/// origin directly could claim any address it liked, which is why the origin is
/// not meant to be reachable directly.
fn writer(headers: &HeaderMap) -> &str {
    headers
        .get("cf-connecting-ip")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("direct")
}

/// One write: check it, charge it, apply it, answer with the board.
async fn write(wall: &wall::State, headers: &HeaderMap, body: &str) -> Response {
    if !same_site(headers) {
        return (
            StatusCode::FORBIDDEN,
            HEADERS,
            "the board takes writes from its own page, or from a shell\n",
        )
            .into_response();
    }

    let Some((x, y, byte)) = wall::parse_write(body) else {
        return (
            StatusCode::BAD_REQUEST,
            HEADERS,
            format!(
                "expected `x y char`, with x under {}, y under {}, and one printable \
                 character, or nothing to clear the cell\n",
                wall::COLS,
                wall::ROWS
            ),
        )
            .into_response();
    };

    if !wall.allowed(writer(headers)) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            HEADERS,
            // Which of the two limits was hit is not worth working out to say:
            // either way the answer is to write less.
            format!(
                "slow down: {} cells a minute each, {} across everyone\n",
                wall::BUDGET,
                wall::CEILING,
            ),
        )
            .into_response();
    }

    // The write still stands in memory if the disk refused it, so the board is
    // right until a restart. Saying so beats failing the request.
    if let Err(error) = wall.write(x, y, byte).await {
        tracing::error!(path = %wall.path().display(), %error, "could not persist the board");
    }

    (HEADERS, wall.render()).into_response()
}
