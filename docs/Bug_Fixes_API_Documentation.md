# Task: Fix Notifications + Chat Sync (Vibe & Book Visit APIs) #64


> **Date:** 6 April 2026
> **Status:** ✅ All Issues Resolved
> **Build:** Compiled & Tested (72 tests passed, 0 failed)

---

## Table of Contents

1. [🔔 Notification System — Fixed](#1--notification-system--fixed)
2. [💬 Chat List — Fixed](#2--chat-list--fixed)
3. [🔑 Reset Password — OTP Now Sent to Phone Also](#3--reset-password--otp-now-sent-to-phone-also)
4. [🔐 New: Change Password API](#4--new-change-password-api)
5. [⚠️ Associate Registration — 422 Error Fixed](#5-️-associate-registration--422-error-fixed)
6. [❌ Provider 404 — Debugging Info](#6--provider-404--debugging-info)

---

## 1. 🔔 Notification System — Fixed

### Problem
- Vibe API ✅ hitting → but ❌ no notification sent to receiver
- Book Visit API ✅ hitting → but ❌ no notification sent
- Frontend couldn't show Accept/Reject buttons

### What Was Fixed
- Notification triggers were already in place (code existed), but the **GET notifications API was not returning the Accept/Reject metadata fields**
- Now the API returns `related_entity_id`, `related_entity_type`, and `action_status` so frontend can render Accept/Reject actions

---

### `GET /api/v1/notifications`

> **Auth:** Bearer Token Required

#### Response (200 OK)

```json
{
  "success": true,
  "message": "Notifications fetched successfully",
  "data": [
    {
      "id": "a1b2c3d4-...",
      "title": "New Vibe! 💫",
      "message": "John Doe showed interest in your property",
      "type": "VIBE",
      "is_read": false,
      "related_entity_id": "vibe-uuid-here",
      "related_entity_type": "VIBE",
      "action_status": null,
      "created_at": "2026-04-06T10:00:00Z"
    },
    {
      "id": "e5f6g7h8-...",
      "title": "New Visit Booking! 📅",
      "message": "Jane booked a site visit on 10 Apr 2026, 02:00 PM",
      "type": "BOOKING",
      "is_read": false,
      "related_entity_id": "visit-uuid-here",
      "related_entity_type": "SITE_VISIT",
      "action_status": "ACCEPTED",
      "created_at": "2026-04-06T09:30:00Z"
    }
  ]
}
```

#### New Fields Explained

| Field | Type | Description |
|-------|------|-------------|
| `related_entity_id` | `UUID` or `null` | ID of the vibe/visit that triggered this notification |
| `related_entity_type` | `String` or `null` | `"VIBE"` or `"SITE_VISIT"` |
| `action_status` | `String` or `null` | `null` = pending (show Accept/Reject), `"ACCEPTED"`, `"REJECTED"` |

#### Frontend Logic

```dart
if (notification.actionStatus == null) {
  // Show Accept / Reject buttons
} else if (notification.actionStatus == "ACCEPTED") {
  // Show "Accepted" badge (green)
} else if (notification.actionStatus == "REJECTED") {
  // Show "Rejected" badge (red)
}
```

---

### Accept/Reject Flow (Already Working)

#### Accept a Vibe
```
POST /api/v1/vibes/{vibe_id}/accept
Authorization: Bearer <token>
```
→ Updates `action_status` to `"ACCEPTED"` on the notification automatically

#### Reject a Vibe
```
POST /api/v1/vibes/{vibe_id}/reject
Authorization: Bearer <token>
```
→ Updates `action_status` to `"REJECTED"` on the notification automatically

#### Accept a Visit (Confirm)
```
PUT /api/visits/{visit_id}/status
Authorization: Bearer <token>

{
  "status": "confirmed"
}
```
→ Updates `action_status` to `"ACCEPTED"` on the notification automatically

#### Reject a Visit (Cancel)
```
PUT /api/visits/{visit_id}/status
Authorization: Bearer <token>

{
  "status": "cancelled",
  "cancellation_reason": "Not available on that date"
}
```
→ Updates `action_status` to `"REJECTED"` on the notification automatically

---

### Mark Notification as Read

```
PATCH /api/v1/notifications/{notification_id}/read
Authorization: Bearer <token>
```

**Response:**
```json
{
  "success": true,
  "message": "Notification marked as read",
  "data": {
    "notification_id": "...",
    "is_read": true
  }
}
```

---

## 2. 💬 Chat List — Fixed

### Problem
- `/api/v1/chats/recent` → empty or no useful info
- Chat was auto-created on Vibe/Visit (code existed) but the **list API wasn't returning participant info**

### What Was Fixed
- Chat auto-creation on Vibe & Book Visit was already working
- The `GET /api/v1/chats/recent` API now returns the **other user's name, image, and ID**

---

### `GET /api/v1/chats/recent`

> **Auth:** Bearer Token Required

#### Response (200 OK)

```json
{
  "success": true,
  "message": "Recent chats fetched successfully",
  "data": [
    {
      "chat_id": "chat-uuid-here",
      "last_message": "👋 John Doe is interested in your property",
      "last_message_time": "2026-04-06T10:00:00Z",
      "other_user_id": "user-uuid-here",
      "other_user_name": "John Doe",
      "other_user_image": "https://example.com/profile.jpg"
    },
    {
      "chat_id": "another-chat-uuid",
      "last_message": "📅 Jane booked a site visit for 10 Apr 2026, 02:00 PM",
      "last_message_time": "2026-04-06T09:30:00Z",
      "other_user_id": "jane-uuid",
      "other_user_name": "Jane Smith",
      "other_user_image": null
    }
  ]
}
```

#### New Fields

| Field | Type | Description |
|-------|------|-------------|
| `other_user_id` | `UUID` | ID of the other person in the chat |
| `other_user_name` | `String` | Full name of the other person |
| `other_user_image` | `String` or `null` | Profile picture URL (nullable) |

---

## 3. 🔑 Reset Password — OTP Now Sent to Phone Also

### Problem
- `POST /api/auth/reset-password` — OTP was only being sent to email
- User also needed OTP on phone

### What Was Fixed
- `POST /api/auth/send-forgot-password-link` now sends OTP to **both email AND phone (SMS)**

---

### `POST /api/auth/send-forgot-password-link`

> **Auth:** Not Required (public)

#### Request Body

```json
{
  "email": "john@example.com"
}
```

#### Response (200 OK)

```json
{
  "success": true,
  "message": "Reset code generated and sent to email and phone",
  "data": {
    "email": "john@example.com",
    "phone_no": "+919876543210",
    "reset_code_sent": true
  }
}
```

> **Note:** The OTP is now sent to both the registered email AND phone number.

---

### `POST /api/auth/reset-password`

> **Auth:** Not Required (public)
> **Note:** This API now actually updates the password in DB (was broken before, now fixed)

#### Request Body

```json
{
  "email": "john@example.com",
  "code": "123456",
  "new_password": "mynewpassword123"
}
```

#### Response (200 OK)

```json
{
  "success": true,
  "message": "Password reset successfully",
  "data": {
    "password_updated": true,
    "user_id": "user-uuid-here"
  }
}
```

#### Error Responses

| Status | Message |
|--------|---------|
| 400 | Invalid or expired reset code |

---

## 4. 🔐 New: Change Password API

### Problem
- No API existed for logged-in users to change their password
- User wanted an endpoint where they enter current password + new password

### What Was Created
- New `POST /api/auth/change-password` endpoint (JWT-protected)

---

### `POST /api/auth/change-password`

> **Auth:** Bearer Token Required
> **New Endpoint ✨**

#### Request Body

```json
{
  "currentPassword": "myoldpassword123",
  "newPassword": "mynewpassword456"
}
```

> ⚠️ **Note:** Field names are in `camelCase` (currentPassword, newPassword)

#### Response — Success (200 OK)

```json
{
  "success": true,
  "message": "Password changed successfully",
  "data": {
    "password_updated": true
  }
}
```

#### Response — Wrong Current Password (401)

```json
{
  "success": false,
  "message": "Current password is incorrect",
  "data": null
}
```

#### Response — User Not Found (404)

```json
{
  "success": false,
  "message": "User not found",
  "data": null
}
```

---

## 5. ⚠️ Associate Registration — 422 Error Fixed

### Problem
- `POST /api/v1/associates/register` was returning 422 error
- Missing required field `associate_type` was not being validated

### What Was Fixed
- `associate_type` is now **required** and **validated**
- Must be one of: `broker`, `carecrew`, `agent`, `owner`
- Clear error message returned if missing or invalid

---

### `POST /api/v1/associates/register`

> **Auth:** Not Required (public)

#### Request Body (Correct ✅)

```json
{
  "name": "Ramesh Kumar",
  "email": "ramesh@example.com",
  "phone": "+919876543210",
  "password": "securepassword123",
  "associate_type": "broker",
  "gender": "male"
}
```

#### Valid `associate_type` Values

| Value | Description |
|-------|-------------|
| `broker` | Real estate broker/agent |
| `carecrew` | CareCrew service provider |
| `agent` | General agent |
| `owner` | Property owner |

> **Note:** Value is **case-insensitive** — `"Broker"`, `"BROKER"`, `"broker"` all work.

#### Response — Success (201 Created)

```json
{
  "success": true,
  "message": "Associate registered successfully",
  "data": {
    "associate_id": "uuid-here",
    "status": "PENDING_KYC"
  }
}
```

#### Response — Missing associate_type (422)

```json
{
  "success": false,
  "message": "Missing required field: associate_type. Must be one of: broker, carecrew, agent, owner",
  "error_code": "UNPROCESSABLE_ENTITY"
}
```

#### Response — Invalid associate_type (422)

```json
{
  "success": false,
  "message": "Invalid associate_type 'xyz'. Must be one of: broker, carecrew, agent, owner",
  "error_code": "UNPROCESSABLE_ENTITY"
}
```

#### Response — Duplicate User (409)

```json
{
  "success": false,
  "message": "User with this email or phone number already exists",
  "error_code": "CONFLICT"
}
```

---

## 6. ❌ Provider 404 — Debugging Info

### Problem
- `GET /api/v1/carecrew/providers/{id}` → 404

### What Was Done
- Logging is already present: `println!("[CareCrew] Fetching provider ID: {}", provider_id)`
- The 404 means the provider ID being passed from frontend **does not exist** in the `carecrew_providers` table

### How to Debug

1. Check server logs for: `[CareCrew] Fetching provider ID: <uuid>`
2. Verify that UUID exists in `carecrew_providers` table in the database
3. If the ID is from the `users` table — that's the wrong table. Providers must be in `carecrew_providers`

### API Reference

```
GET /api/v1/carecrew/providers/{provider_id}
```

**Response (200 OK):**
```json
{
  "success": true,
  "message": "Provider retrieved successfully",
  "data": {
    "provider": {
      "id": "provider-uuid",
      "name": "Plumber Singh",
      "service_type": "plumbing",
      "rating": 4.5,
      "review_count": 12,
      "location": "Mumbai",
      "phone": "+919876543210",
      "bio": "10 years experience in plumbing",
      "profile_image": "https://...",
      "is_verified": true,
      "availability": "available"
    }
  }
}
```

**Response (404):**
```json
{
  "success": false,
  "message": "Provider not found",
  "error_code": "NOT_FOUND"
}
```

---

## 📊 Complete Flow After Fix

### Before (Broken ❌)
```
API hit → success → NOTHING happens
```

### After (Working ✅)
```
Vibe API → DB Insert → Notification Created → Chat Auto-Created → Accept/Reject Available
Book Visit → DB Insert → Notification Created → Chat Auto-Created → Confirm/Cancel Available
```

---

## 🧪 Testing Checklist for Frontend

| Test | Expected Result |
|------|----------------|
| Send Vibe | ✅ Target user sees notification with Accept/Reject |
| Accept Vibe | ✅ `action_status` becomes `"ACCEPTED"` |
| Reject Vibe | ✅ `action_status` becomes `"REJECTED"` |
| Book Visit | ✅ Provider sees notification with Confirm/Cancel |
| Confirm Visit | ✅ `action_status` becomes `"ACCEPTED"` |
| Cancel Visit | ✅ `action_status` becomes `"REJECTED"` |
| Chat auto-created | ✅ Visible in `/api/v1/chats/recent` with user info |
| Reset password | ✅ OTP received on both email and phone |
| Change password | ✅ `POST /api/auth/change-password` works |
| Register associate without `associate_type` | ✅ Returns 422 with clear message |
| Register associate with valid type | ✅ Returns 201 Created |

---

## 📡 All Endpoints Summary

| Method | Endpoint | Auth | Status |
|--------|----------|------|--------|
| `GET` | `/api/v1/notifications` | ✅ JWT | ✅ Fixed |
| `PATCH` | `/api/v1/notifications/{id}/read` | ✅ JWT | ✅ Working |
| `GET` | `/api/v1/chats/recent` | ✅ JWT | ✅ Fixed |
| `POST` | `/api/v1/vibes` | ✅ JWT | ✅ Working |
| `POST` | `/api/v1/vibes/{id}/accept` | ✅ JWT | ✅ Working |
| `POST` | `/api/v1/vibes/{id}/reject` | ✅ JWT | ✅ Working |
| `POST` | `/api/visits` | ✅ JWT | ✅ Working |
| `PUT` | `/api/visits/{id}/status` | ✅ JWT | ✅ Working |
| `POST` | `/api/auth/send-forgot-password-link` | ❌ Public | ✅ Fixed (SMS added) |
| `POST` | `/api/auth/reset-password` | ❌ Public | ✅ Fixed (DB update works) |
| `POST` | `/api/auth/change-password` | ✅ JWT | ✅ **New** |
| `POST` | `/api/v1/associates/register` | ❌ Public | ✅ Fixed (validation) |
| `GET` | `/api/v1/carecrew/providers/{id}` | ❌ Public | ⚠️ Data issue |
