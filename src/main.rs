#![recursion_limit = "256"]
mod app_state;
mod dtos;
mod handlers;
mod models;
mod otp;
mod repository;
mod routes;
mod services;
mod utils;

use axum::{Router, serve};
use sqlx::PgPool;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use dotenvy::dotenv;

use crate::{
    app_state::AppState,
    repository::chat_repository::ChatRepository,
    repository::user_repository::UserRepository,
    routes::{
        analytics_routes, admin_analytics_routes, admin_auth_routes, admin_stats_routes, associate_routes, auth_routes, bookings_routes, broker_routes,
        carecrew_review_routes, carecrew_routes, carecrew_ticket_routes, career_routes,
        careers_routes, community_routes, expo_routes, health_routes, language_routes,
        listing_routes, moderation_routes, notifications_routes, property_filter_routes,
        property_review_routes, property_search_routes, recent_chats_routes, reviews_routes,
        saved_properties_routes, service_listing_routes, share_routes, suggestions_routes,
        user_routes, unified_listing_routes, vibes_routes, admin_users_routes, admin_properties_routes,
    },
    services::chat_db_service::ChatDbService,
    services::user_service::UserService,
};

#[tokio::main]
async fn main() {
    // Load environment variables from .env file
    dotenv().ok();

    // ——————————————— Load env vars ———————————————
    let database_user = env::var("DATABASE_USER").expect("DATABASE_USER must be set");
    let database_host = env::var("DATABASE_HOST").expect("DATABASE_HOST must be set");
    let database_password = env::var("DATABASE_PASSWORD").expect("DATABASE_PASSWORD must be set");
    let database_name = env::var("DATABASE_NAME").expect("DATABASE_NAME must be set");
    let database_port = env::var("DATABASE_PORT").expect("DATABASE_PORT must be set");
    // default back to 8080 if you forgot HTTP_PORT
    let http_port = env::var("HTTP_PORT").unwrap_or_else(|_| "8080".into());
    let jwt_secret = env::var("JWT_SECRET_KEY").unwrap_or_else(|_| "supersecret".into());
    let admin_jwt_secret = env::var("ADMIN_JWT_SECRET").unwrap_or_else(|_| jwt_secret.clone());

    // Google OAuth — needed to verify `aud` in tokeninfo responses.
    // Set GOOGLE_CLIENT_ID in your .env file.
    let google_client_id = env::var("GOOGLE_CLIENT_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            log::warn!("GOOGLE_CLIENT_ID not set or empty — falling back to hardcoded default");
            "680761079668-qhb7rq6d0ufehtkbb70h1bdv2rpcesq2.apps.googleusercontent.com".to_string()
        });

    // ————————————— Build Postgres pool —————————————
    let db_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        database_user, database_password, database_host, database_port, database_name
    );
    let pool = PgPool::connect(&db_url).await.unwrap_or_else(|e| {
        log::error!("DB connect error: {}", e);
        panic!("Could not connect to Postgres");
    });

    // ————————————— Running Migrations —————————————
    log::info!("Running database migrations...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");
    log::info!("Database migrations applied successfully.");

    // ———————————— Wire up repository & service ————————————
    let user_repo = UserRepository::new(pool.clone());
    let user_svc = UserService::new(user_repo);

    // KYC
    let kyc_repo = Arc::new(crate::repository::kyc_repository::KycRepository::new(
        pool.clone(),
    ));

    // AWS Init
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;

    let chime_app_instance_arn =
        env::var("CHIME_APP_INSTANCE_ARN").expect("CHIME_APP_INSTANCE_ARN must be set");
    let chat_svc =
        crate::services::chat_service::ChatService::new(&aws_config, chime_app_instance_arn);

    // S3 & KYC Service
    let s3_client = aws_sdk_s3::Client::new(&aws_config);
    let s3_bucket_name =
        env::var("KYC_BUCKET_NAME").unwrap_or_else(|_| "livana-kyc-documents".to_string());
    let s3_storage = Arc::new(crate::services::storage::S3Storage::new(
        s3_client.clone(),
        s3_bucket_name,
    ));

    let public_bucket_name =
        env::var("PUBLIC_BUCKET_NAME").unwrap_or_else(|_| "livana-public-listings".to_string());
    let public_storage = Arc::new(crate::services::storage::S3Storage::new(
        s3_client.clone(),
        public_bucket_name,
    ));

    let ocr_service = Arc::new(crate::services::ocr::TesseractOcr::new());

    let kyc_svc =
        crate::services::kyc_service::KycService::new(kyc_repo, s3_storage.clone(), ocr_service);

    // ── New: PostgreSQL-backed Chat DB service ──
    let chat_repo = ChatRepository::new(pool.clone());
    let chat_db_svc = ChatDbService::new(chat_repo);

    // ── Redis (optional — gracefully degrades if unavailable) ──
    let redis_pool = match env::var("REDIS_URL") {
        Ok(url) => {
            match redis::Client::open(url.as_str()) {
                Ok(client) => match redis::aio::ConnectionManager::new(client).await {
                    Ok(mgr) => {
                        log::info!("Redis connected successfully at {}", url);
                        Some(mgr)
                    }
                    Err(e) => {
                        log::warn!("Redis connection failed (caching disabled): {}", e);
                        None
                    }
                },
                Err(e) => {
                    log::warn!("Redis client creation failed (caching disabled): {}", e);
                    None
                }
            }
        }
        Err(_) => {
            log::info!("REDIS_URL not set — caching disabled");
            None
        }
    };

    // ——————————— Build your AppState & Router ———————————
    let app_state = AppState {
        user_service: Arc::from(user_svc),
        db: pool.clone(),
        jwt_secret: jwt_secret.clone(),
        admin_jwt_secret: admin_jwt_secret.clone(),
        chat_service: Arc::new(chat_svc),
        kyc_service: Arc::new(kyc_svc),
        chat_db_service: Arc::new(chat_db_svc),
        google_client_id,
        storage_service: s3_storage.clone(),
        public_storage_service: public_storage.clone(),
        redis_pool: redis_pool.clone(),
        active_sockets: Arc::new(dashmap::DashMap::new()),
        news_service: Arc::new(crate::services::news_service::NewsService::new(
            pool.clone(),
            redis_pool,
        )),
    };

    let app = Router::new()
        .merge(health_routes())
        .merge(auth_routes())
        .merge(admin_auth_routes())
        .merge(admin_stats_routes())
        .merge(admin_analytics_routes())
        .merge(admin_users_routes(app_state.clone()))
        .merge(admin_properties_routes(app_state.clone()))
        .merge(user_routes())
        .merge(listing_routes())
        .merge(broker_routes())
        .merge(crate::routes::chat_routes())
        .merge(crate::routes::kyc_routes())
        .merge(crate::routes::chat::api_chats_routes())
        .merge(crate::routes::visit::api_visit_routes())
        // ── Property Search, Filters, Suggestions (Steps 1-3) ──
        .merge(property_search_routes())
        .merge(suggestions_routes())
        // ── CareCrew Module (Step 4) ────────────────────────────
        .merge(carecrew_routes())
        .merge(bookings_routes())
        // ── CareCrew Tickets (Support Module) ───────────────────
        .merge(carecrew_ticket_routes())
        // ── Recent Chats (JWT-protected) ─────────────────────────
        .merge(recent_chats_routes())
        // ── Associate Onboarding ─────────────────────────
        .merge(associate_routes())
        // ── Careers and Reviews ─────────────────────────
        .merge(career_routes())
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
        // ── Service Listing + Reviews (JWT-protected) ────────
        .merge(service_listing_routes())
        .merge(carecrew_review_routes())
        .merge(property_review_routes())
        // ── Analytics (public) ─────────────────────────────────────────────────
        .merge(analytics_routes())
        // ── Unified Listings API (new) ──────────────────────────────────────────
        .merge(unified_listing_routes())
        // ── Property Share (public, no auth) ────────────────────────────────────
        .merge(share_routes())
        // ── News API ────────────────────────────────────────────────────────────
        .merge(crate::routes::news_routes())
        .nest_service("/uploads", ServeDir::new("uploads"))
        .with_state(app_state);

    // —————————————— Bind & serve ——————————————
    let addr: SocketAddr = format!("0.0.0.0:{}", http_port)
        .parse()
        .expect("Invalid HTTP_PORT");
    println!("Running on http://{}", addr);
    println!("Google OAuth endpoint: POST http://{}/auth/google", addr);
    println!(
        "Recent Chats endpoint: GET  http://{}/api/v1/chats/recent",
        addr
    );

    let listener = TcpListener::bind(addr).await.unwrap();
    serve(listener, app).await.unwrap();
}
