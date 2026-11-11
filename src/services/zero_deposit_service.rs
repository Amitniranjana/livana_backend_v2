use crate::models::zero_deposit::{ApplyZeroDepositRequest, ZeroDepositApplication};
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

pub async fn apply_for_zero_deposit(
    db: &Pool<Postgres>,
    user_id: Uuid,
    property_id: Option<Uuid>,
    kyc_id: Uuid,
    payload: &ApplyZeroDepositRequest,
    deposit_amount: f64,
    fee_amount: f64,
    device_id: Option<&str>,
    client_ip: Option<&str>,
) -> Result<ZeroDepositApplication, sqlx::Error> {
    let documents_val = payload
        .documents
        .as_ref()
        .and_then(|d| serde_json::to_value(d).ok())
        .or_else(|| payload.document_urls.clone())
        .unwrap_or(serde_json::json!({}));

    let occupants_val = payload
        .occupants
        .clone()
        .unwrap_or(serde_json::json!([]));

    let zero_deposit = sqlx::query_as::<_, ZeroDepositApplication>(
        r#"
        INSERT INTO zero_deposits (
            user_id, property_id, kyc_id, monthly_rent, requested_deposit_amount,
            deposit_amount, fee_amount, monthly_income, tenancy_months, employment_type,
            employer_name, current_job_start_date, total_experience_months, business_start_date,
            variable_pay_monthly, is_repeat_renter, occupants, document_urls,
            device_fingerprint, bank_account_hash, applicant_ip, stated_employer,
            status, consent_given
        )
        VALUES (
            $1, $2, $3, $4, $5,
            $5, $6, $7, $8, $9,
            $10, $11, $12, $13,
            $14, $15, $16, $17,
            $18, $19, $20::INET, $21,
            'applied'::zero_deposit_status, false
        )
        RETURNING
            id, user_id, property_id, kyc_id,
            monthly_rent::FLOAT8 as monthly_rent,
            requested_deposit_amount::FLOAT8 as requested_deposit_amount,
            monthly_income::FLOAT8 as monthly_income,
            itr_document_url, bank_statement_url, consent_given,
            status, created_at, updated_at,
            tenancy_months, employment_type, employer_name, current_job_start_date,
            total_experience_months, business_start_date,
            variable_pay_monthly::FLOAT8 as variable_pay_monthly,
            is_repeat_renter, occupants, document_urls,
            deposit_amount::FLOAT8 as deposit_amount,
            fee_amount::FLOAT8 as fee_amount,
            pending_actions, bureau_summary
        "#
    )
    .bind(user_id)
    .bind(property_id)
    .bind(kyc_id)
    .bind(payload.monthly_rent)
    .bind(deposit_amount)
    .bind(fee_amount)
    .bind(payload.declared_monthly_income)
    .bind(payload.tenancy_months)
    .bind(&payload.employment_type)
    .bind(&payload.employer_name)
    .bind(payload.current_job_start_date)
    .bind(payload.total_experience_months)
    .bind(payload.business_start_date)
    .bind(payload.variable_pay_monthly)
    .bind(payload.is_repeat_renter)
    .bind(occupants_val)
    .bind(documents_val)
    .bind(device_id)
    .bind(payload.bank_account_hash.as_deref())
    .bind(client_ip)
    .bind(payload.stated_employer.as_deref().or(payload.employer_name.as_deref()))
    .fetch_one(db)
    .await?;

    Ok(zero_deposit)
}

pub enum ConsentUpdateResult {
    Success(ZeroDepositApplication),
    NotOwner,
    NotFound,
}

pub async fn update_consent(
    db: &Pool<Postgres>,
    zero_deposit_id: Uuid,
    caller_user_id: Uuid,
    consent_given: bool,
) -> Result<ConsentUpdateResult, sqlx::Error> {
    // 1. Check ownership
    let owner_row = sqlx::query("SELECT user_id FROM zero_deposits WHERE id = $1")
        .bind(zero_deposit_id)
        .fetch_optional(db)
        .await?;

    let owner_id: Uuid = match owner_row {
        Some(r) => r.get("user_id"),
        None => return Ok(ConsentUpdateResult::NotFound),
    };

    if owner_id != caller_user_id {
        return Ok(ConsentUpdateResult::NotOwner);
    }

    let status_str = if consent_given {
        "ai_review"
    } else {
        "blocked"
    };

    let zero_deposit = sqlx::query_as::<_, ZeroDepositApplication>(
        r#"
        UPDATE zero_deposits 
        SET consent_given = $1, status = $2::zero_deposit_status, updated_at = NOW()
        WHERE id = $3 AND user_id = $4
        RETURNING 
            id, user_id, property_id, kyc_id, 
            monthly_rent::FLOAT8 as monthly_rent,
            requested_deposit_amount::FLOAT8 as requested_deposit_amount,
            monthly_income::FLOAT8 as monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, created_at, updated_at,
            tenancy_months, employment_type, employer_name, current_job_start_date,
            total_experience_months, business_start_date,
            variable_pay_monthly::FLOAT8 as variable_pay_monthly,
            is_repeat_renter, occupants, document_urls,
            deposit_amount::FLOAT8 as deposit_amount,
            fee_amount::FLOAT8 as fee_amount,
            pending_actions, bureau_summary
        "#
    )
    .bind(consent_given)
    .bind(status_str)
    .bind(zero_deposit_id)
    .bind(caller_user_id)
    .fetch_one(db)
    .await?;

    Ok(ConsentUpdateResult::Success(zero_deposit))
}

pub async fn get_zero_deposit_by_id(
    db: &Pool<Postgres>,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<ZeroDepositApplication>, sqlx::Error> {
    let row = sqlx::query_as::<_, ZeroDepositApplication>(
        r#"
        SELECT 
            id, user_id, property_id, kyc_id, 
            monthly_rent::FLOAT8 as monthly_rent,
            requested_deposit_amount::FLOAT8 as requested_deposit_amount,
            monthly_income::FLOAT8 as monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, created_at, updated_at,
            tenancy_months, employment_type, employer_name, current_job_start_date,
            total_experience_months, business_start_date,
            variable_pay_monthly::FLOAT8 as variable_pay_monthly,
            is_repeat_renter, occupants, document_urls,
            deposit_amount::FLOAT8 as deposit_amount,
            fee_amount::FLOAT8 as fee_amount,
            pending_actions, bureau_summary
        FROM zero_deposits 
        WHERE id = $1 AND user_id = $2
        "#
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(row)
}

pub async fn get_my_zero_deposits(
    db: &Pool<Postgres>,
    user_id: Uuid,
) -> Result<Vec<ZeroDepositApplication>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ZeroDepositApplication>(
        r#"
        SELECT 
            id, user_id, property_id, kyc_id, 
            monthly_rent::FLOAT8 as monthly_rent,
            requested_deposit_amount::FLOAT8 as requested_deposit_amount,
            monthly_income::FLOAT8 as monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, created_at, updated_at,
            tenancy_months, employment_type, employer_name, current_job_start_date,
            total_experience_months, business_start_date,
            variable_pay_monthly::FLOAT8 as variable_pay_monthly,
            is_repeat_renter, occupants, document_urls,
            deposit_amount::FLOAT8 as deposit_amount,
            fee_amount::FLOAT8 as fee_amount,
            pending_actions, bureau_summary
        FROM zero_deposits 
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn get_zero_deposits_by_user_id(
    db: &Pool<Postgres>,
    user_id: Uuid,
) -> Result<Vec<ZeroDepositApplication>, sqlx::Error> {
    get_my_zero_deposits(db, user_id).await
}

pub async fn get_zero_deposit_status(
    db: &Pool<Postgres>,
    zero_deposit_id: Uuid,
    user_id: Uuid,
) -> Result<Option<crate::models::zero_deposit::ZeroDepositStatusResponse>, sqlx::Error> {
    let status_response = sqlx::query_as::<_, crate::models::zero_deposit::ZeroDepositStatusResponse>(
        r#"
        SELECT id, status, updated_at
        FROM zero_deposits
        WHERE id = $1 AND user_id = $2
        "#
    )
    .bind(zero_deposit_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(status_response)
}

pub async fn initiate_fee_charge(
    db: &Pool<Postgres>,
    zero_deposit_id: Uuid,
    user_id: Uuid,
    upi_vpa: &str,
    amount: f64,
) -> Result<Option<crate::models::zero_deposit::FeeTransaction>, sqlx::Error> {
    let mut tx = db.begin().await?;

    let existing = sqlx::query(
        "SELECT id FROM zero_deposits WHERE id = $1 AND user_id = $2 FOR UPDATE"
    )
    .bind(zero_deposit_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    if existing.is_none() {
        return Ok(None);
    }

    let fee_transaction = sqlx::query_as::<_, crate::models::zero_deposit::FeeTransaction>(
        r#"
        INSERT INTO fee_transactions (zero_deposit_id, upi_vpa, amount, status)
        VALUES ($1, $2, $3, 'pending')
        RETURNING id, zero_deposit_id, upi_vpa, amount::FLOAT8 as amount, status, created_at, updated_at
        "#
    )
    .bind(zero_deposit_id)
    .bind(upi_vpa)
    .bind(amount)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Some(fee_transaction))
}

pub async fn setup_autopay_mandate(
    db: &Pool<Postgres>,
    zero_deposit_id: Uuid,
    user_id: Uuid,
    payload: &crate::models::zero_deposit::SetupAutopayRequest,
) -> Result<Option<crate::models::zero_deposit::AutopayMandate>, sqlx::Error> {
    let existing = sqlx::query(
        "SELECT id FROM zero_deposits WHERE id = $1 AND user_id = $2"
    )
    .bind(zero_deposit_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    if existing.is_none() {
        return Ok(None);
    }

    let mandate = sqlx::query_as::<_, crate::models::zero_deposit::AutopayMandate>(
        r#"
        INSERT INTO autopay_mandates (zero_deposit_id, mandate_type, upi_vpa, bank_account_number, ifsc, status)
        VALUES ($1, $2::mandate_type, $3, $4, $5, 'pending')
        RETURNING id, zero_deposit_id, mandate_type, upi_vpa, bank_account_number, ifsc, status, created_at, updated_at
        "#
    )
    .bind(zero_deposit_id)
    .bind(match payload.mandate_type {
        crate::models::zero_deposit::MandateType::UpiAutopay => "UPI_AUTOPAY",
        crate::models::zero_deposit::MandateType::Nach => "NACH",
    })
    .bind(&payload.upi_vpa)
    .bind(&payload.bank_account_number)
    .bind(&payload.ifsc)
    .fetch_one(db)
    .await?;

    Ok(Some(mandate))
}

pub async fn get_repayment_schedule(
    db: &Pool<Postgres>,
    zero_deposit_id: Uuid,
    user_id: Uuid,
) -> Result<Option<Vec<crate::models::zero_deposit::RepaymentSchedule>>, sqlx::Error> {
    let existing = sqlx::query(
        "SELECT id FROM zero_deposits WHERE id = $1 AND user_id = $2"
    )
    .bind(zero_deposit_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    if existing.is_none() {
        return Ok(None);
    }

    let schedule = sqlx::query_as::<_, crate::models::zero_deposit::RepaymentSchedule>(
        r#"
        SELECT id, zero_deposit_id, installment_number, due_date, amount_due::FLOAT8 as amount_due, status, created_at, updated_at
        FROM repayment_schedules
        WHERE zero_deposit_id = $1
        ORDER BY installment_number ASC
        "#
    )
    .bind(zero_deposit_id)
    .fetch_all(db)
    .await?;

    Ok(Some(schedule))
}

pub async fn get_ledger_entries(
    db: &Pool<Postgres>,
    zero_deposit_id: Uuid,
    user_id: Uuid,
) -> Result<Option<Vec<crate::models::zero_deposit::LedgerEntry>>, sqlx::Error> {
    let existing = sqlx::query(
        "SELECT id FROM zero_deposits WHERE id = $1 AND user_id = $2"
    )
    .bind(zero_deposit_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    if existing.is_none() {
        return Ok(None);
    }

    let entries = sqlx::query_as::<_, crate::models::zero_deposit::LedgerEntry>(
        r#"
        SELECT id, zero_deposit_id, transaction_type, amount::FLOAT8 as amount, reference_id, description, created_at
        FROM ledger_entries
        WHERE zero_deposit_id = $1
        ORDER BY created_at DESC
        "#
    )
    .bind(zero_deposit_id)
    .fetch_all(db)
    .await?;

    Ok(Some(entries))
}
