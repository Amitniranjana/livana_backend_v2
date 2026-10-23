# Livana Backend — Issues 96 & 97 Frontend Integration Guide

This guide is broken into two distinct parts:
- **Part 1: Applicant Flow (Client-Side)** — How the loan application form submits device identifiers, computes bank account hashes, and handles velocity rate limits.
- **Part 2: Admin Dashboard Flow** — How to consume fraud signals, display anomaly badges/alerts, trigger batch scans, and inspect velocity limits.

---

## Part 1: Applicant Flow (Client-Side Integration)

### 1.1 Loan Application Endpoint

- **Method**: `POST`
- **Path**: `/api/v1/loans/apply` (or `/api/v1/zero-deposit/apply`)
- **Authentication**: `Bearer <user_jwt_token>`

#### Required Headers
| Header | Type | Description |
|---|---|---|
| `Authorization` | `String` | Bearer token from user login/session |
| `X-Device-Id` | `String` | **Recommended:** Unique device fingerprint generated on the client (FingerprintJS / device UUID). |
| `Content-Type` | `String` | `application/json` |

---

### 1.2 Request Body (`ApplyZeroDepositRequest`)

```json
{
  "property_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d",
  "kyc_id": "4a2b9d31-9f93-4927-b08e-16a8d67a1234",
  "monthly_rent": 25000,
  "requested_deposit_amount": 50000,
  "monthly_income": 75000,
  "stated_employer": "Tata Consultancy Services",
  "device_fingerprint": "fp_8a7d2e9f4c1b3a5d",
  "bank_account_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "bank_statement_url": "https://s3.ap-south-1.amazonaws.com/.../statement.pdf",
  "itr_document_url": "https://s3.ap-south-1.amazonaws.com/.../itr.pdf"
}
```

#### Field Specifications
| Field | Type | Required | Description |
|---|---|---|---|
| `property_id` | `UUID String` | No | ID of the property being rented (if linked). |
| `kyc_id` | `UUID String` | Yes | ID of the applicant's verified or submitted KYC record. |
| `monthly_rent` | `Number` | Yes | Monthly rent amount in INR. |
| `requested_deposit_amount` | `Number` | Yes | Deposit loan amount requested in INR. |
| `monthly_income` | `Number` | Yes | Applicant's declared monthly net income in INR. |
| `stated_employer` | `String` | No | Employer / company name (aliases accepted: `employer_name`, `company_name`). |
| `device_fingerprint` | `String` | No | Device identifier (redundant with `X-Device-Id` for maximum compatibility). |
| `bank_account_hash` | `String` | No | Client-side normalized SHA-256 hash (see helper below). Raw account numbers are **never** stored. |
| `bank_statement_url` | `String` | No | URL to uploaded bank statement PDF in S3. |
| `itr_document_url` | `String` | No | URL to uploaded ITR/Form-16 document in S3. |

---

### 1.3 Client-Side Helper: Generating `bank_account_hash`

To protect applicant PII while enabling cross-application deduplication, compute a SHA-256 hash using `Web Crypto API` (works in all modern browsers and React Native):

```typescript
/**
 * Normalizes IFSC and Account Number and generates a SHA-256 hex string.
 * Format: sha256("ifsc:account_number") in lowercase with whitespace stripped.
 */
export async function computeBankAccountHash(ifsc: string, accountNumber: string): Promise<string> {
  const normalized = `${ifsc.trim().toLowerCase()}:${accountNumber.trim().toLowerCase()}`;
  const msgUint8 = new TextEncoder().encode(normalized);
  const hashBuffer = await crypto.subtle.digest('SHA-256', msgUint8);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
}
```

---

### 1.4 Handling Rate Limits & Velocity Throttling (HTTP 429)

If an applicant tries to apply too many times from the same device or IP address within 1 hour:

#### Error Response (`HTTP 429 Too Many Requests`)
```json
{
  "success": false,
  "message": "Too many loan applications from this device. Please wait before trying again.",
  "error_code": "VELOCITY_LIMIT_EXCEEDED",
  "limits": {
    "device": {
      "max_applications": 3,
      "window_seconds": 3600
    },
    "ip": {
      "max_applications": 5,
      "window_seconds": 3600
    }
  }
}
```

#### Frontend UI Handling Recommendation
```typescript
if (response.status === 429) {
  const err = await response.json();
  if (err.error_code === 'VELOCITY_LIMIT_EXCEEDED') {
    toast.error(
      "Application rate limit reached. For security reasons, please wait up to 1 hour before submitting another loan application."
    );
  }
}
```

---

## Part 2: Admin Dashboard Flow (Fraud & Anomaly Management)

All admin endpoints require an **Admin JWT Token**:
`Authorization: Bearer <admin_jwt_token>`

---

### 2.1 View All Flagged Applications & Summary
- **Method**: `GET`
- **Path**: `/api/admin/loans/fraud-signals`

#### Response Shape
```json
{
  "success": true,
  "data": {
    "summary": {
      "total_flagged_applications": 8,
      "by_code": [
        { "code": "SHARED_DEVICE", "severity": "HIGH", "occurrences": 3 },
        { "code": "INCOME_EMPLOYER_INCONSISTENT", "severity": "HIGH", "occurrences": 2 },
        { "code": "SHARED_BANK_ACCOUNT", "severity": "HIGH", "occurrences": 2 },
        { "code": "INCOME_RENT_MISMATCH", "severity": "MEDIUM", "occurrences": 1 }
      ]
    },
    "applications": [
      {
        "zero_deposit_id": "c7a8e124-7f12-482a-a92c-5df189012345",
        "user_id": "123e4567-e89b-12d3-a456-426614174000",
        "applicant_name": "Rohan Sharma",
        "applicant_email": "rohan.sharma@example.com",
        "applicant_phone": "+919876543210",
        "stated_employer": "Google India",
        "monthly_income": 35000,
        "monthly_rent": 30000,
        "requested_deposit_amount": 60000,
        "status": "applied",
        "fraud_signals": [
          {
            "code": "INCOME_EMPLOYER_INCONSISTENT",
            "label": "Income proof inconsistent with stated employer",
            "detail": "Applicant stated employer 'Google India' with monthly income INR 35,000, which is below market salary benchmarks. Bank statement analysis also shows zero salary credits and irregular cash transfers.",
            "severity": "HIGH"
          },
          {
            "code": "SHARED_DEVICE",
            "label": "Device shared across multiple applicants",
            "detail": "The device fingerprint used for this application matches 3 other applicant identities. Possible synthetic identity ring or account takeover.",
            "severity": "HIGH"
          }
        ],
        "fraud_flagged_at": "2026-10-23T04:10:00Z",
        "device_fingerprint": "fp_8a7d2e9f4c1b3a5d",
        "has_bank_account_hash": true,
        "applicant_ip": "103.21.244.1",
        "created_at": "2026-10-23T04:09:45Z",
        "risk_tier": "HIGH",
        "probability_of_default": 0.742
      }
    ]
  }
}
```

---

### 2.2 Trigger On-Demand Batch Scan
- **Method**: `POST`
- **Path**: `/api/admin/loans/fraud-signals/scan`
- **Description**: Re-scans all loan applications from the last 90 days against device, bank-account, Gemini employer consistency, and velocity rules.

#### Response Shape
```json
{
  "success": true,
  "message": "Batch anomaly detection scan completed successfully",
  "data": {
    "scanned_applications": 142,
    "flagged_applications": 9,
    "scanned_at": "2026-10-23T04:18:22.102Z"
  }
}
```

---

### 2.3 Read Velocity Thresholds
- **Method**: `GET`
- **Path**: `/api/admin/loans/velocity-config`
- **Description**: Surfaces active sliding-window configuration to display on admin settings / compliance pages.

#### Response Shape
```json
{
  "success": true,
  "data": {
    "device": {
      "max_applications": 3,
      "window_seconds": 3600,
      "window_human": "1 hour"
    },
    "ip": {
      "max_applications": 5,
      "window_seconds": 3600,
      "window_human": "1 hour"
    },
    "note": "When Redis is available counters are tracked in-memory with TTL. When Redis is absent, Postgres loan_apply_events is used as a sliding-window counter."
  }
}
```

---

### 2.4 Fraud Signal Reference & UI Badge Guide

| Signal Code | Severity | Recommended UI Badge | Meaning |
|---|---|---|---|
| `SHARED_BANK_ACCOUNT` | **HIGH** | Red badge (`bg-red-600`) | Same bank account hash linked to another applicant account (mule / ring). |
| `SHARED_DEVICE` | **HIGH** / **MEDIUM** | Red / Orange badge | Device fingerprint shared across $\ge 3$ (HIGH) or 1-2 (MEDIUM) identities. |
| `INCOME_EMPLOYER_INCONSISTENT` | **HIGH** | Red badge (`bg-red-600`) | Gemini AI detected mismatch between stated company, declared salary, and bank proof. |
| `INCOME_PROOF_MISSING` | **MEDIUM** | Yellow badge (`bg-amber-500`) | Employment declared without attached bank statement or ITR. |
| `INCOME_RENT_MISMATCH` | **HIGH** / **MEDIUM** | Red / Orange badge | Stated income $< 1.5\times$ rent obligation. |
| `RAPID_REAPPLICATION` | **HIGH** / **MEDIUM** | Orange badge (`bg-orange-500`) | Same user identity applied $\ge 2$ times in 24 hours. |
