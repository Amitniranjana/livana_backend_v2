# News API & Trending Notification System Documentation

## Overview

The News API provides endpoints to fetch, create, and interact with short-form news items (5-7 lines max). It includes a built-in trending engine that automatically calculates engagement velocity based on views, clicks, and shares. When an article trends, it triggers push notifications to users. 

## Endpoints

### 1. Fetch News (Public)
`GET /api/v1/news`
Fetches a list of news articles. Results are cached and returned in under 500ms.

**Query Parameters:**
- `category` (optional): Filter news by category string.

**Response (200 OK):**
```json
{
  "success": true,
  "message": "News fetched successfully",
  "data": [
    {
      "id": "uuid",
      "headline": "...",
      "short_summary": "...",
      "source": "...",
      "category": "...",
      "published_at": "2026-05-23T12:00:00Z",
      "thumbnail_url": "...",
      "views": 100,
      "clicks": 20,
      "shares": 5,
      "engagement_velocity": 45.2,
      "is_trending": false,
      "force_trending": false,
      "notifications_disabled": false,
      "created_at": "2026-05-23T12:00:00Z",
      "updated_at": "2026-05-23T12:00:00Z"
    }
  ]
}
```

### 2. Track Interaction (Public)
`POST /api/v1/news/{id}/action`
Track user engagement. This updates views, clicks, and shares, and recalculates the `engagement_velocity`. If the velocity crosses the trending threshold, a notification is automatically triggered.

**Request Body:**
```json
{
  "view": true,
  "click": false,
  "share": true
}
```

**Response (200 OK):**
```json
{
  "success": true,
  "message": "Action tracked successfully",
  "data": { ...updated_news_item... }
}
```

### 3. Create News (Admin)
`POST /api/v1/admin/news`
Creates a new article. The `short_summary` is strictly truncated to 7 lines by the backend.

**Request Body:**
```json
{
  "headline": "string",
  "short_summary": "string",
  "source": "string?",
  "category": "string?",
  "published_at": "datetime?",
  "thumbnail_url": "url?"
}
```

### 4. Update News (Admin)
`PUT /api/v1/admin/news/{id}`
Update an existing article.

### 5. Admin Actions (Admin)
`PATCH /api/v1/admin/news/{id}/action`
Allows admins to force a news item to trend or to disable notifications for it.

**Request Body:**
```json
{
  "force_trending": true,
  "notifications_disabled": false
}
```

## Notifications
When a news item trends (engagement_velocity > 50.0) or is force-trended, a notification is dispatched containing:
- The headline
- A 1-line teaser from the summary
- It is saved to the `notifications` table for the user to see, and ready for broadcast integration.
