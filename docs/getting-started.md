# Getting started

## Install

```toml
[dependencies]
payrs-stripe = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Default features give you the full API surface, webhooks, and rustls TLS.

## Your first call

```rust,no_run
use payrs_stripe::Client;
use payrs_stripe::api::v1::customers::PostCustomers;

#[tokio::main]
async fn main() -> Result<(), payrs_stripe::Error> {
    let client = Client::from_env()?;   // reads STRIPE_SECRET_KEY

    let customer = PostCustomers::new()
        .name("Ada Lovelace")
        .email("ada@example.com")
        .send(&client)
        .await?;

    println!("created {:?}", customer.id);
    Ok(())
}
```

Run it against a test key:

```bash
STRIPE_SECRET_KEY=sk_test_… cargo run
```

## How the API surface is organised

Operations live in `payrs_stripe::api::v1::<section>::<Operation>`, named after
the HTTP method and path:

| You want to | Builder |
|---|---|
| Create a customer | `v1::customers::PostCustomers` |
| Retrieve a payment intent | `v1::payment_intents::GetPaymentIntentsIntent` |
| List invoices | `v1::invoices::GetInvoices` |
| Finalise an invoice | `v1::invoices::PostInvoicesInvoiceFinalize` |
| Expire a checkout session | `v1::checkout::PostCheckoutSessionsSessionExpire` |

Required parameters are arguments to `new()`; optional ones are setters:

```rust,ignore
v1::payment_intents::PostPaymentIntents::new(1999, "usd")  // required
    .customer("cus_123")                                    // optional
    .send(&client)
```

`docs/coverage.md` lists every endpoint and its builder.

## Taking a payment

```rust,no_run
# use payrs_stripe::Client;
use payrs_stripe::api::v1::checkout::PostCheckoutSessions;

# async fn run(client: &Client) -> Result<(), payrs_stripe::Error> {
let session = PostCheckoutSessions::new()
    .mode("payment")
    .success_url("https://example.com/success?sid={CHECKOUT_SESSION_ID}")
    .cancel_url("https://example.com/cancel")
    .param("line_items", serde_json::json!([{
        "price_data": {
            "currency": "usd",
            "unit_amount": 1999,
            "product_data": { "name": "Widget" }
        },
        "quantity": 2
    }]))
    .send(client)
    .await?;

// Redirect the customer here:
println!("{:?}", session.url);
# Ok(())
# }
```

Then confirm fulfilment from the `checkout.session.completed` webhook rather
than the redirect — see [webhooks](webhooks.md). A customer can close the tab
before the redirect fires; the webhook still arrives.

## Amounts

Amounts are `i64` **minor units**: `1999` means $19.99. Zero-decimal
currencies such as JPY take the whole number. Never use floats for money.

## Next steps

- [Configuration](configuration.md) — env vars, timeouts, Connect
- [Errors and retries](errors-and-retries.md) — declines and idempotency
- [Testing](testing.md) — no-network tests
