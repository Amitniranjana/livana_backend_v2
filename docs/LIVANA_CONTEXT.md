---
name: livana_context
description: Livana Backend V2 project ka complete context. Jab bhi is project pe kaam ho, yeh skill automatically load hogi taaki bar bar code scan na karna pade.
---

# Livana Backend V2 — Project Context

## Project Path
`c:\Users\yamit\Downloads\Desktop\livana-backendV2\backend-services`

## Stack
- Language: Rust (Edition 2024)
- Framework: Axum 0.8.6
- DB: PostgreSQL (AWS RDS) via SQLx 0.8
- Async: Tokio
- Auth: JWT + bcrypt/argon2
- Storage: AWS S3 (KYC + public listings)
- OTP: AWS SNS (SMS) + Gmail SMTP (email)
- Cache: Redis (optional)
- Chat: AWS Chime (old) + PG-backed (new)
- KYC/OCR: Tesseract

## Active DB (AWS RDS)
- Host: database-1.c8n64wqukf20.us-east-1.rds.amazonaws.com:5432
- DB: postgres
- User: postgres
- Pass: TN5ko5NsW7D2EPr4R2GU
- DATABASE_URL: postgresql://postgres:TN5ko5NsW7D2EPr4R2GU@database-1.c8n64wqukf20.us-east-1.rds.amazonaws.com:5432/postgres

## Server Config
- HTTP_PORT: 9090
- JWT_SECRET_KEY: E9z3i19gKSVQLGOva0bsOpR0Fal3ZmxR
- AWS_REGION: ap-south-1
- KYC_BUCKET_NAME: kyc-docs-livanaeco-2026
- PUBLIC_BUCKET_NAME: livana-public-listings (default)
- REDIS_URL: redis://127.0.0.1:6379

## Architecture
Routes (routes.rs + routes/) -> Handlers (handlers/) -> Services (services/) -> Repository (repository/) -> PostgreSQL

## Module Map
- Auth: handlers/auth.rs (43KB)
- Listings: handlers/listing.rs (46KB)
- Unified Listings: handlers/unified_listing.rs (33KB)
- CareCrew: handlers/carecrew.rs (35KB)
- KYC: handlers/kyc.rs (32KB)
- Chat: handlers/chat_handler.rs (39KB)
- Admin Users: handlers/admin_users.rs (23KB)
- Admin Properties: handlers/admin_properties.rs (21KB)
- Admin Chat: handlers/admin_chat.rs (11KB)
- Admin Reports: handlers/admin_reports.rs (6KB)
- Builder Analytics: handlers/builder_analytics.rs (20KB)
- Community: handlers/community.rs (18KB)
- News: handlers/news.rs
- Builder Projects: handlers/project.rs
- Piingme (Pings): handlers/pings.rs (15KB)
- Zero Deposit: handlers/zero_deposit.rs (18KB)

## Key Files
- src/main.rs - Entry point
- src/app_state.rs - AppState struct
- src/routes.rs - All routes (434 lines)
- src/handlers/ - 47 handler files
- src/services/ - 15 service files
- src/repository/ - 8 repository files
- src/models/ - 14 model files
- migrations/ - 68 SQL migrations (Nov 2025 - Jul 2026)
- .env - Active config (AWS RDS)
- Cargo.toml - Dependencies

## Known Issues
1. RDS from local: IP 119.252.221.226 whitelist karo AWS security group mein
2. cargo sqlx database create fail: Use cargo sqlx migrate run instead
3. SQLX_OFFLINE: .sqlx/ folder mein cached queries - Docker builds mein use hota hai
4. Redis: Optional - app bina Redis ke bhi chalti hai
5. Two chat systems: Old (Chime) + New (PG-backed) - migration in progress
6. GOOGLE_CLIENT_ID: .env mein nahi - hardcoded fallback in main.rs line 60

## Run Commands
`powershell
cargo run
$env:SQLX_OFFLINE="true"; cargo build
cargo sqlx migrate run
cargo sqlx prepare
`

## AppState Fields
- user_service: Arc<UserService>
- db: Pool<Postgres>
- jwt_secret: String
- admin_jwt_secret: String (falls back to jwt_secret)
- chat_service: Arc<ChatService> (AWS Chime)
- kyc_service: Arc<KycService>
- chat_db_service: Arc<ChatDbService> (PG-backed)
- google_client_id: String
- storage_service: Arc<S3Storage> (KYC bucket)
- public_storage_service: Arc<S3Storage> (listings)
- redis_pool: Option<ConnectionManager>
- active_sockets: Arc<DashMap<Uuid, Sender<String>>> (WebSocket)
- news_service: Arc<NewsService>

## Migrations Timeline
- Nov 2025: Users table
- Jan 2026: Broker profiles
- Feb 2026: User profiles, KYC, Listings, Properties, CareCrew, Google OAuth, Chats
- Mar 2026: Associates, Saved properties, Notifications, Communities, Moderation, Vibes, Language, Expo, Jobs, Services, Reviews, Careers
- Apr 2026: Phone OTP, Listing images, Carecrew bookings
- May 2026: Message status, Unified listings, Expo lat/lng
- Jun 2026: Admin audit logs
- Jul 2026: Referrals, Builder profiles, Builder projects, Admin chat, Property reports, Pending Registrations (OTP-first signup)
- Aug 2026: CRM Leads, App Audit Logs, Property private note, Pings (Piingme feature — pings + ping_responses tables)
- Sep 2026: Zero Deposit Financials (zero_deposits, fee_transactions tables)
