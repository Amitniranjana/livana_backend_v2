use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use axum_extra::extract::Multipart;
use chrono::Utc;
use jsonwebtoken::DecodingKey;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::dtos::kyc::{
    KycSubmitRequest, KycUpdateRequest, KycResponse, KycStatusResponse, UploadResponse, KycRow,
};
use crate::services::ocr::OcrService;

// ---------------------------------------------------------------------------
// JWT helper
// ---------------------------------------------------------------------------

fn extract_user_id_from_jwt(token: &str, decoding_key: &DecodingKey) -> Result<Uuid, String> {
    use jsonwebtoken::{decode, Algorithm, Validation};
    #[derive(serde::Deserialize)]
    struct Claims { sub: String }
    let mut v = Validation::new(Algorithm::HS256);
    v.validate_exp = false;
    let data = decode::<Claims>(token, decoding_key, &v).map_err(|e| e.to_string())?;
    Uuid::parse_str(&data.claims.sub).map_err(|e| e.to_string())
}

macro_rules! require_auth {
    ($headers:expr, $app_state:expr) => {{
        let bearer = $headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer ").map(|s| s.to_string()));
        match bearer {
            Some(b) => {
                let dk = DecodingKey::from_secret($app_state.jwt_secret.as_bytes());
                match extract_user_id_from_jwt(&b, &dk) {
                    Ok(uid) => uid,
                    Err(e) => {
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(json!({"success":false,"message":format!("Auth error: {}",e),"error_code":"UNAUTHORIZED"})),
                        );
                    }
                }
            }
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"success":false,"message":"Missing Authorization header","error_code":"UNAUTHORIZED"})),
                );
            }
        }
    }};
}

// ---------------------------------------------------------------------------
// Helper: read a PgRow → KycRow
// ---------------------------------------------------------------------------

fn pg_row_to_kyc_row(row: &sqlx::postgres::PgRow) -> KycRow {
    KycRow {
        id:                          row.try_get("id").unwrap_or(Uuid::nil()),
        user_id:                     row.try_get("user_id").unwrap_or(Uuid::nil()),
        full_name:                   row.try_get("full_name").unwrap_or_default(),
        mobile_number:               row.try_get("mobile_number").unwrap_or_default(),
        email_id:                    row.try_get("email_id").unwrap_or_default(),
        gender:                      row.try_get("gender").ok().flatten(),
        date_of_birth:               row.try_get("date_of_birth").ok().flatten(),
        profile_picture_url:         row.try_get("profile_picture_url").ok().flatten(),
        apartment_name:              row.try_get("apartment_name").ok().flatten(),
        street_address:              row.try_get("street_address").ok().flatten(),
        landmark:                    row.try_get("landmark").ok().flatten(),
        city:                        row.try_get("city").ok().flatten(),
        zip_code:                    row.try_get("zip_code").ok().flatten(),
        state:                       row.try_get("state").ok().flatten(),
        country:                     row.try_get("country").ok().flatten(),
        permanent_address:           row.try_get("permanent_address").ok().flatten(),
        is_permanent_same_as_present: row.try_get("is_permanent_same_as_present").ok().flatten(),
        latitude:                    row.try_get("latitude").ok().flatten(),
        longitude:                   row.try_get("longitude").ok().flatten(),
        govt_id_type:                row.try_get("govt_id_type").unwrap_or_default(),
        govt_id_number:              row.try_get("govt_id_number").unwrap_or_default(),
        govt_id_document_url:        row.try_get("govt_id_document_url").unwrap_or_default(),
        company_name:                row.try_get("company_name").ok().flatten(),
        services_json:               row.try_get("services").ok().flatten(),
        experience_document_url:     row.try_get("experience_document_url").ok().flatten(),
        verification_status:         row.try_get("verification_status").unwrap_or_else(|_| "pending".to_string()),
        submitted_at:                row.try_get("submitted_at").ok().flatten(),
        verified_at:                 row.try_get("verified_at").ok().flatten(),
        rejection_reason:            row.try_get("rejection_reason").ok().flatten(),
    }
}

// ---------------------------------------------------------------------------
// 1. POST /api/kyc/upload/profile   — multipart file upload
// ---------------------------------------------------------------------------

pub async fn upload_profile(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl axum::response::IntoResponse {
    let _user_id = require_auth!(headers, app_state);

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            let filename = field
                .file_name()
                .unwrap_or("upload.jpg")
                .to_string();
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();

            let data = match field.bytes().await {
                Ok(b) => b,
                Err(_) => {
                    return (StatusCode::BAD_REQUEST,
                        Json(json!({"success":false,"message":"Failed to read file"})));
                }
            };

            let size = data.len() as u64;
            if size > 5 * 1024 * 1024 {
                return (StatusCode::BAD_REQUEST,
                    Json(json!({"success":false,"message":"Profile image must be under 5MB"})));
            }

            let ext = std::path::Path::new(&filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg");
            let key = format!("kyc/profile/{}/{}.{}", _user_id, Uuid::new_v4(), ext);

            if let Err(e) = app_state.kyc_service.storage().upload_file(&key, data.to_vec(), &content_type).await {
                return (StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success":false,"message":format!("Upload failed: {}",e)})));
            }

            let bucket = std::env::var("KYC_BUCKET_NAME").unwrap_or_else(|_| "livana-kyc-documents".to_string());
            let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "ap-south-1".to_string());
            let url = format!("https://{}.s3.{}.amazonaws.com/{}", bucket, region, key);

            let resp = UploadResponse {
                url,
                filename,
                size,
                mime_type: content_type,
                uploaded_at: Utc::now().to_rfc3339(),
            };

            return (StatusCode::OK, Json(json!({
                "success": true,
                "message": "Profile image uploaded successfully",
                "data": resp
            })));
        }
    }

    (StatusCode::BAD_REQUEST, Json(json!({"success":false,"message":"No file field found in request"})))
}

// ---------------------------------------------------------------------------
// 2. POST /api/kyc/upload/document  — ID document upload
// ---------------------------------------------------------------------------

pub async fn upload_document(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl axum::response::IntoResponse {
    let _user_id = require_auth!(headers, app_state);

    let mut doc_type = "id_document".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "document_type" {
            if let Ok(text) = field.text().await {
                doc_type = text;
            }
            continue;
        }

        if field_name == "file" {
            let filename = field.file_name().unwrap_or("document.pdf").to_string();
            let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
            let data = match field.bytes().await {
                Ok(b) => b,
                Err(_) => {
                    return (StatusCode::BAD_REQUEST,
                        Json(json!({"success":false,"message":"Failed to read file"})));
                }
            };

            let size = data.len() as u64;
            if size > 10 * 1024 * 1024 {
                return (StatusCode::BAD_REQUEST,
                    Json(json!({"success":false,"message":"Document must be under 10MB"})));
            }

            let ext = std::path::Path::new(&filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("pdf");
            let key = format!("kyc/{}/{}/{}.{}", doc_type, _user_id, Uuid::new_v4(), ext);

            if let Err(e) = app_state.kyc_service.storage().upload_file(&key, data.to_vec(), &content_type).await {
                return (StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success":false,"message":format!("Upload failed: {}",e)})));
            }

            let bucket = std::env::var("KYC_BUCKET_NAME").unwrap_or_else(|_| "livana-kyc-documents".to_string());
            let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "ap-south-1".to_string());
            let url = format!("https://{}.s3.{}.amazonaws.com/{}", bucket, region, key);

            let resp = UploadResponse { url, filename, size, mime_type: content_type, uploaded_at: Utc::now().to_rfc3339() };
            return (StatusCode::OK, Json(json!({
                "success": true,
                "message": "Document uploaded successfully",
                "data": resp
            })));
        }
    }

    (StatusCode::BAD_REQUEST, Json(json!({"success":false,"message":"No file field found"})))
}

// ---------------------------------------------------------------------------
// 3. POST /api/kyc/upload/experience  — experience document upload
// ---------------------------------------------------------------------------

pub async fn upload_experience(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl axum::response::IntoResponse {
    let _user_id = require_auth!(headers, app_state);

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            let filename = field.file_name().unwrap_or("exp.pdf").to_string();
            let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
            let data = match field.bytes().await {
                Ok(b) => b,
                Err(_) => {
                    return (StatusCode::BAD_REQUEST,
                        Json(json!({"success":false,"message":"Failed to read file"})));
                }
            };

            let size = data.len() as u64;
            if size > 10 * 1024 * 1024 {
                return (StatusCode::BAD_REQUEST,
                    Json(json!({"success":false,"message":"Document must be under 10MB"})));
            }

            let ext = std::path::Path::new(&filename).extension().and_then(|e| e.to_str()).unwrap_or("pdf");
            let key = format!("kyc/experience/{}/{}.{}", _user_id, Uuid::new_v4(), ext);

            if let Err(e) = app_state.kyc_service.storage().upload_file(&key, data.to_vec(), &content_type).await {
                return (StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success":false,"message":format!("Upload failed: {}",e)})));
            }

            let bucket = std::env::var("KYC_BUCKET_NAME").unwrap_or_else(|_| "livana-kyc-documents".to_string());
            let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "ap-south-1".to_string());
            let url = format!("https://{}.s3.{}.amazonaws.com/{}", bucket, region, key);

            let resp = UploadResponse { url, filename, size, mime_type: content_type, uploaded_at: Utc::now().to_rfc3339() };
            return (StatusCode::OK, Json(json!({
                "success": true,
                "message": "Experience document uploaded successfully",
                "data": resp
            })));
        }
    }

    (StatusCode::BAD_REQUEST, Json(json!({"success":false,"message":"No file field found"})))
}

// ---------------------------------------------------------------------------
// 4. DELETE /api/kyc/upload/{file_id}  — soft-delete uploaded file record
// ---------------------------------------------------------------------------

pub async fn delete_upload(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(file_id): Path<String>,
) -> impl axum::response::IntoResponse {
    let _user_id = require_auth!(headers, app_state);

    // Mark the kyc_uploads record as deleted (soft delete)
    let result = sqlx::query(
        "UPDATE kyc_uploads SET deleted_at = $1 WHERE id = $2 AND user_id = $3 AND deleted_at IS NULL"
    )
    .bind(Utc::now())
    .bind(file_id.clone())
    .bind(_user_id)
    .execute(&app_state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => (
            StatusCode::OK,
            Json(json!({"success":true,"message":"File deleted successfully","data":{"file_id":file_id}}))
        ),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({"success":false,"message":"File not found or already deleted"}))
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success":false,"message":format!("Database error: {}",e)}))
        ),
    }
}

// ---------------------------------------------------------------------------
// 5. POST /api/kyc/submit
// ---------------------------------------------------------------------------

pub async fn submit_kyc(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<KycSubmitRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    // Validation
    if payload.full_name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "success": false,
            "message": "Validation failed",
            "error_code": "VALIDATION_ERROR",
            "errors": { "full_name": ["full_name is required"] }
        })));
    }
    if payload.govt_id_number.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "success": false,
            "message": "Validation failed",
            "error_code": "VALIDATION_ERROR",
            "errors": { "govt_id_number": ["govt_id_number is required"] }
        })));
    }

    // Hardcode initial state as requested
    let verification_status = "pending".to_string();
    let kyc_id = Uuid::new_v4();
    let now = Utc::now();
    let services_json = serde_json::to_value(payload.services.unwrap_or_default()).unwrap_or(json!([]));
    let verified_at: Option<chrono::DateTime<Utc>> = None;

    let result = sqlx::query(r#"
        INSERT INTO kyc_submissions (
            id, user_id,
            full_name, mobile_number, email_id, gender, date_of_birth, profile_picture_url,
            apartment_name, street_address, landmark, city, zip_code, state, country,
            permanent_address, is_permanent_same_as_present,
            latitude, longitude,
            govt_id_type, govt_id_number, govt_id_document_url,
            company_name, services, experience_document_url,
            extracted_name, name_match_score,
            verification_status, submitted_at, verified_at, rejection_reason
        ) VALUES (
            $1, $2,
            $3, $4, $5, $6, $7, $8,
            $9, $10, $11, $12, $13, $14, $15,
            $16, $17,
            $18, $19,
            $20, $21, $22,
            $23, $24, $25,
            NULL, NULL,
            $26, $27, $28, NULL
        )
        ON CONFLICT (user_id) DO UPDATE SET
            full_name = EXCLUDED.full_name,
            mobile_number = EXCLUDED.mobile_number,
            email_id = EXCLUDED.email_id,
            gender = EXCLUDED.gender,
            date_of_birth = EXCLUDED.date_of_birth,
            profile_picture_url = EXCLUDED.profile_picture_url,
            apartment_name = EXCLUDED.apartment_name,
            street_address = EXCLUDED.street_address,
            landmark = EXCLUDED.landmark,
            city = EXCLUDED.city,
            zip_code = EXCLUDED.zip_code,
            state = EXCLUDED.state,
            country = EXCLUDED.country,
            permanent_address = EXCLUDED.permanent_address,
            is_permanent_same_as_present = EXCLUDED.is_permanent_same_as_present,
            latitude = EXCLUDED.latitude,
            longitude = EXCLUDED.longitude,
            govt_id_type = EXCLUDED.govt_id_type,
            govt_id_number = EXCLUDED.govt_id_number,
            govt_id_document_url = EXCLUDED.govt_id_document_url,
            company_name = EXCLUDED.company_name,
            services = EXCLUDED.services,
            experience_document_url = EXCLUDED.experience_document_url,
            extracted_name = NULL,
            name_match_score = NULL,
            verification_status = EXCLUDED.verification_status,
            submitted_at = EXCLUDED.submitted_at,
            verified_at = NULL,
            rejection_reason = NULL
        RETURNING *
    "#)
    .bind(kyc_id)
    .bind(user_id)
    .bind(&payload.full_name)
    .bind(&payload.mobile_number)
    .bind(&payload.email_id)
    .bind(&payload.gender)
    .bind(&payload.date_of_birth)
    .bind(&payload.profile_picture_url)
    .bind(&payload.apartment_name)
    .bind(&payload.street_address)
    .bind(&payload.landmark)
    .bind(&payload.city)
    .bind(&payload.zip_code)
    .bind(&payload.state)
    .bind(&payload.country)
    .bind(&payload.permanent_address)
    .bind(payload.is_permanent_same_as_present)
    .bind(payload.latitude)
    .bind(payload.longitude)
    .bind(&payload.govt_id_type)
    .bind(&payload.govt_id_number)
    .bind(&payload.govt_id_document_url)
    .bind(&payload.company_name)
    .bind(services_json)
    .bind(&payload.experience_document_url)
    .bind(&verification_status)
    .bind(now)
    .bind(verified_at)
    .fetch_one(&app_state.db)
    .await;

    match result {
        Ok(row) => {
            let kyc_resp: KycResponse = pg_row_to_kyc_row(&row).into();
            (StatusCode::CREATED, Json(json!({
                "success": true,
                "message": "KYC details submitted successfully",
                "data": kyc_resp
            })))
        },
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            (StatusCode::CONFLICT, Json(json!({
                "success": false,
                "message": "KYC already submitted for this user",
                "error_code": "KYC_ALREADY_EXISTS"
            })))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success":false,"message":format!("Database error: {}",e)}))),
    }
}

// ---------------------------------------------------------------------------
// 6. GET /api/kyc/{user_id}
// ---------------------------------------------------------------------------

pub async fn get_kyc(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(user_id_str): Path<String>,
) -> impl axum::response::IntoResponse {
    let _caller = require_auth!(headers, app_state);

    let user_id = match Uuid::parse_str(&user_id_str) {
        Ok(u) => u,
        Err(_) => {
            return (StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid user_id"})));
        }
    };

    let row = match sqlx::query(
        "SELECT * FROM kyc_submissions WHERE user_id = $1 ORDER BY submitted_at DESC LIMIT 1"
    )
    .bind(user_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (StatusCode::NOT_FOUND,
                Json(json!({"success":false,"message":"KYC record not found"})));
        }
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})));
        }
    };

    let kyc: KycResponse = pg_row_to_kyc_row(&row).into();
    (StatusCode::OK, Json(json!({"success":true,"message":"KYC details fetched","data":{"kyc":kyc}})))
}

// ---------------------------------------------------------------------------
// 7. GET /api/kyc/status/{user_id}
// ---------------------------------------------------------------------------

pub async fn get_kyc_status(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(user_id_str): Path<String>,
) -> impl axum::response::IntoResponse {
    let _caller = require_auth!(headers, app_state);

    let user_id = match Uuid::parse_str(&user_id_str) {
        Ok(u) => u,
        Err(_) => {
            return (StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid user_id"})));
        }
    };

    let row = match sqlx::query(
        "SELECT user_id, verification_status, submitted_at, verified_at, rejection_reason \
         FROM kyc_submissions WHERE user_id = $1 ORDER BY submitted_at DESC LIMIT 1"
    )
    .bind(user_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (StatusCode::NOT_FOUND,
                Json(json!({"success":false,"message":"No KYC submission found"})));
        }
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})));
        }
    };

    let status: String = row.try_get("verification_status").unwrap_or_else(|_| "pending".to_string());
    let is_verified = status == "verified";
    let submitted_at: Option<chrono::DateTime<Utc>> = row.try_get("submitted_at").ok().flatten();
    let verified_at:  Option<chrono::DateTime<Utc>> = row.try_get("verified_at").ok().flatten();
    let rejection_reason: Option<String> = row.try_get("rejection_reason").ok().flatten();

    let resp = KycStatusResponse {
        user_id: user_id.to_string(),
        is_verified,
        verification_status: status,
        submitted_at: submitted_at.map(|d| d.to_rfc3339()),
        verified_at: verified_at.map(|d|d.to_rfc3339()),
        rejection_reason,
    };

    (StatusCode::OK, Json(json!({"success":true,"message":"KYC status fetched","data":resp})))
}

// ---------------------------------------------------------------------------
// 8. PUT /api/kyc/{kyc_id}
// ---------------------------------------------------------------------------

pub async fn update_kyc(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    Path(kyc_id_str): Path<String>,
    Json(payload): Json<KycUpdateRequest>,
) -> impl axum::response::IntoResponse {
    let user_id = require_auth!(headers, app_state);

    let kyc_id = match Uuid::parse_str(&kyc_id_str) {
        Ok(u) => u,
        Err(_) => {
            return (StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":"Invalid kyc_id"})));
        }
    };

    // Confirm record belongs to caller
    let owner: Option<Uuid> = match sqlx::query_scalar(
        "SELECT user_id FROM kyc_submissions WHERE id = $1"
    )
    .bind(kyc_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(o) => o,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}",e)})));
        }
    };

    match owner {
        None => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"KYC record not found"}))),
        Some(o) if o != user_id => return (StatusCode::FORBIDDEN, Json(json!({"success":false,"message":"Forbidden"}))),
        _ => {}
    }

    let now = Utc::now();
    let mut qb: sqlx::QueryBuilder<sqlx::Postgres> =
        sqlx::QueryBuilder::new("UPDATE kyc_submissions SET updated_at = ");
    qb.push_bind(now);

    if let Some(v) = &payload.full_name            { qb.push(", full_name = "); qb.push_bind(v); }
    if let Some(v) = &payload.mobile_number        { qb.push(", mobile_number = "); qb.push_bind(v); }
    if let Some(v) = &payload.email_id             { qb.push(", email_id = "); qb.push_bind(v); }
    if let Some(v) = &payload.gender               { qb.push(", gender = "); qb.push_bind(v); }
    if let Some(v) = &payload.date_of_birth        { qb.push(", date_of_birth = "); qb.push_bind(v); }
    if let Some(v) = &payload.profile_picture_url  { qb.push(", profile_picture_url = "); qb.push_bind(v); }
    if let Some(v) = &payload.apartment_name       { qb.push(", apartment_name = "); qb.push_bind(v); }
    if let Some(v) = &payload.street_address       { qb.push(", street_address = "); qb.push_bind(v); }
    if let Some(v) = &payload.landmark             { qb.push(", landmark = "); qb.push_bind(v); }
    if let Some(v) = &payload.city                 { qb.push(", city = "); qb.push_bind(v); }
    if let Some(v) = &payload.zip_code             { qb.push(", zip_code = "); qb.push_bind(v); }
    if let Some(v) = &payload.state                { qb.push(", state = "); qb.push_bind(v); }
    if let Some(v) = &payload.country              { qb.push(", country = "); qb.push_bind(v); }
    if let Some(v) = &payload.permanent_address    { qb.push(", permanent_address = "); qb.push_bind(v); }
    if let Some(v) = payload.is_permanent_same_as_present { qb.push(", is_permanent_same_as_present = "); qb.push_bind(v); }
    if let Some(v) = payload.latitude              { qb.push(", latitude = "); qb.push_bind(v); }
    if let Some(v) = payload.longitude             { qb.push(", longitude = "); qb.push_bind(v); }
    if let Some(v) = &payload.govt_id_type         { qb.push(", govt_id_type = "); qb.push_bind(v); }
    if let Some(v) = &payload.govt_id_number       { qb.push(", govt_id_number = "); qb.push_bind(v); }
    if let Some(v) = &payload.govt_id_document_url { qb.push(", govt_id_document_url = "); qb.push_bind(v); }
    if let Some(v) = &payload.company_name         { qb.push(", company_name = "); qb.push_bind(v); }
    if let Some(v) = &payload.experience_document_url { qb.push(", experience_document_url = "); qb.push_bind(v); }
    if let Some(v) = &payload.services {
        let j = serde_json::to_value(v).unwrap_or(json!([]));
        qb.push(", services = "); qb.push_bind(j);
    }

    // If govt_id_document_url changed, re-run OCR
    if let (Some(doc_url), Some(full_name)) = (&payload.govt_id_document_url, &payload.full_name) {
        let (extracted_name, score, v_status) =
            run_ocr_on_document_url(doc_url, full_name, &*app_state.kyc_service.ocr()).await;
        qb.push(", extracted_name = "); qb.push_bind(extracted_name);
        qb.push(", name_match_score = "); qb.push_bind(score);
        qb.push(", verification_status = "); qb.push_bind(v_status);
    }

    qb.push(" WHERE id = "); qb.push_bind(kyc_id);

    if let Err(e) = qb.build().execute(&app_state.db).await {
        return (StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success":false,"message":format!("Update failed: {}",e)})));
    }

    (StatusCode::OK, Json(json!({"success":true,"message":"KYC updated successfully","data":{"kyc_id":kyc_id.to_string()}})))
}

// ---------------------------------------------------------------------------
// OCR helper: fetch image from URL and run name matching
// Returns (extracted_name, name_match_score, verification_status)
// ---------------------------------------------------------------------------

async fn run_ocr_on_document_url(
    url: &str,
    full_name: &str,
    ocr: &dyn OcrService,
) -> (Option<String>, Option<f64>, String) {
    // Fetch document bytes from URL
    let bytes = match reqwest::get(url).await {
        Ok(resp) => match resp.bytes().await {
            Ok(b) => b.to_vec(),
            Err(e) => {
                log::warn!("Failed to read document bytes from {}: {}", url, e);
                return (None, None, "pending".to_string());
            }
        },
        Err(e) => {
            log::warn!("Failed to fetch document from {}: {}", url, e);
            return (None, None, "pending".to_string());
        }
    };

    // Run OCR
    let text = match ocr.extract_text(&bytes).await {
        Ok(t) => t,
        Err(e) => {
            log::warn!("OCR failed: {}", e);
            return (None, None, "pending_review".to_string());
        }
    };

    // Fuzzy name match
    let extracted_name = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.len() > 3 && l.chars().any(|c| c.is_alphabetic()))
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(255)
        .collect::<String>();

    let normalize = |s: &str| -> String {
        s.to_uppercase()
            .replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };

    let norm_input     = normalize(full_name);
    let norm_extracted = normalize(&extracted_name);

    let score: f64 = if norm_extracted.contains(&norm_input) || norm_input == norm_extracted {
        1.0
    } else {
        // Simple token overlap ratio
        let input_words: std::collections::HashSet<&str> = norm_input.split_whitespace().collect();
        let extracted_words: std::collections::HashSet<&str> = norm_extracted.split_whitespace().collect();
        let common = input_words.intersection(&extracted_words).count();
        if input_words.is_empty() { 0.0 } else { common as f64 / input_words.len() as f64 }
    };

    let status = if score >= 0.8 { "verified" } else { "pending_review" }.to_string();

    (Some(extracted_name), Some(score), status)
}
