# Admin Zero Deposit & NBFC Partners API Documentation

This document outlines the API endpoints for Admin Zero Deposit operations and NBFC Partner management. 

**Base URL**: `http://<server-ip>:9090`  
**Authentication**: All endpoints require an Admin JWT token passed in the `Authorization` header.

---

## 1. Zero Deposit Application Management

### 1.1 List Zero Deposit Applications
List or filter all Zero Deposit applications.

- **URL**: `/api/admin/zero-deposit`
- **Method**: `GET`
- **Query Parameters**:
  - `status` (optional, string): e.g., `applied`, `pending_review`, `blocked`, `approved`, `rejected`
  - `limit` (optional, integer)
  - `offset` (optional, integer)

**Response** (200 OK):
```json
{
  "success": true,
  "data": [
    {
      "id": "uuid",
      "user_id": "uuid",
      "property_id": "uuid",
      "monthly_rent": 15000.0,
      "requested_deposit_amount": 30000.0,
      "status": "applied",
      "created_at": "2026-09-17T00:00:00Z"
    }
  ]
}
```

### 1.2 Get Zero Deposit Details
Fetch full details of a specific Zero Deposit application, including applicant info, property details, and KYC documents.

- **URL**: `/api/admin/zero-deposit/:id`
- **Method**: `GET`

**Response** (200 OK):
```json
{
  "success": true,
  "data": {
    "id": "uuid",
    "user": {
      "first_name": "John",
      "last_name": "Doe",
      "email": "john@example.com",
      "phone": "9876543210"
    },
    "property_title": "2BHK Apartment in Koramangala",
    "kyc_document_urls": ["url1", "url2"],
    "monthly_rent": 15000.0,
    "requested_deposit_amount": 30000.0,
    "monthly_income": 80000.0,
    "status": "applied",
    "nbfc_id": null,
    "approved_deposit_amount": null,
    "is_defaulted": false
  }
}
```

### 1.3 Approve Zero Deposit
Marks a Zero Deposit application as approved internally (pre-NBFC assignment).

- **URL**: `/api/admin/zero-deposit/:id/approve`
- **Method**: `PATCH`
- **Body**:
```json
{
  "approved_deposit_amount": 30000.0,
  "remarks": "Income documents verified."
}
```

**Response** (200 OK):
```json
{
  "success": true,
  "message": "Zero Deposit approved"
}
```

### 1.4 Reject Zero Deposit
Rejects a Zero Deposit application.

- **URL**: `/api/admin/zero-deposit/:id/reject`
- **Method**: `PATCH`
- **Body**:
```json
{
  "reason": "Insufficient income."
}
```

**Response** (200 OK):
```json
{
  "success": true,
  "message": "Zero Deposit rejected"
}
```

### 1.5 Assign to NBFC Partner
Assigns an approved Zero Deposit application to an NBFC partner for underwriting.

- **URL**: `/api/admin/zero-deposit/:id/assign-nbfc`
- **Method**: `PATCH`
- **Body**:
```json
{
  "nbfc_id": "uuid"
}
```

**Response** (200 OK):
```json
{
  "success": true,
  "message": "Assigned to NBFC"
}
```

---

## 2. Zero Deposit Ledger & Defaults

### 2.1 Get Ledger Entries
Fetch the transaction ledger for a specific Zero Deposit application.

- **URL**: `/api/admin/zero-deposit/:id/ledger`
- **Method**: `GET`

**Response** (200 OK):
```json
{
  "success": true,
  "data": [
    {
      "id": "uuid",
      "transaction_type": "disbursement",
      "amount": 30000.0,
      "reference_id": "REF123",
      "description": "Initial disbursement",
      "created_at": "2026-09-17T00:00:00Z"
    }
  ]
}
```

### 2.2 List Defaulted Zero Deposits
List applications that have been marked as defaulted/delinquent.

- **URL**: `/api/admin/zero-deposit/defaults`
- **Method**: `GET`
- **Query Parameters**:
  - `limit` (optional, integer)
  - `offset` (optional, integer)

**Response** (200 OK):
```json
{
  "success": true,
  "data": [
    {
      "id": "uuid",
      "user_id": "uuid",
      "is_defaulted": true,
      "default_remarks": "Missed 3 payments",
      "status": "blocked"
    }
  ]
}
```

### 2.3 Suspend Defaulted Account
Suspends a defaulted Zero Deposit account (blocks the user from further actions).

- **URL**: `/api/admin/zero-deposit/:id/default/suspend`
- **Method**: `PATCH`
- **Body**:
```json
{
  "remarks": "Suspending account due to non-payment"
}
```

### 2.4 Forfeit Deposit/Collateral
Marks deposit/collateral as forfeited.

- **URL**: `/api/admin/zero-deposit/:id/default/forfeit`
- **Method**: `PATCH`
- **Body**:
```json
{
  "remarks": "Forfeiting collateral"
}
```

### 2.5 Record Recovery
Records a manual recovery against a defaulted Zero Deposit application. This inserts a `repayment` entry into the ledger.

- **URL**: `/api/admin/zero-deposit/:id/default/recover`
- **Method**: `PATCH`
- **Body**:
```json
{
  "recovered_amount": 5000.0,
  "remarks": "Recovered via collection agency"
}
```

---

## 3. NBFC Partner Management

### 3.1 List NBFC Partners
List all onboarded NBFC/Bank partners.

- **URL**: `/api/admin/nbfc-partners`
- **Method**: `GET`

**Response** (200 OK):
```json
[
  {
    "id": "uuid",
    "name": "Acme Bank",
    "entity_type": "Bank",
    "revenue_share_pct": 2.5,
    "fldg_buffer_pct": 5.0,
    "status": "active",
    "created_at": "2026-09-17T00:00:00Z",
    "updated_at": "2026-09-17T00:00:00Z"
  }
]
```

### 3.2 Onboard New NBFC Partner
Onboard a new NBFC/Bank partner.

- **URL**: `/api/admin/nbfc-partners`
- **Method**: `POST`
- **Body**:
```json
{
  "name": "Acme Bank",
  "entity_type": "Bank",
  "revenue_share_pct": 2.5,
  "fldg_buffer_pct": 5.0
}
```

**Response** (200 OK):
```json
{
  "success": true,
  "data": {
    "id": "uuid",
    "name": "Acme Bank",
    "status": "active"
  }
}
```

### 3.3 Update NBFC Partner Status
Update the status of an existing NBFC partner (`active`, `inactive`, `suspended`).

- **URL**: `/api/admin/nbfc-partners/:id`
- **Method**: `PATCH`
- **Body**:
```json
{
  "status": "suspended"
}
```

**Response** (200 OK):
```json
{
  "success": true,
  "message": "NBFC Partner updated"
}
```
