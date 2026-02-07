use anyhow::{Result, Context};
use async_trait::async_trait;

#[async_trait]
pub trait OcrService: Send + Sync {
    async fn extract_text(&self, file_bytes: &[u8]) -> Result<String>;
}

#[derive(Clone)]
pub struct TesseractOcr;

impl TesseractOcr {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl OcrService for TesseractOcr {
    async fn extract_text(&self, file_bytes: &[u8]) -> Result<String> {
        // Load image from memory using the `image` crate
        let img = image::load_from_memory(file_bytes)
            .context("Failed to load image from bytes")?;

        // Convert to Tesseract Image
        let tess_image = rusty_tesseract::Image::from_dynamic_image(&img)
            .context("Failed to create Tesseract image")?;

        // Run OCR
        let output_text = rusty_tesseract::image_to_string(
            &tess_image,
            &rusty_tesseract::Args::default(),
        ).context("Failed to run Tesseract OCR")?;

        Ok(output_text)
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct MockOcr {
    pub expected_text: String,
}

#[async_trait]
impl OcrService for MockOcr {
    async fn extract_text(&self, _file_bytes: &[u8]) -> Result<String> {
        Ok(self.expected_text.clone())
    }
}
