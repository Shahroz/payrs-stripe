# payrs-stripe

> ⚠️ **Pre-alpha (0.0.x).** Functional and tested, API surface may still shift
> before 0.1. Not affiliated with or endorsed by Stripe, Inc.

An ergonomic, unofficial Rust SDK for the [Stripe](https://stripe.com) API —
**full v1 surface + v2 namespace, every model exported, professional webhooks.**

## Highlights

- 🧩 **Full API coverage** — 1,431 models and 587 typed operations across 76
  API sections, generated from Stripe's official OpenAPI spec
  (`codegen/generate.py`); every model exported via `payrs_stripe::models::*`.
  The full endpoint→builder mapping is in [`docs/coverage.md`](docs/coverage.md) —
  checkout, invoices, subscriptions, transactions, charges, refunds, payouts,
  transfers, Connect, Terminal, Treasury, Issuing, Tax, Radar, and the rest
- 🔀 **Both server namespaces** — v1 (form-encoded) and v2 (JSON bodies,
  `Stripe-Context`, thin events) through one client; raw escape hatch reaches
  any endpoint, including ones newer than this SDK
- 🪝 **Webhooks done right** — constant-time signature verification with
  replay protection and secret rotation, a typed `EventType` enum (~65
  variants + forward-compatible `Other`), v2 thin events, and an async
  `WebhookRouter` for per-event handlers
- 🔁 Automatic retries (connection errors, 409/429/5xx) with backoff +
  jitter, honoring `Stripe-Should-Retry` and `Retry-After`
- 🔑 Automatic idempotency keys on every mutating request, reused across
  retries — a retried `POST /v1/payment_intents` can never double-charge
- 🧾 Structured errors carrying Stripe's full error envelope **and** the
  `Request-Id` support will ask you for
- 🕵️ Secret keys redacted in every `Debug`/`Display` (`sk_test_***`)
- 📌 Pinned `Stripe-Version` per release line (currently `2026-06-24.dahlia`),
  per-request override available
- 🔌 Pluggable `HttpTransport` (default: `reqwest` + rustls)

## Quickstart

```toml
[dependencies]
payrs-stripe = "0.0.1"   # features: api + webhooks + rustls on by default
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust,no_run
use payrs_stripe::Client;
use payrs_stripe::api::v1::customers::PostCustomers;
use payrs_stripe::api::v1::payment_intents::PostPaymentIntents;

#[tokio::main]
async fn main() -> Result<(), payrs_stripe::Error> {
    let client = Client::from_env()?; // STRIPE_SECRET_KEY

    let customer = PostCustomers::new()
        .name("Ada Lovelace")
        .email("ada@example.com")
        .send(&client)
        .await?;

    let intent = PostPaymentIntents::new(1999, "usd")
        .customer(customer.id.unwrap_or_default())
        .send(&client)
        .await?;

    println!("intent: {:?}", intent.id);
    Ok(())
}
```

Typed models deserialize webhook payloads too:

```rust,ignore
let pi: payrs_stripe::models::PaymentIntent = event.deserialize_object()?;
```

## Configuration — your choice of style

```rust,ignore
// 1. Fully programmatic:
let client = Client::builder("sk_live_…")
    .timeout(Duration::from_secs(10))
    .retry_policy(RetryPolicy::default().max_retries(5))
    .stripe_account("acct_…")          // Connect default
    .build()?;

// 2. Pure environment (STRIPE_SECRET_KEY; STRIPE_API_BASE and
//    STRIPE_ACCOUNT honored if set):
let client = Client::from_env()?;

// 3. Custom env var names (multi-account):
let acme = Client::from_env_var("STRIPE_KEY_ACME")?;

// 4. Hybrid: start from env, override in code (explicit wins):
let client = ClientBuilder::from_env()?
    .api_base("http://localhost:12111")   // e.g. stripe-mock in tests
    .build()?;

// Webhooks: secret from code or env, same pattern:
let router = WebhookRouter::from_env()?;                       // STRIPE_WEBHOOK_SECRET
let connect = WebhookRouter::from_env_var("WHSEC_CONNECT")?;   // custom name
```

Missing/empty variables produce actionable errors naming the variable.

## Pagination

```rust,ignore
let mut pager = GetCustomers::new().limit(100).paginate();
while let Some(page) = pager.next_page(&client).await? {
    for c in &page.data { /* … */ }
}
// or bounded drain:
let all = GetInvoices::new().paginate().collect_all(&client, 10_000).await?;
```

`starting_after` cursors are threaded automatically; available on all 127
list endpoints.

## Webhooks

```rust,ignore
use payrs_stripe::webhooks::{EventType, WebhookRouter};

let router = WebhookRouter::new(std::env::var("STRIPE_WEBHOOK_SECRET")?)
    .on(EventType::CheckoutSessionCompleted, |event| async move {
        fulfill_order(&event.data.object).await?;   // your logic
        Ok(())
    })
    .on(EventType::InvoicePaymentFailed, |event| async move {
        start_dunning(&event).await?;
        Ok(())
    })
    .fallback(|event| async move {
        tracing::info!("unhandled stripe event: {}", event.event_type);
        Ok(())
    });

// axum handler: raw body + Stripe-Signature header
match router.handle(&body, sig).await {
    Ok(()) => StatusCode::OK,
    Err(e) if e.is_verification_failure() => StatusCode::BAD_REQUEST,
    Err(_) => StatusCode::INTERNAL_SERVER_ERROR, // Stripe retries
}
```

Verification is HMAC-SHA256 in constant time, with a 5-minute replay
tolerance and multi-signature support for secret rotation. Unmatched events
are acknowledged; handler failures map to `5xx` so Stripe redelivers.

## The v2 namespace

```rust,no_run
# use payrs_stripe::{Client, Method};
# async fn run(client: &Client) -> Result<(), payrs_stripe::Error> {
use payrs_stripe::api::v2;

// Typed v2 core endpoints:
let dest = v2::CreateEventDestination::new(
        "prod hooks", "webhook_endpoint", "thin", vec!["ping".into()])
    .webhook_endpoint_url("https://example.com/hooks")
    .send(client)
    .await?;

// Any v2 endpoint via the raw client (JSON bodies):
let payout_methods: serde_json::Value = client
    .request(Method::Get, "/v2/money_management/payout_methods")
    .stripe_context("acct_child_123")
    .send_json()
    .await?;
# Ok(())
# }
```

(Heads-up: Stripe's server APIs are `/v1` and `/v2`. "v3" is Stripe.js — a
browser library, not something a server SDK calls.)

## Testing your integration

```bash
./scripts/stripe-mock.sh        # boots stripe/stripe-mock on :12111
cargo test -- --ignored         # contract tests against it
```

Unit/behavioral tests need no network: inject a mock `HttpTransport`.

## Workspace layout

| Crate | Purpose |
|---|---|
| `payrs-stripe` | Facade — the crate you depend on (features: `api`, `webhooks`, `rustls`/`native-tls`) |
| `payrs-stripe-api` | Generated full-surface bindings: `models`, `v1::*` operations, `v2` core |
| `payrs-stripe-webhooks` | Signature verification, typed events, async router |
| `payrs-stripe-client` | Transport core: auth, retries, idempotency, errors, raw requests |
| `payrs-stripe-types` | Shared primitives: typed IDs, `Currency`, `Expandable`, `List`, `Timestamp` |

Regenerate bindings after a spec bump: `python3 codegen/generate.py`.

## License

MIT OR Apache-2.0, at your option.
