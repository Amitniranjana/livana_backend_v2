# Loan Application System Context

## Overview
This document outlines the architecture, entity structure, and file locations for the newly added Loan feature in Livana Backend V2.

## Entity Structure: `Loan`
The central entity for the loan module is the `LoanApplication` (or simply `Loan`).

### Fields
- **id**: `Uuid` (Primary Key)
- **user_id**: `Uuid` (Foreign Key -> `users.id`, Owner of the loan)
- **property_id**: `Uuid` (Foreign Key -> `properties.id`, Optional or Required depending on flow)
- **kyc_id**: `Uuid` (Foreign Key -> `kyc_submissions.id`)
- **monthly_rent**: `f64` / `Numeric`
- **requested_deposit_amount**: `f64` / `Numeric`
- **monthly_income**: `f64` / `Numeric`
- **itr_document_url**: `String` (Optional, URL to S3)
- **bank_statement_url**: `String` (Optional, URL to S3)
- **consent_given**: `bool` (Default: `false`, records if credit bureau check consent is provided)
- **status**: `LoanStatus` (Enum)
- **created_at**: `DateTime<Utc>`
- **updated_at**: `DateTime<Utc>`

### Enums & Status Values
**LoanStatus** (`loan_status` type in Postgres):
- `applied`: Initial status when the application is submitted.
- `pending_review`: Status indicating the application is under review.
- `blocked`: Status when consent is denied or other blocking issues arise.
- `approved`: Status when the loan is approved (future use).
- `rejected`: Status when the loan is rejected (future use).

## File Locations
- **Model**: `src/models/loan.rs` (Contains `LoanApplication`, `LoanStatus`, and DTOs like `ApplyLoanRequest`, `CreditCheckConsentRequest`)
- **Handler**: `src/handlers/loan.rs` (Contains endpoint logic, response mapping, and auth validation using `require_auth`)
- **Service**: `src/services/loan_service.rs` (Contains DB queries, business logic, status transitions)
- **Routes**: `src/routes/loan.rs` (Defines the Axum `Router` and endpoint mappings: POST `/api/v1/loans/apply`, POST `/api/v1/loans/credit-check/consent`, GET `/api/v1/loans/{loan_id}`)
- **Migration**: `migrations/XXXXXX_create_loans_table.sql` (Creates the `loan_status` enum and `loans` table)

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
