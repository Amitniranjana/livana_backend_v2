# Livana Backend — Implementation Writeup
> **Date:** 24 February 2026 | **Author:** Amit Niranjan | **Server:** `http://localhost:9090`

---

## Overview

This document covers all features implemented on the Livana backend (`Rust + Axum + PostgreSQL`) across two sessions on 23–24 February 2026.

**Total:** 16 REST API endpoints | 3 database migrations | 64 unit tests passing | 0 compile errors

---

## 1. Property Search Module

### Endpoints

| Method | URL | Auth | Description |
|--------|-----|------|-------------|
| GET | `/api/v1/properties/search` | ❌ | Search properties with dynamic filters |
| GET | `/api/v1/properties/filters` | ❌ | Get filter options & ranges |
| GET | `/api/v1/search/suggestions` | ❌ | Autocomplete suggestions |

### Features
- **14 search filters:** city, locality, pincode, price range, BHK, property type, furnishing, area range, amenities, posted by
- **Pagination:** `page`, `limit`, returns `total_count`, `current_page`, `total_pages`
- **Sorting:** relevance, newest, price ascending/descending
- **Suggestions:** categorized by city, locality, project, builder, landmark

### Migration
```
migrations/20260223000001_create_properties.sql
```

### Files Created
```
src/repository/property_search_repository.rs
src/services/property_search_service.rs
src/handlers/property_search.rs
```

### Live Test Results
```json
GET /api/v1/properties/filters → 200 OK
{
  "success": true,
  "data": {
    "filters": {
      "bhkOptions": [{"value": 1, "label": "1 BHK"}, ...],
      "priceRange": {"min": 0, "max": 100000000},
      "areaRange":  {"min": 0, "max": 10000},
      "propertyTypes": [...],
      "amenities": ["lift", "parking", "gym", ...]
    }
  }
}

GET /api/v1/search/suggestions?q=mu → 200 OK
{
  "success": true,
  "data": { "suggestions": { "city": [], "locality": [], "builder": [], ... } }
}
```

---

## 2. CareCrew Services & Bookings Module

### Endpoints

| Method | URL | Auth | Description |
|--------|-----|------|-------------|
| GET | `/api/v1/carecrew/services` | ❌ | List all active services |
| GET | `/api/v1/carecrew/services/{id}` | ❌ | Get service detail |
| GET | `/api/v1/carecrew/providers` | ❌ | Search providers |
| GET | `/api/v1/carecrew/providers/featured` | ❌ | Get featured providers |
| GET | `/api/v1/carecrew/providers/{id}` | ❌ | Get provider detail |
| POST | `/api/v1/carecrew/bookings` | 🔒 | Create booking |
| PUT | `/api/v1/carecrew/bookings/{id}/status` | 🔒 | Update booking status |
| GET | `/api/v1/carecrew/providers/{id}/bookings` | 🔒 | Provider booking history |

### Booking State Machine
```
pending → confirmed → in_progress → completed
   ↓            ↓           ↓
cancelled    cancelled   cancelled
```

### Migration
```
migrations/20260223000002_create_carecrew.sql
```
Includes seed data: **8 CareCrew services** (Plumbing, Electrical, AC Service, Deep Cleaning, Pest Control, Painting, Carpentry, Appliance Repair)

### Files Created
```
src/repository/carecrew_repository.rs
src/services/carecrew_service.rs
src/handlers/carecrew.rs
src/models/carecrew.rs
```

### Live Test Results
```json
GET /api/v1/carecrew/services → 200 OK
{
  "success": true,
  "data": {
    "total": 8,
    "services": [
      { "name": "AC Service",    "category": "Appliances" },
      { "name": "Plumbing",      "category": "Home Repair" },
      { "name": "Deep Cleaning", "category": "Cleaning" },
      ...
    ]
  }
}
```

---

## 3. CareCrew Support — Ticketing Module (P1)

### Endpoints

| Method | URL | Auth | Description |
|--------|-----|------|-------------|
| POST | `/api/v1/carecrew/tickets` | 🔒 | Create a support ticket |
| GET | `/api/v1/carecrew/tickets` | 🔒 | List my tickets (paginated) |
| GET | `/api/v1/carecrew/tickets/{ticketId}` | 🔒 | Ticket detail + all comments |
| PATCH | `/api/v1/carecrew/tickets/{ticketId}` | 🔒 | Update status / assign agent |
| POST | `/api/v1/carecrew/tickets/{ticketId}/comments` | 🔒 | Add a comment |

### Ticket State Machine
```
OPEN → IN_PROGRESS → RESOLVED → CLOSED (terminal)
          ↓               ↓
         OPEN         IN_PROGRESS (re-open)
```

### Priority Levels
`LOW` | `MEDIUM` (default) | `HIGH`

### Issue Types
`service` | `operational` | `billing` | `property` | `provider_complaint` | `other`

### Migration
```
migrations/20260223000003_carecrew_tickets.sql
```
Two tables: `carecrew_tickets`, `carecrew_ticket_comments`
With 5 indexes + auto `updated_at` trigger

### Files Created
```
src/repository/carecrew_tickets_repository.rs
src/services/carecrew_tickets_service.rs
src/handlers/carecrew_tickets.rs
```

### Live Test Results ✅

```
TEST 1 — POST /carecrew/tickets (Create)
  → 201 Created | Status: OPEN | Priority: HIGH ✅

TEST 2 — GET /carecrew/tickets?page=1&limit=5 (List)
  → 200 OK | total_count: 2 | total_pages: 1 ✅

TEST 3 — GET /carecrew/tickets/{id} (Detail)
  → 200 OK | status: OPEN | comments: 0 ✅

TEST 4 — POST /carecrew/tickets/{id}/comments (Add Comment)
  → 201 Created | "Comment added successfully" ✅

TEST 5 — PATCH /carecrew/tickets/{id} status: OPEN → IN_PROGRESS
  → 200 OK | new status: IN_PROGRESS ✅

TEST 6 — PATCH invalid transition: IN_PROGRESS → CLOSED (skip)
  → 422 Unprocessable | error_code: INVALID_TRANSITION ✅

TEST 7 — GET /tickets/not-a-uuid
  → 400 Bad Request | error_code: INVALID_UUID ✅

TEST 8 — Any endpoint without token
  → 401 Unauthorized | error_code: UNAUTHORIZED ✅
```

---

## API Response Format

### Success
```json
{
  "success": true,
  "message": "Description",
  "data": { ... }
}
```

### Paginated Success
```json
{
  "success": true,
  "message": "...",
  "data": {
    "items": [...],
    "pagination": {
      "total_count":  25,
      "current_page": 1,
      "total_pages":  3,
      "limit":        10
    }
  }
}
```

### Error
```json
{
  "success": false,
  "message": "Human-readable reason",
  "error_code": "MACHINE_READABLE_CODE"
}
```

---

## Error Code Reference

| Code | HTTP | When |
|------|------|------|
| `UNAUTHORIZED` | 401 | Missing or invalid Bearer token |
| `FORBIDDEN` | 403 | Resource belongs to another user |
| `INVALID_UUID` | 400 | Malformed UUID in path/body |
| `MISSING_FIELDS` | 400 | Required field missing |
| `INVALID_PRIORITY` | 400 | Priority not LOW/MEDIUM/HIGH |
| `INVALID_STATUS` | 400 | Status not a valid enum value |
| `INVALID_TRANSITION` | 422 | State machine violation |
| `TICKET_CLOSED` | 422 | Ticket is terminal |
| `NOT_FOUND` | 404 | Resource doesn't exist |
| `DB_ERROR` | 500 | Database error |

---

## Test Summary

```
cargo test → 64 passed, 0 failed, 2 ignored
cargo check → EXIT:0 (zero compile errors)
```

---

## Documentation Files

| File | Purpose |
|------|---------|
| `API_DOCS.md` | Full backend API reference |
| `PROPERTY_SEARCH_CARECREW_API.md` | Property Search + CareCrew for frontend team |
| `CARECREW_TICKETS_API.md` | Ticketing module API for frontend team |
| `IMPLEMENTATION_SUMMARY.md` | Implementation summary with live test results |
| `LIVANA_BACKEND_WRITEUP.md` | This file — complete feature writeup |

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (stable) |
| Web Framework | Axum 0.8 |
| Async Runtime | Tokio |
| Database | PostgreSQL (via sqlx 0.8) |
| Auth | JWT (jsonwebtoken) |
| File Storage | AWS S3 |
| Migrations | sqlx-cli (`sqlx migrate run`) |
| Port | `9090` |
