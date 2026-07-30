# Errors and retries

## The error type

Every fallible call returns `payrs_stripe::Error`:

| Variant | Meaning | Typical response |
|---|---|---|
| `Api` | Stripe returned an error envelope (4xx/5xx) | Inspect `code` |
| `Network` | No usable HTTP response after retries | Retry later, alert |
| `Deserialization` | 2xx body did not match the expected type | Report a bug |
| `Config` | Client-side misconfiguration | Fix and redeploy |

## Always log the request ID

```rust,no_run
# use payrs_stripe::{Client, Error};
# async fn run(client: &Client) -> Result<(), Error> {
# let result: Result<serde_json::Value, Error> = Err(Error::Config("x".into()));
if let Err(err) = result {
    eprintln!("stripe call failed: {err} (request-id: {:?})", err.request_id());
}
# Ok(())
# }
```

`Request-Id` is the first thing Stripe support asks for. It is attached to both
`Api` and `Deserialization` errors.

## Card declines

```rust,ignore
match charge_result {
    Err(err) => {
        if let Some(api) = err.as_api_error() {
            if api.is_card_declined() {
                // Safe to show the customer for card errors
                let msg = api.message.as_deref().unwrap_or("Your card was declined.");
                // `decline_code` tells you *why*: insufficient_funds, lost_card, …
                log::info!("decline_code={:?}", api.decline_code);
                return Err(UserFacing::Declined(msg.to_owned()));
            }
            if api.is_rate_limited() { /* back off */ }
            if api.is_idempotency_conflict() { /* key reused with new params */ }
        }
        Err(UserFacing::Internal)
    }
    Ok(charge) => Ok(charge),
}
```

Only card-error messages are written for end users. Do not surface other
messages verbatim — they may describe your integration, not the customer.

## What is retried automatically

| Condition | Retried |
|---|---|
| Connection failure, timeout | yes |
| HTTP 409, 429, 500, 503 | yes |
| `Stripe-Should-Retry: true` | yes, overrides the status rule |
| `Stripe-Should-Retry: false` | no, overrides the status rule |
| Other 4xx (400, 401, 402, 404) | no — a retry would fail identically |

Backoff is exponential with full jitter, capped, and honours `Retry-After`.
Defaults: 3 retries (4 attempts), 500 ms base, 8 s cap.

```rust,no_run
use std::time::Duration;
use payrs_stripe::{Client, RetryPolicy};

# fn run() -> Result<(), payrs_stripe::Error> {
let client = Client::builder("sk_test_…")
    .retry_policy(
        RetryPolicy::default()
            .max_retries(5)
            .base_delay(Duration::from_millis(250))
            .max_delay(Duration::from_secs(10)),
    )
    .build()?;

// Or turn retries off entirely and drive them yourself:
let manual = Client::builder("sk_test_…")
    .retry_policy(RetryPolicy::none())
    .build()?;
# Ok(())
# }
```

## Idempotency

Every `POST`/`DELETE` gets an idempotency key generated **before the first
attempt** and reused on every retry. That is what makes an automatic retry of
`POST /v1/payment_intents` safe: Stripe recognises the replay and returns the
original result instead of charging twice.

Automatic keys are UUIDs, which protect a single in-process call. To make an
operation idempotent **across process restarts, deploys, and job retries**,
supply your own key derived from a business identifier:

```rust,ignore
client.request(Method::Post, "/v1/payment_intents")
    .idempotency_key(format!("order-{order_id}-capture"))
```

Now a worker that crashes and restarts cannot double-charge. Keys are limited
to 255 characters, retained by Stripe for roughly 24 hours, and should not
contain personal data.

Reusing a key with **different parameters** is an error
(`is_idempotency_conflict()`), which is a useful signal that two code paths
disagree about what they are sending.
