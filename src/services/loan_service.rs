use crate::models::loan::{LoanApplication, LoanStatus};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

pub async fn apply_for_loan(
    db: &Pool<Postgres>,
    user_id: Uuid,
    property_id: Option<Uuid>,
    kyc_id: Uuid,
    monthly_rent: f64,
    requested_deposit_amount: f64,
    monthly_income: f64,
    itr_document_url: Option<&str>,
    bank_statement_url: Option<&str>,
) -> Result<LoanApplication, sqlx::Error> {
    let loan = sqlx::query_as::<_, LoanApplication>(
        r#"
        INSERT INTO loans (
            user_id, property_id, kyc_id, monthly_rent, requested_deposit_amount, 
            monthly_income, itr_document_url, bank_statement_url, status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'applied')
        RETURNING 
            id, user_id, property_id, kyc_id, 
            monthly_rent, requested_deposit_amount, monthly_income, 
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

    Ok(loan)
}

pub async fn update_consent(
    db: &Pool<Postgres>,
    loan_id: Uuid,
    user_id: Uuid,
    consent_given: bool,
) -> Result<Option<LoanApplication>, sqlx::Error> {
    let status_str = if consent_given {
        "pending_review"
    } else {
        "blocked"
    };

    let loan = sqlx::query_as::<_, LoanApplication>(
        r#"
        UPDATE loans 
        SET consent_given = $1, status = $2::loan_status, updated_at = NOW()
        WHERE id = $3 AND user_id = $4
        RETURNING 
            id, user_id, property_id, kyc_id, 
            monthly_rent, requested_deposit_amount, monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, 
            created_at, updated_at
        "#
    )
    .bind(consent_given)
    .bind(status_str)
    .bind(loan_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(loan)
}

pub async fn get_loan_by_id(
    db: &Pool<Postgres>,
    loan_id: Uuid,
    user_id: Uuid,
) -> Result<Option<LoanApplication>, sqlx::Error> {
    let loan = sqlx::query_as::<_, LoanApplication>(
        r#"
        SELECT 
            id, user_id, property_id, kyc_id, 
            monthly_rent, requested_deposit_amount, monthly_income, 
            itr_document_url, bank_statement_url, consent_given, 
            status, 
            created_at, updated_at
        FROM loans
        WHERE id = $1 AND user_id = $2
        "#
    )
    .bind(loan_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(loan)
}
