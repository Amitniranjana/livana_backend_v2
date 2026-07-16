use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dtos::admin_kyc::{
        AdminKycDetailResponse, AdminKycListData, AdminKycListItemResponse,
        AdminKycListQuery, AdminKycListResponse, AdminKycPagination, AdminKycRejectRequest,
    },
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
         k.govt_id_type, k.experience_document_url, u.role \
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
        query.push(" AND u.role = ");
        query.push_bind(role);
        count_query.push(" AND u.role = ");
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
        "SELECT k.*, u.role \
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
        govt_id_type: row.try_get("govt_id_type").unwrap_or_default(),
        govt_id_number: row.try_get("govt_id_number").unwrap_or_default(),
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
    axum::extract::Extension(admin_id): axum::extract::Extension<String>,
    Path(kyc_id): Path<Uuid>,
) -> impl axum::response::IntoResponse {
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
    axum::extract::Extension(admin_id): axum::extract::Extension<String>,
    Path(kyc_id): Path<Uuid>,
    Json(payload): Json<AdminKycRejectRequest>,
) -> impl axum::response::IntoResponse {
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
