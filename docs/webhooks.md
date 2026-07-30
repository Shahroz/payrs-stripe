# Webhooks

Webhooks are how Stripe tells you what happened. For anything that matters —
fulfilling an order, provisioning access, starting dunning — react to the
webhook, not to an API response or a browser redirect. The customer can close
the tab; the webhook still arrives.

## Minimal receiver

```rust,no_run
use payrs_stripe::webhooks::{EventType, WebhookRouter};

# fn build() -> Result<WebhookRouter, String> {
let router = WebhookRouter::from_env()?      // STRIPE_WEBHOOK_SECRET
    .on(EventType::CheckoutSessionCompleted, |event| async move {
        let session_id = event.data.object["id"].as_str().unwrap_or_default();
        println!("fulfil order for {session_id}");
        Ok(())
    })
    .on(EventType::InvoicePaymentFailed, |event| async move {
        println!("start dunning: {}", event.id);
        Ok(())
    })
    .fallback(|event| async move {
        println!("unhandled event type: {}", event.event_type);
        Ok(())
    });
# Ok(router)
# }
```

## Wiring to a framework

Framework-agnostic, but two rules are non-negotiable.

**Rule 1 — pass the raw body bytes.** A JSON extractor re-serialises the body,
which changes the bytes, which breaks the signature. Take the raw body.

**Rule 2 — map the outcome to the right status.** Stripe retries non-2xx
deliveries with backoff for up to three days.

```rust,ignore
// axum
async fn stripe_webhook(
    State(router): State<Arc<WebhookRouter>>,
    headers: HeaderMap,
    body: Bytes,                       // raw bytes, not Json<T>
) -> StatusCode {
    let Some(sig) = headers.get("stripe-signature").and_then(|v| v.to_str().ok())
    else {
        return StatusCode::BAD_REQUEST;
    };

    match router.handle(&body, sig).await {
        Ok(()) => StatusCode::OK,
        // Bad signature or malformed payload: retrying will not help.
        Err(e) if e.is_verification_failure() => StatusCode::BAD_REQUEST,
        // Your handler failed: ask Stripe to retry.
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
```

## Typed payloads

`event.data.object` is raw JSON so no event type can fail to parse. Deserialise
it when you need structure:

```rust,ignore
let intent: payrs_stripe::models::PaymentIntent = event.deserialize_object()?;
println!("{:?} {:?}", intent.amount, intent.currency);
```

Unknown event types arrive as `EventType::Other("some.new.event")` rather than
failing — Stripe can add event types without breaking your deployment.

## Four properties worth understanding

**Verification is constant-time.** HMAC-SHA256 over `"{timestamp}.{raw_body}"`,
compared without early exit, so the comparison leaks no timing information.

**Replay is bounded.** Deliveries older than 5 minutes are rejected. A captured
request cannot be replayed against you tomorrow.

**Rotation is supported.** During a secret rotation Stripe may send several
`v1` signatures; verification succeeds if any matches, so no deliveries are
lost mid-rotation.

**Delivery is at-least-once.** Stripe may send the same event more than once,
and events can arrive out of order. Make handlers idempotent — key on
`event.id`, or on the object's own state:

```rust,ignore
if already_processed(&event.id).await? {
    return Ok(());          // ack without redoing the work
}
```

## Return fast

Stripe's delivery attempt times out. Acknowledge immediately and move slow work
to a background task:

```rust,ignore
.on(EventType::CheckoutSessionCompleted, |event| async move {
    tokio::spawn(async move { slow_fulfilment(event).await });
    Ok(())            // 200 straight away
})
```

The trade-off: once you spawn, a crash loses the work, since Stripe has already
been told 200. For critical paths, write to a durable queue before acking.

## v2 thin events

Event destinations created in the v2 namespace can deliver **thin** payloads —
a pointer rather than a snapshot:

```rust,ignore
let thin = Webhook::construct_thin_event(payload, sig, secret)?;
if let Some(obj) = thin.related_object {
    // Fetch current state; thin payloads carry no stale object data
    let fresh: serde_json::Value =
        client.request(Method::Get, obj.url).send_json().await?;
}
```

## Local development

```bash
stripe listen --forward-to localhost:3000/webhooks   # prints a whsec_… secret
stripe trigger payment_intent.succeeded
```

See [testing](testing.md) for signing payloads in unit tests without a network.
