use axum::{
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const PLAY_STORE_URL: &str =
    "https://play.google.com/store/apps/details?id=com.LiveInBuddy.livein";
const APP_STORE_URL: &str =
    "https://apps.apple.com/in/app/livana-eco/id6742744565";
const CUSTOM_SCHEME: &str = "livanaeco";
const ANDROID_PACKAGE: &str = "com.LiveInBuddy.livein";

// ─────────────────────────────────────────────────────────────────────────────
// ID Sanitisation
// ─────────────────────────────────────────────────────────────────────────────

/// Only allow alphanumeric, hyphens, and underscores.
fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler: GET /share/property/:id
// ─────────────────────────────────────────────────────────────────────────────

pub async fn share_property(
    Path(id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // 1. Sanitise
    if !is_valid_id(&id) {
        return (
            StatusCode::BAD_REQUEST,
            Html("Invalid property id".to_string()),
        )
            .into_response();
    }

    // 2. Detect platform from User-Agent
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ua.contains("android") {
        android_redirect(&id).into_response()
    } else if ua.contains("iphone") || ua.contains("ipad") {
        ios_page(&id).into_response()
    } else {
        desktop_page().into_response()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Android — 302 redirect to intent URI
// ─────────────────────────────────────────────────────────────────────────────

fn android_redirect(id: &str) -> impl IntoResponse {
    let fallback = urlencoding::encode(PLAY_STORE_URL);
    let intent_uri = format!(
        "intent://property/{id}#Intent;scheme={scheme};package={package};S.browser_fallback_url={fallback};end",
        id = id,
        scheme = CUSTOM_SCHEME,
        package = ANDROID_PACKAGE,
        fallback = fallback,
    );
    Redirect::to(&intent_uri)
}

// ─────────────────────────────────────────────────────────────────────────────
// iOS — HTML page with iframe deep-link + App Store fallback
// ─────────────────────────────────────────────────────────────────────────────

fn ios_page(id: &str) -> Html<String> {
    let deep_link = format!("{CUSTOM_SCHEME}:///property/{id}");
    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>Open in Livana</title>
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
  .logo {{ font-size: 48px; margin-bottom: 12px; }}
  h1 {{ font-size: 22px; font-weight: 700; margin-bottom: 8px; }}
  p {{ font-size: 14px; color: rgba(255,255,255,0.65); margin-bottom: 28px; }}
  .btn {{
    display: block;
    width: 100%;
    padding: 14px;
    border-radius: 12px;
    font-size: 16px;
    font-weight: 600;
    text-decoration: none;
    margin-bottom: 12px;
    transition: transform 0.15s ease, box-shadow 0.15s ease;
  }}
  .btn:active {{ transform: scale(0.97); }}
  .btn-primary {{
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    color: #fff;
    box-shadow: 0 4px 20px rgba(99,102,241,0.4);
  }}
  .btn-secondary {{
    background: rgba(255,255,255,0.1);
    color: #fff;
    border: 1px solid rgba(255,255,255,0.2);
  }}
</style>
</head>
<body>
<div class="card">
  <div class="logo">🏠</div>
  <h1>View Property on Livana</h1>
  <p>Tap the button below to open this property in the Livana app.</p>
  <a class="btn btn-primary" id="open-app" href="{deep_link}">Open in Livana</a>
  <a class="btn btn-secondary" href="{app_store}">Download on the App Store</a>
</div>
<iframe id="launcher" style="display:none;" width="0" height="0"></iframe>
<script>
  document.addEventListener('DOMContentLoaded', function() {{
    document.getElementById('launcher').src = '{deep_link}';
  }});
</script>
</body>
</html>"#,
        deep_link = deep_link,
        app_store = APP_STORE_URL,
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Desktop / Other — HTML page with store buttons
// ─────────────────────────────────────────────────────────────────────────────

fn desktop_page() -> Html<String> {
    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>Livana Eco — Download the App</title>
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
    padding: 48px 40px;
    max-width: 440px;
    width: 100%;
  }}
  .logo {{ font-size: 56px; margin-bottom: 16px; }}
  h1 {{ font-size: 26px; font-weight: 700; margin-bottom: 8px; }}
  p {{ font-size: 15px; color: rgba(255,255,255,0.65); margin-bottom: 32px; line-height: 1.5; }}
  .buttons {{ display: flex; flex-direction: column; gap: 12px; }}
  .btn {{
    display: block;
    width: 100%;
    padding: 16px;
    border-radius: 12px;
    font-size: 16px;
    font-weight: 600;
    text-decoration: none;
    transition: transform 0.15s ease, box-shadow 0.15s ease;
  }}
  .btn:active {{ transform: scale(0.97); }}
  .btn-android {{
    background: linear-gradient(135deg, #34d399, #059669);
    color: #fff;
    box-shadow: 0 4px 20px rgba(5,150,105,0.4);
  }}
  .btn-ios {{
    background: rgba(255,255,255,0.1);
    color: #fff;
    border: 1px solid rgba(255,255,255,0.2);
  }}
</style>
</head>
<body>
<div class="card">
  <div class="logo">🏠</div>
  <h1>Livana Eco</h1>
  <p>Download the app to view this property and discover thousands more listings near you.</p>
  <div class="buttons">
    <a class="btn btn-android" href="{play_store}">Get it on Google Play</a>
    <a class="btn btn-ios" href="{app_store}">Download on the App Store</a>
  </div>
</div>
</body>
</html>"#,
        play_store = PLAY_STORE_URL,
        app_store = APP_STORE_URL,
    ))
}
