use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::Mailbox,
    transport::smtp::authentication::Credentials,
};

#[tokio::main]
async fn main() {
    let smtp_username = "thelive.inbuddy@gmail.com".to_string();
    let smtp_password = "umixdalimqsvdjas".to_string();
    let creds = Credentials::new(smtp_username, smtp_password);

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay("smtp.gmail.com")
        .expect("relay")
        .credentials(creds)
        .port(587)
        .build();

    let email = Message::builder()
        .from("Livana <thelive.inbuddy@gmail.com>".parse().unwrap())
        .to("test <thelive.inbuddy@gmail.com>".parse().unwrap())
        .subject("Test Email")
        .body("This is a test".to_string())
        .unwrap();

    match mailer.send(email).await {
        Ok(_) => println!("SUCCESS: Email sent!"),
        Err(e) => println!("FAILED: {:?}", e),
    }
}
