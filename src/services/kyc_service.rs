use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;
use sha2::{Digest, Sha256};
use crate::models::kyc::{KycSubmission, KycDocType, KycStatus};
use crate::repository::kyc_repository::KycRepository;
use crate::services::storage::StorageService;
use crate::services::ocr::OcrService;
use anyhow::{Result, Context};

#[derive(Clone)]
pub struct KycService {
    repo: Arc<KycRepository>,
    storage: Arc<dyn StorageService + Send + Sync>,
    ocr: Arc<dyn OcrService + Send + Sync>,
}

impl KycService {
    pub fn new(
        repo: Arc<KycRepository>,
        storage: Arc<dyn StorageService + Send + Sync>,
        ocr: Arc<dyn OcrService + Send + Sync>,
    ) -> Self {
        Self { repo, storage, ocr }
    }

    pub async fn submit_kyc(
        &self,
        user_id: Uuid,
        email: String,
        input_name: String,
        doc_type: KycDocType,
        file_bytes: Vec<u8>,
        file_ext: String,
    ) -> Result<KycSubmission> {
        // 1. Compute Hash
        let mut hasher = Sha256::new();
        hasher.update(&file_bytes);
        let file_sha256 = hex::encode(hasher.finalize());

        // 2. Generate ID and S3 Key
        let submission_id = Uuid::new_v4();
        let s3_bucket = "livana-kyc-documents".to_string(); // In real app, from env // TODO: Use env
        let s3_key = format!("kyc/{}/{}/original.{}", user_id, submission_id, file_ext);

        let content_type = mime_guess::from_ext(&file_ext)
            .first_or_octet_stream()
            .to_string();

        // 3. Upload to S3
        self.storage
            .upload_file(&s3_key, file_bytes.clone(), &content_type)
            .await
            .context("Failed to upload document")?;

        // 4. Perform OCR
        let extracted_text = match self.ocr.extract_text(&file_bytes).await {
            Ok(text) => Some(text),
            Err(e) => {
                log::warn!("OCR failed for submission {}: {}", submission_id, e);
                None
            }
        };

        // 5. Name Matching Logic
        let (extracted_name, name_match, status) = if let Some(text) = &extracted_text {
            let extracted_name = self.extract_name_from_text(text, &doc_type);
            let normalized_input = self.normalize_name(&input_name);
            let normalized_extracted = self.normalize_name(&extracted_name);

            let is_match = normalized_extracted.contains(&normalized_input) || normalized_input == normalized_extracted; // Simple heuristic

            let status = if is_match {
                KycStatus::Verified
            } else {
                KycStatus::PendingReview // Mismatch or partial match needs review
            };

            (Some(extracted_name), Some(is_match), status)
        } else {
            (None, None, KycStatus::PendingReview) // OCR failed
        };

        // 6. Save to DB
        let submission = KycSubmission {
            id: submission_id,
            user_id,
            email,
            input_name,
            doc_type,
            s3_bucket,
            s3_key,
            file_sha256,
            extracted_name,
            name_match,
            status,
            rejection_reason: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Map repo errors to anyhow
        self.repo.create(submission).await.map_err(|e| anyhow::anyhow!("DB Error: {}", e))
    }

    fn normalize_name(&self, name: &str) -> String {
        name.to_uppercase()
            .replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn extract_name_from_text(&self, text: &str, _doc_type: &KycDocType) -> String {
        text.lines()
            .map(|l| l.trim())
            .filter(|l| l.len() > 3 && l.chars().any(|c| c.is_alphabetic()))
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(255)
            .collect()
    }
}
