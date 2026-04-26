# Rental Analytics API Documentation

This document outlines the newly added analytics endpoints available for the frontend team. These APIs provide insights into rental price trends, locality-based heatmaps, and city-to-city comparisons.

Base URL: `http://<your-backend-url>`

---

## 1. Rent Trends
Retrieves daily average rent over time, along with percentage change and trend direction.

**Endpoint:**
`GET /api/v1/analytics/rent-trends`

**Query Parameters:**
| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `city` | `string` | ✅ Yes | - | Name of the city (e.g., Ahmedabad, Mumbai) |
| `days` | `integer`| ❌ No | `30` | Look-back window in days (1 to 365) |
| `property_type` | `string` | ❌ No | - | Filter by type (e.g., apartment, villa, pg) |
| `locality` | `string` | ❌ No | - | Optional micro-area filter |

**Example Request:**
`GET /api/v1/analytics/rent-trends?city=Ahmedabad&days=30&property_type=apartment`

**Example Response:**
```json
{
  "success": true,
  "message": "Rent trends retrieved successfully",
  "data": {
    "average_rent": 2500.00,
    "percentage_change": -13.79,
    "trend": "down",     // Can be "up", "down", or "stable"
    "currency": "INR",
    "total_listings": 42,
    "data_points": [
      { "date": "2026-03-01", "avg_rent": 2900.00, "listing_count": 8 },
      { "date": "2026-03-05", "avg_rent": 2800.00, "listing_count": 12 }
    ]
  }
}
```

---

## 2. Rent Heatmap (Area Breakdown)
Provides average rent broken down by locality within a single city. Ideal for rendering a visual heatmap.

**Endpoint:**
`GET /api/v1/analytics/rent-heatmap`

**Query Parameters:**
| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `city` | `string` | ✅ Yes | - | Name of the city |
| `days` | `integer`| ❌ No | `30` | Look-back window in days (1 to 365) |
| `property_type` | `string` | ❌ No | - | Filter by type |

**Example Request:**
`GET /api/v1/analytics/rent-heatmap?city=Ahmedabad&days=30`

**Example Response:**
```json
{
  "success": true,
  "message": "Rent heatmap retrieved successfully",
  "data": {
    "city": "Ahmedabad",
    "currency": "INR",
    "overall_avg_rent": 2650.00,
    "locality_count": 5,
    "total_listings": 120,
    "localities": [
      { 
        "locality": "Satellite", 
        "avg_rent": 3200.00, 
        "min_rent": 2800.00, 
        "max_rent": 4000.00, 
        "listing_count": 25 
      },
      { 
        "locality": "Vastrapur", 
        "avg_rent": 2800.00, 
        "min_rent": 2200.00, 
        "max_rent": 3500.00, 
        "listing_count": 30 
      }
    ]
  }
}
```

---

## 3. Area Comparison
Compares rent statistics side-by-side across multiple cities.

**Endpoint:**
`GET /api/v1/analytics/rent-comparison`

**Query Parameters:**
| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `cities` | `string` | ✅ Yes | - | Comma-separated list of cities (e.g. `Ahmedabad,Mumbai`). Min 2, Max 10. |
| `days` | `integer`| ❌ No | `30` | Look-back window in days (1 to 365) |
| `property_type` | `string` | ❌ No | - | Filter by type |

**Example Request:**
`GET /api/v1/analytics/rent-comparison?cities=Ahmedabad,Mumbai&days=30`

**Example Response:**
```json
{
  "success": true,
  "message": "Rent comparison retrieved successfully",
  "data": {
    "currency": "INR",
    "cities_compared": ["Ahmedabad", "Mumbai"],
    "cheapest": "Ahmedabad",
    "most_expensive": "Mumbai",
    "difference_percentage": 620.00, // Only present if exactly 2 cities are compared
    "cities": [
      { 
        "city": "Mumbai", 
        "avg_rent": 18000.00, 
        "min_rent": 8000.00, 
        "max_rent": 45000.00, 
        "listing_count": 200 
      },
      { 
        "city": "Ahmedabad", 
        "avg_rent": 2500.00, 
        "min_rent": 1200.00, 
        "max_rent": 6000.00, 
        "listing_count": 150 
      }
    ]
  }
}
```

---

## Error Handling
All endpoints follow a consistent error structure:

```json
{
  "success": false,
  "message": "Human readable error description",
  "error_code": "MISSING_CITY" 
}
```

**Common Error Codes:**
- `MISSING_CITY`: The `city` parameter was omitted or empty.
- `INSUFFICIENT_CITIES`: Less than 2 cities were provided to the comparison API.
- `TOO_MANY_CITIES`: More than 10 cities were provided to the comparison API.
- `DB_ERROR`: An internal database error occurred.
