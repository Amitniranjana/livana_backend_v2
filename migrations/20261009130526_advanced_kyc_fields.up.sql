CREATE TYPE kyc_integrity_status AS ENUM ('CLEAN', 'SUSPICIOUS', 'FLAGGED');

ALTER TABLE kyc_submissions
ADD COLUMN extracted_doc_number VARCHAR(255),
ADD COLUMN extracted_dob DATE,
ADD COLUMN extracted_income NUMERIC(15, 2),
ADD COLUMN document_integrity_status kyc_integrity_status,
ADD COLUMN face_match_score DOUBLE PRECISION,
ADD COLUMN face_match_status BOOLEAN;
