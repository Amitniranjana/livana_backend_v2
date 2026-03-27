use anyhow::Result;
use async_trait::async_trait;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::primitives::ByteStream;

#[async_trait]
pub trait StorageService: Send + Sync {
    async fn upload_file(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<()>;
}

#[derive(Clone)]
pub struct S3Storage {
    client: S3Client,
    bucket: String,
}

impl S3Storage {
    pub fn new(client: S3Client, bucket: String) -> Self {
        Self { client, bucket }
    }
}

#[async_trait]
impl StorageService for S3Storage {
    async fn upload_file(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(data))
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("S3 Upload Error: {}", e))?;

        Ok(())
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct MockStorage;

#[async_trait]
impl StorageService for MockStorage {
    async fn upload_file(&self, _key: &str, _data: Vec<u8>, _content_type: &str) -> Result<()> {
        Ok(())
    }
}
