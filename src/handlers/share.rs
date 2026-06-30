use crate::app_state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// HTML Escaping Helper
// ─────────────────────────────────────────────────────────────────────────────

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler: GET /share/property/:id
// ─────────────────────────────────────────────────────────────────────────────

pub async fn share_property(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // 1. Validate ID
    let parsed_id = match Uuid::parse_str(&id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                "Invalid property id".to_string(),
            )
                .into_response();
        }
    };

    // 2. Query the database
    let query = r#"
SELECT
    p.title,
    p.city,
    p.price,
    p.deposit,
    p.listing_type,
    p.bhk,
    p.bathrooms,
    p.images
FROM properties p
WHERE p.id = $1
  AND p.status != 'deleted'
LIMIT 1
"#;

    let mut row = sqlx::query(query)
        .bind(parsed_id)
        .fetch_optional(&state.db)
        .await;

    // Fallback to listings_v2 if not found in properties
    if let Ok(None) = row {
        let query_v2 = r#"
SELECT
    l.title,
    l.city,
    l.price,
    l.deposit,
    l.listing_type,
    l.bedrooms AS bhk,
    l.bathrooms,
    (
        SELECT COALESCE(jsonb_agg(image_url), '[]'::jsonb)
        FROM listing_images_v2
        WHERE listing_id = l.id
    ) AS images
FROM listings_v2 l
WHERE l.id = $1
LIMIT 1
"#;
        row = sqlx::query(query_v2)
            .bind(parsed_id)
            .fetch_optional(&state.db)
            .await;
    }
    else if row.is_err() {
        let query_v2 = r#"
SELECT
    l.title,
    l.city,
    l.price,
    l.deposit,
    l.listing_type,
    l.bedrooms AS bhk,
    l.bathrooms,
    (
        SELECT COALESCE(jsonb_agg(image_url), '[]'::jsonb)
        FROM listing_images_v2
        WHERE listing_id = l.id
    ) AS images
FROM listings_v2 l
WHERE l.id = $1
LIMIT 1
"#;
        row = sqlx::query(query_v2)
            .bind(parsed_id)
            .fetch_optional(&state.db)
            .await;
    }

    // Fallback values if DB query fails or property is not found
    let mut og_title = "View Property on Livana Eco".to_string();
    let mut og_desc = "Check out this property on the Livana Eco app".to_string();
    let mut og_image = "https://livanaeco.com/default-share-image.png".to_string();

    if let Ok(Some(db_row)) = row {
        use sqlx::Row;

        let title: Option<String> = db_row.try_get("title").unwrap_or(None);
        let city: Option<String> = db_row.try_get("city").unwrap_or(None);
        let price: Option<i64> = db_row.try_get("price").unwrap_or(None);

        if let Some(t) = title {
            if !t.trim().is_empty() {
                og_title = t;
            }
        }

        let price_val = price.unwrap_or(0);
        if let Some(c) = city {
            if !c.trim().is_empty() {
                og_desc = format!("₹{} — {}", price_val, c);
            } else {
                og_desc = format!("₹{}", price_val);
            }
        } else {
            og_desc = format!("₹{}", price_val);
        }

        // 3. Extract first image
        if let Ok(images) = db_row.try_get::<serde_json::Value, _>("images") {
            if let Some(arr) = images.as_array() {
                for img in arr {
                    if let Some(s) = img.as_str() {
                        if s.starts_with("https://") {
                            og_image = s.to_string();
                            break;
                        }
                    }
                }
            }
        }
    }

    // 4. Return HTML response
    let html = render_share_html(&og_title, &og_desc, &og_image, &id, "property", "property");
    (StatusCode::OK, Html(html)).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler: GET /share/news/:id
// ─────────────────────────────────────────────────────────────────────────────
pub async fn share_news(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let parsed_id = match Uuid::parse_str(&id) {
        Ok(uuid) => uuid,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid news id".to_string()).into_response(),
    };

    let query = r#"
SELECT headline, short_summary, thumbnail_url, images
FROM news_items
WHERE id = $1
LIMIT 1
"#;

    let mut og_title = "Livana Eco News".to_string();
    let mut og_desc = "Check out the latest news on Livana Eco".to_string();
    let mut og_image = "https://livanaeco.com/default-share-image.png".to_string();

    if let Ok(Some(row)) = sqlx::query(query).bind(parsed_id).fetch_optional(&state.db).await {
        use sqlx::Row;
        if let Ok(title) = row.try_get::<String, _>("headline") {
            if !title.trim().is_empty() { og_title = title; }
        }
        if let Ok(desc) = row.try_get::<String, _>("short_summary") {
            if !desc.trim().is_empty() { og_desc = desc; }
        }
        if let Ok(Some(thumb)) = row.try_get::<Option<String>, _>("thumbnail_url") {
            if thumb.starts_with("https://") { og_image = thumb; }
        } else if let Ok(images) = row.try_get::<serde_json::Value, _>("images") {
            if let Some(arr) = images.as_array() {
                for img in arr {
                    if let Some(s) = img.as_str() {
                        if s.starts_with("https://") { og_image = s.to_string(); break; }
                    }
                }
            }
        }
    }

    let html = render_share_html(&og_title, &og_desc, &og_image, &id, "news", "news article");
    (StatusCode::OK, Html(html)).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler: GET /share/expo/:id
// ─────────────────────────────────────────────────────────────────────────────
pub async fn share_expo(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let parsed_id = match Uuid::parse_str(&id) {
        Ok(uuid) => uuid,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid expo id".to_string()).into_response(),
    };

    let query = "SELECT title, description, images FROM expo_events WHERE id = $1 LIMIT 1";

    let mut og_title = "Livana Eco Expo".to_string();
    let mut og_desc = "Check out this expo event on Livana Eco".to_string();
    let mut og_image = "https://livanaeco.com/default-share-image.png".to_string();

    if let Ok(Some(row)) = sqlx::query(query).bind(parsed_id).fetch_optional(&state.db).await {
        use sqlx::Row;
        if let Ok(title) = row.try_get::<String, _>("title") {
            if !title.trim().is_empty() { og_title = title; }
        }
        if let Ok(desc) = row.try_get::<Option<String>, _>("description") {
            if let Some(d) = desc { if !d.trim().is_empty() { og_desc = d; } }
        }
        if let Ok(images) = row.try_get::<serde_json::Value, _>("images") {
            if let Some(arr) = images.as_array() {
                for img in arr {
                    if let Some(s) = img.as_str() {
                        if s.starts_with("https://") { og_image = s.to_string(); break; }
                    }
                }
            }
        }
    }

    let html = render_share_html(&og_title, &og_desc, &og_image, &id, "expo", "expo event");
    (StatusCode::OK, Html(html)).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler: GET /share/carecrew/:id
// ─────────────────────────────────────────────────────────────────────────────
pub async fn share_carecrew(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let parsed_id = match Uuid::parse_str(&id) {
        Ok(uuid) => uuid,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid carecrew id".to_string()).into_response(),
    };

    let query = "SELECT name, service_type, images FROM carecrew WHERE id = $1 LIMIT 1";

    let mut og_title = "Livana CareCrew".to_string();
    let mut og_desc = "Check out this CareCrew service on Livana Eco".to_string();
    let mut og_image = "https://livanaeco.com/default-share-image.png".to_string();

    if let Ok(Some(row)) = sqlx::query(query).bind(parsed_id).fetch_optional(&state.db).await {
        use sqlx::Row;
        if let Ok(name) = row.try_get::<String, _>("name") {
            if !name.trim().is_empty() { og_title = name; }
        }
        if let Ok(service_type) = row.try_get::<String, _>("service_type") {
            if !service_type.trim().is_empty() { og_desc = service_type; }
        }
        if let Ok(images) = row.try_get::<serde_json::Value, _>("images") {
            if let Some(arr) = images.as_array() {
                for img in arr {
                    if let Some(s) = img.as_str() {
                        if s.starts_with("https://") { og_image = s.to_string(); break; }
                    }
                }
            }
        }
    }

    let html = render_share_html(&og_title, &og_desc, &og_image, &id, "carecrew", "CareCrew provider");
    (StatusCode::OK, Html(html)).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared HTML Renderer
// ─────────────────────────────────────────────────────────────────────────────
fn render_share_html(
    og_title: &str,
    og_desc: &str,
    og_image: &str,
    id: &str,
    scheme: &str,
    entity_name: &str,
) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>{escaped_title} | Livana Eco</title>

<!-- OG Tags -->
<meta property="og:title"       content="{escaped_title}" />
<meta property="og:description" content="{escaped_desc}" />
<meta property="og:image"       content="{escaped_image}" />
<meta property="og:type"        content="website" />

<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: linear-gradient(135deg, #0f0c29, #302b63, #24243e);
    color: #fff;
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 24px;
  }}
  .card {{
    background: rgba(255,255,255,0.08);
    backdrop-filter: blur(16px);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 20px;
    padding: 40px 32px;
    max-width: 380px;
    width: 100%;
  }}
  .spinner {{
    width: 48px;
    height: 48px;
    border: 4px solid rgba(255, 255, 255, 0.2);
    border-top-color: #fff;
    border-radius: 50%;
    animation: spin 1s linear infinite;
    margin: 0 auto 24px;
  }}
  @keyframes spin {{ 100% {{ transform: rotate(360deg); }} }}
  h1 {{ font-size: 22px; font-weight: 700; margin-bottom: 8px; }}
  p {{ font-size: 15px; color: rgba(255,255,255,0.7); line-height: 1.5; }}
</style>
</head>
<body>

<div class="card">
  <div class="spinner" id="spinner"></div>
  <h1 id="main-text">Opening Livana Eco…</h1>
  <p id="sub-text">Taking you to the {entity_name}</p>
</div>

<script>
  const ANDROID_PACKAGE = 'com.LiveInBuddy.livein';
  const PLAY_STORE_URL = 'https://play.google.com/store/apps/details?id=' + ANDROID_PACKAGE;
  const APP_STORE_URL = 'https://apps.apple.com/in/app/livana-eco/id6742744565';
  const ENTITY_ID = '{id}';
  const DEEP_LINK = 'livanaeco://{scheme}/' + ENTITY_ID;

  const ua = navigator.userAgent.toLowerCase();
  const isAndroid = ua.indexOf('android') > -1;
  const isIOS = /ipad|iphone|ipod/.test(ua);

  window.onload = function() {{
    if (isAndroid) {{
        const fallback = encodeURIComponent(PLAY_STORE_URL);
        const intentUri = 'intent://{scheme}/' + ENTITY_ID + '#Intent;scheme=livanaeco;package=' + ANDROID_PACKAGE + ';S.browser_fallback_url=' + fallback + ';end';
        window.location.href = intentUri;
    }} else if (isIOS) {{
        window.location.href = DEEP_LINK;
        let timeoutCleared = false;

        const clearFallback = function() {{
            timeoutCleared = true;
        }};

        document.addEventListener('visibilitychange', function() {{
            if (document.hidden) clearFallback();
        }});
        window.addEventListener('pagehide', clearFallback);
        window.addEventListener('blur', clearFallback);

        setTimeout(function() {{
            if (!timeoutCleared && !document.hidden) {{
                document.getElementById('sub-text').innerText = 'App not found — redirecting to store…';
                setTimeout(function() {{
                    window.location.href = APP_STORE_URL;
                }}, 500);
            }}
        }}, 1500);
    }} else {{
        // Desktop or other
        document.getElementById('spinner').style.display = 'none';
        document.getElementById('main-text').innerText = 'Livana Eco';
        document.getElementById('sub-text').innerText = 'Open this link on your mobile device to view the {entity_name} in the app.';
    }}
  }};
</script>

</body>
</html>"#,
        escaped_title = escape_html(og_title),
        escaped_desc = escape_html(og_desc),
        escaped_image = escape_html(og_image),
        id = escape_html(id),
        scheme = scheme,
        entity_name = entity_name
    )
}

