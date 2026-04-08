use jsonwebtoken::{encode, Header, EncodingKey};
use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secret = "supersecret";
    let claims = json!({
        "sub": "74039329-e27d-418e-aee3-3e322409289f",
        "exp": 2000000000
    });

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes())
    )?;

    println!("Token generated: {}", token);

    let client = Client::new();
    let res = client.post("http://localhost:9090/api/visits")
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "property_id": "44089683-d723-40d0-ac58-a267c7055cec",
            "provider_id": "0989aef6-268f-4d11-9ba3-5e5e528f39f9",
            "scheduled_date_time": "2026-05-01T10:00:00Z",
            "contact_number": "1234567890",
            "notes": "Testing endpoint"
        }))
        .send()
        .await?;

    println!("Status: {}", res.status());
    println!("Response: {}", res.text().await?);

    Ok(())
}
