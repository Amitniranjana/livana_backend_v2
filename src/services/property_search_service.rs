/// Property Search Service
/// Business logic layer for property search, filters, and suggestions.
/// Converts raw DB rows into structured JSON responses with pagination metadata.

use sqlx::{Pool, Postgres, Row};
use serde_json::{json, Value};
use crate::repository::property_search_repository::{
    self, PropertySearchFilters,
};

// ─── Helper: row → JSON property object ──────────────────────────────────────

fn row_to_property_json(row: &sqlx::postgres::PgRow) -> Value {
    let images_val: Value = row.try_get("images").unwrap_or(json!([]));
    let images: Vec<String> = serde_json::from_value(images_val).unwrap_or_default();
    let amenities_val: Value = row.try_get("amenities").unwrap_or(json!([]));

    json!({
        "propertyId":    row.try_get::<uuid::Uuid,_>("id").map(|u|u.to_string()).unwrap_or_default(),
        "title":         row.try_get::<String,_>("title").unwrap_or_default(),
        "price":         row.try_get::<Option<i64>,_>("price").unwrap_or_default(),
        "area":          row.try_get::<Option<i32>,_>("area_sqft").unwrap_or_default(),
        "address": {
            "city":     row.try_get::<String,_>("city").unwrap_or_default(),
            "locality": row.try_get::<Option<String>,_>("locality").unwrap_or_default(),
            "pincode":  row.try_get::<Option<String>,_>("pincode").unwrap_or_default(),
            "full":     row.try_get::<Option<String>,_>("address").unwrap_or_default(),
        },
        "geo": {
            "lat": row.try_get::<Option<f64>,_>("lat").unwrap_or_default(),
            "lng": row.try_get::<Option<f64>,_>("lng").unwrap_or_default(),
        },
        "propertyType":  row.try_get::<String,_>("property_type").unwrap_or_default(),
        "bhk":           row.try_get::<Option<i32>,_>("bhk").unwrap_or_default(),
        "furnishing":    row.try_get::<Option<String>,_>("furnishing").unwrap_or_default(),
        "availability":  row.try_get::<Option<String>,_>("availability").unwrap_or_default(),
        "images":        images,
        "primaryImage":  row.try_get::<Option<String>,_>("primary_image").unwrap_or_default(),
        "isVerified":    row.try_get::<bool,_>("is_verified").unwrap_or(false),
        "postedBy":      row.try_get::<Option<String>,_>("posted_by").unwrap_or_default(),
        "projectName":   row.try_get::<Option<String>,_>("project_name").unwrap_or_default(),
        "builderName":   row.try_get::<Option<String>,_>("builder_name").unwrap_or_default(),
        "landmark":      row.try_get::<Option<String>,_>("landmark").unwrap_or_default(),
        "amenities":     amenities_val,
        "postedDate":    row.try_get::<chrono::DateTime<chrono::Utc>,_>("created_at")
                            .map(|d| d.to_rfc3339())
                            .unwrap_or_default(),
    })
}

// ─── Search ───────────────────────────────────────────────────────────────────

pub struct SearchResult {
    pub properties: Vec<Value>,
    pub total_count: i64,
    pub current_page: i32,
    pub total_pages: i32,
    pub limit: i32,
}

pub async fn search_properties(
    db: &Pool<Postgres>,
    filters: PropertySearchFilters,
) -> Result<SearchResult, sqlx::Error> {
    let (rows, total) = tokio::try_join!(
        property_search_repository::search_properties(db, &filters),
        property_search_repository::count_search_results(db, &filters),
    )?;

    let properties: Vec<Value> = rows.iter().map(row_to_property_json).collect();
    let limit = filters.limit;
    let page = filters.page;
    let total_pages = ((total as f64) / (limit as f64)).ceil() as i32;
    let total_pages = total_pages.max(1);

    Ok(SearchResult { properties, total_count: total, current_page: page, total_pages, limit })
}

// ─── Filters ──────────────────────────────────────────────────────────────────

pub async fn get_filters(
    db: &Pool<Postgres>,
    city: Option<&str>,
) -> Result<Value, sqlx::Error> {
    let range_row = property_search_repository::get_price_area_ranges(db, city).await?;

    let min_price: i64 = range_row.try_get("min_price").unwrap_or(0);
    let max_price: i64 = range_row.try_get("max_price").unwrap_or(100_000_000);
    let min_area: i32 = range_row.try_get("min_area").unwrap_or(0);
    let max_area: i32 = range_row.try_get("max_area").unwrap_or(10_000);

    Ok(json!({
        "priceRange": { "min": min_price, "max": max_price },
        "areaRange":  { "min": min_area,  "max": max_area  },
        "bhkOptions": [
            { "value": 1, "label": "1 BHK" },
            { "value": 2, "label": "2 BHK" },
            { "value": 3, "label": "3 BHK" },
            { "value": 4, "label": "4+ BHK" }
        ],
        "propertyTypes": [
            { "value": "flat",       "label": "Flat / Apartment" },
            { "value": "villa",      "label": "Villa / House"    },
            { "value": "plot",       "label": "Plot / Land"      },
            { "value": "commercial", "label": "Commercial"       }
        ],
        "furnishingOptions": [
            { "value": "unfurnished", "label": "Unfurnished"      },
            { "value": "semi",        "label": "Semi-Furnished"   },
            { "value": "furnished",   "label": "Fully Furnished"  }
        ],
        "availabilityOptions": [
            { "value": "ready_to_move",      "label": "Ready to Move"       },
            { "value": "under_construction", "label": "Under Construction"  }
        ],
        "postedByOptions": [
            { "value": "owner",   "label": "Owner"   },
            { "value": "broker",  "label": "Broker"  },
            { "value": "builder", "label": "Builder" }
        ],
        "amenities": [
            "lift", "parking", "gym", "swimming_pool", "power_backup",
            "security", "garden", "club_house", "intercom", "fire_safety"
        ],
        "sortOptions": [
            { "value": "relevance",  "label": "Relevance"        },
            { "value": "newest",     "label": "Newest First"     },
            { "value": "price_asc",  "label": "Price: Low → High"},
            { "value": "price_desc", "label": "Price: High → Low"}
        ]
    }))
}

// ─── Suggestions ─────────────────────────────────────────────────────────────

pub async fn get_suggestions(
    db: &Pool<Postgres>,
    q: &str,
) -> Result<Value, sqlx::Error> {
    if q.trim().len() < 2 {
        return Ok(json!({
            "city": [], "locality": [], "project": [], "builder": [], "landmark": []
        }));
    }

    let rows = property_search_repository::get_suggestions(db, q, 25).await?;

    let mut cities: Vec<String> = vec![];
    let mut localities: Vec<String> = vec![];
    let mut projects: Vec<String> = vec![];
    let mut builders: Vec<String> = vec![];
    let mut landmarks: Vec<String> = vec![];

    for row in &rows {
        let category: String = row.try_get("category").unwrap_or_default();
        let value: String = row.try_get("value").unwrap_or_default();
        match category.as_str() {
            "city"     => cities.push(value),
            "locality" => localities.push(value),
            "project"  => projects.push(value),
            "builder"  => builders.push(value),
            "landmark" => landmarks.push(value),
            _          => {}
        }
    }

    Ok(json!({
        "city":     cities,
        "locality": localities,
        "project":  projects,
        "builder":  builders,
        "landmark": landmarks,
    }))
}
