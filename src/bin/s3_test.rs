use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::primitives::ByteStream;
use std::fs;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;
    let s3_client = S3Client::new(&aws_config);
    let bucket = std::env::var("KYC_BUCKET_NAME").unwrap_or_else(|_| "livana-kyc-documents".to_string());

    let res = s3_client
        .put_object()
        .bucket(&bucket)
        .key("test-key.txt")
        .body(ByteStream::from("hello world".as_bytes().to_vec()))
        .content_type("text/plain")
        .send()
        .await;

    let err_str = match res {
        Ok(_) => "Success!".to_string(),
        Err(e) => format!("{:#?}", e),
    };

    fs::write("s3_error_output.txt", err_str).unwrap();
    println!("Done logging to s3_error_output.txt");
}
