
write documentation to my frontendt team regarding the issue




Here is the documentation for the Rental Analytics API that you can share with your frontend team.

Subject: Backend API Documentation: Rental Analytics & Heatmaps

Hi Team,

The backend now fully supports the Rental Analytics API, allowing you to fetch trend data, heatmaps, and side-by-side area comparisons for property rentals.

(Note: Currently, these analytics pull from the legacy properties table. If/when we migrate this to the new listings_v2 table, the API contracts below will remain the same).

Here are the endpoints you can integrate:

1️⃣ Rent Trends (Daily Aggregation)
Fetches average rent over time, calculates percentage changes, and determines the overall trend direction (up/down/stable).

Method: GET /api/v1/analytics/rent-trends
Auth: Public
Query Parameters:
city (string, required): e.g., "Ahmedabad", "Mumbai"
locality (string, optional): e.g., "Bandra West"
property_type (string, optional): e.g., "Residential", "Commercial"
days (integer, optional): Look-back window in days (defaults to 30, max 365).
Example Request: /api/v1/analytics/rent-trends?city=Ahmedabad&days=30
Response:
json
{
  "success": true,
  "message": "Rent trends retrieved successfully",
  "data": {
    "average_rent": 25000.0,
    "percentage_change": -15.0,
    "trend": "down",
    "currency": "INR",
    "total_listings": 150,
    "data_points": [
      { "date": "2026-03-01", "avg_rent": 29000.0, "listing_count": 10 },
      { "date": "2026-03-05", "avg_rent": 28000.0, "listing_count": 12 }
    ]
  }
}
(Note: If there are fewer than 2 data points, percentage_change will be 0.0 and trend will be "stable").

2️⃣ Rent Heatmap (By Locality)
Returns average, min, and max rent broken down by localities within a specific city. Ideal for rendering map overlays or heatmap UI components.

Method: GET /api/v1/analytics/rent-heatmap
Auth: Public
Query Parameters:
city (string, required)
property_type (string, optional)
days (integer, optional): Look-back window in days (default 30).
Example Request: /api/v1/analytics/rent-heatmap?city=Mumbai
Response:
json
{
  "success": true,
  "message": "Rent heatmap retrieved successfully",
  "data": {
    "city": "Mumbai",
    "currency": "INR",
    "overall_avg_rent": 45000.0,
    "locality_count": 2,
    "total_listings": 300,
    "localities": [
      {
        "locality": "Bandra West",
        "avg_rent": 85000.0,
        "min_rent": 60000.0,
        "max_rent": 120000.0,
        "listing_count": 45
      },
      {
        "locality": "Andheri East",
        "avg_rent": 35000.0,
        "min_rent": 25000.0,
        "max_rent": 50000.0,
        "listing_count": 120
      }
    ]
  }
}
3️⃣ Area Comparison (Multiple Cities)
Compares high-level rent statistics across 2 to 10 cities side-by-side.

Method: GET /api/v1/analytics/rent-comparison
Auth: Public
Query Parameters:
cities (string, required): A comma-separated list of cities (e.g., Ahmedabad,Mumbai). Must contain at least 2 and at most 10 cities.
property_type (string, optional)
days (integer, optional): Look-back window in days (default 30).
Example Request: /api/v1/analytics/rent-comparison?cities=Ahmedabad,Mumbai
Response:
json
{
  "success": true,
  "message": "Rent comparison retrieved successfully",
  "data": {
    "currency": "INR",
    "cities_compared": ["Ahmedabad", "Mumbai"],
    "cheapest": "Ahmedabad",
    "most_expensive": "Mumbai",
    "difference_percentage": 150.5,
    "cities": [
      {
        "city": "Ahmedabad",
        "avg_rent": 18000.0,
        "min_rent": 10000.0,
        "max_rent": 35000.0,
        "listing_count": 400
      },
      {
        "city": "Mumbai",
        "avg_rent": 45000.0,
        "min_rent": 15000.0,
        "max_rent": 200000.0,
        "listing_count": 800
      }
    ]
  }
}
(Note: difference_percentage is only calculated if exactly 2 cities are compared; otherwise, it will be null).