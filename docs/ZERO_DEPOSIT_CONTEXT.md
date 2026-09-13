# Zero Deposit Application System Context

## Overview
This document outlines the architecture, entity structure, and file locations for the newly added Zero Deposit feature in Livana Backend V2.

## Entity Structure: `ZeroDeposit`
The central entity for the zero deposit module is the `ZeroDepositApplication` (or simply `ZeroDeposit`).

### Fields
- **id**: `Uuid` (Primary Key)
- **user_id**: `Uuid` (Foreign Key -> `users.id`, Owner of the zero deposit application)
- **property_id**: `Uuid` (Foreign Key -> `properties.id`, Optional or Required depending on flow)
- **kyc_id**: `Uuid` (Foreign Key -> `kyc_submissions.id`)
- **monthly_rent**: `f64` / `Numeric`
- **requested_deposit_amount**: `f64` / `Numeric`
- **monthly_income**: `f64` / `Numeric`
- **itr_document_url**: `String` (Optional, URL to S3)
- **bank_statement_url**: `String` (Optional, URL to S3)
- **consent_given**: `bool` (Default: `false`, records if credit bureau check consent is provided)
- **status**: `ZeroDepositStatus` (Enum)
- **created_at**: `DateTime<Utc>`
- **updated_at**: `DateTime<Utc>`

### Enums & Status Values
**ZeroDepositStatus** (`zero_deposit_status` type in Postgres):
- `applied`: Initial status when the application is submitted.
- `pending_review`: Status indicating the application is under review.
- `blocked`: Status when consent is denied or other blocking issues arise.
- `approved`: Status when the zero deposit application is approved (future use).
- `rejected`: Status when the zero deposit application is rejected (future use).
- `fee_pending`: Status when the processing fee charge is initiated but awaiting confirmation.

## Entity Structure: `FeeTransaction` (Issue 57)
To support processing fees, a related fee transaction entity will be introduced.
### Fields
- **id**: `Uuid` (Primary Key)
- **zero_deposit_id**: `Uuid` (Foreign Key -> `zero_deposits.id`)
- **upi_vpa**: `String`
- **amount**: `f64` / `Numeric`
- **status**: `FeeTransactionStatus` (Enum: `pending`, `success`, `failed`)
- **created_at**: `DateTime<Utc>`
- **updated_at**: `DateTime<Utc>`

> **Note on Webhook Dependency (Issue 85):**
> The `FeeTransaction` record will initially be created with a `pending` status. It will remain pending until a webhook (to be implemented in Issue 85) receives confirmation from the payment gateway and updates the transaction to `success` or `failed`.

## Entity Structure: `AutopayMandate` (Issue 58)
Stores the automated repayment setup details.
### Fields
- **id**: `Uuid` (Primary Key)
- **zero_deposit_id**: `Uuid` (Foreign Key -> `zero_deposits.id`)
- **mandate_type**: `String` (e.g., 'UPI_AUTOPAY', 'NACH')
- **upi_vpa**: `String` (Optional, if type is UPI)
- **bank_account_number**: `String` (Optional, if type is NACH)
- **ifsc**: `String` (Optional, if type is NACH)
- **status**: `String` / Enum (`pending`, `active`, `failed`)
- **created_at**: `DateTime<Utc>`
- **updated_at**: `DateTime<Utc>`

## Entity Structure: `RepaymentSchedule` (Issue 59)
Represents EMI-style repayment due dates and amounts.
### Fields
- **id**: `Uuid` (Primary Key)
- **zero_deposit_id**: `Uuid` (Foreign Key -> `zero_deposits.id`)
- **installment_number**: `i32`
- **due_date**: `NaiveDate` / `DateTime<Utc>`
- **amount_due**: `f64` / `Numeric`
- **status**: `String` / Enum (`pending`, `paid`, `overdue`)
- **created_at**: `DateTime<Utc>`
- **updated_at**: `DateTime<Utc>`

## Entity Structure: `LedgerEntry` (Issue 60)
Records every financial transaction affecting the zero deposit application.
### Fields
- **id**: `Uuid` (Primary Key)
- **zero_deposit_id**: `Uuid` (Foreign Key -> `zero_deposits.id`)
- **transaction_type**: `String` / Enum (`disbursement`, `repayment`, `fee`, `refund`)
- **amount**: `f64` / `Numeric`
- **reference_id**: `String` (External gateway tx ID)
- **description**: `String`
- **created_at**: `DateTime<Utc>`

## Naming Convention
- **Strict Adherence:** All code, variables, endpoints, and database tables MUST use the terms "Zero Deposit", "zero deposit", or "zero_deposit".
- **Forbidden:** The term "loan" must NOT be used anywhere in this module.

## File Locations
- **Model**: `src/models/zero_deposit.rs` (Contains `ZeroDepositApplication`, `ZeroDepositStatus`, and DTOs like `ApplyZeroDepositRequest`, `CreditCheckConsentRequest`)
- **Handler**: `src/handlers/zero_deposit.rs` (Contains endpoint logic, response mapping, and auth validation using `require_auth`)
- **Service**: `src/services/zero_deposit_service.rs` (Contains DB queries, business logic, status transitions)
- **Routes**: `src/routes/zero_deposit.rs` (Defines the Axum `Router` and endpoint mappings: POST `/api/v1/zero-deposit/apply`, POST `/api/v1/zero-deposit/credit-check/consent`, GET `/api/v1/zero-deposit/{zero_deposit_id}`)
- **Migration**: `migrations/XXXXXX_create_zero_deposits_table.sql` (Creates the `zero_deposit_status` enum and `zero_deposits` table)

## Conventions Identified & Followed
- **Auth**: Use the existing `require_auth(&headers, &app_state.jwt_secret)` helper inside handlers or `jwt_middleware` in the route definition.
- **Responses**: Return standard JSON format:
  ```json
  {
    "success": true,
    "message": "...",
    "data": { ... }
  }
  ```
- **Error Handling**: Log errors with `log::error!` and return proper `error_code` strings along with HTTP status codes (e.g., `INTERNAL_SERVER_ERROR`, `BAD_REQUEST`, `UNAUTHORIZED`).
- **Data Validation**: Model fields will be strictly parsed (e.g., `Uuid::parse_str`).
