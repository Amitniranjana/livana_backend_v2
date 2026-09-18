use crate::models::zero_deposit::ZeroDepositApplication;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

pub async fn apply_for_zero_deposit(
    db: &Pool<Postgres>,
    user_id: Uuid,
    property_id: Option<Uuid>,
    kyc_id: Uuid,
    monthly_rent: f64,
    requested_deposit_amount: f64,
    monthly_income: f64,
    itr_document_url: Option<&str>,
    bank_statement_url: Option<&str>,
) -> Result<ZeroDepositApplication, sqlx::Error> {
    let zero_deposit = sqlx::query_as::<_, ZeroDepositApplication>(
        r#"
        INSERT INTO zero_deposits (
            user_id, property_id, kyc_id, monthly_rent, requested_deposit_amount, 
            monthly_income, itr_document_url, bank_statement_url, status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'applied')
        RETURNING 
            id, user_id, property_id, kyc_id, 
            monthly_rent::FLOAT8 as monthly_rent, requested_deposit_amount::FLOAT8 as requested_deposit_amount, monthly_income::FLOAT8 as monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, 
            created_at, updated_at
        "#
    )
    .bind(user_id)
    .bind(property_id)
    .bind(kyc_id)
    .bind(monthly_rent)
    .bind(requested_deposit_amount)
    .bind(monthly_income)
    .bind(itr_document_url)
    .bind(bank_statement_url)
    .fetch_one(db)
    .await?;

    Ok(zero_deposit)
}

pub async fn update_consent(
    db: &Pool<Postgres>,
    zero_deposit_id: Uuid,
    user_id: Uuid,
    consent_given: bool,
) -> Result<Option<ZeroDepositApplication>, sqlx::Error> {
    let status_str = if consent_given {
        "pending_review"
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
            monthly_rent::FLOAT8 as monthly_rent, requested_deposit_amount::FLOAT8 as requested_deposit_amount, monthly_income::FLOAT8 as monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, 
            created_at, updated_at
        "#
    )
    .bind(consent_given)
    .bind(status_str)
    .bind(zero_deposit_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(zero_deposit)
}

pub async fn get_zero_deposit_by_id(
    db: &Pool<Postgres>,
    zero_deposit_id: Uuid,
    user_id: Uuid,
) -> Result<Option<ZeroDepositApplication>, sqlx::Error> {
    let zero_deposit = sqlx::query_as::<_, ZeroDepositApplication>(
        r#"
        SELECT 
            id, user_id, property_id, kyc_id, 
            monthly_rent::FLOAT8 as monthly_rent, requested_deposit_amount::FLOAT8 as requested_deposit_amount, monthly_income::FLOAT8 as monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, 
            created_at, updated_at
        FROM zero_deposits
        WHERE id = $1 AND user_id = $2
        "#
    )
    .bind(zero_deposit_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(zero_deposit)
}

pub async fn get_my_zero_deposits(
    db: &Pool<Postgres>,
    user_id: Uuid,
) -> Result<Vec<ZeroDepositApplication>, sqlx::Error> {
    let zero_deposits = sqlx::query_as::<_, ZeroDepositApplication>(
        r#"
        SELECT 
            id, user_id, property_id, kyc_id, 
            monthly_rent::FLOAT8 as monthly_rent, requested_deposit_amount::FLOAT8 as requested_deposit_amount, monthly_income::FLOAT8 as monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, 
            created_at, updated_at
        FROM zero_deposits
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(zero_deposits)
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
    // Start transaction
    let mut tx = db.begin().await?;

    // 1. Verify ownership and lock row
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

    // 2. Update status to fee_pending
    sqlx::query(
        "UPDATE zero_deposits SET status = 'fee_pending', updated_at = NOW() WHERE id = $1"
    )
    .bind(zero_deposit_id)
    .execute(&mut *tx)
    .await?;

    // 3. Create fee transaction
    let fee_transaction = sqlx::query_as::<_, crate::models::zero_deposit::FeeTransaction>(
        r#"
        INSERT INTO fee_transactions (zero_deposit_id, upi_vpa, amount, status)
        VALUES ($1, $2, $3, 'pending')
        RETURNING id, zero_deposit_id, upi_vpa, amount, status, created_at, updated_at
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
    // Verify ownership
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
    // Verify ownership
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
        SELECT id, zero_deposit_id, installment_number, due_date, amount_due, status, created_at, updated_at
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
    // Verify ownership
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
        SELECT id, zero_deposit_id, transaction_type, amount, reference_id, description, created_at
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
