# payrs-stripe-types

Shared primitive types for [`payrs-stripe`](https://crates.io/crates/payrs-stripe),
an unofficial Rust SDK for the Stripe API.

You normally depend on `payrs-stripe` (which re-exports everything here) rather
than on this crate directly. Depend on it directly only if you are building a
library that must speak Stripe's vocabulary without pulling in the HTTP client.

## What's in here

| Type | Purpose |
|---|---|
| `ids::*` | Prefix-validated typed IDs — `CustomerId`, `PaymentIntentId`, … |
| `Currency` | ISO-4217 codes, with an `Other` catch-all |
| `Timestamp` | Unix epoch seconds, Stripe's wire format |
| `Expandable<T>` | A field that is either an ID or the expanded object |
| `List<T>` | Stripe's cursor-paginated list envelope |
| `Metadata` | The `metadata` map (`BTreeMap<String, String>`) |
| `ApiVersion` | The pinned `Stripe-Version` for this release line |

## Design rules

These hold across the whole SDK and are worth knowing:

- **Money is always `i64` minor units** (cents, pence). Never floating point.
- **Typed IDs prevent mix-ups at compile time.** Passing a `PriceId` where a
  `ProductId` belongs is a type error, not a runtime 400.
  ```rust
  use payrs_stripe_types::ids::CustomerId;

  let id: CustomerId = "cus_123".parse()?;      // validates the `cus_` prefix
  assert!("price_123".parse::<CustomerId>().is_err());
  # Ok::<(), payrs_stripe_types::IdError>(())
  ```
  Parsing validates, but **deserialization is deliberately lenient**: an SDK
  must never fail to read data Stripe actually sent.
- **Every Stripe-owned enum has an `Other` variant.** New API values can never
  break deserialization.
- **Response structs are `#[non_exhaustive]`.** Stripe adding a field is not a
  breaking change for your code.

## License

MIT OR Apache-2.0
