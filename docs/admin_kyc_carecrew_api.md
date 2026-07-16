# Livana Backend APIs - Frontend Documentation

This document outlines the endpoints recently implemented for the **Admin KYC Flow** and the **CareCrew Directory**.

---

## 1. Get All KYC Submissions
Retrieves a paginated list of all KYC submissions. Supports filtering by status and role.

- **Endpoint:** `GET /api/admin/kyc`
- **Auth Required:** Yes (Admin Role)
- **Query Parameters:**
  - `status` (string, optional): Filter by verification status (`pending`, `verified`, `rejected`, `all`).
  - `user_role` (string, optional): Filter by user role (`builder`, `broker`, `associate`, `user`).
  - `limit` (number, optional): Number of records per page (default: 10, max: 100).
  - `offset` (number, optional): Offset for pagination (default: 0).

**Response (200 OK):**
```json
{
  "success": true,
  "message": "KYC submissions fetched successfully",
  "data": {
    "kyc_records": [
      {
        "id": "uuid",
        "user_id": "uuid",
        "name": "John Doe",
        "role": "builder",
        "submitted_docs_summary": "aadhaar, Experience Doc",
        "status": "pending",
        "submitted_at": "2026-07-17T02:00:00Z"
      }
    ],
    "pagination": {
      "total_count": 50,
      "limit": 10,
      "offset": 0
    }
  }
}
```

---

## 2. Get KYC Submission Details
Retrieves the full details for a single KYC submission, including uploaded documents and any linked professional profiles (builder/broker).

- **Endpoint:** `GET /api/admin/kyc/{kyc_id}`
- **Auth Required:** Yes (Admin Role)
- **Path Parameters:**
  - `kyc_id` (uuid): The ID of the KYC submission.

**Response (200 OK):**
```json
{
  "success": true,
  "message": "KYC detail fetched successfully",
  "data": {
    "id": "uuid",
    "user_id": "uuid",
    "role": "builder",
    "full_name": "John Doe",
    "mobile_number": "1234567890",
    "email_id": "john@example.com",
    "gender": "Male",
    "date_of_birth": "1990-01-01",
    "profile_picture_url": "https://s3-url.com/profile.jpg",
    "address": "123 Main St",
    "govt_id_type": "aadhaar",
    "govt_id_number": "XXXX-XXXX-XXXX",
    "govt_id_document_url": "https://s3-url.com/doc.jpg",
    "company_name": "Doe Builders",
    "services": null,
    "experience_document_url": "https://s3-url.com/exp.pdf",
    "verification_status": "pending",
    "submitted_at": "2026-07-17T02:00:00Z",
    "verified_at": null,
    "reviewed_by": null,
    "reviewed_at": null,
    "rejection_reason": null,
    "linked_profile": {
      "profile_type": "builder",
      "company_name": "Doe Builders",
      "experience_years": 5
    }
  }
}
```

---

## 3. Approve KYC Submission
Approves a KYC submission. This updates the status to `verified`, stamps the `reviewed_by` and `reviewed_at` fields with the current admin's details, creates an admin audit log, and triggers a notification to the user.

- **Endpoint:** `PATCH /api/admin/kyc/{kyc_id}/approve`
- **Auth Required:** Yes (Admin Role)
- **Path Parameters:**
  - `kyc_id` (uuid): The ID of the KYC submission.

**Response (200 OK):**
```json
{
  "success": true,
  "message": "KYC approved successfully"
}
```

---

## 4. Reject KYC Submission
Rejects a KYC submission. This updates the status to `rejected`, saves the reason, stamps the review fields, creates an admin audit log, and triggers a notification to the user.

- **Endpoint:** `PATCH /api/admin/kyc/{kyc_id}/reject`
- **Auth Required:** Yes (Admin Role)
- **Body:**
```json
{
  "reason": "Document unclear or invalid"
}
```

**Response (200 OK):**
```json
{
  "success": true,
  "message": "KYC rejected successfully"
}
```

---

## 5. CareCrew Directory
Lists all active CareCrew members (Users where `role = 'associate'` and `associate_type = 'carecrew'`). If the request includes authentication headers, private contact information (phone, email) is included.

- **Endpoint:** `GET /api/carecrew`
- **Auth Required:** Optional (Public lists members, Authenticated returns full contact details)
- **Query Parameters:**
  - `city` (string, optional): Search/filter by city.
  - `service_type` (string, optional): Search/filter by the type of service offered.
  - `limit` (number, optional): Records per page (default: 10).
  - `page` (number, optional): Page number (default: 1).
  - `offset` (number, optional): Raw offset (if provided, overrides `page`).

**Response (200 OK):**
```json
{
  "success": true,
  "message": "CareCrew members retrieved successfully",
  "data": {
    "members": [
      {
        "id": "uuid",
        "name": "Jane Smith",
        "photo": "https://s3-url.com/jane.jpg",
        "city": "Mumbai",
        "services_offered": ["Plumbing", "Cleaning"],
        "rating": 4.5,
        "verified_kyc_status": "verified",
        "phone": "9876543210", 
        "email": "jane@example.com" 
      }
    ],
    "pagination": {
      "total_count": 100,
      "limit": 10,
      "offset": 0
    }
  }
}
```
> [!NOTE]
> `phone` and `email` properties inside `members` will only be returned if a valid JWT token is provided in the `Authorization` header. Otherwise, they will be `null`.
