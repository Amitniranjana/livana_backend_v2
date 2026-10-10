# Advanced KYC APIs (Issues 90, 91, 92)

These endpoints introduce new capabilities for automated KYC verification, including document data extraction (OCR), document tampering detection, and face-match verification (Liveness & selfie matching).

> [!NOTE]
> All endpoints require a valid user JWT in the `Authorization: Bearer <token>` header.

---

## 1. Document Extraction & Tampering Detection

Extracts structured data from uploaded KYC documents (Aadhaar, PAN, Bank Statements, ITR) and runs an integrity check to detect tampering or forgery.

**Endpoint:** `POST /api/v1/kyc/documents/{doc_id}/extract`

**Auth Required:** Yes (User/Internal)

**Path Parameters:**
- `doc_id` (String, UUID): The `id` of the KYC submission record.

**Request Body:** None

**Response Payload:**
```json
{
  "success": true,
  "data": {
    "extracted_name": "John Doe",
    "extracted_doc_number": "ABCDE1234F",
    "extracted_dob": "1990-01-01",
    "extracted_income": null,
    "integrity_status": "CLEAN",
    "message": "Document extracted successfully"
  }
}
```

**Fields:**
- `extracted_name`: Name read from the document.
- `extracted_doc_number`: Identifier like PAN number or Aadhaar Number.
- `extracted_dob`: Date of birth extracted.
- `extracted_income`: Numeric income (applicable for ITR/Bank statements).
- `integrity_status`: `CLEAN`, `SUSPICIOUS`, or `FLAGGED`. 
  - *If `SUSPICIOUS` or `FLAGGED`, the frontend should alert the user or direct them to manual review.*

---

## 2. Face-Match & Liveness Verification

Compares the user's live selfie with the photo present on their uploaded ID document.

**Endpoint:** `POST /api/v1/kyc/face-match`

**Auth Required:** Yes (User)

**Request Body:**
```json
{
  "selfie_url": "https://bucket.s3.region.amazonaws.com/kyc/selfie.jpg",
  "id_photo_url": "https://bucket.s3.region.amazonaws.com/kyc/id_photo.jpg" 
}
```
> [!TIP]
> `id_photo_url` is optional. If omitted, the backend will attempt to fetch it from the latest ID document uploaded by the user. `selfie_url` is mandatory.

**Response Payload:**
```json
{
  "success": true,
  "data": {
    "match_score": 98.5,
    "match_status": true,
    "message": "Face matched successfully"
  }
}
```

**Fields:**
- `match_score`: Float between 0 and 100 representing confidence.
- `match_status`: Boolean. `true` if the score is above the configured threshold (e.g., 85%).
- `message`: Contextual message regarding the match status.
