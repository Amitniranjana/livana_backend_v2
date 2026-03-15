mod handlers;
mod routes;
mod app_state;
mod otp;
mod services;
mod repository;
mod models;
mod utils;
mod dtos;

use std::{env, sync::Arc, net::SocketAddr};
use axum::{Router, serve};
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use dotenvy::dotenv;

use crate::{
    app_state::AppState,
    repository::user_repository::UserRepository,
    repository::chat_repository::ChatRepository,
    routes::{
        auth_routes, user_routes, listing_routes, health_routes, broker_routes,
        property_search_routes, suggestions_routes, carecrew_routes, carecrew_ticket_routes,
        recent_chats_routes, associate_routes, careers_routes, reviews_routes,
        saved_properties_routes, notifications_routes,
        property_filter_routes, community_routes,
        moderation_routes, vibes_routes, language_routes,
        expo_routes,
    },


    services::user_service::UserService,
    services::chat_db_service::ChatDbService,
};


#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenv().ok();

    // ——————————————— Load env vars ———————————————
    let database_user =
        env::var("DATABASE_USER").expect("DATABASE_USER must be set");
    let database_host =
        env::var("DATABASE_HOST").expect("DATABASE_HOST must be set");
    let database_password =
        env::var("DATABASE_PASSWORD").expect("DATABASE_PASSWORD must be set");
    let database_name =
        env::var("DATABASE_NAME").expect("DATABASE_NAME must be set");
    let database_port =
        env::var("DATABASE_PORT").expect("DATABASE_PORT must be set");
    // default back to 8080 if you forgot HTTP_PORT
    let http_port = env::var("HTTP_PORT").unwrap_or_else(|_| "8080".into());
    let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "supersecret".into());

    // Google OAuth — needed to verify `aud` in tokeninfo responses.
    // Set GOOGLE_CLIENT_ID in your .env file.
    let google_client_id = env::var("GOOGLE_CLIENT_ID")
        .unwrap_or_else(|_| {
            log::warn!("GOOGLE_CLIENT_ID not set — Google Sign-In will reject all tokens");
            "".to_string()
        });

    // ————————————— Build Postgres pool —————————————
    let db_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        database_user, database_password, database_host, database_port, database_name
    );
    let pool = PgPool::connect(&db_url)
        .await
        .unwrap_or_else(|e| {
            log::error!("DB connect error: {}", e);
            panic!("Could not connect to Postgres");
        });

    // ———————————— Wire up repository & service ————————————
    let user_repo = UserRepository::new(pool.clone());
    let user_svc = UserService::new(user_repo);

    // KYC
    let kyc_repo = Arc::new(crate::repository::kyc_repository::KycRepository::new(pool.clone()));

    // AWS Init
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;

    let chime_app_instance_arn = env::var("CHIME_APP_INSTANCE_ARN").expect("CHIME_APP_INSTANCE_ARN must be set");
    let chat_svc = crate::services::chat_service::ChatService::new(&aws_config, chime_app_instance_arn);

    // S3 & KYC Service
    let s3_client = aws_sdk_s3::Client::new(&aws_config);
    let s3_bucket_name = env::var("KYC_BUCKET_NAME").unwrap_or_else(|_| "livana-kyc-documents".to_string());
    let s3_storage = Arc::new(crate::services::storage::S3Storage::new(s3_client, s3_bucket_name));

    let ocr_service = Arc::new(crate::services::ocr::TesseractOcr::new());

    let kyc_svc = crate::services::kyc_service::KycService::new(
        kyc_repo,
        s3_storage,
        ocr_service
    );

    // ── New: PostgreSQL-backed Chat DB service ──
    let chat_repo = ChatRepository::new(pool.clone());
    let chat_db_svc = ChatDbService::new(chat_repo);

    // ——————————— Build your AppState & Router ———————————
    let app_state = AppState {
        user_service: Arc::from(user_svc),
        db: pool.clone(),
        jwt_secret: jwt_secret.clone(),
        chat_service: Arc::new(chat_svc),
        kyc_service: Arc::new(kyc_svc),
        chat_db_service: Arc::new(chat_db_svc),
        google_client_id,
    };

    let app = Router::new()
        .merge(health_routes())
        .merge(auth_routes())
        .merge(user_routes())
        .merge(listing_routes())
        .merge(broker_routes())
        .merge(crate::routes::chat_routes())
        .merge(crate::routes::kyc_routes())
        // ── Property Search, Filters, Suggestions (Steps 1-3) ──
        .merge(property_search_routes())
        .merge(suggestions_routes())
        // ── CareCrew Module (Step 4) ────────────────────────────
        .merge(carecrew_routes())
        // ── CareCrew Tickets (Support Module) ───────────────────
        .merge(carecrew_ticket_routes())
        // ── Recent Chats (JWT-protected) ─────────────────────────
        .merge(recent_chats_routes())
        // ── Associate Onboarding ─────────────────────────
        .merge(associate_routes())
        // ── Careers and Reviews ─────────────────────────
        .merge(careers_routes())
        .merge(reviews_routes())
        // ── Saved Properties & Notifications (JWT-protected) ────
        .merge(saved_properties_routes())
        .merge(notifications_routes())
        // ── Property Filter & Community (JWT-protected) ──────
        .merge(property_filter_routes())
        .merge(community_routes())
        // ── Moderation & Vibe (JWT-protected) ─────────────
        .merge(moderation_routes())
        .merge(vibes_routes())
        // ── Language Preferences (JWT-protected) ───────────
        .merge(language_routes())
        // ── Expo Event System (JWT-protected) ────────────────
        .merge(expo_routes())
        .nest_service("/uploads", ServeDir::new("uploads"))

        .with_state(app_state);

    // —————————————— Bind & serve ——————————————
    let addr: SocketAddr = format!("0.0.0.0:{}", http_port)
        .parse()
        .expect("Invalid HTTP_PORT");
    println!("Running on http://{}", addr);
    println!("Google OAuth endpoint: POST http://{}/auth/google", addr);
    println!("Recent Chats endpoint: GET  http://{}/api/v1/chats/recent", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    serve(listener, app).await.unwrap();
}
