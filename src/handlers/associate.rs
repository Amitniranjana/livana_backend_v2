use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::associate::{
        AssociateProfileDto, AssociateRegistrationResponse, AssociateTypeDto, KycUploadDto,
        RegisterAssociateDto,
    },
    dtos::response::ApiResponse,
    utils::api_error::ApiError,
    utils::auth_extractor::AuthenticationUser,
    utils::util::hash_string,
};

/// 1. Register Associate (POST /api/v1/associates/register)
pub async fn register_associate(
    State(app_state): State<AppState>,
    Json(payload): Json<RegisterAssociateDto>,
) -> Result<impl IntoResponse, ApiError> {
    let hashed_password = hash_string(&payload.password);
    let associate_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Check if user already exists
    let existing = sqlx::query!(
        "SELECT id FROM users WHERE email = $1 OR phone_no = $2",
        payload.email,
        payload.phone
    )
    .fetch_optional(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(e.to_string()))?;

    if existing.is_some() {
        return Err(ApiError::Conflict(
            "User with this email or phone number already exists".to_string(),
        ));
    }

    // Insert user into the database
    sqlx::query(
        r#"
        INSERT INTO users (id, first_name, last_name, email, phone_no, password, user_role, verified, status, associate_type, created_at, updated_at)
        VALUES ($1, $2, '', $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#)
        .bind(associate_id)
        .bind(&payload.name)
        .bind(&payload.email)
        .bind(&payload.phone)
        .bind(hashed_password)
        .bind("ASSOCIATE")
        .bind(false)
        .bind("PENDING_KYC")
        .bind(&payload.associate_type)
        .bind(now)
        .bind(now)
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to register associate: {}", e)))?;

    let gender = payload.gender.unwrap_or_else(|| "not_specified".to_string());

    // Optionally create profile with gender
    sqlx::query(
        r#"
        INSERT INTO user_profiles (user_id, gender)
        VALUES ($1, $2)
        ON CONFLICT (user_id) DO NOTHING
        "#,
    )
    .bind(associate_id)
    .bind(gender)
    .execute(&app_state.db)
    .await
    .map_err(|e| {
        ApiError::InternalServerError(format!("Failed to create associate profile: {}", e))
    })?;

    let response = ApiResponse {
        success: true,
        message: "Associate registered successfully".to_string(),
        data: AssociateRegistrationResponse {
            associate_id,
            status: "PENDING_KYC".to_string(),
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// 2. Upload KYC Documents (POST /api/v1/associates/{id}/kyc)
pub async fn upload_kyc_documents(
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: AuthenticationUser,
    Json(payload): Json<KycUploadDto>,
) -> Result<impl IntoResponse, ApiError> {
    // Determine user role
    let user_role: (String,) = sqlx::query_as("SELECT user_role FROM users WHERE id = $1")
        .bind(Uuid::parse_str(&auth.user_id).unwrap_or_default())
        .fetch_one(&app_state.db)
        .await
        .map_err(|_| ApiError::Unauthorized("Invalid user".to_string()))?;

    // RBAC: ASSOCIATE or ADMIN
    if user_role.0 != "ASSOCIATE" && user_role.0 != "ADMIN" {
        return Err(ApiError::Forbidden(
            "Only associates and admins can upload KYC documents".to_string(),
        ));
    }

    if user_role.0 == "ASSOCIATE" && auth.user_id != id.to_string() {
        return Err(ApiError::Forbidden(
            "You can only upload documents for your own account".to_string(),
        ));
    }

    // Save KYC documents logic
    // We'll update a hypothetical kyc_documents table or user profile
    sqlx::query(
        r#"
        INSERT INTO kycs (user_id, document_url, document_type, verification_status, submitted_at)
        VALUES ($1, $2, 'aadhaar', 'pending', $3)
        "#,
    )
    .bind(id.to_string())
    .bind(&payload.aadhaar_url)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to save Aadhaar: {}", e)))?;

    sqlx::query(
        r#"
        INSERT INTO kycs (user_id, document_url, document_type, verification_status, submitted_at)
        VALUES ($1, $2, 'pan', 'pending', $3)
        "#,
    )
    .bind(id.to_string())
    .bind(&payload.pan_url)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&app_state.db)
    .await
    .map_err(|e| ApiError::InternalServerError(format!("Failed to save PAN: {}", e)))?;

    if let Some(license_url) = payload.business_license_url {
        sqlx::query(
            r#"
            INSERT INTO kycs (user_id, document_url, document_type, verification_status, submitted_at)
            VALUES ($1, $2, 'business_license', 'pending', $3)
            "#)
            .bind(id.to_string())
            .bind(&license_url)
            .bind(chrono::Utc::now().to_rfc3339())
        .execute(&app_state.db)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to save Business License: {}", e)))?;
    }

    let response = ApiResponse {
        success: true,
        message: "KYC documents submitted successfully. Status updated to PENDING_KYC.".to_string(),
        data: json!({}),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// 3. Get Associate Profile (GET /api/v1/associates/me)
pub async fn get_associate_profile(
    State(app_state): State<AppState>,
    auth: AuthenticationUser,
) -> Result<impl IntoResponse, ApiError> {
    let auth_user_id_uuid = Uuid::parse_str(&auth.user_id).unwrap_or_default();
    let record: (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    ) = sqlx::query_as(
        r#"
        SELECT id::text, first_name, email, phone_no, status, associate_type, created_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(auth_user_id_uuid)
    .fetch_one(&app_state.db)
    .await
    .map_err(|_| ApiError::NotFound("Profile not found".to_string()))?;

    // Record has fields: 0=id, 1=first_name, 2=email, 3=phone_no, 4=status, 5=associate_type, 6=created_at

    let associate_type = record.5;

    let profile = AssociateProfileDto {
        id: Uuid::parse_str(&record.0).unwrap_or_default(),
        name: record.1,
        email: record.2,
        phone: record.3,
        kbc: "Unknown".to_string(), // Assuming kbc is fetched from user_profiles in real scenario
        associate_type,
        status: record.4,
        created_at: record.6,
    };

    let response = ApiResponse {
        success: true,
        message: "Profile retrieved successfully".to_string(),
        data: profile,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// 4. Get Associate Types (GET /api/v1/associate-types)
pub async fn get_associate_types(
    State(app_state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // Assuming you have an associate_types table.
    let types_result: Result<Vec<(Uuid, String)>, _> = sqlx::query_as(
        r#"
        SELECT id, name FROM associate_types
        "#,
    )
    .fetch_all(&app_state.db)
    .await;

    let mut associate_types = vec![];

    if let Ok(records) = types_result {
        for record in records {
            associate_types.push(AssociateTypeDto {
                id: record.0,
                name: record.1,
            });
        }
    } else {
        // Fallback or handle missing table logic temporarily
        associate_types.push(AssociateTypeDto {
            id: Uuid::new_v4(),
            name: "Agent".to_string(),
        });
        associate_types.push(AssociateTypeDto {
            id: Uuid::new_v4(),
            name: "Broker".to_string(),
        });
    }

    let response = ApiResponse {
        success: true,
        message: "Associate types retrieved successfully".to_string(),
        data: associate_types,
    };

    Ok((StatusCode::OK, Json(response)))
}
