use serde::{Deserialize, Serialize};

/// A single language entry.
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct LanguageDto {
    pub code: String,
    pub name: String,
}

/// Request body for setting the preferred language.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SetLanguageDto {
    pub language_code: String,
}
