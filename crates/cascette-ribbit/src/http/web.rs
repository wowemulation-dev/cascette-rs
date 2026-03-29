//! Web UI handlers for the Ribbit server dashboard.
//!
//! Serves HTML pages styled after the growingSWE "blueprint" design system:
//! monospace typography, blue accent, corner marks, dotted grid background,
//! dashed separators, and card-based layouts.

use crate::config::CdnConfig;
use crate::server::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use std::fmt::Write;
use std::sync::Arc;

/// Shared CSS embedded in every page.
const STYLE: &str = r#"
:root {
  --bp-blue: #3b82f6;
  --bp-blue-light: #93c5fd;
  --bp-blue-bg: rgba(59,130,246,0.08);
  --bp-blue-bg-soft: rgba(59,130,246,0.03);
  --bp-text: #334155;
  --bp-text-label: #475569;
  --bp-muted: #94a3b8;
  --bp-border: #cbd5e1;
  --bp-border-strong: #94a3b8;
  --background: #fff;
  --foreground: #1a1a1a;
}

*, *::before, *::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html {
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

body {
  font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
  font-size: 16px;
  line-height: 1.6;
  color: var(--foreground);
  background: var(--background);
}

.bp-grid-bg {
  background-image: radial-gradient(circle, rgba(59,130,246,0.12) 1px, transparent 1px);
  background-size: 24px 24px;
  min-height: 100vh;
  position: relative;
}

/* Corner marks */
.bp-corner-marks {
  position: absolute;
  inset: 0;
  pointer-events: none;
}
.bp-corner-mark {
  border-color: var(--bp-muted);
  pointer-events: none;
  opacity: 0.5;
  width: 12px;
  height: 12px;
  position: absolute;
}
.bp-corner-mark--tl { border-top: 1px solid; border-left: 1px solid; top: 0; left: 0; }
.bp-corner-mark--tr { border-top: 1px solid; border-right: 1px solid; top: 0; right: 0; }
.bp-corner-mark--bl { border-bottom: 1px solid; border-left: 1px solid; bottom: 0; left: 0; }
.bp-corner-mark--br { border-bottom: 1px solid; border-right: 1px solid; bottom: 0; right: 0; }

.container {
  max-width: 1000px;
  margin: 0 auto;
  padding: 0 24px 64px;
  position: relative;
}

/* Header */
header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 32px 0;
}

.logo {
  font-size: 1.1rem;
  font-weight: 700;
  color: var(--bp-text);
  text-decoration: none;
  letter-spacing: -0.02em;
}
.logo-accent {
  color: var(--bp-blue);
}

.nav-link {
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--bp-muted);
  font-size: 0.72rem;
  font-weight: 500;
  text-decoration: none;
  transition: color 0.2s;
}
.nav-link:hover {
  color: var(--bp-blue);
}

/* Page titles */
h1 {
  text-transform: uppercase;
  letter-spacing: 0.02em;
  color: var(--bp-text);
  font-size: clamp(1.4rem, 3vw, 2rem);
  font-weight: 700;
  line-height: 1.25;
  margin-bottom: 8px;
}

.tagline {
  color: var(--bp-muted);
  font-size: 0.88rem;
  line-height: 1.6;
  margin-bottom: 4px;
}

.subtitle-link {
  color: var(--bp-muted);
  font-size: 0.82rem;
  text-decoration: none;
  transition: color 0.2s;
}
.subtitle-link:hover {
  color: var(--bp-blue);
}

/* Homepage hero section */
.hero {
  padding: 64px 0 0;
}
.hero h1 {
  font-size: 2rem;
  margin-bottom: 16px;
}
.hero-body {
  color: var(--bp-muted);
  font-size: 0.88rem;
  line-height: 1.75;
  max-width: 600px;
  margin-bottom: 24px;
}

.cta-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--bp-blue);
  font-size: 1rem;
  font-weight: 600;
  text-decoration: none;
  transition: gap 0.2s;
}
.cta-link:hover {
  gap: 10px;
}

/* Section dividers */
.section-divider {
  border: none;
  border-top: 1px dashed var(--bp-border);
  margin: 24px 0;
}

/* Stat cards */
.stat-cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin: 24px 0;
}
@media (max-width: 640px) {
  .stat-cards {
    grid-template-columns: 1fr;
    gap: 6px;
  }
}
.stat-card {
  border: 1px solid var(--bp-border);
  background: #fff;
  border-radius: 2px;
  padding: 16px 20px;
  transition: 0.25s;
}
.stat-card:hover {
  border-color: var(--bp-blue);
  box-shadow: 0 0 0 1px var(--bp-blue);
  transform: translateY(-2px);
}
.stat-label {
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--bp-text-label);
  font-size: 0.7rem;
  font-weight: 600;
  margin-bottom: 4px;
}
.stat-value {
  color: var(--bp-text);
  font-size: 1.25rem;
  font-weight: 700;
}

/* Product list (homepage) */
.product-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 8px;
  margin-top: 16px;
}
.product-card {
  border: 1px solid var(--bp-border);
  background: #fff;
  border-radius: 2px;
  padding: 20px;
  text-decoration: none;
  color: inherit;
  transition: 0.25s;
  display: block;
}
.product-card:hover {
  border-color: var(--bp-blue);
  box-shadow: 0 0 0 1px var(--bp-blue);
  transform: translateY(-2px);
}
.product-name {
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--bp-text);
  margin-bottom: 6px;
}
.product-meta {
  font-size: 0.75rem;
  color: var(--bp-muted);
  line-height: 1.6;
}

/* Info box */
.info-box {
  border: 1px dashed var(--bp-border-strong);
  background: var(--bp-blue-bg-soft);
  border-radius: 2px;
  padding: 12px 16px;
  font-size: 0.82rem;
  color: var(--bp-text);
  line-height: 1.6;
}

/* Tag filter buttons */
.tag-filters {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 16px 0;
}
.tag-btn {
  text-transform: uppercase;
  letter-spacing: 0.02em;
  font-family: inherit;
  font-size: 0.75rem;
  color: var(--bp-muted);
  background: transparent;
  border: 1px solid var(--bp-muted);
  border-radius: 3px;
  padding: 4px 10px;
  cursor: pointer;
  text-decoration: none;
  transition: 0.15s;
}
.tag-btn:hover, .tag-btn.active {
  color: var(--bp-blue);
  border-color: var(--bp-blue);
  background: var(--bp-blue-bg);
}

/* Build list */
.build-list {
  display: flex;
  flex-direction: column;
}
.build-entry {
  display: block;
  text-decoration: none;
  color: inherit;
  padding: 20px 0;
  border-top: 1px dashed var(--bp-border);
  transition: background 0.15s;
}
.build-entry:last-child {
  border-bottom: 1px dashed var(--bp-border);
}
.build-entry:hover {
  background: var(--bp-blue-bg-soft);
}
.build-time {
  font-size: 0.68rem;
  color: var(--bp-muted);
  margin-bottom: 4px;
}
.build-version {
  font-size: 1.1rem;
  font-weight: 800;
  color: var(--foreground);
  margin-bottom: 4px;
}
.build-hashes {
  font-size: 0.78rem;
  color: var(--bp-muted);
  line-height: 1.7;
}
.build-hashes code {
  background: var(--bp-blue-bg);
  color: var(--bp-blue);
  border: 1px solid rgba(59,130,246,0.15);
  border-radius: 2px;
  padding: 1px 5px;
  font-family: inherit;
  font-size: 0.9em;
}
.build-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 8px;
}
.build-tag {
  font-size: 0.68rem;
  text-transform: uppercase;
  letter-spacing: 0.02em;
  color: var(--bp-muted);
  border: 1px solid var(--bp-border);
  border-radius: 3px;
  padding: 2px 8px;
}

/* Footer area */
.footer-info {
  margin-top: 32px;
  padding-top: 16px;
  border-top: 1px dashed var(--bp-border);
  font-size: 0.72rem;
  color: var(--bp-muted);
}

/* Dark mode */
@media (prefers-color-scheme: dark) {
  :root {
    --bp-blue: #60a5fa;
    --bp-blue-light: #93c5fd;
    --bp-blue-bg: rgba(96,165,250,0.12);
    --bp-blue-bg-soft: rgba(96,165,250,0.06);
    --bp-text: #cbd5e1;
    --bp-text-label: #94a3b8;
    --bp-muted: #64748b;
    --bp-border: #334155;
    --bp-border-strong: #475569;
    --background: #0f172a;
    --foreground: #e2e8f0;
  }

  .bp-grid-bg {
    background-image: radial-gradient(circle, rgba(96,165,250,0.08) 1px, transparent 1px);
  }

  .stat-card,
  .product-card {
    background: #1e293b;
  }

  .info-box {
    background: rgba(96,165,250,0.04);
  }

  .build-hashes code {
    background: rgba(96,165,250,0.15);
    border-color: rgba(96,165,250,0.25);
  }
}
"#;

/// Render the shared HTML head, opening body, grid background, and corner marks.
fn page_open(title: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>{STYLE}</style>
</head>
<body>
<div class="bp-grid-bg">
<div class="bp-corner-marks">
  <div class="bp-corner-mark bp-corner-mark--tl"></div>
  <div class="bp-corner-mark bp-corner-mark--tr"></div>
  <div class="bp-corner-mark bp-corner-mark--bl"></div>
  <div class="bp-corner-mark bp-corner-mark--br"></div>
</div>
<div class="container">
"#
    )
}

/// Closing tags.
const PAGE_CLOSE: &str = "</div>\n</div>\n</body>\n</html>";

/// Format an uptime in seconds to a human-readable string.
fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}

/// Escape HTML special characters.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Handle GET / -- server status homepage.
pub async fn handle_index(State(state): State<Arc<AppState>>) -> Html<String> {
    let db = state.database().await;
    let mut products: Vec<&str> = db.products();
    products.sort_unstable();

    let uptime = format_uptime(state.uptime_seconds());
    let total_builds = db.total_builds();
    let product_count = products.len();

    let mut html = page_open("cascette-ribbit");

    // Header
    html.push_str(
        r#"<header>
  <a href="/" class="logo">cascette<span class="logo-accent">&lt;ribbit/&gt;</span></a>
</header>
"#,
    );

    // Hero
    html.push_str(
        r##"<div class="hero">
  <h1>Ribbit Protocol Server.</h1>
  <p class="hero-body">
    A replacement Ribbit/TACT version service.
    Serves build metadata over HTTP and TCP (v1/v2) protocols.
  </p>
  <a href="#products" class="cta-link">Browse products &#x2192;</a>
</div>
"##,
    );

    html.push_str(r#"<hr class="section-divider">"#);

    // Stats
    html.push_str(r#"<div class="stat-cards">"#);
    let _ = write!(
        html,
        r#"<div class="stat-card"><div class="stat-label">Products</div><div class="stat-value">{product_count}</div></div>"#
    );
    let _ = write!(
        html,
        r#"<div class="stat-card"><div class="stat-label">Total Builds</div><div class="stat-value">{total_builds}</div></div>"#
    );
    let _ = write!(
        html,
        r#"<div class="stat-card"><div class="stat-label">Uptime</div><div class="stat-value">{uptime}</div></div>"#
    );
    html.push_str("</div>");

    html.push_str(r#"<hr class="section-divider">"#);

    // Endpoints info box
    html.push_str(r#"<div class="info-box">
  <strong>HTTP v1</strong> &mdash; raw BPSV<br>
  GET /&lt;product&gt;/versions &mdash; version info (latest)<br>
  GET /&lt;product&gt;/cdns &mdash; CDN configuration (latest)<br>
  GET /&lt;product&gt;/bgdl &mdash; background download info (latest)<br>
  GET /&lt;product&gt;/versions/&lt;build&gt; &mdash; version info for specific build<br>
  GET /&lt;product&gt;/cdns/&lt;build&gt; &mdash; CDN config for specific build<br>
  GET /&lt;product&gt;/bgdl/&lt;build&gt; &mdash; BGDL for specific build<br>
  <br>
  <strong>HTTPS v2</strong> &mdash; raw BPSV over TLS<br>
  GET /v2/products/&lt;product&gt;/versions &mdash; version info (latest)<br>
  GET /v2/products/&lt;product&gt;/cdns &mdash; CDN configuration (latest)<br>
  GET /v2/products/&lt;product&gt;/bgdl &mdash; background download info (latest)<br>
  GET /v2/products/&lt;product&gt;/versions/&lt;build&gt; &mdash; version info for specific build<br>
  GET /v2/products/&lt;product&gt;/cdns/&lt;build&gt; &mdash; CDN config for specific build<br>
  GET /v2/products/&lt;product&gt;/bgdl/&lt;build&gt; &mdash; BGDL for specific build<br>
  GET /v2/products/summary &mdash; product summary<br>
  <br>
  <strong>TCP v1</strong> &mdash; MIME-wrapped with SHA-256 checksums (:1119)<br>
  <strong>TCP v2</strong> &mdash; raw BPSV (:1119)
</div>"#);

    html.push_str(r#"<hr class="section-divider">"#);

    // Products section
    html.push_str(r#"<div id="products">"#);
    html.push_str(r#"<div class="stat-label" style="margin-bottom:12px">Products</div>"#);
    html.push_str(r#"<div class="product-grid">"#);

    for product in &products {
        let latest = db.latest_build(product);
        let build_count = db.builds_for_product(product).len();
        let version_str = latest.map(|b| escape_html(&b.version)).unwrap_or_default();
        let build_time = latest
            .map(|b| escape_html(&b.build_time))
            .unwrap_or_default();
        let seqn = state.current_seqn(product);

        let _ = write!(
            html,
            r#"<a href="/{product}/builds" class="product-card">
  <div class="product-name">{product}</div>
  <div class="product-meta">
    Latest: {version_str}<br>
    Builds: {build_count} &middot; Seqn: {seqn}<br>
    {build_time}
  </div>
</a>"#
        );
    }

    html.push_str("</div></div>"); // product-grid, #products

    // Footer
    html.push_str(
        r#"<div class="footer-info">cascette-ribbit &middot; Ribbit/TACT protocol server</div>"#,
    );

    html.push_str(PAGE_CLOSE);
    Html(html)
}

/// Handle GET /:product/builds -- build list for a product.
pub async fn handle_builds(
    Path(product): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, StatusCode> {
    let db = state.database().await;
    let builds = db.builds_for_product(&product);

    if builds.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let cdn_config = db
        .latest_build(&product)
        .map(|b| CdnConfig::resolve_for_build(b, state.cdn_config()));
    let seqn = state.current_seqn(&product);
    let product_escaped = escape_html(&product);

    let mut html = page_open(&format!("{product_escaped} builds | cascette-ribbit"));

    // Header
    let _ = write!(
        html,
        r#"<header>
  <a href="/" class="logo">cascette<span class="logo-accent">&lt;ribbit/&gt;</span></a>
  <a href="/" class="nav-link">Home</a>
</header>
"#
    );

    // Title section
    let _ = write!(
        html,
        r#"<h1>{product_escaped}</h1>
<p class="tagline">{} builds tracked &middot; seqn {seqn}</p>
"#,
        builds.len()
    );

    // CDN info
    if let Some(ref cdn) = cdn_config {
        let _ = write!(
            html,
            r#"<div class="info-box" style="margin:16px 0">
  <strong>CDN</strong>: {} &middot; Path: {} &middot; Config: {}
</div>"#,
            escape_html(&cdn.hosts),
            escape_html(&cdn.path),
            escape_html(&cdn.config_path),
        );
    }

    // BPSV endpoint links
    let _ = write!(
        html,
        r#"<div class="tag-filters">
  <a href="/{product}/versions" class="tag-btn">v1 versions</a>
  <a href="/{product}/cdns" class="tag-btn">v1 cdns</a>
  <a href="/{product}/bgdl" class="tag-btn">v1 bgdl</a>
  <a href="/v2/products/{product}/versions" class="tag-btn">v2 versions</a>
  <a href="/v2/products/{product}/cdns" class="tag-btn">v2 cdns</a>
  <a href="/v2/products/{product}/bgdl" class="tag-btn">v2 bgdl</a>
</div>"#
    );

    html.push_str(r#"<hr class="section-divider">"#);

    // Build list
    html.push_str(r#"<div class="build-list">"#);

    for build in builds {
        let version = escape_html(&build.version);
        let build_time = escape_html(&build.build_time);
        let build_config = escape_html(&build.build_config);
        let cdn_config_hash = escape_html(&build.cdn_config);

        let _ = write!(
            html,
            r#"<div class="build-entry">
  <div class="build-time">{build_time}</div>
  <div class="build-version">{version}</div>
  <div class="build-hashes">
    BuildConfig: <code>{build_config}</code><br>
    CDNConfig: <code>{cdn_config_hash}</code>"#
        );

        if let Some(ref pc) = build.product_config {
            let _ = write!(html, "<br>ProductConfig: <code>{}</code>", escape_html(pc));
        }
        if let Some(ref ek) = build.encoding_ekey {
            let _ = write!(html, "<br>Encoding: <code>{}</code>", escape_html(ek));
        }
        if let Some(ref rk) = build.root_ekey {
            let _ = write!(html, "<br>Root: <code>{}</code>", escape_html(rk));
        }

        html.push_str("</div>"); // build-hashes

        // Tags showing build number and id
        let _ = write!(
            html,
            r#"<div class="build-tags">
    <span class="build-tag">build {}</span>
    <span class="build-tag">id {}</span>
  </div>"#,
            escape_html(&build.build),
            build.id
        );

        html.push_str("</div>"); // build-entry
    }

    html.push_str("</div>"); // build-list

    // Footer
    let _ = write!(
        html,
        r#"<div class="footer-info">cascette-ribbit &middot; {product_escaped} &middot; {} builds</div>"#,
        builds.len()
    );

    html.push_str(PAGE_CLOSE);
    Ok(Html(html).into_response())
}
