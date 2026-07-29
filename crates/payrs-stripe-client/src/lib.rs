//! Transport core for the `payrs-stripe` SDK.
//!
//! This crate owns everything between "a typed request builder produced a
//! [`RequestSpec`]" and "the caller got a `Result<T, Error>`":
//!
//! * [`Client`] — cheap-to-clone handle; authentication, pinned
//!   `Stripe-Version`, Connect headers, telemetry.
//! * Automatic **idempotency keys** on mutating requests, reused across
//!   retries (that reuse is what makes retrying safe).
//! * **Retries** with exponential backoff + full jitter, honoring
//!   `Stripe-Should-Retry` and `Retry-After` ([`RetryPolicy`]).
//! * A structured [`Error`] carrying Stripe's error envelope and the
//!   `Request-Id` support will ask you for.
//! * A **raw escape hatch** ([`Client::request`]) for any endpoint the typed
//!   surface doesn't cover yet.
//! * An [`HttpTransport`] trait so tests inject mock transports and advanced
//!   users bring their own HTTP stack; the default is `reqwest` + rustls.
//!
//! # Quickstart
//!
//! ```no_run
//! use payrs_stripe_client::{Client, Method};
//!
//! # async fn run() -> Result<(), payrs_stripe_client::Error> {
//! let client = Client::from_env()?; // reads STRIPE_SECRET_KEY
//!
//! let customer: serde_json::Value = client
//!     .request(Method::Post, "/v1/customers")
//!     .form_pairs([("name", "Ada Lovelace"), ("email", "ada@example.com")])
//!     .send_json()
//!     .await?;
//!
//! println!("created {}", customer["id"]);
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod error;
pub mod request;
pub mod retry;
pub mod secret;
pub mod transport;

pub use client::{AppInfo, Client, ClientBuilder};
pub use error::{ApiError, ApiErrorCode, ApiErrorType, Error, RequestId};
pub use request::{Body, Method, RequestBuilder, RequestSpec};
pub use retry::RetryPolicy;
pub use secret::SecretKey;
pub use transport::{HttpTransport, Request, Response, TransportError, TransportErrorKind};
