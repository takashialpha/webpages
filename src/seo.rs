//! Head metadata for the one page this site has.
//!
//! The structured data is built here rather than sitting in a JSON file so that
//! [`SITE_URL`] is the only place the domain is written. The old site spelled it
//! out twelve times across four schema files, which silently went stale.

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};

use crate::SITE_URL;
use crate::fs;

/// Title, description, canonical, Open Graph, and the schema.org graph.
#[component]
pub fn Seo() -> impl IntoView {
    let image = format!("{SITE_URL}/og.png");
    let description = fs::intro();

    // One graph: the person the site is about, and the site itself.
    let json_ld = format!(
        r#"{{"@context":"https://schema.org","@graph":[
{{"@type":"Person","name":"takashialpha","url":"{SITE_URL}","jobTitle":"Systems Developer",
"description":"{description}",
"knowsAbout":["Rust","Linux","Systems programming","Terminal applications"],
"sameAs":["https://github.com/takashialpha","https://x.com/takashialphax","https://www.reddit.com/user/takashialpha"]}},
{{"@type":"WebSite","name":"takashialpha","url":"{SITE_URL}","inLanguage":"en"}}]}}"#
    );

    view! {
        <Title text="takashialpha"/>
        // Same single source as the login banner, so the description can never
        // drift from what the page actually says.
        <Meta name="description" content=description/>
        <Link rel="canonical" href=SITE_URL/>

        <Meta property="og:type" content="profile"/>
        <Meta property="og:site_name" content="takashialpha"/>
        <Meta property="og:locale" content="en_US"/>
        <Meta property="og:title" content="takashialpha"/>
        <Meta property="og:description" content=description/>
        <Meta property="og:url" content=SITE_URL/>
        <Meta property="og:image" content=image.clone()/>
        <Meta property="og:image:width" content="1200"/>
        <Meta property="og:image:height" content="630"/>
        <Meta property="og:image:type" content="image/png"/>
        <Meta property="og:image:alt" content="takashialpha terminal"/>

        <Meta name="twitter:card" content="summary_large_image"/>
        <Meta name="twitter:title" content="takashialpha"/>
        <Meta name="twitter:description" content=description/>
        <Meta name="twitter:image" content=image/>

        // `inner_html` keeps the JSON raw; script content is not HTML-decoded.
        <script type="application/ld+json" inner_html=json_ld></script>
    }
}
