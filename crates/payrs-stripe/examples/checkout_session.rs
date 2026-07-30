//! Create a Stripe Checkout session and print the URL to redirect to.
//!
//! ```bash
//! STRIPE_SECRET_KEY=sk_test_… cargo run -p payrs-stripe --example checkout_session
//! ```
//!
//! Against `stripe-mock` (no Stripe account needed):
//!
//! ```bash
//! ./scripts/stripe-mock.sh
//! STRIPE_SECRET_KEY=sk_test_123 STRIPE_API_BASE=http://localhost:12111 \
//!     cargo run -p payrs-stripe --example checkout_session
//! ```

use payrs_stripe::api::v1::checkout::PostCheckoutSessions;
use payrs_stripe::{Client, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Reads STRIPE_SECRET_KEY, and STRIPE_API_BASE if set.
    let client = Client::from_env()?;

    let session = PostCheckoutSessions::new()
        .mode("payment")
        .success_url("https://example.com/success?sid={CHECKOUT_SESSION_ID}")
        .cancel_url("https://example.com/cancel")
        .client_reference_id("order_42")
        // Nested parameters go through `.param()`, which serialises to
        // Stripe's bracket form encoding (line_items[0][price_data][...]).
        .param(
            "line_items",
            serde_json::json!([{
                "price_data": {
                    "currency": "usd",
                    "unit_amount": 1999,      // minor units: $19.99
                    "product_data": { "name": "Widget" }
                },
                "quantity": 2
            }]),
        )
        .param("metadata", serde_json::json!({ "order_id": "ord_42" }))
        .send(&client)
        .await?;

    println!("session:  {:?}", session.id);
    println!(
        "total:    {:?} {:?}",
        session.amount_total, session.currency
    );
    println!("redirect: {:?}", session.url);

    // Fulfil the order from the `checkout.session.completed` webhook, not
    // from the success_url redirect — the customer may never load it.
    Ok(())
}
