# Testing your integration

Three levels, from fastest to most realistic.

## 1. Mock transport — no network at all

Inject an `HttpTransport` and assert on exactly what your code sends. This is
how this SDK tests itself.

```rust,ignore
use std::sync::{Arc, Mutex};
use payrs_stripe::{Client, HttpTransport};
use payrs_stripe_client::transport::{Request, Response, TransportFuture};

struct Mock { seen: Mutex<Vec<Request>> }

impl HttpTransport for Mock {
    fn execute(&self, request: Request) -> TransportFuture<'_> {
        self.seen.lock().unwrap().push(request);
        Box::pin(async {
            Ok(Response::new(200, Default::default(), br#"{"id":"cus_1"}"#.to_vec()))
        })
    }
}

let mock = Arc::new(Mock { seen: Mutex::new(Vec::new()) });
let client = Client::builder("sk_test_x")
    .transport(Arc::clone(&mock) as Arc<dyn HttpTransport>)
    .build()?;
```

Good for asserting request shape: that the right amount was sent, that an
idempotency key was attached, that a filter reached the query string.

## 2. stripe-mock — real HTTP, no Stripe account

Stripe's official mock server validates requests against the OpenAPI spec and
returns realistic fixtures.

```bash
./scripts/stripe-mock.sh          # docker, listens on :12111
```

```rust,ignore
let client = Client::builder("sk_test_123")
    .api_base("http://localhost:12111")
    .build()?;
```

Or without touching code, since the client reads it from the environment:

```bash
STRIPE_API_BASE=http://localhost:12111 cargo test
```

Good for catching malformed requests. It does **not** simulate business logic:
state does not persist between calls.

## 3. Stripe test mode — the real API

Use a `sk_test_…` key against the real API for end-to-end flows, with
[test cards](https://docs.stripe.com/testing) for specific outcomes such as
`4000000000000002` (declined) or `4000000000009995` (insufficient funds).

## Testing webhooks

Sign a payload yourself and feed it to your router — no network, no tunnel:

```rust,ignore
// The signature scheme is HMAC-SHA256 over "{timestamp}.{body}".
let router = WebhookRouter::new("whsec_test").on(EventType::Ping, handler);
router.handle(payload_bytes, &signature_header).await?;
```

For live deliveries during development, the Stripe CLI forwards real events:

```bash
stripe listen --forward-to localhost:3000/webhooks
stripe trigger payment_intent.succeeded
```

## Checklist

- Amounts are minor units — assert `1999`, not `19.99`.
- Assert that idempotency keys are stable across retries of the same logical
  operation.
- Cover the decline path, not just the happy path.
- Cover a truncated listing (`has_more: true`) if you paginate.
