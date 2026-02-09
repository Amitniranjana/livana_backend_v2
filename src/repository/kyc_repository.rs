use sqlx::PgPool;
use uuid::Uuid;
use crate::models::kyc::{KycSubmission, KycStatus};

#[derive(Clone, Debug)]
pub struct KycRepository {
    pool: PgPool,
}

impl KycRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, submission: KycSubmission) -> Result<KycSubmission, String> {
        let result = sqlx::query_as::<_, KycSubmission>(
            r#"
            INSERT INTO kyc_submissions (
                id, user_id, email, input_name, doc_type, s3_bucket, s3_key,
                file_sha256, status, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, user_id, email, input_name, doc_type, s3_bucket, s3_key, file_sha256, extracted_name, name_match, status, rejection_reason, created_at, updated_at
            "#
        )
        .bind(submission.id)
        .bind(submission.user_id)
        .bind(submission.email)
        .bind(submission.input_name)
        .bind(submission.doc_type)
        .bind(submission.s3_bucket)
        .bind(submission.s3_key)
        .bind(submission.file_sha256)
        .bind(submission.status)
        .bind(submission.created_at)
        .bind(submission.updated_at)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(record) => Ok(record),
            Err(e) => {
                log::error!("Failed to create KYC submission: {:?}", e);
                Err(e.to_string())
            }
        }
    }

    #[allow(dead_code)]
    pub async fn update_status(
        &self,
        id: Uuid,
        status: KycStatus,
        extracted_name: Option<String>,
        name_match: Option<bool>,
        rejection_reason: Option<String>
    ) -> Result<KycSubmission, String> {
        let result = sqlx::query_as::<_, KycSubmission>(
            r#"
            UPDATE kyc_submissions
            SET status = $1,
                extracted_name = $2,
                name_match = $3,
                rejection_reason = $4,
                updated_at = NOW()
            WHERE id = $5
            RETURNING id, user_id, email, input_name, doc_type, s3_bucket, s3_key, file_sha256, extracted_name, name_match, status, rejection_reason, created_at, updated_at
            "#
        )
        .bind(status)
        .bind(extracted_name)
        .bind(name_match)
        .bind(rejection_reason)
        .bind(id)
        .fetch_one(&self.pool)
        .await;

        match result {
            Ok(record) => Ok(record),
            Err(e) => Err(e.to_string()),
        }
    }

    #[allow(dead_code)]
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<KycSubmission>, String> {
        let result = sqlx::query_as::<_, KycSubmission>(
            "SELECT id, user_id, email, input_name, doc_type, s3_bucket, s3_key, file_sha256, extracted_name, name_match, status, rejection_reason, created_at, updated_at FROM kyc_submissions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(record) => Ok(record),
            Err(e) => Err(e.to_string()),
        }
    }

    #[allow(dead_code)]
    pub async fn find_by_user_id(&self, user_id: Uuid) -> Result<Vec<KycSubmission>, String> {
         let result = sqlx::query_as::<_, KycSubmission>(
            "SELECT id, user_id, email, input_name, doc_type, s3_bucket, s3_key, file_sha256, extracted_name, name_match, status, rejection_reason, created_at, updated_at FROM kyc_submissions WHERE user_id = $1 ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await;

        match result {
            Ok(records) => Ok(records),
            Err(e) => Err(e.to_string()),
        }
    }
}
