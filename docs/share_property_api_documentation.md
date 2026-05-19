# Share Property — API Documentation

## Overview
This endpoint serves an HTML page that automatically deep-links the recipient into the Livana Eco app when they tap a shared property link. No JSON, no auth.

## Endpoint
`GET /share/property/:id`

| Field | Value |
| --- | --- |
| **Method** | `GET` |
| **Auth** | None (public) |
| **Response type** | `text/html` |
| **Status** | Active — registered in `routes.rs` and mounted in `main.rs` |

## Path Parameter

| Param | Type | Required | Description |
| --- | --- | --- | --- |
| `id` | `UUID` | Yes | The property UUID |

## Handler Behavior (`src/handlers/share.rs`)

1. **Validate the ID**
   - Parses the `id` as a UUID.
   - If invalid, returns `400 Bad Request` in plain text.

2. **Query the Database**
   - Queries the `properties` table for the target listing details.
   - **Note:** If the DB query fails or the property is not found, the handler **does NOT return an error page**. It renders the HTML with generic fallback text so the deep link continues to work.

   ```sql
   SELECT
       p.title,
       p.city,           -- used as location display
       p.price,          -- rent price (BIGINT)
       p.deposit,        -- deposit amount (BIGINT, nullable)
       p.listing_type,   -- "Rent" | "Sell" | "Lease" etc.
       p.bhk,            -- number of bedrooms (aliased as bedrooms in app)
       p.bathrooms,
       p.images          -- JSONB array of image URLs e.g. ["https://...","https://..."]
   FROM properties p
   WHERE p.id = $1
     AND p.status != 'deleted'
   LIMIT 1
   ```

3. **Extract First Image**
   - Parses the `images` JSONB column as a `Vec<String>`.
   - Takes the first URL that starts with `https://`. This is used for the WhatsApp/iMessage link preview (OG tag).

4. **Return HTML Response**
   - Returns status `200 OK` with header `Content-Type: text/html; charset=utf-8`.

## HTML Page Behavior

The page returned by the handler automatically triggers the following actions on load (no button, no user interaction needed):

### Android
Uses an Intent URI so the OS handles everything:
- **If app installed** → opens app directly at `livanaeco://property/{id}`
- **If app not installed** → auto-redirects to Play Store using fallback URL
- Intent format: `intent://property/{id}#Intent;scheme=livanaeco;package=com.LiveInBuddy.livein;S.browser_fallback_url=https%3A%2F%2Fplay.google.com%2Fstore%2Fapps%2Fdetails%3Fid%3Dcom.LiveInBuddy.livein;end`

### iOS
- Immediately sets `window.location.href = 'livanaeco://property/{id}'`
- Starts a 1500ms timeout — if the page is still visible after 1.5s, redirects to the App Store.
- Listens for `visibilitychange` or `blur` events — if the page goes to the background (app opened), it clears the timeout to prevent store redirection.

### Desktop
Shows a plain message: `"Open this link on your mobile device to view the property in the app."`

### Loading UI (while redirect fires)
Shows a minimal spinner + text while the redirect is happening:
- **Spinner animation**
- **Text:** "Opening Livana Eco…"
- **Sub-text:** "Taking you to the property"
- *If app not found after timeout (iOS), updates sub-text to:* "App not found — redirecting to store…" and redirects to store after 500ms.

## App Store Links
- **App Store URL:** [https://apps.apple.com/in/app/livana-eco/id6742744565](https://apps.apple.com/in/app/livana-eco/id6742744565)
- **Play Store URL:** [https://play.google.com/store/apps/details?id=com.LiveInBuddy.livein](https://play.google.com/store/apps/details?id=com.LiveInBuddy.livein)

## OG Meta Tags (for WhatsApp / iMessage previews)

The HTML `<head>` includes these for rich link previews (all values are HTML-escaped before embedding):

```html
<meta property="og:title"       content="{property title}" />
<meta property="og:description" content="{price label} — {city}" />
<meta property="og:image"       content="{first image URL}" />
<meta property="og:type"        content="website" />
```

## Example URLs

This URL is what gets shared via WhatsApp/SMS. When the recipient taps it:
1. Browser opens
2. App launches instantly (if installed)
3. DeepLinkService in the Flutter app catches `livanaeco://property/550e8400-...` and navigates to the property screen.

**Sample Request:**
```http
GET http://13.216.208.31:9090/share/property/550e8400-e29b-41d4-a716-446655440000
```
