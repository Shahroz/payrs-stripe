//! An ergonomic, **unofficial** Rust SDK for the [Stripe](https://stripe.com) API.
//!
//! > Not affiliated with or endorsed by Stripe, Inc.
//!
//! # What's inside
//!
//! * **Transport core** — [`Client`] with automatic retries (backoff +
//!   jitter, `Stripe-Should-Retry` honored), automatic idempotency keys
//!   reused across retries, redacted secrets, and Stripe's error envelope
//!   with `Request-Id` on every failure.
//! * **Both API namespaces** — v1 (form-encoded) and v2 (JSON,
//!   `Stripe-Context`, thin events) through the same client.
//! * **Full API surface** (feature `api`, on by default) — every v1 model as
//!   an exported struct in [`models`] and every v1 operation as a typed
//!   builder in [`api::v1`], generated from Stripe's official `OpenAPI` spec;
//!   v2 core endpoints in [`api::v2`].
//! * **Webhooks** (feature `webhooks`, on by default) — signature
//!   verification, typed [`webhooks::Event`]/[`webhooks::EventType`], thin
//!   events, and the async [`webhooks::WebhookRouter`].
//! * **Raw escape hatch** — [`Client::request`] reaches any endpoint in
//!   either namespace, including ones newer than this SDK.
//!
//! # Quickstart
//!
//! ```no_run
//! use payrs_stripe::Client;
//! use payrs_stripe::api::v1::customers::PostCustomers;
//! use payrs_stripe::api::v1::payment_intents::PostPaymentIntents;
//!
//! # async fn run() -> Result<(), payrs_stripe::Error> {
//! let client = Client::from_env()?; // STRIPE_SECRET_KEY
//!
//! let customer = PostCustomers::new()
//!     .name("Ada Lovelace")
//!     .email("ada@example.com")
//!     .send(&client)
//!     .await?;
//!
//! let intent = PostPaymentIntents::new(1999, "usd")
//!     .customer(customer.id.clone().unwrap_or_default())
//!     .send(&client)
//!     .await?;
//! println!("intent: {:?}", intent.id);
//! # Ok(())
//! # }
//! ```
//!
//! # Webhooks
//!
//! ```
//! # #[cfg(feature = "webhooks")] {
//! use payrs_stripe::webhooks::{EventType, WebhookRouter};
//!
//! let router = WebhookRouter::new("whsec_…")
//!     .on(EventType::PaymentIntentSucceeded, |event| async move {
//!         println!("paid: {}", event.data.object["id"]);
//!         Ok(())
//!     })
//!     .fallback(|event| async move {
//!         println!("unhandled: {}", event.event_type);
//!         Ok(())
//!     });
//! # let _ = router;
//! # }
//! ```

pub use payrs_stripe_client::{
    ApiError, ApiErrorCode, ApiErrorType, AppInfo, Body, Client, ClientBuilder, Error,
    HttpTransport, Method, RequestBuilder, RequestId, RequestSpec, RetryPolicy, SecretKey,
    TransportError, TransportErrorKind,
};
pub use payrs_stripe_types as types;
pub use payrs_stripe_types::{ApiVersion, Currency, Expandable, List, Metadata, Object, Timestamp};

/// Full generated API surface: models + v1 operations + v2 core.
#[cfg(feature = "api")]
pub use payrs_stripe_api as api;

/// Cursor pagination over list endpoints (`.paginate()` on generated
/// list builders).
#[cfg(feature = "api")]
pub use payrs_stripe_api::Paginator;

/// Every Stripe API object model, re-exported at the crate root for
/// client-code convenience: `payrs_stripe::models::PaymentIntent`.
#[cfg(feature = "api")]
pub use payrs_stripe_api::models;

/// Webhook verification, typed events, and the async event router.
#[cfg(feature = "webhooks")]
pub use payrs_stripe_webhooks as webhooks;

/// One-line import for application code:
/// `use payrs_stripe::prelude::*;`
pub mod prelude {
    #[cfg(feature = "api")]
    pub use crate::api::v1;
    #[cfg(feature = "api")]
    pub use crate::models;
    #[cfg(feature = "webhooks")]
    pub use crate::webhooks::{Event, EventType, WebhookRouter};
    pub use crate::{Client, Error, Method, RetryPolicy};
    pub use payrs_stripe_types::{Currency, List, Metadata, Timestamp};
}
