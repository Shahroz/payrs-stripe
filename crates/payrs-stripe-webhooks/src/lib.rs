//! Stripe webhook handling: signature verification, typed events, and an
//! async event router.
//!
//! Three layers, use as much as you need:
//!
//! 1. [`Webhook::construct_event`] — verify the `Stripe-Signature` header
//!    (constant-time HMAC-SHA256, replay-attack tolerance window) and parse
//!    a typed [`Event`]. **Always pass the raw request body bytes** — any
//!    framework body transformation breaks the signature.
//! 2. [`Event`] / [`EventType`] — a typed event with a strongly-typed
//!    `type` enum (plus `Other` for event types newer than this SDK) and
//!    helpers to deserialize `data.object` into your own types.
//! 3. [`WebhookRouter`] — register async handlers per event type and let the
//!    router verify + dispatch in one call. Unmatched events fall through to
//!    an optional catch-all.
//!
//! # Quickstart (any framework)
//!
//! ```
//! use payrs_stripe_webhooks::{Event, EventType, WebhookRouter};
//!
//! # async fn demo() -> Result<(), payrs_stripe_webhooks::WebhookError> {
//! let router = WebhookRouter::new("whsec_test_secret")
//!     .on(EventType::PaymentIntentSucceeded, |event: Event| async move {
//!         let object = event.data.object; // serde_json::Value
//!         println!("payment succeeded: {}", object["id"]);
//!         Ok(())
//!     })
//!     .on(EventType::InvoicePaymentFailed, |event| async move {
//!         println!("dunning time: {}", event.id);
//!         Ok(())
//!     })
//!     .fallback(|event| async move {
//!         println!("unhandled event type: {}", event.event_type);
//!         Ok(())
//!     });
//!
//! // In your HTTP handler — raw body bytes + Stripe-Signature header value:
//! # let (payload, signature): (&[u8], &str) = (b"", "");
//! # let _ = (payload, signature, &router);
//! // router.handle(payload, signature).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Axum example
//!
//! ```ignore
//! async fn stripe_webhook(
//!     State(router): State<Arc<WebhookRouter>>,
//!     headers: HeaderMap,
//!     body: Bytes, // raw bytes — do NOT use a JSON extractor here
//! ) -> StatusCode {
//!     let Some(sig) = headers.get("stripe-signature").and_then(|v| v.to_str().ok()) else {
//!         return StatusCode::BAD_REQUEST;
//!     };
//!     match router.handle(&body, sig).await {
//!         Ok(()) => StatusCode::OK,
//!         Err(e) if e.is_verification_failure() => StatusCode::BAD_REQUEST,
//!         Err(_) => StatusCode::INTERNAL_SERVER_ERROR, // Stripe will retry
//!     }
//! }
//! ```
//!
//! Operational guidance baked into the design: return `2xx` fast and do slow
//! work in a background task; Stripe retries non-2xx responses with backoff.
//! Verification failures should map to `400` so misconfigured secrets are
//! visible in your dashboard.
//!
//! # v2 thin events
//!
//! Event destinations created in the v2 namespace can deliver **thin**
//! payloads (a pointer, not a snapshot). Parse them with
//! [`Webhook::construct_thin_event`] — same signature scheme — then fetch
//! the full object via the API using [`ThinEvent::related_object`].

mod event;
mod router;
mod signature;

pub use event::{Event, EventData, EventRequest, EventType, RelatedObject, ThinEvent};
pub use router::{HandlerError, WebhookRouter};
pub use signature::{SignatureError, Webhook, DEFAULT_TOLERANCE};

/// Everything that can go wrong while handling a webhook delivery.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WebhookError {
    /// The `Stripe-Signature` header failed verification.
    #[error(transparent)]
    Signature(#[from] SignatureError),
    /// The payload verified but was not a valid event JSON document.
    #[error("failed to parse verified event payload: {0}")]
    Parse(#[from] serde_json::Error),
    /// A registered handler returned an error.
    #[error("event handler failed: {0}")]
    Handler(#[from] HandlerError),
}

impl WebhookError {
    /// True when the delivery itself was invalid (bad signature, stale
    /// timestamp, malformed payload) — map these to HTTP `400`. Handler
    /// failures return `false` — map those to `5xx` so Stripe retries.
    #[must_use]
    pub fn is_verification_failure(&self) -> bool {
        matches!(self, Self::Signature(_) | Self::Parse(_))
    }
}
