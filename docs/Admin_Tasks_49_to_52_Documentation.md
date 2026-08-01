# Admin Backend — Tasks 49–52 Frontend Documentation

> **Scope:** Audit Log · Rate Limiting · Global Response Envelope · Schema Endpoint  
> **Last Updated:** 2026-08-01  
> **Auth Required:** All endpoints require an `admin_session` cookie or `Authorization: Bearer <token>`

---

## Table of Contents

1. [Global Response Envelope (Task 51)](#task-51--global-response-envelope)
2. [Rate Limiting (Task 50)](#task-50--rate-limiting)
3. [Audit Log (Task 49)](#task-49--audit-log)
4. [GET /api/admin/schema (Task 52)](#task-52--get-apiadminschema)

---

## Task 51 — Global Response Envelope

> **Read this section first** — it defines the base response structure used by all other endpoints.

The backend now returns **all success responses** in a standardized shape:

### Success Response Shape

```json
{
  "success": true,
  "data": <actual payload>,
  "message": "Optional human-readable message",
  "code": null
}
```

### Error Response Shape

```json
{
  "success": false,
  "message": "What went wrong",
  "error_code": "MACHINE_READABLE_CODE",
  "errors": {
    "field_name": ["Validation error detail"]
  }
}
```

> The `errors` field is only present on `VALIDATION_ERROR` responses — it is omitted otherwise.

### Error Codes Reference

| `error_code` | HTTP Status | Meaning |
|---|---|---|
| `BAD_REQUEST` | 400 | Malformed request |
| `VALIDATION_ERROR` | 422 | Field-level validation failed |
| `UNAUTHORIZED` | 401 | Not logged in / token expired |
| `FORBIDDEN` | 403 | Authenticated but lacks permission |
| `NOT_FOUND` | 404 | Resource not found |
| `CONFLICT` | 409 | Duplicate resource |
| `RATE_LIMIT_EXCEEDED` | 429 | Too many requests |
| `INTERNAL_SERVER_ERROR` | 500 | Backend error |

### Frontend Integration Pattern

```javascript
async function adminApiCall(url, options = {}) {
  const res = await fetch(url, {
    ...options,
    credentials: 'include',
    headers: { 'Content-Type': 'application/json', ...options.headers },
  });

  const body = await res.json();

  if (!body.success) {
    // On error — use body.message and body.error_code
    if (body.error_code === 'VALIDATION_ERROR') {
      // body.errors contains field-level details
      console.error('Validation errors:', body.errors);
    }
    throw new Error(body.message);
  }

  // On success — the actual payload is in body.data
  return body.data;
}
```

---

## Task 50 — Rate Limiting

The backend uses a **Redis-backed** rate limiting middleware. The frontend only needs to handle `429` responses gracefully.

### Limits

| Endpoint / Action | Limit | Window | Keyed By |
|---|---|---|---|
| `POST /api/admin/login` | 5 attempts | 15 minutes | Per IP address |
| Force-delete operations | 20 attempts | 1 minute | Per admin session |

### Rate Limit Response — `429 Too Many Requests`

**Login rate limit:**
```json
{
  "success": false,
  "message": "Too many login attempts. Please try again later.",
  "error_code": "RATE_LIMIT_EXCEEDED"
}
```

**Force-delete rate limit:**
```json
{
  "success": false,
  "message": "Too many force delete attempts. Please wait 1 minute.",
  "error_code": "RATE_LIMIT_EXCEEDED"
}
```

### Frontend Handling

```javascript
async function adminLogin(email, password) {
  try {
    const res = await fetch('/api/admin/login', {
      method: 'POST',
      credentials: 'include',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, password }),
    });
    const body = await res.json();

    if (res.status === 429) {
      // Rate limited — show countdown and disable the button
      showError('Too many attempts. Please wait before trying again.');
      disableLoginButton(15 * 60); // 15-minute countdown
      return;
    }

    if (!body.success) {
      showError(body.message);
      return;
    }

    // Success
    redirectToDashboard();
  } catch (err) {
    showError('Network error. Please try again.');
  }
}
```

> **Note:** If Redis is not available, rate limiting is silently skipped (fail-open behavior). Make sure `REDIS_URL` is configured in production.

---

## Task 49 — Audit Log

### Overview

Every mutating admin action automatically writes an entry to the `app_audit_logs` table. This is **append-only** — no records are ever deleted or updated.

**The frontend does not call this directly** — logs are tracked automatically by the backend on every admin operation. To display logs in the admin panel, fetch from the `admin_action_logs` table via `GET /api/admin/logs`.

---

### Table: `app_audit_logs` (Internal Audit — Task 49)

This table is populated automatically on every admin operation.

| Field | Type | Description |
|---|---|---|
| `id` | UUID | Unique log entry ID |
| `admin_email` | string | Email of the admin who performed the action |
| `action` | string | What was done (e.g., `"kyc_approve"`, `"user_ban"`) |
| `entity_type` | string | Which entity type was affected (e.g., `"user"`, `"listing"`, `"kyc"`) |
| `entity_id` | UUID? | ID of the target entity (nullable) |
| `reason` | string? | Admin-provided reason (nullable) |
| `metadata` | JSONB? | Additional context data (nullable) |
| `created_at` | timestamp | Set automatically by the database |

---

### GET /api/admin/logs — Logs List

**URL:** `GET /api/admin/logs`  
**Auth:** Admin session required  

#### Query Parameters

| Param | Type | Required | Description |
|---|---|---|---|
| `limit` | number | No | Results per page (default: 20, max: 100) |
| `offset` | number | No | Pagination offset (default: 0) |
| `admin_id` | string | No | Filter by admin email or ID |
| `action_type` | string | No | Filter by action type |
| `target_type` | string | No | Filter by entity type (e.g., `"user"`) |
| `from_date` | ISO datetime | No | Return logs on or after this date |
| `to_date` | ISO datetime | No | Return logs on or before this date |

#### Example Request

```
GET /api/admin/logs?limit=20&offset=0&target_type=user&action_type=ban
```

#### Response — `200 OK`

```json
{
  "success": true,
  "data": {
    "total": 142,
    "logs": [
      {
        "id": "uuid-here",
        "admin_id": "admin@livana.com",
        "admin_name": null,
        "action_type": "ban",
        "target_type": "user",
        "target_id": "user-uuid-here",
        "details": {
          "reason": "Violated community guidelines",
          "duration_days": 7
        },
        "created_at": "2026-08-01T07:00:00Z"
      }
    ]
  }
}
```

#### Response Fields

| Field | Type | Description |
|---|---|---|
| `total` | number | Total matching records (use for pagination) |
| `logs` | array | Array of log entries |
| `logs[].id` | UUID | Log entry ID |
| `logs[].admin_id` | string | Admin who performed the action |
| `logs[].admin_name` | string? | Admin display name (currently `null`) |
| `logs[].action_type` | string | What action was performed |
| `logs[].target_type` | string | Entity type (`user`, `listing`, `kyc`, etc.) |
| `logs[].target_id` | UUID? | ID of the target entity |
| `logs[].details` | object? | Additional JSONB detail |
| `logs[].created_at` | ISO datetime | When the action occurred |

---

### GET /api/admin/logs/:targetType/:targetId — Logs by Entity

Fetches all audit log entries for a specific entity.

**URL:** `GET /api/admin/logs/{target_type}/{target_id}`

#### Example

```
GET /api/admin/logs/user/3fa85f64-5717-4562-b3fc-2c963f66afa6
```

#### Response — `200 OK`

Same structure as the list endpoint, but returns all logs for the entity without additional pagination.

```json
{
  "success": true,
  "data": {
    "total": 5,
    "logs": [ ... ]
  }
}
```

---

### Pagination Helper (JavaScript)

```javascript
async function getAdminLogs({ page = 1, limit = 20, filters = {} } = {}) {
  const offset = (page - 1) * limit;
  const params = new URLSearchParams({ limit, offset, ...filters });

  const res = await fetch(`/api/admin/logs?${params}`, {
    credentials: 'include',
  });
  const body = await res.json();

  if (!body.success) throw new Error(body.message);

  return {
    logs: body.data.logs,
    total: body.data.total,
    totalPages: Math.ceil(body.data.total / limit),
    currentPage: page,
  };
}

// Usage
const { logs, totalPages } = await getAdminLogs({
  page: 1,
  filters: { target_type: 'user', action_type: 'ban' },
});
```

---

## Task 52 — GET /api/admin/schema

### Overview

This endpoint returns the **live database schema** — all PostgreSQL public tables with their column names, data types, nullability, primary key flags, and live row counts. Useful for building a database explorer or debugging tool in the admin dashboard.

**URL:** `GET /api/admin/schema`  
**Method:** `GET`  
**Auth:** Admin session required  
**No request body or query parameters required**

---

### Response — `200 OK`

```json
{
  "success": true,
  "data": [
    {
      "table_name": "users",
      "live_row_count": 1523,
      "columns": [
        {
          "name": "id",
          "data_type": "uuid",
          "is_nullable": false,
          "is_primary_key": true
        },
        {
          "name": "email",
          "data_type": "character varying",
          "is_nullable": false,
          "is_primary_key": false
        },
        {
          "name": "created_at",
          "data_type": "timestamp with time zone",
          "is_nullable": true,
          "is_primary_key": false
        }
      ]
    },
    {
      "table_name": "kyc_submissions",
      "live_row_count": 87,
      "columns": [ ... ]
    }
  ],
  "message": "Schema fetched successfully"
}
```

### Response Fields

| Field | Type | Description |
|---|---|---|
| `data` | array | All tables in the database |
| `data[].table_name` | string | PostgreSQL table name |
| `data[].live_row_count` | number | Current number of rows in the table |
| `data[].columns` | array | All columns in this table |
| `data[].columns[].name` | string | Column name |
| `data[].columns[].data_type` | string | PostgreSQL data type |
| `data[].columns[].is_nullable` | boolean | `true` if the column allows NULL |
| `data[].columns[].is_primary_key` | boolean | `true` if this column is a primary key |

> **Ordering:** Tables are sorted **alphabetically** by `table_name`. Columns follow their `ordinal_position` (the order they were defined in the schema).

### Common PostgreSQL Data Types You'll See

| `data_type` value | Meaning |
|---|---|
| `uuid` | UUID identifier |
| `character varying` | VARCHAR / bounded text |
| `text` | Unbounded text |
| `integer` | 32-bit integer |
| `bigint` | 64-bit integer |
| `boolean` | `true` / `false` |
| `timestamp with time zone` | Timestamp with timezone (TIMESTAMPTZ) |
| `jsonb` | Binary JSON — can be any object or array |
| `numeric` | Decimal number |
| `USER-DEFINED` | A PostgreSQL ENUM type |

### Error Response — `500 Internal Server Error`

```json
{
  "success": false,
  "message": "Failed to fetch schema: <db error detail>",
  "error_code": "INTERNAL_SERVER_ERROR"
}
```

### Frontend Integration Example

```javascript
async function getDatabaseSchema() {
  const res = await fetch('/api/admin/schema', {
    credentials: 'include',
  });
  const body = await res.json();

  if (!body.success) throw new Error(body.message);

  return body.data; // Array of TableSchema objects
}

// Render a schema explorer
async function renderSchemaExplorer() {
  const tables = await getDatabaseSchema();

  tables.forEach(table => {
    console.log(`Table: ${table.table_name} (${table.live_row_count} rows)`);
    table.columns.forEach(col => {
      const pk = col.is_primary_key ? ' [PK]' : '';
      const nullable = col.is_nullable ? ' (nullable)' : '';
      console.log(`  ${col.name}: ${col.data_type}${pk}${nullable}`);
    });
  });
}

// Find which tables contain a specific column name
function findColumn(tables, columnName) {
  return tables
    .filter(t => t.columns.some(c => c.name === columnName))
    .map(t => ({
      table: t.table_name,
      column: t.columns.find(c => c.name === columnName),
    }));
}
```

---

## Quick Reference

| Task | What it does | Frontend Impact |
|---|---|---|
| **Task 49** | Audit log writer — automatically tracks every admin mutation | No direct call needed; read via `GET /api/admin/logs` |
| **Task 50** | Redis rate limiter — 5 logins/15 min, 20 force-deletes/min | Handle `429` responses gracefully |
| **Task 51** | Global response envelope `{ success, data, message, code }` | All responses now follow this shape |
| **Task 52** | `GET /api/admin/schema` — live DB schema with row counts | Use to build a DB explorer or debugging panel |

---

## Common Gotchas

> [!WARNING]
> **Task 51 — Breaking Change:** Any endpoint that previously returned a direct object now wraps the payload inside `{ success, data }`. If existing code reads `response.email`, it must be updated to `response.data.email`.

> [!IMPORTANT]
> **Task 50 — Redis Dependency:** If Redis is not configured locally, rate limiting is silently skipped — login will still work. Ensure the `REDIS_URL` environment variable is set in production.

> [!NOTE]
> **Task 52 — Performance:** The schema endpoint runs a live `COUNT(*)` query on every table. With 50+ tables this can take ~200–500 ms. Consider caching the response on the frontend if you call it frequently.
