use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::admin_kyc::{
        AdminKycDetailResponse, AdminKycListData, AdminKycListItemResponse,
        AdminKycListQuery, AdminKycListResponse, AdminKycPagination, AdminKycRejectRequest,
        OverrideKycImagesReq, RevealGovtIdReq, RevealGovtIdResponse,
    },
    services::zd_verification_service,
};

// ---------------------------------------------------------------------------
// GET /api/admin/kyc
// ---------------------------------------------------------------------------
pub async fn get_kyc_submissions(
    State(app_state): State<AppState>,
    Query(q): Query<AdminKycListQuery>,
) -> impl axum::response::IntoResponse {
    let limit = q.limit.unwrap_or(10).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let mut query = sqlx::QueryBuilder::new(
        "SELECT k.id, k.user_id, k.full_name as name, k.verification_status as status, k.submitted_at, \
         k.govt_id_type, k.experience_document_url, u.user_role as role \
         FROM kyc_submissions k \
         LEFT JOIN users u ON k.user_id = u.id \
         WHERE 1=1"
    );

    let mut count_query = sqlx::QueryBuilder::new(
        "SELECT COUNT(*) FROM kyc_submissions k \
         LEFT JOIN users u ON k.user_id = u.id \
         WHERE 1=1"
    );

    if let Some(status) = &q.status {
        if status != "all" {
            query.push(" AND k.verification_status = ");
            query.push_bind(status);
            count_query.push(" AND k.verification_status = ");
            count_query.push_bind(status);
        }
    }

    if let Some(role) = &q.user_role {
        query.push(" AND u.user_role = ");
        query.push_bind(role);
        count_query.push(" AND u.user_role = ");
        count_query.push_bind(role);
    }

    query.push(" ORDER BY k.submitted_at DESC NULLS LAST LIMIT ");
    query.push_bind(limit);
    query.push(" OFFSET ");
    query.push_bind(offset);

    let total_count: i64 = match count_query.build_query_scalar().fetch_one(&app_state.db).await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}", e)})),
            );
        }
    };

    let rows = match query.build().fetch_all(&app_state.db).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}", e)})),
            );
        }
    };

    let mut kyc_records = Vec::new();
    for row in rows {
        let govt_id_type: String = row.try_get("govt_id_type").unwrap_or_default();
        let exp_doc: Option<String> = row.try_get("experience_document_url").unwrap_or(None);
        
        let mut docs = vec![govt_id_type];
        if exp_doc.is_some() {
            docs.push("Experience Doc".to_string());
        }

        kyc_records.push(AdminKycListItemResponse {
            id: row.get("id"),
            user_id: row.get("user_id"),
            name: row.try_get("name").unwrap_or_default(),
            role: row.try_get("role").unwrap_or(None),
            submitted_docs_summary: docs.join(", "),
            status: row.try_get("status").unwrap_or_else(|_| "pending".to_string()),
            submitted_at: row.try_get("submitted_at").unwrap_or(None),
        });
    }

    let resp = AdminKycListResponse {
        success: true,
        message: "KYC submissions fetched successfully".to_string(),
        data: AdminKycListData {
            kyc_records,
            pagination: AdminKycPagination {
                total_count,
                limit,
                offset,
            },
        },
    };

    (StatusCode::OK, Json(json!(resp)))
}

// ---------------------------------------------------------------------------
// GET /api/admin/kyc/{kyc_id}
// ---------------------------------------------------------------------------
pub async fn get_kyc_submission_detail(
    State(app_state): State<AppState>,
    Path(kyc_id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let row = match sqlx::query(
        "SELECT k.*, u.user_role as role \
         FROM kyc_submissions k \
         LEFT JOIN users u ON k.user_id = u.id \
         WHERE k.id = $1"
    )
    .bind(kyc_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success":false,"message":"KYC record not found"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("Database error: {}", e)})),
            );
        }
    };

    let user_id: Uuid = row.get("user_id");
    let role: Option<String> = row.try_get("role").unwrap_or(None);
    
    // Fetch linked profile based on role
    let mut linked_profile: Option<serde_json::Value> = None;
    if let Some(r) = &role {
        match r.as_str() {
            "builder" => {
                if let Ok(Some(profile_row)) = sqlx::query("SELECT * FROM builder_profiles WHERE user_id = $1")
                    .bind(user_id)
                    .fetch_optional(&app_state.db)
                    .await 
                {
                    let comp_name: Option<String> = profile_row.try_get("company_name").unwrap_or(None);
                    let exp: Option<i32> = profile_row.try_get("experience_years").unwrap_or(None);
                    linked_profile = Some(json!({
                        "profile_type": "builder",
                        "company_name": comp_name,
                        "experience_years": exp
                    }));
                }
            },
            "broker" => {
                if let Ok(Some(profile_row)) = sqlx::query("SELECT * FROM broker_profiles WHERE user_id = $1")
                    .bind(user_id)
                    .fetch_optional(&app_state.db)
                    .await 
                {
                    let agency: Option<String> = profile_row.try_get("agency_name").unwrap_or(None);
                    let area: Option<String> = profile_row.try_get("operating_area").unwrap_or(None);
                    linked_profile = Some(json!({
                        "profile_type": "broker",
                        "agency_name": agency,
                        "operating_area": area
                    }));
                }
            },
            "associate" => {
                // Associate (carecrew) doesn't have a separate profile table usually, or maybe they do?
                // For now, skip or handle if there is an associate_profiles table.
            },
            _ => {}
        }
    }

    let detail = AdminKycDetailResponse {
        id: row.get("id"),
        user_id,
        role,
        full_name: row.try_get("full_name").unwrap_or_default(),
        mobile_number: row.try_get("mobile_number").unwrap_or_default(),
        email_id: row.try_get("email_id").unwrap_or_default(),
        gender: row.try_get("gender").unwrap_or(None),
        date_of_birth: row.try_get("date_of_birth").unwrap_or(None),
        profile_picture_url: row.try_get("profile_picture_url").unwrap_or(None),
        address: row.try_get("street_address").unwrap_or(None),
        govt_id_type: {
            let t: String = row.try_get("govt_id_type").unwrap_or_default();
            t
        },
        govt_id_number: {
            let t: String = row.try_get("govt_id_type").unwrap_or_default();
            let n: String = row.try_get("govt_id_number").unwrap_or_default();
            zd_verification_service::mask_govt_id(&t, &n)
        },
        govt_id_document_url: row.try_get("govt_id_document_url").unwrap_or_default(),
        company_name: row.try_get("company_name").unwrap_or(None),
        services: row.try_get("services").unwrap_or(None),
        experience_document_url: row.try_get("experience_document_url").unwrap_or(None),
        verification_status: row.try_get("verification_status").unwrap_or_else(|_| "pending".to_string()),
        submitted_at: row.try_get("submitted_at").unwrap_or(None),
        verified_at: row.try_get("verified_at").unwrap_or(None),
        reviewed_by: row.try_get("reviewed_by").unwrap_or(None),
        reviewed_at: row.try_get("reviewed_at").unwrap_or(None),
        rejection_reason: row.try_get("rejection_reason").unwrap_or(None),
        linked_profile,
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "KYC detail fetched successfully",
            "data": detail
        })),
    )
}

// ---------------------------------------------------------------------------
// PATCH /api/admin/kyc/{kyc_id}/approve
// ---------------------------------------------------------------------------
pub async fn approve_kyc_submission(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<crate::handlers::admin_auth::AdminClaims>,
    Path(kyc_id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let admin_id = admin_claims.sub.clone();
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    let user_id: Uuid = match sqlx::query_scalar("SELECT user_id FROM kyc_submissions WHERE id = $1")
        .bind(kyc_id)
        .fetch_optional(&mut *tx)
        .await
    {
        Ok(Some(uid)) => uid,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"KYC record not found"}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    match sqlx::query("UPDATE kyc_submissions SET verification_status = 'verified', verified_at = NOW(), reviewed_by = $2, reviewed_at = NOW(), rejection_reason = NULL WHERE id = $1")
        .bind(kyc_id)
        .bind(&admin_id)
        .execute(&mut *tx)
        .await
    {
        Ok(_) => {},
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    }

    // Insert admin audit log
    let _ = sqlx::query("INSERT INTO admin_audit_logs (admin_id, target_user_id, action, reason) VALUES ($1, $2, $3, $4)")
        .bind(&admin_id)
        .bind(user_id)
        .bind("APPROVE_KYC")
        .bind("Approved via Admin Panel")
        .execute(&mut *tx)
        .await;
        
    let _ = crate::utils::admin_logger::log_admin_action(
        &app_state.db,
        &admin_id,
        "kyc_approve",
        "kyc",
        Some(kyc_id),
        Some(json!({ "reason": "Approved via Admin Panel" }))
    ).await;
        
    // Insert notification
    let _ = sqlx::query("INSERT INTO notifications (id, user_id, type, title, message, is_read, created_at) VALUES ($1, $2, $3, $4, $5, false, NOW())")
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind("kyc_approved")
        .bind("KYC Approved")
        .bind("Your KYC submission has been approved.")
        .execute(&mut *tx)
        .await;

    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)})));
    }

    (StatusCode::OK, Json(json!({"success":true,"message":"KYC approved successfully"})))
}

// ---------------------------------------------------------------------------
// PATCH /api/admin/kyc/{kyc_id}/reject
// ---------------------------------------------------------------------------
pub async fn reject_kyc_submission(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<crate::handlers::admin_auth::AdminClaims>,
    Path(kyc_id): Path<Uuid>,
    Json(payload): Json<AdminKycRejectRequest>,
) -> impl axum::response::IntoResponse {
    let admin_id = admin_claims.sub.clone();
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    let user_id: Uuid = match sqlx::query_scalar("SELECT user_id FROM kyc_submissions WHERE id = $1")
        .bind(kyc_id)
        .fetch_optional(&mut *tx)
        .await
    {
        Ok(Some(uid)) => uid,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"KYC record not found"}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    match sqlx::query("UPDATE kyc_submissions SET verification_status = 'rejected', rejection_reason = $2, reviewed_by = $3, reviewed_at = NOW() WHERE id = $1")
        .bind(kyc_id)
        .bind(&payload.reason)
        .bind(&admin_id)
        .execute(&mut *tx)
        .await
    {
        Ok(_) => {},
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    }

    // Insert admin audit log
    let _ = sqlx::query("INSERT INTO admin_audit_logs (admin_id, target_user_id, action, reason) VALUES ($1, $2, $3, $4)")
        .bind(&admin_id)
        .bind(user_id)
        .bind("REJECT_KYC")
        .bind(&payload.reason)
        .execute(&mut *tx)
        .await;
        
    let _ = crate::utils::admin_logger::log_admin_action(
        &app_state.db,
        &admin_id,
        "kyc_reject",
        "kyc",
        Some(kyc_id),
        Some(json!({ "reason": payload.reason }))
    ).await;
        
    // Insert notification
    let _ = sqlx::query("INSERT INTO notifications (id, user_id, type, title, message, is_read, created_at) VALUES ($1, $2, $3, $4, $5, false, NOW())")
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind("kyc_rejected")
        .bind("KYC Rejected")
        .bind(format!("Your KYC submission was rejected. Reason: {}", payload.reason))
        .execute(&mut *tx)
        .await;

    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)})));
    }

    (StatusCode::OK, Json(json!({"success":true,"message":"KYC rejected successfully"})))
}

// ---------------------------------------------------------------------------
// POST /api/admin/kyc/{kyc_id}/extract
// ---------------------------------------------------------------------------
pub async fn extract_kyc_submission(
    State(app_state): State<AppState>,
    Path(kyc_id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
    let row = match sqlx::query(
        "SELECT full_name, govt_id_type, govt_id_number, date_of_birth, govt_id_document_url, extracted_name, extracted_doc_number, extracted_dob, document_integrity_status::TEXT as integrity_status FROM kyc_submissions WHERE id = $1"
    )
    .bind(kyc_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success":false,"message":"KYC record not found"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("DB error: {}", e)})),
            );
        }
    };

    let doc_type: String = row.try_get("govt_id_type").unwrap_or_default();
    let extracted_name: Option<String> = row.try_get("extracted_name").ok().flatten();
    let extracted_doc_num: Option<String> = row.try_get("extracted_doc_number").ok().flatten();
    let extracted_dob: Option<String> = row
        .try_get::<Option<chrono::NaiveDate>, _>("extracted_dob")
        .ok()
        .flatten()
        .map(|d| d.to_string());
    let integrity_status: Option<String> = row.try_get("integrity_status").ok().flatten();

    // Mask extracted doc number if present
    let masked_doc_num = extracted_doc_num.as_ref().map(|d| zd_verification_service::mask_govt_id(&doc_type, d));

    let response = json!({
        "success": true,
        "message": if extracted_name.is_some() || extracted_doc_num.is_some() {
            "Document extraction data retrieved"
        } else {
            "Document extraction pending or no data available"
        },
        "data": {
            "extracted_name": extracted_name,
            "extracted_doc_number": masked_doc_num,
            "extracted_dob": extracted_dob,
            "extracted_income": null,
            "integrity_status": integrity_status,
            "message": if extracted_name.is_some() || extracted_doc_num.is_some() {
                "Document extracted"
            } else {
                "Extraction pending"
            }
        }
    });

    (StatusCode::OK, Json(response))
}

// ---------------------------------------------------------------------------
// POST /api/admin/kyc/{kyc_id}/face-match
// ---------------------------------------------------------------------------
pub async fn face_match_kyc_submission(
    State(app_state): State<AppState>,
    Path(kyc_id): Path<Uuid>,
    _payload: Option<Json<serde_json::Value>>,
) -> impl axum::response::IntoResponse {
    let row = match sqlx::query(
        "SELECT id, profile_picture_url, govt_id_document_url, image_override_urls, face_match_score, face_match_status, face_match_reason FROM kyc_submissions WHERE id = $1"
    )
    .bind(kyc_id)
    .fetch_optional(&app_state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"success":false,"message":"KYC record not found"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success":false,"message":format!("DB error: {}", e)})),
            );
        }
    };

    let profile_picture_url: Option<String> = row.try_get("profile_picture_url").ok();
    let govt_id_document_url: Option<String> = row.try_get("govt_id_document_url").ok();
    let override_urls: Option<serde_json::Value> = row.try_get("image_override_urls").ok();

    // Check image override if present
    let (selfie, id_photo) = if let Some(ref ov) = override_urls {
        let s = ov.get("selfie_url").and_then(|v| v.as_str()).map(|s| s.to_string()).or(profile_picture_url);
        let id = ov.get("id_photo_url").and_then(|v| v.as_str()).map(|s| s.to_string()).or(govt_id_document_url);
        (s, id)
    } else {
        (profile_picture_url, govt_id_document_url)
    };

    if selfie.as_deref().unwrap_or("").trim().is_empty() {
        let _ = sqlx::query(
            "UPDATE kyc_submissions SET face_match_score = NULL, face_match_status = false, face_match_reason = $2 WHERE id = $1"
        )
        .bind(kyc_id)
        .bind(zd_verification_service::REASON_SELFIE_MISSING)
        .execute(&app_state.db)
        .await;

        return (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Selfie missing: face match not evaluated",
                "data": {
                    "match_score": null,
                    "match_status": false,
                    "status": "not_evaluated",
                    "reason": zd_verification_service::REASON_SELFIE_MISSING,
                    "message": "Selfie image is missing"
                }
            })),
        );
    }

    if id_photo.as_deref().unwrap_or("").trim().is_empty() {
        let _ = sqlx::query(
            "UPDATE kyc_submissions SET face_match_score = NULL, face_match_status = false, face_match_reason = $2 WHERE id = $1"
        )
        .bind(kyc_id)
        .bind(zd_verification_service::REASON_ID_PHOTO_MISSING)
        .execute(&app_state.db)
        .await;

        return (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "ID photo missing: face match not evaluated",
                "data": {
                    "match_score": null,
                    "match_status": false,
                    "status": "not_evaluated",
                    "reason": zd_verification_service::REASON_ID_PHOTO_MISSING,
                    "message": "Government ID photo is missing"
                }
            })),
        );
    }

    let existing_score: Option<f64> = row.try_get("face_match_score").ok();
    let existing_status: Option<bool> = row.try_get("face_match_status").ok();
    let existing_reason: Option<String> = row.try_get("face_match_reason").ok();

    let (score, match_status, status_str, reason) = if let Some(s) = existing_score {
        let passed = existing_status.unwrap_or(s >= 85.0);
        (Some(s), passed, if passed { "pass" } else { "review" }, existing_reason)
    } else {
        (None, false, "not_evaluated", Some(zd_verification_service::REASON_KYC_INCOMPLETE.to_string()))
    };

    let response = json!({
        "success": true,
        "message": if score.is_some() { "Face match result fetched" } else { "Face match not yet evaluated" },
        "data": {
            "match_score": score,
            "match_status": match_status,
            "status": status_str,
            "reason": reason,
            "message": if score.is_some() { "Face match evaluated" } else { "Face match has not been computed" }
        }
    });

    (StatusCode::OK, Json(response))
}

// ---------------------------------------------------------------------------
// POST /api/admin/kyc/{kyc_id}/override-images
// ---------------------------------------------------------------------------
pub async fn admin_override_kyc_images(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<crate::handlers::admin_auth::AdminClaims>,
    Path(kyc_id): Path<Uuid>,
    Json(payload): Json<OverrideKycImagesReq>,
) -> impl axum::response::IntoResponse {
    let admin_id = admin_claims.sub.clone();

    if payload.reason.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success":false,"message":"A valid reason must be provided for image override"})),
        );
    }

    // SSRF Prevention: Validate URLs against HTTPS and S3 whitelist
    if let Some(ref selfie) = payload.selfie_url {
        if let Err(e) = zd_verification_service::validate_s3_https_url(selfie) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":format!("Selfie URL rejected: {}", e)})),
            );
        }
    }
    if let Some(ref id_photo) = payload.id_photo_url {
        if let Err(e) = zd_verification_service::validate_s3_https_url(id_photo) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success":false,"message":format!("ID photo URL rejected: {}", e)})),
            );
        }
    }

    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    let user_id: Uuid = match sqlx::query_scalar("SELECT user_id FROM kyc_submissions WHERE id = $1")
        .bind(kyc_id)
        .fetch_optional(&mut *tx)
        .await
    {
        Ok(Some(uid)) => uid,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"KYC record not found"}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    let override_json = json!({
        "selfie_url": payload.selfie_url,
        "id_photo_url": payload.id_photo_url,
        "overridden_by": admin_id,
        "reason": payload.reason,
        "overridden_at": Utc::now()
    });

    let audit_entry = json!({
        "admin_id": admin_id,
        "reason": payload.reason,
        "timestamp": Utc::now(),
        "source": "admin_override_inputs"
    });

    // Invalidate previous face match scores and save override
    let update_res = sqlx::query(
        r#"UPDATE kyc_submissions
           SET image_override_urls = $2,
               image_override_audit = $3,
               face_match_score = NULL,
               face_match_status = NULL,
               face_match_reason = NULL,
               updated_at = NOW()
           WHERE id = $1"#
    )
    .bind(kyc_id)
    .bind(&override_json)
    .bind(&audit_entry)
    .execute(&mut *tx)
    .await;

    if let Err(e) = update_res {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("Failed to update image override: {}", e)})));
    }

    // Log admin action inside the transaction
    let _ = sqlx::query(
        r#"INSERT INTO admin_action_logs (admin_id, action_type, target_type, target_id, details)
           VALUES ($1::TEXT, 'override_kyc_images', 'kyc_submission', $2::TEXT, $3)
           ON CONFLICT DO NOTHING"#
    )
    .bind(&admin_id)
    .bind(kyc_id.to_string())
    .bind(&override_json)
    .execute(&mut *tx)
    .await;

    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("Transaction commit failed: {}", e)})));
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "KYC image URLs overridden successfully. Prior face match scores invalidated.",
            "data": {
                "kyc_id": kyc_id,
                "user_id": user_id,
                "image_override_urls": override_json,
                "source": "admin_override_inputs"
            }
        })),
    )
}

// ---------------------------------------------------------------------------
// POST /api/admin/kyc/{kyc_id}/reveal-id
// ---------------------------------------------------------------------------
pub async fn admin_reveal_govt_id(
    State(app_state): State<AppState>,
    axum::extract::Extension(admin_claims): axum::extract::Extension<crate::handlers::admin_auth::AdminClaims>,
    Path(kyc_id): Path<Uuid>,
    Json(payload): Json<RevealGovtIdReq>,
) -> impl axum::response::IntoResponse {
    let admin_id = admin_claims.sub.clone();

    if payload.reason.trim().len() < 5 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success":false,"message":"A specific reason of at least 5 characters must be provided to reveal unmasked PII"})),
        );
    }

    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    let row = match sqlx::query(
        "SELECT user_id, govt_id_type, govt_id_number FROM kyc_submissions WHERE id = $1"
    )
    .bind(kyc_id)
    .fetch_optional(&mut *tx)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"success":false,"message":"KYC record not found"}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("DB error: {}", e)}))),
    };

    let user_id: Uuid = row.get("user_id");
    let govt_id_type: String = row.get("govt_id_type");
    let govt_id_number: String = row.get("govt_id_number");

    // Audit log this reveal event inside the transaction
    let audit_details = json!({
        "admin_id": admin_id,
        "kyc_id": kyc_id,
        "user_id": user_id,
        "govt_id_type": govt_id_type,
        "reason": payload.reason,
        "revealed_at": Utc::now()
    });

    let _ = sqlx::query(
        r#"INSERT INTO admin_action_logs (admin_id, action_type, target_type, target_id, details)
           VALUES ($1::TEXT, 'reveal_govt_id', 'kyc_submission', $2::TEXT, $3)
           ON CONFLICT DO NOTHING"#
    )
    .bind(&admin_id)
    .bind(kyc_id.to_string())
    .bind(&audit_details)
    .execute(&mut *tx)
    .await;

    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success":false,"message":format!("Transaction commit failed: {}", e)})));
    }

    let response = RevealGovtIdResponse {
        kyc_id,
        user_id,
        govt_id_type,
        govt_id_number,
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "Government ID revealed and audit logged",
            "data": response
        })),
    )
}
