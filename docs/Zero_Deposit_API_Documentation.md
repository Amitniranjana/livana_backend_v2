# Zero Deposit Application API Documentation

This document outlines the endpoints available for the Zero Deposit Application module (Issues 52, 53, 54). All endpoints are prefixed with `/api/v1/zero-deposit` and require a valid JWT token.

---

## 1. Apply for a Zero Deposit
Submit a new zero deposit application. The initial status of the zero deposit will be set to `applied`.

- **Endpoint**: `POST /api/v1/zero-deposit/apply`
- **Auth Required**: `Bearer <token>`
- **Content-Type**: `application/json`

### Request Body
```json
{
  "property_id": "123e4567-e89b-12d3-a456-426614174000", // Optional (can be omitted or set to null)
  "kyc_id": "123e4567-e89b-12d3-a456-426614174001",
  "monthly_rent": 15000.0,
  "requested_deposit_amount": 45000.0,
  "monthly_income": 80000.0,
  "itr_document_url": "https://s3.amazonaws.com/.../itr.pdf", // Optional
  "bank_statement_url": "https://s3.amazonaws.com/.../bank.pdf" // Optional
}
```

### Success Response (201 Created)
```json
{
  "success": true,
  "message": "Zero deposit application submitted successfully",
  "data": {
    "zero_deposit": {
      "id": "123e4567-e89b-12d3-a456-426614174002",
      "user_id": "123e4567-e89b-12d3-a456-426614174003",
      "property_id": "123e4567-e89b-12d3-a456-426614174000",
      "kyc_id": "123e4567-e89b-12d3-a456-426614174001",
      "monthly_rent": 15000.0,
      "requested_deposit_amount": 45000.0,
      "monthly_income": 80000.0,
      "itr_document_url": "https://s3.amazonaws.com/.../itr.pdf",
      "bank_statement_url": "https://s3.amazonaws.com/.../bank.pdf",
      "consent_given": false,
      "status": "applied",
      "created_at": "2026-09-08T10:15:30Z",
      "updated_at": "2026-09-08T10:15:30Z"
    }
  }
}
```

---

## 2. Submit Credit Check Consent
Update the `consent_given` flag for the credit bureau check. This will transition the zero deposit status.

- **Endpoint**: `POST /api/v1/zero-deposit/credit-check/consent`
- **Auth Required**: `Bearer <token>` (Must be the owner of the zero deposit application)
- **Content-Type**: `application/json`

### Workflow Transitions
- If `consent_given` is `true` -> Status becomes `pending_review`.
- If `consent_given` is `false` -> Status becomes `blocked`.

### Request Body
```json
{
  "zero_deposit_id": "123e4567-e89b-12d3-a456-426614174002",
  "consent_given": true
}
```

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Zero deposit consent updated successfully",
  "data": {
    "zero_deposit": {
      "id": "123e4567-e89b-12d3-a456-426614174002",
      "consent_given": true,
      "status": "pending_review",
      "...": "other fields"
    }
  }
}
```

---

## 3. Get Zero Deposit Details
Retrieve the full details of a specific zero deposit application.

- **Endpoint**: `GET /api/v1/zero-deposit/{zero_deposit_id}`
- **Auth Required**: `Bearer <token>` (Must be the owner of the zero deposit application)

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Zero deposit application retrieved successfully",
  "data": {
    "zero_deposit": {
      "id": "123e4567-e89b-12d3-a456-426614174002",
      "user_id": "123e4567-e89b-12d3-a456-426614174003",
      "property_id": "123e4567-e89b-12d3-a456-426614174000",
      "kyc_id": "123e4567-e89b-12d3-a456-426614174001",
      "monthly_rent": 15000.0,
      "requested_deposit_amount": 45000.0,
      "monthly_income": 80000.0,
      "itr_document_url": "https://s3.amazonaws.com/.../itr.pdf",
      "bank_statement_url": "https://s3.amazonaws.com/.../bank.pdf",
      "consent_given": true,
      "status": "pending_review",
      "created_at": "2026-09-08T10:15:30Z",
      "updated_at": "2026-09-08T10:20:00Z"
    }
  }
}
```

### Error Responses
- **401 Unauthorized**: If the token is invalid or missing.
- **404 Not Found**: If the zero deposit ID is invalid, does not exist, or the user does not own the zero deposit application.
