ALTER TABLE kyc_submissions
DROP COLUMN face_match_status,
DROP COLUMN face_match_score,
DROP COLUMN document_integrity_status,
DROP COLUMN extracted_income,
DROP COLUMN extracted_dob,
DROP COLUMN extracted_doc_number;

DROP TYPE kyc_integrity_status;
