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

---

## 4. Get My Zero Deposits
Retrieve all zero deposit applications created by the currently authenticated user.

- **Endpoint**: `GET /api/v1/zero-deposit/me`
- **Auth Required**: `Bearer <token>`

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Zero deposits retrieved successfully",
  "data": {
    "zero_deposits": [
      {
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
    ]
  }
}
```

---

## 5. Polling: Get Zero Deposit Status
Retrieve a lightweight status payload for a specific zero deposit application. Useful for the frontend to poll during long-running background processes (like fee confirmations).

- **Endpoint**: `GET /api/v1/zero-deposit/{application_id}/status`
- **Auth Required**: `Bearer <token>` (Must be the owner of the zero deposit application)

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Status retrieved successfully",
  "data": {
    "zero_deposit": {
      "id": "123e4567-e89b-12d3-a456-426614174002",
      "status": "fee_pending",
      "updated_at": "2026-09-10T14:20:00Z"
    }
  }
}
```

---

## 6. Initiate Processing Fee Charge
Initiate a processing fee charge using UPI for a given zero deposit application. This will create a pending fee transaction and update the application status to `fee_pending` while awaiting a webhook confirmation.

- **Endpoint**: `POST /api/v1/zero-deposit/{application_id}/fee/charge`
- **Auth Required**: `Bearer <token>` (Must be the owner of the zero deposit application)
- **Content-Type**: `application/json`

### Request Body
```json
{
  "upi_vpa": "user@upi"
}
```

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Fee charge initiated successfully",
  "data": {
    "fee_transaction": {
      "id": "987e6543-e21b-12d3-a456-426614174111",
      "zero_deposit_id": "123e4567-e89b-12d3-a456-426614174002",
      "upi_vpa": "user@upi",
      "amount": 500.0,
      "status": "pending",
      "created_at": "2026-09-10T14:20:00Z",
      "updated_at": "2026-09-10T14:20:00Z"
    }
  }
}
```

---

## 7. Setup Autopay Mandate
Initiate the setup of an autopay mandate for the zero deposit application.

- **Endpoint**: `POST /api/v1/zero-deposit/{application_id}/autopay/setup`
- **Auth Required**: `Bearer <token>` (Must be the owner of the zero deposit application)
- **Content-Type**: `application/json`

### Request Body (UPI Autopay)
```json
{
  "mandate_type": "UPI_AUTOPAY",
  "upi_vpa": "user@upi"
}
```

### Request Body (NACH)
```json
{
  "mandate_type": "NACH",
  "bank_account_number": "1234567890",
  "ifsc": "HDFC0001234"
}
```

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Autopay mandate setup initiated",
  "data": {
    "autopay_mandate": {
      "id": "111e4567-e89b-12d3-a456-426614174000",
      "zero_deposit_id": "123e4567-e89b-12d3-a456-426614174002",
      "mandate_type": "UPI_AUTOPAY",
      "upi_vpa": "user@upi",
      "bank_account_number": null,
      "ifsc": null,
      "status": "pending",
      "created_at": "2026-09-11T14:20:00Z",
      "updated_at": "2026-09-11T14:20:00Z"
    }
  }
}
```

---

## 8. Get Repayment Schedule
Retrieve the EMI-style repayment schedule for the zero deposit application.

- **Endpoint**: `GET /api/v1/zero-deposit/{application_id}/repayment-schedule`
- **Auth Required**: `Bearer <token>` (Must be the owner of the zero deposit application)

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Repayment schedule retrieved successfully",
  "data": {
    "repayment_schedule": [
      {
        "id": "222e4567-e89b-12d3-a456-426614174000",
        "zero_deposit_id": "123e4567-e89b-12d3-a456-426614174002",
        "installment_number": 1,
        "due_date": "2026-10-01",
        "amount_due": 15000.0,
        "status": "pending",
        "created_at": "2026-09-11T14:20:00Z",
        "updated_at": "2026-09-11T14:20:00Z"
      }
    ]
  }
}
```

---

## 9. Get Ledger
Retrieve the full transaction ledger (disbursements, repayments, fees, refunds) for the zero deposit application.

- **Endpoint**: `GET /api/v1/zero-deposit/{application_id}/ledger`
- **Auth Required**: `Bearer <token>` (Must be the owner of the zero deposit application)

### Success Response (200 OK)
```json
{
  "success": true,
  "message": "Ledger retrieved successfully",
  "data": {
    "ledger": [
      {
        "id": "333e4567-e89b-12d3-a456-426614174000",
        "zero_deposit_id": "123e4567-e89b-12d3-a456-426614174002",
        "transaction_type": "fee",
        "amount": 500.0,
        "reference_id": "txn_abc123",
        "description": "Processing fee charge",
        "created_at": "2026-09-11T14:20:00Z"
      }
    ]
  }
}
```
