# payrs-stripe-client

The transport core of [`payrs-stripe`](https://crates.io/crates/payrs-stripe),
an unofficial Rust SDK for the Stripe API.

You normally depend on `payrs-stripe` (which re-exports this crate) rather than
on it directly. Use it directly if you want the HTTP behaviour — auth, retries,
idempotency, error mapping — without the generated API surface.

## What it does

- **Authentication** and the pinned `Stripe-Version` header on every request.
- **Automatic idempotency keys** on `POST`/`DELETE`, generated *before* the
  first attempt and reused across retries — which is what makes retrying a
  payment safe.
- **Retries** with exponential backoff plus full jitter, on connection errors
  and HTTP 409/429/500/503, honouring `Stripe-Should-Retry` and `Retry-After`.
- **Structured errors** carrying Stripe's error envelope *and* the `Request-Id`
  that Stripe support asks for.
- **Redacted secrets** — `SecretKey` prints as `sk_live_***` in every `Debug`
  and `Display`, so keys cannot leak through logs or panics.
- **Both namespaces** — form-encoded bodies for `/v1`, JSON for `/v2`, with
  `Stripe-Context` and per-request version overrides.
- **A pluggable transport** — `HttpTransport` is a small object-safe trait.
  The default is `reqwest` + rustls; tests inject mocks, production can inject
  proxies or a different HTTP stack.

## Example

```rust,no_run
use payrs_stripe_client::{Client, Method};

# async fn run() -> Result<(), payrs_stripe_client::Error> {
let client = Client::from_env()?;             // STRIPE_SECRET_KEY

let customer: serde_json::Value = client
    .request(Method::Post, "/v1/customers")
    .form_pairs([("name", "Ada Lovelace")])
    .idempotency_key("signup-user-42")        // survives process restarts
    .send_json()
    .await?;
# Ok(())
# }
```

## Handling errors

```rust,no_run
# use payrs_stripe_client::{Client, Error, Method};
# async fn run(client: &Client) -> Result<(), Error> {
match client.request(Method::Post, "/v1/charges").send_json().await {
    Ok(charge) => println!("{charge}"),
    Err(err) => {
        if let Some(api) = err.as_api_error() {
            if api.is_card_declined() {
                // `message` is safe to show the customer for card errors
                eprintln!("declined: {:?}", api.message);
            }
        }
        // Always log this — it is what Stripe support will ask for
        eprintln!("request-id: {:?}", err.request_id());
    }
}
# Ok(())
# }
```

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `rustls` | yes | TLS via rustls (no system OpenSSL needed) |
| `native-tls` | no | TLS via the platform's native stack |

## License

MIT OR Apache-2.0
