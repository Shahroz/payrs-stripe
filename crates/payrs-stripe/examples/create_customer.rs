//! Create a customer using the raw escape hatch (Phase 0).
//! Run: `STRIPE_SECRET_KEY=sk_test_… cargo run -p payrs-stripe --example create_customer`
//! Or against stripe-mock: `STRIPE_SECRET_KEY=sk_test_123 STRIPE_API_BASE=http://localhost:12111 cargo run …`

#![allow(clippy::unwrap_used, clippy::expect_used)]

use payrs_stripe::{Client, Method};

#[tokio::main]
async fn main() -> Result<(), payrs_stripe::Error> {
    let key = std::env::var("STRIPE_SECRET_KEY")
        .map_err(|_| payrs_stripe::Error::Config("set STRIPE_SECRET_KEY".into()))?;
    let mut builder = Client::builder(key);
    if let Ok(base) = std::env::var("STRIPE_API_BASE") {
        builder = builder.api_base(base);
    }
    let client = builder
        .app_info("payrs-example", Some(env!("CARGO_PKG_VERSION")), None)
        .build()?;

    let customer: serde_json::Value = client
        .request(Method::Post, "/v1/customers")
        .form_pairs([
            ("name", "Ada Lovelace"),
            ("email", "ada@example.com"),
            ("metadata[source]", "payrs-example"),
        ])
        .idempotency_key("example-create-ada-1")
        .send_json()
        .await?;

    println!("created customer: {}", customer["id"]);
    Ok(())
}
