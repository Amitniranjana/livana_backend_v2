# Care Connect App — Careers API Reference
## For Frontend Team | Issue #49

**Base URLs**
| Environment | URL |
|---|---|
| Dev | http://localhost:8080 |
| Staging | https://api-staging.livanaeco.com |
| Production | https://api.livanaeco.com |

**Auth:** ❌ No authentication required — these are public endpoints
**Date Format:** ISO 8601 UTC — e.g. `2026-03-28T10:00:00Z`

---

## Error Format
```json
{
  "success": false,
  "message": "Human readable error",
  "error_code": "ERROR_CODE",
  "errors": []
}
```

| Error Code | HTTP Status | When |
|---|---|---|
| `NOT_FOUND` | 404 | Job ID doesn't exist or job is inactive |
| `INTERNAL_SERVER_ERROR` | 500 | Database or server error |

---

## Endpoint 1 — Careers Listing

### GET /api/careers

Returns all **active** job openings sorted by latest first.
Use this to power the **Careers Page / Job Listing Screen**.

**Auth Required:** No
**Query Params:** None

**200 Success Response:**
```json
{
  "success": true,
  "message": "Careers retrieved successfully",
  "data": [
    {
      "job_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      "title": "Backend Developer",
      "location": "Ahmedabad",
      "employment_type": "Full-time",
      "experience": "2-4 years",
      "posted_at": "2026-03-28T10:00:00Z"
    },
    {
      "job_id": "b2c3d4e5-f6a7-8901-bcde-f12345678901",
      "title": "Flutter Developer",
      "location": "Remote",
      "employment_type": "Full-time",
      "experience": "1-3 years",
      "posted_at": "2026-03-25T09:00:00Z"
    }
  ]
}
```

**Empty State Response (no active jobs):**
```json
{
  "success": true,
  "message": "Careers retrieved successfully",
  "data": []
}
```

**Notes for Frontend:**
- `data` is always an array — handle empty array for empty state UI
- `description` is NOT included here — fetch it from Endpoint 2 when user taps a job
- Jobs are always sorted newest first (use `posted_at` for display)
- `employment_type` values: `"Full-time"` `"Part-time"` `"Contract"` `"Internship"`

**cURL Example:**
```bash
curl -X GET https://api.livanaeco.com/api/careers
```

---

## Endpoint 2 — Job Description Detail

### GET /api/careers/{job_id}

Returns full job details for a single position.
Use this to power the **Job Description Page** when a user taps on a job card.

**Auth Required:** No
**Path Parameter:**

| Param | Type | Required | Description |
|---|---|---|---|
| job_id | UUID | ✅ | The `job_id` from the listing response |

**200 Success Response:**
```json
{
  "success": true,
  "message": "Job details retrieved successfully",
  "data": {
    "job_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "title": "Backend Developer",
    "location": "Ahmedabad",
    "employment_type": "Full-time",
    "experience": "2-4 years",
    "description": "We are looking for an experienced Backend Developer to join our team...",
    "posted_at": "2026-03-28T10:00:00Z"
  }
}
```

**404 Response (job not found OR inactive):**
```json
{
  "success": false,
  "message": "Job not found",
  "error_code": "NOT_FOUND",
  "errors": []
}
```

**Notes for Frontend:**
- Both "job not found" and "job inactive" return the same `404` response — do not distinguish between them
- Always use `job_id` from Endpoint 1's `data[].job_id` field to call this endpoint
- Handle `404` gracefully — show "This position is no longer available" message

**cURL Examples:**
```bash
# Valid job
curl -X GET https://api.livanaeco.com/api/careers/a1b2c3d4-e5f6-7890-abcd-ef1234567890

# Invalid job → 404
curl -X GET https://api.livanaeco.com/api/careers/00000000-0000-0000-0000-000000000000
```

---

## Complete Flow

```
User opens Careers screen
        ↓
GET /api/careers
→ Show list of job cards (title, location, type, experience, posted_at)
        ↓
User taps a job card
        ↓
GET /api/careers/{job_id}
→ Show full job description page
        ↓
[Apply button — future feature]
```

---

## Field Reference

| Field | Type | Present In | Description |
|---|---|---|---|
| job_id | UUID string | Both | Unique job identifier |
| title | string | Both | Job title |
| location | string | Both | City or "Remote" |
| employment_type | string | Both | Full-time / Part-time / Contract / Internship |
| experience | string | Both | e.g. "2-4 years" |
| posted_at | ISO 8601 string | Both | When the job was posted |
| description | string | Detail only | Full job description (HTML or plain text) |
