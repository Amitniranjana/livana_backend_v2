# Livana Zero Deposit (Credit Tokens) v2 — Operations & Deployment Guide

> **Audience**: Backend Engineers, DevOps, Underwriting Admins, Legal & Compliance
> **Backend Version**: LivanaEco Axum Backend v2 (`release/zero-deposit-v2`)
> **Status**: Production-Ready / Pre-Launch

---

## 1. Regulatory & Legal Pre-Flight Notice

> [!CAUTION]
> **LEGAL / REGULATORY MANDATE — CONFIRM WITH LEGAL BEFORE LAUNCH**:
> The regulatory position of Livana funding security deposits directly on behalf of tenants without a partner NBFC is subject to local lending regulations and rental tenancy legislation. Prior to activating live payouts (`auto_issue_enabled: true` or admin disbursements), written legal confirmation must be obtained from counsel regarding RBI Digital Lending Guidelines and Non-Banking Financial Company exemptions.

---

## 2. Launch Configuration & Safe Staging

### 2.1 Recommended Initial Launch Setting: Manual Review Mode
For the first 2–4 weeks after commercial launch, it is strongly recommended to set:
```json
{
  "auto_issue_enabled": false
}
```
When `auto_issue_enabled` is set to `false`, **every single application** is automatically routed to `ai_manual_review` for human underwriter sign-off, regardless of whether AI rules evaluated to `auto_approve`. This allows the risk team to observe scoring distributions, verify document checks, and calibrate rules without exposure.

To verify or update this setting via the Admin API:
```bash
# Fetch live configuration
GET /api/admin/zero-deposit/ai-config
Authorization: Bearer <ADMIN_JWT>

# Set manual review mode
PATCH /api/admin/zero-deposit/ai-config
Authorization: Bearer <ADMIN_JWT>
Content-Type: application/json

{
  "auto_issue_enabled": false
}
```

---

## 3. Configuration Reference (`zd_ai_config`)

The table `zd_ai_config` stores the live operational thresholds used by the deterministic rules engine and redemption workflows:

| Key | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `auto_issue_enabled` | `boolean` | `true` (use `false` at launch) | Enables automated token issuance on Tier A/B passes. |
| `age_min` | `integer` | `21` | Minimum eligible tenant age (from KYC DOB). |
| `age_max` | `integer` | `55` | Maximum eligible tenant age. |
| `min_tenancy_months` | `integer` | `11` | Minimum tenancy agreement duration. |
| `deposit_rent_multiple`| `number` | `3.0` | Token multiple applied to monthly rent (Deposit = 3 × Rent). |
| `fee_flat_amount` | `number` | `2000.00` | Flat fee in ₹ added to first month's rent. |
| `income_multiple` | `object` | `{"salaried": 3.0, "self_employed": 3.0, "gig": 3.5, "student_co_applicant": 3.0}` | Minimum income required as multiple of rent. |
| `income_haircut_pct` | `object` | `{"self_employed": 50, "gig": 50, "variable_pay": 50}` | Haircut applied to non-fixed income streams. |
| `max_obligations_pct` | `number` | `50.0` | Maximum obligations cap (% of net monthly income). |
| `max_rent_pct` | `number` | `35.0` | Maximum rent-to-income ratio. |
| `min_cibil_score` | `integer` | `650` | Hard threshold; < 650 auto-rejects (`CIBIL_BELOW_MINIMUM`). |
| `co_applicant_min_cibil`| `integer` | `700` | Minimum score for guarantor/co-applicant. |
| `co_applicant_window_days`| `integer` | `7` | Days before pending co-applicant invite auto-rejects. |
| `max_enquiries_3m` | `integer` | `4` | Max bureau credit enquiries in last 90 days. |
| `tiers` | `object` | `{"A": {"min_score": 750, "max_rent": 40000}, "B": {"min_score": 700, "max_rent": 25000}, "C": {"min_score": 650, "max_rent": 15000}}` | Credit tiers and property rent ceilings. |
| `tier_upgrade_after_months` | `integer` | `6` | Consecutive on-time rent payments to trigger auto-upgrade. |
| `maker_checker_enabled` | `boolean` | `true` | Requires different admins for `approve` and `mark-released`. |
| `token_validity_days` | `integer` | `30` | **Immutable.** Token validity window (fixed at 30 days). |

---

## 4. Background Jobs & Scheduled Workers

The backend background worker (`crate::services::zd_jobs::start_zd_background_workers`) spawns an asynchronous Tokio daemon on server startup:

```
┌────────────────────────────────────────────────────────┐
│           Tokio Background Scheduler Daemon            │
├──────────────────┬──────────────────┬──────────────────┤
│ Every 15 Minutes │   Every 1 Hour   │  Every 24 Hours  │
│  - Token Expiry  │ - Co-Applicant   │  - Nightly Tier  │
│  - Warning Notifs│    7-Day Timeout │    Recalculation │
└──────────────────┴──────────────────┴──────────────────┘
```

1. **Token Expiry Job (Every 15 minutes)**:
   - Queries `zd_token_allocations` past `NOW() > expires_at`.
   - Executes `zd_tokens::expire` in an atomic SQL transaction with `SELECT ... FOR UPDATE`.
   - Closes abandoned `docs_pending` redemptions as `expired`.
   - Moves applications with 0 locked and 0 redeemed balances to `expired`.
   - *Protected Invariant*: Redemptions submitted before Day 30 maintain their locked balances until reviewed.
2. **Expiry Warning Notifications**:
   - Sent automatically at **7 days** and **2 days** before allocation expiration.
3. **Co-Applicant Timeout Job (Every 1 hour)**:
   - Identifies applications in `action_required` waiting on co-applicants for > 7 days.
   - Transitions application to `rejected` with audit reason code `CO_APPLICANT_EXPIRED`.
4. **Nightly Tier Recalculation Job (Every 24 hours / 00:00 UTC)**:
   - Upgrades tenants up one tier after 6 months of consecutive on-time rent.
   - Downgrades tenants one tier and resets streak on any late payment in the last 30 days.
   - *Preservation Rule*: Never overrides manual tier changes made by an admin (`source = 'admin'`).

---

## 5. Webhooks & Cryptographic Secrets

The backend interacts with external and internal microservices via HMAC-SHA256 signatures:

### 5.1 Environment Variables
Set the following secrets in your production environment (`.env` or secret manager):

```ini
# Zero Deposit AI Engine
ZD_AI_SERVICE_URL=https://ai.livana.eco
ZD_AI_SERVICE_TOKEN=your-high-entropy-ai-bearer-token
ZD_AI_HMAC_SECRET=your-shared-hmac-secret-at-least-32-chars

# Base URL for AI Callback Webhooks
API_BASE_URL=https://api.livana.eco

# Admin & Authentication
JWT_SECRET_KEY=your-production-tenant-jwt-secret
ADMIN_JWT_SECRET=your-production-admin-jwt-secret
```

### 5.2 Callback Verification Headers
Incoming callbacks to:
- `POST /internal/zero-deposit/assessments/{id}/result`
- `POST /internal/zero-deposit/document-checks/{id}/result`

Require:
1. `X-Signature`: Hex-encoded `HMAC-SHA256(secret=ZD_AI_HMAC_SECRET, body=raw_request_payload)`.
2. `X-Timestamp`: Unix epoch timestamp in seconds. Requests older than **300 seconds (5 minutes)** are rejected with `401 BAD_HMAC` to prevent replay attacks.

---

## 6. Underwriter & Operations Runbook

### 6.1 Overriding an Application
When an application is in `ai_manual_review` or auto-rejected:
- Admin can review underwriter signals: `GET /api/admin/zero-deposit/{id}/assessment`.
- Admin can override: `PATCH /api/admin/zero-deposit/{id}/assessment/override`:
  ```json
  {
    "decision": "approve",
    "tier": "B",
    "reason": "Salary slips cross-verified with bank branch manager directly."
  }
  ```
- Override automatically issues 3 × rent tokens (`source: admin_override`) and logs old and new states in the immutable audit log.

### 6.2 Maker-Checker Payout Protocol
To prevent internal fraud, payout disbursement requires two distinct administrative actors:
1. **Admin 1 (Maker)**:
   `PATCH /api/admin/zero-deposit/redemptions/{id}/approve`
   Enters verified landlord bank details and locks tokens. Recorded as `approved_by`.
2. **Admin 2 (Checker)**:
   `PATCH /api/admin/zero-deposit/redemptions/{id}/mark-released`
   Header: `Idempotency-Key: <UUID>`
   Transfers funds via external corporate banking, inputs bank UTR.
   *System rejects with `403 MAKER_CHECKER` if Admin 2 is identical to Admin 1.*

---

## 7. Monthly Reporting Queries
Underwriting performance and regulatory fair-housing audits can be retrieved via:
```bash
GET /api/admin/zero-deposit/reports/monthly?year=2026&month=11
Authorization: Bearer <ADMIN_JWT>
```
Returns approval, rejection, and pending volumes grouped by property city and applicant income bracket (`< ₹25k`, `₹25k - ₹50k`, `₹50k - ₹100k`, `> ₹100k`).
