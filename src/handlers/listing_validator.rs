use crate::dtos::unified_listing::CreateListingPayload;

// ─────────────────────────────────────────────────────────────────────────────
// Valid enum values
// ─────────────────────────────────────────────────────────────────────────────

const VALID_PROPERTY_TYPES: &[&str] = &["Residential", "Commercial", "Land"];
const VALID_LISTING_TYPES: &[&str] = &["Rent", "Sell", "PG", "Space Sharing"];
const VALID_USER_TYPES: &[&str] = &["User", "Broker", "Associate"];
const VALID_FURNISHING: &[&str] = &["Unfurnished", "Semi-Furnished", "Fully-Furnished"];
const VALID_FACING: &[&str] = &["North", "South", "East", "West", "North-East", "North-West", "South-East", "South-West"];

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Validate the incoming `CreateListingPayload` according to business rules.
/// Returns `Ok(())` on success, or `Err(Vec<String>)` with a list of human-readable errors.
pub fn validate_listing(payload: &CreateListingPayload) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    // ── 1. Enum validation ──────────────────────────────────────────────────
    if !VALID_PROPERTY_TYPES.contains(&payload.property_type.as_str()) {
        errors.push(format!(
            "Invalid property_type '{}'. Must be one of: {:?}",
            payload.property_type, VALID_PROPERTY_TYPES
        ));
    }
    if !VALID_LISTING_TYPES.contains(&payload.listing_type.as_str()) {
        errors.push(format!(
            "Invalid listing_type '{}'. Must be one of: {:?}",
            payload.listing_type, VALID_LISTING_TYPES
        ));
    }
    if !VALID_USER_TYPES.contains(&payload.user_type.as_str()) {
        errors.push(format!(
            "Invalid user_type '{}'. Must be one of: {:?}",
            payload.user_type, VALID_USER_TYPES
        ));
    }
    if let Some(ref f) = payload.furnishing {
        if !VALID_FURNISHING.contains(&f.as_str()) {
            errors.push(format!(
                "Invalid furnishing '{}'. Must be one of: {:?}",
                f, VALID_FURNISHING
            ));
        }
    }
    if let Some(ref f) = payload.facing {
        if !VALID_FACING.contains(&f.as_str()) {
            errors.push(format!(
                "Invalid facing '{}'. Must be one of: {:?}",
                f, VALID_FACING
            ));
        }
    }

    // Stop early if enums are invalid — subsequent rules depend on them
    if !errors.is_empty() {
        return Err(errors);
    }

    // ── 2. Common field validation ──────────────────────────────────────────
    if payload.title.trim().is_empty() {
        errors.push("title: cannot be empty".to_string());
    }
    if payload.description.trim().is_empty() {
        errors.push("description: cannot be empty".to_string());
    }
    if payload.location.trim().is_empty() {
        errors.push("location: cannot be empty".to_string());
    }
    if payload.price < 0 {
        errors.push("price: must be a non-negative integer".to_string());
    }
    if payload.deposit < 0 {
        errors.push("deposit: must be a non-negative integer".to_string());
    }
    if payload.area_sqft <= 0 {
        errors.push("area_sqft: must be a positive integer".to_string());
    }

    // ── 3. Sell listings: deposit MUST be 0 ─────────────────────────────────
    if payload.listing_type == "Sell" && payload.deposit != 0 {
        errors.push("deposit: must be 0 for Sell listings".to_string());
    }

    // ── 4. User type restriction: Users cannot create Sell listings ──────────
    if payload.user_type == "User" && payload.listing_type == "Sell" {
        errors.push("Users are not allowed to create Sell listings. Only Brokers and Associates can.".to_string());
    }

    // ── 5. Residential: require bedroom/bathroom fields ─────────────────────
    if payload.property_type == "Residential" {
        if payload.bedrooms.is_none() {
            errors.push("bedrooms: required for Residential properties".to_string());
        }
        if payload.bathrooms.is_none() {
            errors.push("bathrooms: required for Residential properties".to_string());
        }
        if payload.no_of_toilets.is_none() {
            errors.push("no_of_toilets: required for Residential properties".to_string());
        }
        if payload.no_of_balconies.is_none() {
            errors.push("no_of_balconies: required for Residential properties".to_string());
        }
    }

    // ── 6. Commercial: must NOT have residential fields ─────────────────────
    if payload.property_type == "Commercial" {
        if payload.bedrooms.is_some() {
            errors.push("bedrooms: not allowed for Commercial properties".to_string());
        }
        if payload.bathrooms.is_some() {
            errors.push("bathrooms: not allowed for Commercial properties".to_string());
        }
        if payload.furnishing.is_some() {
            errors.push("furnishing: not allowed for Commercial properties".to_string());
        }
        if payload.no_of_toilets.is_some() {
            errors.push("no_of_toilets: not allowed for Commercial properties".to_string());
        }
        if payload.no_of_balconies.is_some() {
            errors.push("no_of_balconies: not allowed for Commercial properties".to_string());
        }
    }

    // ── 7. Land: only land_type + common fields ─────────────────────────────
    if payload.property_type == "Land" {
        if payload.bedrooms.is_some() {
            errors.push("bedrooms: not allowed for Land properties".to_string());
        }
        if payload.bathrooms.is_some() {
            errors.push("bathrooms: not allowed for Land properties".to_string());
        }
        if payload.furnishing.is_some() {
            errors.push("furnishing: not allowed for Land properties".to_string());
        }
        if payload.floor.is_some() {
            errors.push("floor: not allowed for Land properties".to_string());
        }
        if payload.total_floors.is_some() {
            errors.push("total_floors: not allowed for Land properties".to_string());
        }
        if payload.no_of_toilets.is_some() {
            errors.push("no_of_toilets: not allowed for Land properties".to_string());
        }
        if payload.no_of_balconies.is_some() {
            errors.push("no_of_balconies: not allowed for Land properties".to_string());
        }
    }

    // ── 8. Space Sharing: gender_preference + roommates required ────────────
    if payload.listing_type == "Space Sharing" {
        if payload.gender_preference.is_none() {
            errors.push("gender_preference: required for Space Sharing listings".to_string());
        }
        if payload.roommates.is_none() {
            errors.push("roommates: required for Space Sharing listings".to_string());
        }
    }

    // ── 9. PG: gender_preference + roommates required ───────────────────────
    if payload.listing_type == "PG" {
        if payload.gender_preference.is_none() {
            errors.push("gender_preference: required for PG listings".to_string());
        }
        if payload.roommates.is_none() {
            errors.push("roommates: required for PG listings".to_string());
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Auto-derive parking from amenities list.
/// If amenities contains "Parking lot" (case-insensitive), parking = true.
pub fn auto_derive_parking(payload: &CreateListingPayload) -> bool {
    if let Some(true) = payload.parking {
        return true;
    }
    if let Some(ref amenities) = payload.amenities {
        return amenities
            .iter()
            .any(|a| a.eq_ignore_ascii_case("Parking lot") || a.eq_ignore_ascii_case("Parking"));
    }
    false
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn base_residential_rent() -> CreateListingPayload {
        CreateListingPayload {
            title: "Modern 2BHK Apartment".into(),
            description: "Beautiful apartment in city center".into(),
            property_type: "Residential".into(),
            listing_type: "Rent".into(),
            user_type: "User".into(),
            price: 25000,
            deposit: 50000,
            location: "Bandra West, Mumbai".into(),
            area: Some("Bandra".into()),
            city: Some("Mumbai".into()),
            pincode: Some("400050".into()),
            latitude: Some(19.0596),
            longitude: Some(72.8295),
            area_sqft: 1200,
            bedrooms: Some(2),
            bathrooms: Some(2),
            no_of_toilets: Some(2),
            no_of_balconies: Some(1),
            furnishing: Some("Semi-Furnished".into()),
            facing: Some("East".into()),
            floor: Some(5),
            total_floors: Some(12),
            commercial_type: None,
            land_type: None,
            gender_preference: None,
            roommates: None,
            amenities: Some(vec!["Gym".into(), "Swimming Pool".into()]),
            parking: Some(true),
            broker_contact_allowed: Some(true),
            age_years: Some(3),
            image_urls: None,
        }
    }

    #[test]
    fn test_valid_residential_rent() {
        let payload = base_residential_rent();
        assert!(validate_listing(&payload).is_ok());
    }

    #[test]
    fn test_sell_deposit_must_be_zero() {
        let mut payload = base_residential_rent();
        payload.listing_type = "Sell".into();
        payload.user_type = "Broker".into();
        payload.deposit = 10000;
        let result = validate_listing(&payload);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("deposit")));
    }

    #[test]
    fn test_user_cannot_sell() {
        let mut payload = base_residential_rent();
        payload.listing_type = "Sell".into();
        payload.deposit = 0;
        let result = validate_listing(&payload);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("Users are not allowed")));
    }

    #[test]
    fn test_commercial_rejects_residential_fields() {
        let mut payload = base_residential_rent();
        payload.property_type = "Commercial".into();
        payload.commercial_type = Some("Office".into());
        // bedrooms, bathrooms, furnishing are set — should fail
        let result = validate_listing(&payload);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("bedrooms")));
        assert!(errors.iter().any(|e| e.contains("bathrooms")));
        assert!(errors.iter().any(|e| e.contains("furnishing")));
    }

    #[test]
    fn test_space_sharing_requires_gender_and_roommates() {
        let mut payload = base_residential_rent();
        payload.listing_type = "Space Sharing".into();
        // gender_preference and roommates are None — should fail
        let result = validate_listing(&payload);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("gender_preference")));
        assert!(errors.iter().any(|e| e.contains("roommates")));
    }

    #[test]
    fn test_auto_derive_parking_from_amenities() {
        let mut payload = base_residential_rent();
        payload.parking = None;
        payload.amenities = Some(vec!["Gym".into(), "Parking lot".into()]);
        assert!(auto_derive_parking(&payload));
    }

    #[test]
    fn test_auto_derive_parking_absent() {
        let mut payload = base_residential_rent();
        payload.parking = None;
        payload.amenities = Some(vec!["Gym".into()]);
        assert!(!auto_derive_parking(&payload));
    }

    #[test]
    fn test_pg_requires_gender_and_roommates() {
        let mut payload = base_residential_rent();
        payload.listing_type = "PG".into();
        let result = validate_listing(&payload);
        assert!(result.is_err());
    }

    #[test]
    fn test_land_rejects_building_fields() {
        let mut payload = base_residential_rent();
        payload.property_type = "Land".into();
        payload.land_type = Some("Agricultural".into());
        // bedrooms, bathrooms etc are set — should fail
        let result = validate_listing(&payload);
        assert!(result.is_err());
    }
}
