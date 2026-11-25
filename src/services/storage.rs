use anyhow::{Result, anyhow};
use async_trait::async_trait;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use std::time::Duration;

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

    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// Generates a real SigV4 presigned `GetObject` URL for the given bucket and key.
    pub async fn presigned_get_url(
        &self,
        bucket: &str,
        key: &str,
        expires_in: Duration,
    ) -> Result<String> {
        let presigning_config = PresigningConfig::expires_in(expires_in)
            .map_err(|e| anyhow!("Invalid presigning config: {:?}", e))?;

        let presigned = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .presigned(presigning_config)
            .await
            .map_err(|e| {
                log::error!(
                    "S3 presign GetObject failed: bucket={}, key={}, error={:?}",
                    bucket,
                    key,
                    e
                );
                anyhow!("S3 Presign Error (bucket={}, key={}): {:?}", bucket, key, e)
            })?;

        Ok(presigned.uri().to_string())
    }

    /// Resolves a stored S3 URL (virtual-hosted, path-style, legacy fake-signed, s3://)
    /// or bare key and generates a real SigV4 presigned `GetObject` URL.
    pub async fn presign_stored_url(
        &self,
        stored_url_or_key: &str,
        expires_in: Duration,
    ) -> Result<String> {
        let (bucket, key) = resolve_s3_location(stored_url_or_key, &self.bucket)?;
        self.presigned_get_url(&bucket, &key, expires_in).await
    }
}

/// Normalizes bucket names from legacy hardcoded placeholders (e.g. `livana-docs-private`
/// from the old fake signer) to the configured default bucket.
fn normalize_bucket(bucket: &str, default_bucket: &str) -> String {
    if bucket == "livana-docs-private" {
        default_bucket.to_string()
    } else {
        bucket.to_string()
    }
}

fn validate_s3_key(raw_key: &str) -> Result<String> {
    let key = raw_key.trim().trim_start_matches('/');
    if key.is_empty() {
        return Err(anyhow!("S3 object key is empty"));
    }
    if key.split('/').any(|seg| seg == "..") {
        return Err(anyhow!("Invalid path traversal in S3 key"));
    }
    Ok(key.to_string())
}

/// Resolves `(bucket, key)` from:
/// - Virtual-hosted S3 URLs (`https://<bucket>.s3.<region>.amazonaws.com/<key>`,
///   `https://<bucket>.s3.amazonaws.com/<key>`, `https://<bucket>.s3-<region>.amazonaws.com/<key>`)
/// - Path-style S3 URLs (`https://s3.<region>.amazonaws.com/<bucket>/<key>`,
///   `https://s3.amazonaws.com/<bucket>/<key>`)
/// - Old fake-signed URLs (`?X-Amz-Expires=...&X-Amz-Signed=true`)
/// - `s3://<bucket>/<key>` URIs
/// - Bare S3 keys (`kyc/profile/user/file.jpg`) using `default_bucket`
pub fn resolve_s3_location(input: &str, default_bucket: &str) -> Result<(String, String)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("S3 URL or key cannot be empty"));
    }

    // Handle accidental double-wrapped fake URLs if any exist in DB:
    // e.g. https://s3.amazonaws.com/livana-docs-private/https://...
    let mut candidate = trimmed;
    if let Some(idx) = candidate.find("livana-docs-private/http") {
        let after = &candidate[idx + "livana-docs-private/".len()..];
        if after.starts_with("http://") || after.starts_with("https://") {
            candidate = after;
        }
    }

    // Strip query string and fragment before parsing path/key
    let without_frag = candidate.split('#').next().unwrap_or(candidate);
    let clean = without_frag.split('?').next().unwrap_or(without_frag).trim();

    if let Some(rest) = clean.strip_prefix("s3://") {
        let (bucket, raw_key) = rest
            .split_once('/')
            .ok_or_else(|| anyhow!("Malformed s3:// URI: missing key"))?;
        let bucket = bucket.trim();
        if bucket.is_empty() {
            return Err(anyhow!("Malformed s3:// URI: empty bucket"));
        }
        let key = validate_s3_key(raw_key)?;
        return Ok((normalize_bucket(bucket, default_bucket), key));
    }

    if clean.starts_with("http://") || clean.starts_with("https://") {
        let after_scheme = clean
            .strip_prefix("https://")
            .or_else(|| clean.strip_prefix("http://"))
            .unwrap_or(clean);

        let (host_port, raw_path) = after_scheme
            .split_once('/')
            .ok_or_else(|| anyhow!("Malformed S3 URL: missing path"))?;

        let host = host_port
            .split(':')
            .next()
            .unwrap_or(host_port)
            .to_ascii_lowercase();

        let decoded_path = urlencoding::decode(raw_path)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| raw_path.to_string());
        let path = decoded_path.trim_start_matches('/');

        if path.is_empty() {
            return Err(anyhow!("Malformed S3 URL: empty path"));
        }

        // 1. AWS S3 Path-style: s3.amazonaws.com / s3.<region>.amazonaws.com / s3-<region>.amazonaws.com
        let is_aws_path_style = host == "s3.amazonaws.com"
            || ((host.starts_with("s3.") || host.starts_with("s3-"))
                && (host.ends_with(".amazonaws.com") || host.ends_with(".amazonaws.com.cn")));

        if is_aws_path_style {
            let (bucket, raw_key) = path
                .split_once('/')
                .ok_or_else(|| anyhow!("Path-style S3 URL missing object key"))?;
            if bucket.trim().is_empty() {
                return Err(anyhow!("Path-style S3 URL has empty bucket"));
            }
            let key = validate_s3_key(raw_key)?;
            return Ok((normalize_bucket(bucket.trim(), default_bucket), key));
        }

        // 2. AWS S3 Virtual-hosted style:
        // <bucket>.s3.amazonaws.com
        // <bucket>.s3.<region>.amazonaws.com
        // <bucket>.s3-<region>.amazonaws.com
        // <bucket>.s3.dualstack.<region>.amazonaws.com
        if host.ends_with(".amazonaws.com") || host.ends_with(".amazonaws.com.cn") {
            if let Some((bucket, suffix)) = host.rsplit_once(".s3.").or_else(|| host.rsplit_once(".s3-")) {
                if !bucket.is_empty() && !suffix.is_empty() {
                    let key = validate_s3_key(path)?;
                    return Ok((normalize_bucket(bucket, default_bucket), key));
                }
            }
        }

        // 3. Optional custom S3-compatible endpoint configured via env (path-style: <endpoint>/<bucket>/<key>)
        for var_name in ["S3_ENDPOINT", "AWS_ENDPOINT_URL"] {
            if let Ok(ep) = std::env::var(var_name) {
                let ep_host = ep
                    .trim()
                    .strip_prefix("https://")
                    .or_else(|| ep.trim().strip_prefix("http://"))
                    .unwrap_or(ep.trim())
                    .split('/')
                    .next()
                    .unwrap_or("")
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !ep_host.is_empty() && host == ep_host {
                    let (bucket, raw_key) = path
                        .split_once('/')
                        .ok_or_else(|| anyhow!("Custom endpoint S3 URL missing object key"))?;
                    if bucket.trim().is_empty() {
                        return Err(anyhow!("Custom endpoint S3 URL has empty bucket"));
                    }
                    let key = validate_s3_key(raw_key)?;
                    return Ok((normalize_bucket(bucket.trim(), default_bucket), key));
                }
            }
        }

        return Err(anyhow!("Unsupported non-S3 URL host: {}", host));
    }

    if clean.contains("://") {
        return Err(anyhow!("Unsupported URI scheme"));
    }

    // Bare key (e.g. "kyc/profile/123/456.jpg" or "/zero-deposit/redemptions/...")
    let key = validate_s3_key(clean)?;
    Ok((default_bucket.to_string(), key))
}

#[async_trait]
impl StorageService for S3Storage {
    async fn upload_file(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<()> {
        log::info!(
            "S3 upload: bucket={}, key={}, size={}, content_type={}",
            self.bucket,
            key,
            data.len(),
            content_type
        );
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(data))
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| {
                log::error!(
                    "S3 PutObject failed: bucket={}, key={}, error={:?}",
                    self.bucket,
                    key,
                    e
                );
                anyhow!("S3 Upload Error (bucket={}): {:?}", self.bucket, e)
            })?;

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

#[cfg(test)]
mod tests {
    use super::resolve_s3_location;

    const DEFAULT_BUCKET: &str = "kyc-docs-livanaeco-2026";

    #[test]
    fn resolves_virtual_hosted_regional_url() {
        let (bucket, key) = resolve_s3_location(
            "https://kyc-docs-livanaeco-2026.s3.ap-south-1.amazonaws.com/kyc/profile/u1/img.jpg",
            DEFAULT_BUCKET,
        )
        .unwrap();
        assert_eq!(bucket, "kyc-docs-livanaeco-2026");
        assert_eq!(key, "kyc/profile/u1/img.jpg");
    }

    #[test]
    fn resolves_virtual_hosted_global_and_dash_urls() {
        let (b1, k1) = resolve_s3_location(
            "https://livana-kyc-documents.s3.amazonaws.com/kyc/govt_id/u1/pan.pdf",
            DEFAULT_BUCKET,
        )
        .unwrap();
        assert_eq!(b1, "livana-kyc-documents");
        assert_eq!(k1, "kyc/govt_id/u1/pan.pdf");

        let (b2, k2) = resolve_s3_location(
            "https://my.dotted.bucket.s3-ap-south-1.amazonaws.com/docs/a%20b.pdf",
            DEFAULT_BUCKET,
        )
        .unwrap();
        assert_eq!(b2, "my.dotted.bucket");
        assert_eq!(k2, "docs/a b.pdf");
    }

    #[test]
    fn resolves_path_style_urls() {
        let (b1, k1) = resolve_s3_location(
            "https://s3.ap-south-1.amazonaws.com/livana-kyc-documents/kyc/exp/123.png",
            DEFAULT_BUCKET,
        )
        .unwrap();
        assert_eq!(b1, "livana-kyc-documents");
        assert_eq!(k1, "kyc/exp/123.png");

        let (b2, k2) = resolve_s3_location(
            "https://s3.amazonaws.com/other-bucket/folder/file.jpg",
            DEFAULT_BUCKET,
        )
        .unwrap();
        assert_eq!(b2, "other-bucket");
        assert_eq!(k2, "folder/file.jpg");
    }

    #[test]
    fn resolves_legacy_fake_signed_urls() {
        let (bucket, key) = resolve_s3_location(
            "https://s3.amazonaws.com/livana-docs-private/zero-deposit/redemptions/r1/rental_agreement_v1_abc.pdf?X-Amz-Expires=1795000000&X-Amz-Signed=true",
            DEFAULT_BUCKET,
        )
        .unwrap();
        assert_eq!(bucket, DEFAULT_BUCKET);
        assert_eq!(key, "zero-deposit/redemptions/r1/rental_agreement_v1_abc.pdf");
    }

    #[test]
    fn strips_real_presigned_query_params() {
        let (bucket, key) = resolve_s3_location(
            "https://kyc-docs-livanaeco-2026.s3.ap-south-1.amazonaws.com/kyc/doc.pdf?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Expires=600&X-Amz-Signature=deadbeef",
            DEFAULT_BUCKET,
        )
        .unwrap();
        assert_eq!(bucket, "kyc-docs-livanaeco-2026");
        assert_eq!(key, "kyc/doc.pdf");
    }

    #[test]
    fn resolves_s3_scheme_and_bare_keys() {
        let (b1, k1) = resolve_s3_location("s3://custom-bkt/a/b/c.pdf", DEFAULT_BUCKET).unwrap();
        assert_eq!(b1, "custom-bkt");
        assert_eq!(k1, "a/b/c.pdf");

        let (b2, k2) = resolve_s3_location("kyc/profile/u1/selfie.jpg", DEFAULT_BUCKET).unwrap();
        assert_eq!(b2, DEFAULT_BUCKET);
        assert_eq!(k2, "kyc/profile/u1/selfie.jpg");

        let (b3, k3) = resolve_s3_location("/zero-deposit/redemptions/doc.png?X-Amz-Signed=true", DEFAULT_BUCKET).unwrap();
        assert_eq!(b3, DEFAULT_BUCKET);
        assert_eq!(k3, "zero-deposit/redemptions/doc.png");
    }

    #[test]
    fn rejects_invalid_or_external_urls() {
        assert!(resolve_s3_location("", DEFAULT_BUCKET).is_err());
        assert!(resolve_s3_location("   ", DEFAULT_BUCKET).is_err());
        assert!(resolve_s3_location("https://evil.example.com/secret.pdf", DEFAULT_BUCKET).is_err());
        assert!(resolve_s3_location("https://s3.amazonaws.com/bucket/", DEFAULT_BUCKET).is_err());
        assert!(resolve_s3_location("kyc/../etc/passwd", DEFAULT_BUCKET).is_err());
        assert!(resolve_s3_location("ftp://bucket/file", DEFAULT_BUCKET).is_err());
    }
}
