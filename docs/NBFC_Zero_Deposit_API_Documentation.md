# NBFC Partner API Documentation

This document outlines the API endpoints available for the NBFC (Non-Banking Financial Company) Partner Dashboard. These endpoints allow NBFC underwriters to log in, view, and process Zero Deposit applications assigned to them.

**Base URL**: `http://<server-ip>:9090/api/nbfc`  

---

## 1. NBFC Authentication

### 1.1 NBFC Login
Authenticates an NBFC user (underwriter or admin) and returns an NBFC-scoped JWT. This JWT must be included in the `Authorization` header as a Bearer token for all subsequent requests.

- **URL**: `/auth/login`
- **Method**: `POST`
- **Auth Required**: No
- **Content-Type**: `application/json`

#### Request Body
```json
{
  "email": "underwriter@acmebank.com",
  "password": "securepassword123"
}
```

#### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Login successful",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI..."
}
```

#### Error Responses
- **401 Unauthorized**: Invalid credentials or suspended account.

---

## 2. Zero Deposit Portfolio Management

**Authentication**: All endpoints below require the NBFC JWT token passed in the `Authorization` header (`Bearer <token>`). The server automatically derives the assigned `nbfc_id` from the token to securely scope data access.

### 2.1 List Assigned Zero Deposits
Retrieve a paginated list of Zero Deposit applications assigned to the authenticated NBFC partner.

- **URL**: `/zero-deposits`
- **Method**: `GET`
- **Auth Required**: Yes (`Bearer <token>`)

#### Query Parameters
- `status` (optional, string): Filter by status (e.g., `applied`, `pending_review`, `approved`, `rejected`).
- `limit` (optional, integer): Number of records to return (default: 20).
- `offset` (optional, integer): Number of records to skip (default: 0).

#### Request Example
`GET /api/nbfc/zero-deposits?status=pending_review&limit=10&offset=0`

#### Success Response (200 OK)
```json
{
  "success": true,
  "data": [
    {
      "id": "123e4567-e89b-12d3-a456-426614174000",
      "user_id": "987e6543-e21b-12d3-a456-426614174111",
      "property_id": "555e4567-e89b-12d3-a456-426614174222",
      "kyc_id": "111e4567-e89b-12d3-a456-426614174333",
      "monthly_rent": 15000.0,
      "requested_deposit_amount": 45000.0,
      "monthly_income": 80000.0,
      "itr_document_url": "https://s3.amazonaws.com/.../itr.pdf",
      "bank_statement_url": "https://s3.amazonaws.com/.../bank.pdf",
      "consent_given": true,
      "status": "pending_review",
      "created_at": "2026-09-22T10:15:30Z",
      "updated_at": "2026-09-22T10:15:30Z"
    }
  ]
}
```

### 2.2 Get Zero Deposit Details
Retrieve full details of a specific Zero Deposit application for underwriting review. The server ensures the requested application belongs to the authenticated NBFC.

- **URL**: `/zero-deposits/:id`
- **Method**: `GET`
- **Auth Required**: Yes (`Bearer <token>`)

#### Path Parameters
- `id` (UUID): The unique identifier of the Zero Deposit application.

#### Request Example
`GET /api/nbfc/zero-deposits/123e4567-e89b-12d3-a456-426614174000`

#### Success Response (200 OK)
```json
{
  "success": true,
  "data": {
    "id": "123e4567-e89b-12d3-a456-426614174000",
    "user_id": "987e6543-e21b-12d3-a456-426614174111",
    "property_id": "555e4567-e89b-12d3-a456-426614174222",
    "kyc_id": "111e4567-e89b-12d3-a456-426614174333",
    "monthly_rent": 15000.0,
    "requested_deposit_amount": 45000.0,
    "monthly_income": 80000.0,
    "itr_document_url": "https://s3.amazonaws.com/.../itr.pdf",
    "bank_statement_url": "https://s3.amazonaws.com/.../bank.pdf",
    "consent_given": true,
    "status": "pending_review",
    "created_at": "2026-09-22T10:15:30Z",
    "updated_at": "2026-09-22T10:15:30Z"
  }
}
```

#### Error Responses
- **401 Unauthorized**: Missing or invalid token.
- **404 Not Found**: If the zero deposit ID does not exist, or if it is assigned to a *different* NBFC partner. The response will explicitly state `"Zero Deposit not found or not assigned to your NBFC"`.
