// See lib.rs: deeply nested view-tree future types overflow the default depth in release.
#![recursion_limit = "256"]

#[cfg(feature = "ssr")]
const BASE_URL: &str = "https://takashialpha.com";

#[cfg(feature = "ssr")]
fn build_sitemap(routes: &[leptos_axum::AxumRouteListing]) -> String {
    // Every route is a static, ASCII path segment, so the `<loc>` values need no
    // XML escaping or percent-encoding. Revisit if a route ever carries `&`,
    // non-ASCII, or other reserved characters.
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#,
    );
    for route in routes {
        let path = route.path().trim_matches('/');
        // Skip Leptos wildcard (`*`) and dynamic param (`:`) segments.
        if path.contains('*') || path.contains(':') {
            continue;
        }
        xml.push_str("<url><loc>");
        xml.push_str(BASE_URL);
        if !path.is_empty() {
            xml.push('/');
            xml.push_str(path);
        }
        xml.push_str("</loc></url>");
    }
    xml.push_str("</urlset>");
    xml
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use axum::routing::get;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use webpages::app::*;

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);
    let sitemap = build_sitemap(&routes);

    let app = Router::new()
        .route(
            "/sitemap.xml",
            get(move || {
                let body = sitemap.clone();
                async move { ([("content-type", "application/xml")], body) }
            }),
        )
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
