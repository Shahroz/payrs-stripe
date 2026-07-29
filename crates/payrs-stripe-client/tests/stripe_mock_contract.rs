//! Contract tests against Stripe's official `stripe-mock` server.
//! Boot it first: `./scripts/stripe-mock.sh`, then `cargo test -- --ignored`.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines
)]

use payrs_stripe_client::{Client, Method};

fn mock_client() -> Client {
    Client::builder("sk_test_123")
        .api_base(
            std::env::var("STRIPE_MOCK_URL").unwrap_or_else(|_| "http://localhost:12111".into()),
        )
        .build()
        .unwrap()
}

#[tokio::test]
#[ignore = "requires a running stripe-mock (./scripts/stripe-mock.sh)"]
async fn create_and_retrieve_customer_against_stripe_mock() {
    let client = mock_client();

    let created: serde_json::Value = client
        .request(Method::Post, "/v1/customers")
        .form_pairs([("name", "Ada Lovelace"), ("email", "ada@example.com")])
        .send()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap();
    assert!(id.starts_with("cus_"));

    let fetched: serde_json::Value = client
        .request(Method::Get, format!("/v1/customers/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(fetched["object"], "customer");
}

#[tokio::test]
#[ignore = "requires a running stripe-mock (./scripts/stripe-mock.sh)"]
async fn list_customers_against_stripe_mock() {
    let client = mock_client();
    let list: serde_json::Value = client
        .request(Method::Get, "/v1/customers")
        .query("limit", "3")
        .send()
        .await
        .unwrap();
    assert_eq!(list["object"], "list");
}
