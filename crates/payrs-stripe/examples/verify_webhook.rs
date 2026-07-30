//! Verify and route a webhook delivery — no web framework required.
//!
//! Shows the shape your HTTP handler should have: raw body bytes in, a status
//! code out. Run it with no arguments; it feeds the router a payload it signs
//! itself, then demonstrates a rejected (tampered) delivery.
//!
//! ```bash
//! cargo run -p payrs-stripe --example verify_webhook
//! ```

#![allow(clippy::expect_used)]

use payrs_stripe::webhooks::{EventType, WebhookError, WebhookRouter};

/// Status code your handler should return for a given outcome.
fn status_for(outcome: &Result<(), WebhookError>) -> u16 {
    match outcome {
        Ok(()) => 200,
        // Bad signature or malformed payload: retrying cannot help.
        Err(e) if e.is_verification_failure() => 400,
        // Handler failed: ask Stripe to redeliver.
        Err(_) => 500,
    }
}

#[tokio::main]
async fn main() {
    let secret = "whsec_example_secret";

    let router = WebhookRouter::new(secret)
        .on(EventType::PaymentIntentSucceeded, |event| async move {
            let amount = event.data.object["amount"].as_i64().unwrap_or_default();
            println!("  handler: payment succeeded, amount={amount}");
            Ok(())
        })
        .fallback(|event| async move {
            println!("  fallback: unhandled {}", event.event_type);
            Ok(())
        });

    let payload = serde_json::json!({
        "id": "evt_1",
        "object": "event",
        "type": "payment_intent.succeeded",
        "created": 1_700_000_000,
        "data": { "object": { "id": "pi_1", "amount": 1999 } },
        "livemode": false
    })
    .to_string();

    // In production this header comes from the `Stripe-Signature` request
    // header. Here we produce a valid one so the example is self-contained.
    let signature = sign_for_demo(payload.as_bytes(), secret);

    println!("valid delivery:");
    let outcome = router.handle(payload.as_bytes(), &signature).await;
    println!("  -> HTTP {}", status_for(&outcome));

    println!("tampered delivery:");
    let outcome = router.handle(b"{\"id\":\"evt_evil\"}", &signature).await;
    println!("  -> HTTP {}", status_for(&outcome));
}

/// Produce a `Stripe-Signature` header value: `t=<ts>,v1=<hex hmac>` over
/// `"{ts}.{body}"`. Stripe does this for you; we only need it to make this
/// example runnable offline.
fn sign_for_demo(payload: &[u8], secret: &str) -> String {
    use std::fmt::Write as _;
    use std::time::{SystemTime, UNIX_EPOCH};

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();

    let mut mac = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any length");
    hmac::Mac::update(&mut mac, ts.to_string().as_bytes());
    hmac::Mac::update(&mut mac, b".");
    hmac::Mac::update(&mut mac, payload);

    let hex = hmac::Mac::finalize(mac)
        .into_bytes()
        .iter()
        .fold(String::new(), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        });
    format!("t={ts},v1={hex}")
}
