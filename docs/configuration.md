# Configuration

Four styles, all supported. Pick whichever suits your deployment.

## 1. Fully programmatic

```rust,no_run
use std::time::Duration;
use payrs_stripe::{Client, RetryPolicy};

# fn run() -> Result<(), payrs_stripe::Error> {
let client = Client::builder("sk_live_…")
    .timeout(Duration::from_secs(10))
    .retry_policy(RetryPolicy::default().max_retries(5))
    .stripe_account("acct_123")             // Connect: act as this account
    .app_info("my-platform", Some("1.4.0"), Some("https://example.com"))
    .build()?;
# Ok(())
# }
```

## 2. From the environment

```rust,no_run
# fn run() -> Result<(), payrs_stripe::Error> {
let client = payrs_stripe::Client::from_env()?;
# Ok(())
# }
```

| Variable | Required | Effect |
|---|---|---|
| `STRIPE_SECRET_KEY` | yes | API key |
| `STRIPE_API_BASE` | no | Base URL override (e.g. `stripe-mock`) |
| `STRIPE_ACCOUNT` | no | Default Connect account |

## 3. Custom variable names

For multi-account setups, or when your org has its own naming conventions:

```rust,no_run
# fn run() -> Result<(), payrs_stripe::Error> {
let acme = payrs_stripe::Client::from_env_var("STRIPE_KEY_ACME")?;
let umbrella = payrs_stripe::Client::from_env_var("STRIPE_KEY_UMBRELLA")?;
# Ok(())
# }
```

## 4. Hybrid — environment plus overrides

Start from the environment, then override in code. **Explicit builder calls
always win.**

```rust,no_run
use payrs_stripe::ClientBuilder;

# fn run() -> Result<(), payrs_stripe::Error> {
let client = ClientBuilder::from_env()?
    .api_base("http://localhost:12111")   // point at stripe-mock in tests
    .build()?;
# Ok(())
# }
```

Missing or empty variables produce an error naming the variable, so a
misconfigured deployment fails immediately and legibly rather than at the first
API call.

## Webhook secrets

The same pattern:

```rust,no_run
use payrs_stripe::webhooks::WebhookRouter;

# fn run() -> Result<(), String> {
let router = WebhookRouter::from_env()?;                      // STRIPE_WEBHOOK_SECRET
let connect = WebhookRouter::from_env_var("WHSEC_CONNECT")?;  // custom name
# Ok(())
# }
```

## Sharing a client

`Client` is cheap to clone (internally reference-counted) and is `Send + Sync`.
Create **one per process** and share it — each client owns a connection pool,
so constructing one per request throws away connection reuse.

```rust,ignore
#[derive(Clone)]
struct AppState { stripe: payrs_stripe::Client }
```

## Connect

Set a default account on the builder, or override per request — the
per-request value wins:

```rust,ignore
client.request(Method::Post, "/v1/charges")
    .stripe_account("acct_customer_123")
```

## Secrets never leak

`SecretKey` prints as `sk_live_***` in `Debug` and `Display`, so a key cannot
reach your logs through a panic, a `dbg!`, or a derived `Debug` on a struct
that holds the client.

## API version

Requests send a pinned `Stripe-Version` matching the generated types. Override
per request for preview features:

```rust,ignore
client.request(Method::Get, "/v1/customers")
    .stripe_version("2026-06-24.dahlia; feature_beta=v3")
```

Prefer `send_json()` when overriding, since generated types match the pinned
version.
