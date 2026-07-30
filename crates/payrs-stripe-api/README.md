# payrs-stripe-api

The generated, full-surface Stripe API bindings behind
[`payrs-stripe`](https://crates.io/crates/payrs-stripe), an unofficial Rust SDK.

Usually reached via `payrs-stripe`'s `api` feature (on by default).

## Contents

| Module | What it holds |
|---|---|
| `models` | **1,431** API object structs, grouped into 77 domain modules |
| `v1` | **587** typed operations across 76 API sections |
| `v2` | Hand-written bindings for the v2 core namespace |
| `Paginator` | Cursor pagination, available on all 127 list endpoints |

Everything is generated from Stripe's official OpenAPI specification by
`codegen/generate.py`. The endpoint-to-builder map is in `docs/coverage.md`.

## Operations

Required parameters are arguments to `new()`; optional ones are chainable
setters:

```rust,no_run
# use payrs_stripe_client::{Client, Error};
use payrs_stripe_api::v1;

# async fn run(client: &Client) -> Result<(), Error> {
let intent = v1::payment_intents::PostPaymentIntents::new(1999, "usd")
    .customer("cus_123")
    .capture_method("manual")
    .send(client)
    .await?;
# Ok(())
# }
```

Anything the typed setters do not express — deeply nested or brand-new
parameters — goes through `.param()`, which serialises to Stripe's bracket
form encoding:

```rust,ignore
v1::checkout::PostCheckoutSessions::new()
    .mode("payment")
    .param("line_items", serde_json::json!([
        { "price": "price_123", "quantity": 2 }
    ]))
```

## Models

Grouped by domain and **also** re-exported flat, so both paths work:

```rust
use payrs_stripe_api::models::Customer;             // flat
use payrs_stripe_api::models::customers::Customer as C2;  // grouped
```

Every field is `Option`, because Stripe omits fields by context and adds new
ones over time; unknown JSON fields are ignored. That means model structs never
fail to deserialise a real Stripe response.

## Pagination

```rust,no_run
# use payrs_stripe_client::{Client, Error};
use payrs_stripe_api::v1::customers::GetCustomers;

# async fn run(client: &Client) -> Result<(), Error> {
let mut pager = GetCustomers::new().limit(100).paginate();
while let Some(page) = pager.next_page(client).await? {
    for customer in &page.data {
        println!("{:?}", customer.email);
    }
}
# Ok(())
# }
```

`collect_all(client, max_items)` drains a listing with an explicit bound, so a
large account cannot silently exhaust memory.

## Regenerating

```bash
python3 codegen/generate.py     # after updating codegen/spec3.json
```

## License

MIT OR Apache-2.0
