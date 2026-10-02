# Zero Deposit API Documentation

This document outlines the API endpoints for the **Zero Deposit** feature. These endpoints are meant to be consumed by the Frontend (Mobile/Web) applications, with a dedicated section at the bottom for NBFC Partner Webhooks.

---

## 1. Apply for Zero Deposit

Submits a new Zero Deposit application.

- **Endpoint:** `POST /api/v1/zero-deposit/apply`
- **Auth Required:** Yes (`Bearer <JWT>`)

### Request Body (JSON)
```json
{
  "property_id": "UUID-String", // Optional
  "kyc_id": "UUID-String",
  "monthly_rent": 15000.00,
  "requested_deposit_amount": 30000.00,
  "monthly_income": 60000.00,
  "itr_document_url": "https://s3.url/doc.pdf", // Optional
  "bank_statement_url": "https://s3.url/statement.pdf" // Optional
}
```

### Response
- **201 Created**
```json
{
  "success": true,
  "message": "zero_deposit application submitted successfully",
  "data": {
    "zero_deposit": {
      "id": "...",
      "status": "applied",
      "monthly_rent": 15000.0,
      ...
    }
  }
}
```

---

## 2. Submit Credit Check Consent

Records user consent for the NBFC credit check. If true, changes status to `pending_review`, otherwise `blocked`.

- **Endpoint:** `POST /api/v1/zero-deposit/credit-check/consent`
- **Auth Required:** Yes (`Bearer <JWT>`)

### Request Body (JSON)
```json
{
  "zero_deposit_id": "UUID-String",
  "consent_given": true
}
```

### Response
- **200 OK**
```json
{
  "success": true,
  "message": "zero_deposit consent updated successfully",
  "data": { "zero_deposit": { ... } }
}
```

---

## 3. Get My Zero Deposits

Retrieves all zero deposit applications created by the authenticated user.

- **Endpoint:** `GET /api/v1/zero-deposit/me`
- **Auth Required:** Yes (`Bearer <JWT>`)

### Response
- **200 OK**
```json
{
  "success": true,
  "message": "Zero deposits retrieved successfully",
  "data": {
    "zero_deposits": [ { ... } ]
  }
}
```

---

## 4. Get Zero Deposit Details

Retrieves details for a specific zero deposit application.

- **Endpoint:** `GET /api/v1/zero-deposit/{zero_deposit_id}`
- **Auth Required:** Yes (`Bearer <JWT>`)

### Response
- **200 OK**
```json
{
  "success": true,
  "message": "zero_deposit retrieved successfully",
  "data": { "zero_deposit": { ... } }
}
```

---

## 5. Get Zero Deposit Status

Quickly check the status of a specific application.

- **Endpoint:** `GET /api/v1/zero-deposit/{application_id}/status`
- **Auth Required:** Yes (`Bearer <JWT>`)

### Response
- **200 OK**
```json
{
  "success": true,
  "message": "Status retrieved successfully",
  "data": {
    "zero_deposit": {
      "id": "...",
      "status": "approved", // applied, pending_review, blocked, approved, rejected, fee_pending, disbursed
      "updated_at": "..."
    }
  }
}
```

---

## 6. Charge Processing Fee

Initiates a fee charge for an approved zero deposit (flips status to `fee_pending`). Currently configured for a flat fee of ₹500.

- **Endpoint:** `POST /api/v1/zero-deposit/{application_id}/fee/charge`
- **Auth Required:** Yes (`Bearer <JWT>`)

### Request Body (JSON)
```json
{
  "upi_vpa": "user@upi"
}
```

### Response
- **200 OK**
```json
{
  "success": true,
  "message": "Fee charge initiated successfully",
  "data": {
    "fee_transaction": {
      "id": "...",
      "amount": 500.0,
      "status": "pending",
      ...
    }
  }
}
```

---

## 7. Setup Autopay Mandate

Configures EMI autopay mandates.

- **Endpoint:** `POST /api/v1/zero-deposit/{application_id}/autopay/setup`
- **Auth Required:** Yes (`Bearer <JWT>`)

### Request Body (JSON)
Depending on the mandate type, provide the respective fields:

**For UPI Autopay:**
```json
{
  "mandate_type": "UPI_AUTOPAY",
  "upi_vpa": "user@upi"
}
```

**For NACH:**
```json
{
  "mandate_type": "NACH",
  "bank_account_number": "1234567890",
  "ifsc": "HDFC0001234"
}
```

### Response
- **200 OK**
```json
{
  "success": true,a
  "message": "Autopay mandate setup initiated",
  "data": { "autopay_mandate": { ... } }
}
```

---

## 8. Get Repayment Schedule

Retrieves the monthly repayment installments for an active zero deposit.

- **Endpoint:** `GET /api/v1/zero-deposit/{application_id}/repayment-schedule`
- **Auth Required:** Yes (`Bearer <JWT>`)

### Response
- **200 OK**
```json
{
  "success": true,
  "message": "Repayment schedule retrieved successfully",
  "data": {
    "repayment_schedule": [
      {
        "id": "...",
        "installment_number": 1,
        "due_date": "2026-11-01",
        "amount_due": 3000.0,
        "status": "pending" // pending, paid, overdue
      }
    ]
  }
}
```

---

## 9. Get Ledger

Retrieves all financial transactions (disbursements, repayments, fees, refunds) associated with the deposit.

- **Endpoint:** `GET /api/v1/zero-deposit/{application_id}/ledger`
- **Auth Required:** Yes (`Bearer <JWT>`)

### Response
- **200 OK**
```json
{
  "success": true,
  "message": "Ledger retrieved successfully",
  "data": {
    "ledger": [
      {
        "transaction_type": "disbursement", // disbursement, repayment, fee, refund
        "amount": 30000.0,
        "description": "Disbursement to property owner",
        "created_at": "..."
      }
    ]
  }
}
```

---

## NBFC Partner Webhooks (Backend-to-Backend Only)

**Note for Frontend:** These endpoints are called by NBFC Partners, not by the client applications. They use HMAC-SHA256 signatures for authentication via the `x-webhook-signature` header.

1. **Disbursement Status:**
   - `POST /api/v1/zero-deposit/webhook/disbursement-status`
   - Body: `{ "zero_deposit_id": "...", "status": "disbursed", "amount": 30000.0, "disbursed_at": "..." }`

2. **Repayment Status:**
   - `POST /api/v1/zero-deposit/webhook/repayment-status`
   - Body: `{ "zero_deposit_id": "...", "repayment_id": "...", "due_date": "2026-11-01", "amount_paid": 3000.0, "status": "paid", "paid_at": "..." }`

3. **Fee Status:**
   - `POST /api/v1/zero-deposit/webhook/fee-status`
   - Body: `{ "zero_deposit_id": "...", "upi_txn_ref": "TXN123", "status": "success" }`
