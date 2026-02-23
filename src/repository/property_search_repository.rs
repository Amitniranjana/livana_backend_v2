/// Property Search Repository
/// Handles all raw SQL queries for property search, filters, and suggestions.
/// Uses `sqlx::query()` inline approach — consistent with the rest of the codebase.

use sqlx::{Pool, Postgres, Row};

/// Strongly-typed filter params passed from the service layer.
#[derive(Debug, Clone, Default)]
pub struct PropertySearchFilters {
    pub q: Option<String>,
    pub city: Option<String>,
    pub locality: Option<String>,
    pub pincode: Option<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
    pub bhk: Option<Vec<i32>>,
    pub property_type: Option<Vec<String>>,
    pub furnishing: Option<Vec<String>>,
    pub min_area: Option<i32>,
    pub max_area: Option<i32>,
    pub amenities: Option<Vec<String>>,
    pub posted_by: Option<Vec<String>>,
    pub sort: Option<String>,
    pub page: i32,
    pub limit: i32,
}

/// Executes the dynamic property search query.
/// Returns the matching rows as raw sqlx AnyRows.
pub async fn search_properties(
    db: &Pool<Postgres>,
    filters: &PropertySearchFilters,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    let offset = ((filters.page - 1) * filters.limit) as i64;

    let order_by = match filters.sort.as_deref() {
        Some("newest") => "p.created_at DESC",
        Some("price_asc") => "p.price ASC",
        Some("price_desc") => "p.price DESC",
        _ => "p.is_featured DESC, p.created_at DESC", // relevance default
    };

    // Build dynamic WHERE clauses as a string with positional params tracked manually.
    // We use a parameterized query with sqlx::query and bind params.
    // Because Rust's sqlx doesn't support fully dynamic binding with query!, we use sqlx::query.

    let mut conditions: Vec<String> = vec!["p.status = 'active'".to_string()];
    let mut param_idx: i32 = 1;

    // We'll build bind values in order and collect them.
    // text search (q) — searches across city, locality, project_name, builder_name, landmark, pincode
    if let Some(q) = &filters.q {
        if !q.trim().is_empty() {
            conditions.push(format!(
                "(p.city ILIKE ${pi} OR p.locality ILIKE ${pi} OR p.pincode ILIKE ${pi} \
                 OR p.project_name ILIKE ${pi} OR p.builder_name ILIKE ${pi} OR p.landmark ILIKE ${pi})",
                pi = param_idx
            ));
            param_idx += 1;
        }
    }

    if let Some(city) = &filters.city {
        if !city.trim().is_empty() {
            conditions.push(format!("p.city ILIKE ${}", param_idx));
            param_idx += 1;
        }
    }

    if let Some(locality) = &filters.locality {
        if !locality.trim().is_empty() {
            conditions.push(format!("p.locality ILIKE ${}", param_idx));
            param_idx += 1;
        }
    }

    if let Some(pincode) = &filters.pincode {
        if !pincode.trim().is_empty() {
            conditions.push(format!("p.pincode = ${}", param_idx));
            param_idx += 1;
        }
    }

    if filters.min_price.is_some() {
        conditions.push(format!("p.price >= ${}", param_idx));
        param_idx += 1;
    }

    if filters.max_price.is_some() {
        conditions.push(format!("p.price <= ${}", param_idx));
        param_idx += 1;
    }

    if let Some(bhks) = &filters.bhk {
        if !bhks.is_empty() {
            let placeholders: Vec<String> = bhks.iter().enumerate()
                .map(|(i, _)| format!("${}", param_idx + i as i32))
                .collect();
            conditions.push(format!("p.bhk IN ({})", placeholders.join(", ")));
            param_idx += bhks.len() as i32;
        }
    }

    if let Some(ptypes) = &filters.property_type {
        if !ptypes.is_empty() {
            let placeholders: Vec<String> = ptypes.iter().enumerate()
                .map(|(i, _)| format!("${}", param_idx + i as i32))
                .collect();
            conditions.push(format!("p.property_type IN ({})", placeholders.join(", ")));
            param_idx += ptypes.len() as i32;
        }
    }

    if let Some(furnishings) = &filters.furnishing {
        if !furnishings.is_empty() {
            let placeholders: Vec<String> = furnishings.iter().enumerate()
                .map(|(i, _)| format!("${}", param_idx + i as i32))
                .collect();
            conditions.push(format!("p.furnishing IN ({})", placeholders.join(", ")));
            param_idx += furnishings.len() as i32;
        }
    }

    if filters.min_area.is_some() {
        conditions.push(format!("p.area_sqft >= ${}", param_idx));
        param_idx += 1;
    }

    if filters.max_area.is_some() {
        conditions.push(format!("p.area_sqft <= ${}", param_idx));
        param_idx += 1;
    }

    if let Some(amenity_list) = &filters.amenities {
        if !amenity_list.is_empty() {
            // Check JSONB array contains each provided amenity
            for amenity in amenity_list {
                let _ = amenity; // used below
                conditions.push(format!("p.amenities @> ${}", param_idx));
                param_idx += 1;
            }
        }
    }

    if let Some(posted_by) = &filters.posted_by {
        if !posted_by.is_empty() {
            let placeholders: Vec<String> = posted_by.iter().enumerate()
                .map(|(i, _)| format!("${}", param_idx + i as i32))
                .collect();
            conditions.push(format!("p.posted_by IN ({})", placeholders.join(", ")));
            param_idx += posted_by.len() as i32;
        }
    }

    // LIMIT & OFFSET params
    let limit_param = param_idx;
    let offset_param = param_idx + 1;

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        r#"
        SELECT
            p.id, p.title, p.price, p.area_sqft, p.city, p.locality, p.address, p.pincode,
            p.lat, p.lng, p.property_type, p.bhk, p.furnishing, p.availability,
            p.images, p.primary_image, p.is_verified, p.posted_by, p.user_id,
            p.amenities, p.project_name, p.builder_name, p.landmark,
            p.created_at
        FROM properties p
        WHERE {where_clause}
        ORDER BY {order_by}
        LIMIT ${limit_param} OFFSET ${offset_param}
        "#
    );

    // Now bind params in the EXACT same order as we added conditions.
    let mut q = sqlx::query(&sql);

    if let Some(text) = &filters.q {
        if !text.trim().is_empty() {
            let pattern = format!("%{}%", text);
            q = q.bind(pattern);
        }
    }
    if let Some(city) = &filters.city {
        if !city.trim().is_empty() {
            q = q.bind(format!("%{}%", city));
        }
    }
    if let Some(locality) = &filters.locality {
        if !locality.trim().is_empty() {
            q = q.bind(format!("%{}%", locality));
        }
    }
    if let Some(pincode) = &filters.pincode {
        if !pincode.trim().is_empty() {
            q = q.bind(pincode.clone());
        }
    }
    if let Some(v) = filters.min_price {
        q = q.bind(v);
    }
    if let Some(v) = filters.max_price {
        q = q.bind(v);
    }
    if let Some(bhks) = &filters.bhk {
        for bhk in bhks {
            q = q.bind(*bhk);
        }
    }
    if let Some(ptypes) = &filters.property_type {
        for pt in ptypes {
            q = q.bind(pt.clone());
        }
    }
    if let Some(furnishings) = &filters.furnishing {
        for f in furnishings {
            q = q.bind(f.clone());
        }
    }
    if let Some(v) = filters.min_area {
        q = q.bind(v);
    }
    if let Some(v) = filters.max_area {
        q = q.bind(v);
    }
    if let Some(amenity_list) = &filters.amenities {
        for amenity in amenity_list {
            let json_val = serde_json::json!([amenity]);
            q = q.bind(json_val);
        }
    }
    if let Some(posted_by) = &filters.posted_by {
        for pb in posted_by {
            q = q.bind(pb.clone());
        }
    }
    q = q.bind(filters.limit as i64);
    q = q.bind(offset);

    q.fetch_all(db).await
}

/// Counts total matching properties for pagination.
pub async fn count_search_results(
    db: &Pool<Postgres>,
    filters: &PropertySearchFilters,
) -> Result<i64, sqlx::Error> {
    let mut conditions: Vec<String> = vec!["p.status = 'active'".to_string()];
    let mut param_idx: i32 = 1;

    if let Some(q) = &filters.q {
        if !q.trim().is_empty() {
            conditions.push(format!(
                "(p.city ILIKE ${pi} OR p.locality ILIKE ${pi} OR p.pincode ILIKE ${pi} \
                 OR p.project_name ILIKE ${pi} OR p.builder_name ILIKE ${pi} OR p.landmark ILIKE ${pi})",
                pi = param_idx
            ));
            param_idx += 1;
        }
    }
    if let Some(city) = &filters.city {
        if !city.trim().is_empty() {
            conditions.push(format!("p.city ILIKE ${}", param_idx));
            param_idx += 1;
        }
    }
    if let Some(locality) = &filters.locality {
        if !locality.trim().is_empty() {
            conditions.push(format!("p.locality ILIKE ${}", param_idx));
            param_idx += 1;
        }
    }
    if let Some(pincode) = &filters.pincode {
        if !pincode.trim().is_empty() {
            conditions.push(format!("p.pincode = ${}", param_idx));
            param_idx += 1;
        }
    }
    if filters.min_price.is_some() {
        conditions.push(format!("p.price >= ${}", param_idx));
        param_idx += 1;
    }
    if filters.max_price.is_some() {
        conditions.push(format!("p.price <= ${}", param_idx));
        param_idx += 1;
    }
    if let Some(bhks) = &filters.bhk {
        if !bhks.is_empty() {
            let placeholders: Vec<String> = bhks.iter().enumerate()
                .map(|(i, _)| format!("${}", param_idx + i as i32))
                .collect();
            conditions.push(format!("p.bhk IN ({})", placeholders.join(", ")));
            param_idx += bhks.len() as i32;
        }
    }
    if let Some(ptypes) = &filters.property_type {
        if !ptypes.is_empty() {
            let placeholders: Vec<String> = ptypes.iter().enumerate()
                .map(|(i, _)| format!("${}", param_idx + i as i32))
                .collect();
            conditions.push(format!("p.property_type IN ({})", placeholders.join(", ")));
            param_idx += ptypes.len() as i32;
        }
    }
    if let Some(furnishings) = &filters.furnishing {
        if !furnishings.is_empty() {
            let placeholders: Vec<String> = furnishings.iter().enumerate()
                .map(|(i, _)| format!("${}", param_idx + i as i32))
                .collect();
            conditions.push(format!("p.furnishing IN ({})", placeholders.join(", ")));
            param_idx += furnishings.len() as i32;
        }
    }
    if filters.min_area.is_some() {
        conditions.push(format!("p.area_sqft >= ${}", param_idx));
        param_idx += 1;
    }
    if filters.max_area.is_some() {
        conditions.push(format!("p.area_sqft <= ${}", param_idx));
        param_idx += 1;
    }
    if let Some(amenity_list) = &filters.amenities {
        for _ in amenity_list {
            conditions.push(format!("p.amenities @> ${}", param_idx));
            param_idx += 1;
        }
    }
    if let Some(posted_by) = &filters.posted_by {
        if !posted_by.is_empty() {
            let placeholders: Vec<String> = posted_by.iter().enumerate()
                .map(|(i, _)| format!("${}", param_idx + i as i32))
                .collect();
            conditions.push(format!("p.posted_by IN ({})", placeholders.join(", ")));
        }
    }

    let sql = format!(
        "SELECT COUNT(*) as total FROM properties p WHERE {}",
        conditions.join(" AND ")
    );

    let mut q = sqlx::query(&sql);

    if let Some(text) = &filters.q {
        if !text.trim().is_empty() {
            q = q.bind(format!("%{}%", text));
        }
    }
    if let Some(city) = &filters.city {
        if !city.trim().is_empty() {
            q = q.bind(format!("%{}%", city));
        }
    }
    if let Some(locality) = &filters.locality {
        if !locality.trim().is_empty() {
            q = q.bind(format!("%{}%", locality));
        }
    }
    if let Some(pincode) = &filters.pincode {
        if !pincode.trim().is_empty() {
            q = q.bind(pincode.clone());
        }
    }
    if let Some(v) = filters.min_price { q = q.bind(v); }
    if let Some(v) = filters.max_price { q = q.bind(v); }
    if let Some(bhks) = &filters.bhk { for b in bhks { q = q.bind(*b); } }
    if let Some(ptypes) = &filters.property_type { for p in ptypes { q = q.bind(p.clone()); } }
    if let Some(furnishings) = &filters.furnishing { for f in furnishings { q = q.bind(f.clone()); } }
    if let Some(v) = filters.min_area { q = q.bind(v); }
    if let Some(v) = filters.max_area { q = q.bind(v); }
    if let Some(amenity_list) = &filters.amenities {
        for amenity in amenity_list {
            q = q.bind(serde_json::json!([amenity]));
        }
    }
    if let Some(posted_by) = &filters.posted_by {
        for pb in posted_by { q = q.bind(pb.clone()); }
    }

    let row = q.fetch_one(db).await?;
    Ok(row.get::<i64, _>("total"))
}

/// Returns price range and area range for contextual filter display.
pub async fn get_price_area_ranges(
    db: &Pool<Postgres>,
    city: Option<&str>,
) -> Result<sqlx::postgres::PgRow, sqlx::Error> {
    let sql = if let Some(c) = city {
        if !c.is_empty() {
            return sqlx::query(
                r#"
                SELECT
                    COALESCE(MIN(price), 0) AS min_price,
                    COALESCE(MAX(price), 100000000) AS max_price,
                    COALESCE(MIN(area_sqft), 0) AS min_area,
                    COALESCE(MAX(area_sqft), 10000) AS max_area
                FROM properties
                WHERE status = 'active' AND city ILIKE $1
                "#
            )
            .bind(format!("%{}%", c))
            .fetch_one(db)
            .await;
        }
        "SELECT COALESCE(MIN(price),0) AS min_price, COALESCE(MAX(price),100000000) AS max_price, COALESCE(MIN(area_sqft),0) AS min_area, COALESCE(MAX(area_sqft),10000) AS max_area FROM properties WHERE status = 'active'"
    } else {
        "SELECT COALESCE(MIN(price),0) AS min_price, COALESCE(MAX(price),100000000) AS max_price, COALESCE(MIN(area_sqft),0) AS min_area, COALESCE(MAX(area_sqft),10000) AS max_area FROM properties WHERE status = 'active'"
    };

    sqlx::query(sql).fetch_one(db).await
}

/// Returns distinct suggestions for autocomplete across all search dimensions.
pub async fn get_suggestions(
    db: &Pool<Postgres>,
    q: &str,
    limit: i64,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    let pattern = format!("%{}%", q);
    sqlx::query(
        r#"
        (SELECT 'city'     AS category, city        AS value FROM properties WHERE status='active' AND city        ILIKE $1 GROUP BY city        LIMIT $2)
        UNION ALL
        (SELECT 'locality' AS category, locality    AS value FROM properties WHERE status='active' AND locality    ILIKE $1 AND locality IS NOT NULL GROUP BY locality    LIMIT $2)
        UNION ALL
        (SELECT 'project'  AS category, project_name AS value FROM properties WHERE status='active' AND project_name ILIKE $1 AND project_name IS NOT NULL GROUP BY project_name LIMIT $2)
        UNION ALL
        (SELECT 'builder'  AS category, builder_name AS value FROM properties WHERE status='active' AND builder_name ILIKE $1 AND builder_name IS NOT NULL GROUP BY builder_name LIMIT $2)
        UNION ALL
        (SELECT 'landmark' AS category, landmark    AS value FROM properties WHERE status='active' AND landmark    ILIKE $1 AND landmark IS NOT NULL GROUP BY landmark    LIMIT $2)
        LIMIT $3
        "#
    )
    .bind(pattern)
    .bind(5i64)   // max 5 per category
    .bind(limit)
    .fetch_all(db)
    .await
}
