# Jobs & Applications API Documentation

This document outlines the recent updates to the Jobs and Applications API, specifically the new endpoints for brokers to manage their posted jobs and handle job applications, as well as the updated applicant DTO.

## 1. Get My Posted Jobs
Retrieve a list of jobs posted by the currently authenticated broker.

- **Endpoint**: `GET /api/v1/jobs/mine`
- **Auth Required**: Yes (Bearer Token)
- **Role**: Broker/Associate

### Query Parameters
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `page` | `integer` | No | Page number for pagination (default: 1) |
| `limit` | `integer` | No | Number of items per page (default: 10, max: 50) |
| `location` | `string` | No | Filter jobs by location |
| `job_type` | `string` | No | Filter by job type (e.g., full-time, part-time) |
| `status` | `string` | No | Filter by job status (default: ACTIVE) |

### Response Example (200 OK)
```json
{
  "success": true,
  "message": "My jobs retrieved successfully",
  "data": {
    "jobs": [
      {
        "id": "uuid-here",
        "title": "Senior Frontend Developer",
        "company_name": "Tech Corp",
        "location": "Remote",
        "salary_range": "$100k - $120k",
        "job_type": "full-time",
        "status": "ACTIVE",
        "created_at": "2026-05-17T04:00:00Z"
      }
    ],
    "pagination": {
      "total_count": 1,
      "current_page": 1,
      "total_pages": 1,
      "limit": 10
    }
  }
}
```

---

## 2. Get Applicants for a Job
Retrieve all applications submitted for a specific job.

- **Endpoint**: `GET /api/v1/jobs/{job_id}/applicants`
- **Auth Required**: Yes (Bearer Token)
- **Role**: Broker/Associate (Must be the creator of the job)

### Path Parameters
- `job_id` (UUID): The ID of the job.

### Updates to Response
The `ApplicantDto` now includes a `status` field, indicating the current state of the application (`PENDING`, `ACCEPTED`, `REJECTED`, etc.).

### Response Example (200 OK)
```json
{
  "success": true,
  "message": "Applicants retrieved successfully",
  "data": [
    {
      "application_id": "app-uuid",
      "user_id": "user-uuid",
      "resume_url": "https://s3.aws.com/resume.pdf",
      "cover_letter": "I would love to work here...",
      "status": "PENDING",
      "applied_at": "2026-05-17T04:10:00Z"
    }
  ]
}
```

---

## 3. Update Application Status
Update the status of a specific job application. Only the creator of the job can perform this action.

- **Endpoint**: `PATCH /api/v1/jobs/{job_id}/applications/{application_id}/status`
- **Auth Required**: Yes (Bearer Token)
- **Role**: Broker/Associate (Must be the creator of the job)

### Path Parameters
- `job_id` (UUID): The ID of the job.
- `application_id` (UUID): The ID of the application.

### Request Body
```json
{
  "status": "ACCEPTED" 
}
```
*Note: We recommend using standard statuses like `ACCEPTED`, `REJECTED`, or `SELECTED`. The status will be stored in uppercase.*

### Response Example (200 OK)
```json
{
  "success": true,
  "message": "Application status updated to ACCEPTED",
  "data": {
    "status": "ACCEPTED"
  }
}
```

### Important Automation Trigger
When the application status is updated to **`ACCEPTED`**, the backend automatically performs the following actions:
1. **Chat Initiation**: Creates a chat session (if it doesn't already exist) between the broker and the applicant.
2. **First Message**: Sends an automated initial message: *"Congratulations! Your application for '{job_title}' has been accepted. Let's chat."*
3. **Notification**: Sends an in-app notification of type `JOB` to the applicant with the same message.

---

## 4. Automatic System Notifications (Background)
The frontend team doesn't need to manually trigger notifications; they are handled by the backend:
- **On Job Posted (`POST /api/v1/jobs`)**: The broker will receive a `JOB` type notification confirming their job is active.
- **On Job Applied (`POST /api/v1/jobs/{job_id}/apply`)**: The broker (job creator) will receive a `JOB` type notification alerting them of a new applicant.
