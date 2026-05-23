use crate::dtos::news::{AdminNewsActionRequest, NewsActionRequest, NewsCreateRequest, NewsUpdateRequest};
use crate::models::news::NewsItem;
use crate::utils::api_error::ApiError;
use sqlx::{PgPool, QueryBuilder, Postgres};
use uuid::Uuid;
use redis::AsyncCommands;
use std::sync::Arc;

pub struct NewsService {
    db: PgPool,
    redis_pool: Option<redis::aio::ConnectionManager>,
}

impl NewsService {
    pub fn new(db: PgPool, redis_pool: Option<redis::aio::ConnectionManager>) -> Self {
        Self { db, redis_pool }
    }

    /// Helper to enforce max 7 lines
    fn truncate_summary(summary: &str) -> String {
        let lines: Vec<&str> = summary.lines().collect();
        if lines.len() <= 7 {
            return summary.to_string();
        }
        lines[0..7].join("\n") + "\n..."
    }

    pub async fn fetch_news(&self, category: Option<String>) -> Result<Vec<NewsItem>, ApiError> {
        let cache_key = format!("news_items:cat_{}", category.as_deref().unwrap_or("all"));

        // Try Redis Cache
        if let Some(mut redis_mgr) = self.redis_pool.clone() {
            if let Ok(cached) = redis_mgr.get::<_, String>(&cache_key).await {
                if let Ok(news) = serde_json::from_str(&cached) {
                    return Ok(news);
                }
            }
        }

        let mut query = sqlx::QueryBuilder::<'_, Postgres>::new("SELECT * FROM news_items ");

        if let Some(cat) = &category {
            query.push("WHERE category = ");
            query.push_bind(cat);
        }

        query.push(" ORDER BY is_trending DESC, published_at DESC LIMIT 50");

        let news: Vec<NewsItem> = query
            .build_query_as()
            .fetch_all(&self.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?;

        // Set Redis Cache
        if let Some(mut redis_mgr) = self.redis_pool.clone() {
            if let Ok(json) = serde_json::to_string(&news) {
                let _ = redis_mgr.set_ex::<_, _, ()>(&cache_key, json, 300).await; // 5 min cache
            }
        }

        Ok(news)
    }

    pub async fn create_news(&self, req: NewsCreateRequest) -> Result<NewsItem, ApiError> {
        let id = Uuid::new_v4();
        let summary = Self::truncate_summary(&req.short_summary);

        let news = sqlx::query_as::<_, NewsItem>(
            r#"
            INSERT INTO news_items (id, headline, short_summary, source, category, published_at, thumbnail_url)
            VALUES ($1, $2, $3, $4, $5, COALESCE($6, NOW()), $7)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(req.headline)
        .bind(summary)
        .bind(req.source)
        .bind(req.category)
        .bind(req.published_at)
        .bind(req.thumbnail_url)
        .fetch_one(&self.db)
        .await
        .map_err(|e| {
            if e.to_string().contains("duplicate key") {
                ApiError::BadRequest("A news item with this headline and source already exists.".to_string())
            } else {
                ApiError::InternalServerError(format!("DB error: {}", e))
            }
        })?;

        // Invalidate cache
        self.invalidate_cache().await;

        Ok(news)
    }

    pub async fn update_news(&self, id: Uuid, req: NewsUpdateRequest) -> Result<NewsItem, ApiError> {
        let mut builder = sqlx::QueryBuilder::<'_, Postgres>::new("UPDATE news_items SET ");
        let mut has_updates = false;

        if let Some(h) = req.headline {
            builder.push("headline = ");
            builder.push_bind(h);
            has_updates = true;
        }
        if let Some(s) = req.short_summary {
            if has_updates { builder.push(", "); }
            builder.push("short_summary = ");
            builder.push_bind(Self::truncate_summary(&s));
            has_updates = true;
        }
        if let Some(src) = req.source {
            if has_updates { builder.push(", "); }
            builder.push("source = ");
            builder.push_bind(src);
            has_updates = true;
        }
        if let Some(c) = req.category {
            if has_updates { builder.push(", "); }
            builder.push("category = ");
            builder.push_bind(c);
            has_updates = true;
        }
        if let Some(p) = req.published_at {
            if has_updates { builder.push(", "); }
            builder.push("published_at = ");
            builder.push_bind(p);
            has_updates = true;
        }
        if let Some(t) = req.thumbnail_url {
            if has_updates { builder.push(", "); }
            builder.push("thumbnail_url = ");
            builder.push_bind(t);
            has_updates = true;
        }

        if !has_updates {
            return Err(ApiError::BadRequest("No updates provided".to_string()));
        }

        builder.push(", updated_at = NOW() WHERE id = ");
        builder.push_bind(id);
        builder.push(" RETURNING *");

        let news = builder.build_query_as::<NewsItem>()
            .fetch_optional(&self.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?
            .ok_or(ApiError::NotFound("News item not found".to_string()))?;

        self.invalidate_cache().await;

        Ok(news)
    }

    pub async fn admin_action(&self, id: Uuid, req: AdminNewsActionRequest) -> Result<NewsItem, ApiError> {
        let mut builder = sqlx::QueryBuilder::<'_, Postgres>::new("UPDATE news_items SET ");
        let mut has_updates = false;

        if let Some(ft) = req.force_trending {
            builder.push("force_trending = ");
            builder.push_bind(ft);
            builder.push(", is_trending = ");
            builder.push_bind(ft);
            has_updates = true;
        }
        if let Some(nd) = req.notifications_disabled {
            if has_updates { builder.push(", "); }
            builder.push("notifications_disabled = ");
            builder.push_bind(nd);
            has_updates = true;
        }

        if !has_updates {
            return Err(ApiError::BadRequest("No updates provided".to_string()));
        }

        builder.push(", updated_at = NOW() WHERE id = ");
        builder.push_bind(id);
        builder.push(" RETURNING *");

        let news = builder.build_query_as::<NewsItem>()
            .fetch_optional(&self.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?
            .ok_or(ApiError::NotFound("News item not found".to_string()))?;

        self.invalidate_cache().await;

        Ok(news)
    }

    pub async fn track_action(&self, id: Uuid, req: NewsActionRequest) -> Result<NewsItem, ApiError> {
        // Increment view/click/share and calculate engagement velocity
        let mut builder = sqlx::QueryBuilder::<'_, Postgres>::new("UPDATE news_items SET ");
        let mut has_updates = false;

        if req.view {
            builder.push("views = views + 1");
            has_updates = true;
        }
        if req.click {
            if has_updates { builder.push(", "); }
            builder.push("clicks = clicks + 1");
            has_updates = true;
        }
        if req.share {
            if has_updates { builder.push(", "); }
            builder.push("shares = shares + 1");
            has_updates = true;
        }

        if !has_updates {
            return Err(ApiError::BadRequest("No actions provided".to_string()));
        }

        // Add engagement velocity calculation
        // Formula: (views + clicks * 2 + shares * 5) / (EXTRACT(EPOCH FROM (NOW() - published_at))/3600 + 1)
        builder.push(", engagement_velocity = (views + clicks * 2 + shares * 5) / (GREATEST(EXTRACT(EPOCH FROM (NOW() - published_at))/3600.0, 1.0))");
        
        builder.push(", updated_at = NOW() WHERE id = ");
        builder.push_bind(id);
        builder.push(" RETURNING *");

        let news = builder.build_query_as::<NewsItem>()
            .fetch_optional(&self.db)
            .await
            .map_err(|e| ApiError::InternalServerError(format!("DB error: {}", e)))?
            .ok_or(ApiError::NotFound("News item not found".to_string()))?;

        // Check if it should trigger trending
        if !news.is_trending && !news.force_trending && news.engagement_velocity > 50.0 {
            // Mark as trending
            sqlx::query("UPDATE news_items SET is_trending = TRUE WHERE id = $1")
                .bind(id)
                .execute(&self.db)
                .await
                .ok();

            if !news.notifications_disabled {
                self.trigger_trending_notification(&news).await;
            }
            self.invalidate_cache().await;
        }

        Ok(news)
    }

    async fn trigger_trending_notification(&self, news: &NewsItem) {
        let title = "Trending News 📈";
        let body = format!("{}\n{}", news.headline, news.short_summary.lines().next().unwrap_or(""));

        // Push notification logic: Batch insert into notifications table for all active users
        // Note: For a real app, AWS SNS / FCM broadcast should be used here.
        let result = sqlx::query(
            r#"
            INSERT INTO notifications (id, user_id, title, message, type, is_read, related_entity_id, related_entity_type, created_at)
            SELECT gen_random_uuid(), id, $1, $2, 'news_trending', FALSE, $3, 'news', NOW()
            FROM users
            WHERE is_deleted = FALSE AND fcm_token IS NOT NULL
            "#
        )
        .bind(title)
        .bind(&body)
        .bind(news.id)
        .execute(&self.db)
        .await;

        if let Err(e) = result {
            log::error!("Failed to trigger trending notifications: {}", e);
        } else {
            log::info!("Trending notification triggered for news: {}", news.id);
        }
    }

    async fn invalidate_cache(&self) {
        if let Some(mut redis_mgr) = self.redis_pool.clone() {
            // A simple approach is to delete all news cache keys. 
            // `KEYS news_items:*` -> `DEL`
            if let Ok(keys) = redis_mgr.keys::<_, Vec<String>>("news_items:*").await {
                if !keys.is_empty() {
                    let _ = redis_mgr.del::<_, ()>(&keys).await;
                }
            }
        }
    }
}
