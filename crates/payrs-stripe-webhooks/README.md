# payrs-stripe-webhooks

Webhook handling for [`payrs-stripe`](https://crates.io/crates/payrs-stripe),
an unofficial Rust SDK for the Stripe API: signature verification, typed
events, and an async event router.

Usually reached via `payrs-stripe`'s `webhooks` feature (on by default). Depend
on it directly if your webhook receiver is a separate service from your API
client.

## Three layers — use as much as you need

**1. Verify and parse.**

```rust,no_run
use payrs_stripe_webhooks::Webhook;

# fn run(payload: &[u8], sig: &str) -> Result<(), payrs_stripe_webhooks::WebhookError> {
let event = Webhook::construct_event(payload, sig, "whsec_…")?;
println!("{} -> {}", event.id, event.event_type);
# Ok(())
# }
```

**2. Deserialize the payload into a typed model.**

```rust,ignore
let intent: payrs_stripe::models::PaymentIntent = event.deserialize_object()?;
```

**3. Route events to async handlers.**

```rust,no_run
use payrs_stripe_webhooks::{EventType, WebhookRouter};

let router = WebhookRouter::from_env()          // STRIPE_WEBHOOK_SECRET
    .expect("secret must be set")
    .on(EventType::CheckoutSessionCompleted, |event| async move {
        println!("fulfil order for {}", event.data.object["id"]);
        Ok(())
    })
    .on(EventType::InvoicePaymentFailed, |event| async move {
        println!("start dunning for {}", event.id);
        Ok(())
    })
    .fallback(|event| async move {
        println!("unhandled: {}", event.event_type);
        Ok(())
    });
```

## Wiring it to a web framework

The router is framework-agnostic. Two rules matter:

1. **Pass the raw request body bytes.** Any JSON re-serialisation by an
   extractor changes the bytes and the signature will not verify.
2. **Map the outcome to the right status code**, because Stripe retries on
   non-2xx:

```rust,ignore
match router.handle(&body, signature).await {
    Ok(()) => StatusCode::OK,                                    // 200: done
    Err(e) if e.is_verification_failure() => StatusCode::BAD_REQUEST, // 400: bad delivery, do not retry
    Err(_) => StatusCode::INTERNAL_SERVER_ERROR,                 // 5xx: handler failed, please retry
}
```

Return 2xx quickly and move slow work to a background task — Stripe's delivery
attempt has a timeout.

## Security properties

- HMAC-SHA256 over `"{timestamp}.{raw_body}"`, compared in **constant time**.
- **Replay protection**: deliveries older than the tolerance window (5 minutes
  by default, matching Stripe's official SDKs) are rejected.
- **Secret rotation**: a header carrying several `v1` signatures verifies if
  *any* of them matches, so you can rotate without dropping deliveries.
- Error messages never contain the secret or the payload.

## Event types

`EventType` covers the common events as first-class variants and keeps anything
else in `EventType::Other(_)`, so an event type newer than your SDK version can
never fail to parse. v2 **thin events** are parsed with
`Webhook::construct_thin_event`.

## License

MIT OR Apache-2.0
