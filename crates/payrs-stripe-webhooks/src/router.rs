//! The event router: register async handlers per event type, then hand it
//! raw deliveries. Verification, parsing, and dispatch in one call.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::event::{Event, EventType};
use crate::signature::Webhook;
use crate::WebhookError;

/// An error returned by a user-registered handler.
///
/// Wraps any `Error + Send + Sync`; the router maps handler failures to
/// [`WebhookError::Handler`] so your HTTP layer can return `5xx` and let
/// Stripe retry the delivery.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct HandlerError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

impl HandlerError {
    /// Wrap any error as a handler error.
    pub fn new(err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self(err.into())
    }
}

type BoxFuture = Pin<Box<dyn Future<Output = Result<(), HandlerError>> + Send>>;
type Handler = Arc<dyn Fn(Event) -> BoxFuture + Send + Sync>;

/// Verifies deliveries and dispatches events to registered async handlers.
///
/// * Exact-match handlers via [`WebhookRouter::on`].
/// * An optional catch-all via [`WebhookRouter::fallback`].
/// * Events with no matching handler are acknowledged successfully —
///   subscribing an endpoint to more event types than you handle is normal.
///
/// The router is `Send + Sync`; wrap it in an `Arc` and share it with your
/// HTTP framework's state.
///
/// ```
/// use payrs_stripe_webhooks::{EventType, WebhookRouter};
///
/// let router = WebhookRouter::new("whsec_…")
///     .on(EventType::CheckoutSessionCompleted, |event| async move {
///         // fulfill the order…
///         let _ = event;
///         Ok(())
///     });
/// ```
pub struct WebhookRouter {
    secret: String,
    handlers: HashMap<EventType, Handler>,
    fallback: Option<Handler>,
}

impl fmt::Debug for WebhookRouter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookRouter")
            .field("secret", &"whsec_***")
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .field("has_fallback", &self.fallback.is_some())
            .finish()
    }
}

impl WebhookRouter {
    /// Create a router bound to an endpoint signing secret (`whsec_…`).
    #[must_use]
    pub fn new(endpoint_secret: impl Into<String>) -> Self {
        Self {
            secret: endpoint_secret.into(),
            handlers: HashMap::new(),
            fallback: None,
        }
    }

    /// Create a router reading the signing secret from the
    /// `STRIPE_WEBHOOK_SECRET` environment variable.
    ///
    /// # Errors
    /// A [`crate::SignatureError::MissingHeader`]-adjacent config problem is
    /// not possible here; this returns `Err` (with the variable name) only
    /// if the variable is unset or empty.
    pub fn from_env() -> Result<Self, String> {
        Self::from_env_var("STRIPE_WEBHOOK_SECRET")
    }

    /// Like [`WebhookRouter::from_env`], with a custom variable name — for
    /// multiple endpoints (`STRIPE_WEBHOOK_SECRET_CHECKOUT`,
    /// `STRIPE_WEBHOOK_SECRET_CONNECT`, …).
    ///
    /// # Errors
    /// The variable name, if it is unset or empty.
    pub fn from_env_var(var_name: &str) -> Result<Self, String> {
        match std::env::var(var_name) {
            Ok(secret) if !secret.trim().is_empty() => Ok(Self::new(secret)),
            _ => Err(format!("environment variable `{var_name}` is not set or empty")),
        }
    }

    /// Register an async handler for one event type. Registering the same
    /// type twice replaces the previous handler.
    #[must_use]
    pub fn on<F, Fut>(mut self, event_type: EventType, handler: F) -> Self
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), HandlerError>> + Send + 'static,
    {
        self.handlers
            .insert(event_type, Arc::new(move |event| Box::pin(handler(event))));
        self
    }

    /// Register a catch-all for events with no exact-match handler. Useful
    /// for logging/metrics on unhandled types.
    #[must_use]
    pub fn fallback<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(Event) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), HandlerError>> + Send + 'static,
    {
        self.fallback = Some(Arc::new(move |event| Box::pin(handler(event))));
        self
    }

    /// Verify a delivery, parse the event, and dispatch it.
    ///
    /// Map the result in your HTTP handler:
    /// `Ok(())` → `200`; [`WebhookError::is_verification_failure`] → `400`;
    /// otherwise → `5xx` (Stripe retries).
    ///
    /// # Errors
    /// [`WebhookError::Signature`] / [`WebhookError::Parse`] on invalid
    /// deliveries; [`WebhookError::Handler`] if the matched handler failed.
    pub async fn handle(&self, payload: &[u8], signature_header: &str) -> Result<(), WebhookError> {
        let event = Webhook::construct_event(payload, signature_header, &self.secret)?;
        self.dispatch(event).await
    }

    /// Dispatch an already-verified event (e.g. from a queue you verified at
    /// the edge). Prefer [`WebhookRouter::handle`] in HTTP handlers.
    ///
    /// # Errors
    /// [`WebhookError::Handler`] if the matched handler failed.
    pub async fn dispatch(&self, event: Event) -> Result<(), WebhookError> {
        let handler = self
            .handlers
            .get(&event.event_type)
            .or(self.fallback.as_ref());
        match handler {
            Some(handler) => handler(event).await.map_err(WebhookError::Handler),
            // Unhandled types are acknowledged: endpoints commonly receive
            // more types than an app reacts to.
            None => Ok(()),
        }
    }

    /// The event types with registered handlers (excluding the fallback).
    /// Handy for asserting your endpoint's subscribed events match the code.
    pub fn handled_types(&self) -> impl Iterator<Item = &EventType> {
        self.handlers.keys()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn event(event_type: &str) -> Event {
        serde_json::from_value(serde_json::json!({
            "id": "evt_1", "object": "event", "type": event_type,
            "created": 1_700_000_000,
            "data": {"object": {"id": "pi_1"}},
            "livemode": false
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn dispatches_to_exact_handler() {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let router = WebhookRouter::new("whsec_x").on(
            EventType::PaymentIntentSucceeded,
            move |event: Event| {
                let counter = Arc::clone(&counter);
                async move {
                    assert_eq!(event.data.object["id"], "pi_1");
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        );

        router
            .dispatch(event("payment_intent.succeeded"))
            .await
            .unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unmatched_events_hit_fallback_then_ack() {
        let fallback_hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&fallback_hits);
        let router = WebhookRouter::new("whsec_x").fallback(move |_event| {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });

        router.dispatch(event("price.created")).await.unwrap();
        assert_eq!(fallback_hits.load(Ordering::SeqCst), 1);

        // No fallback registered: still acknowledged.
        let bare = WebhookRouter::new("whsec_x");
        bare.dispatch(event("price.created")).await.unwrap();
    }

    #[tokio::test]
    async fn handler_errors_surface_as_handler_variant() {
        let router = WebhookRouter::new("whsec_x")
            .on(EventType::InvoicePaymentFailed, |_event| async move {
                Err(HandlerError::new("database down"))
            });
        let err = router
            .dispatch(event("invoice.payment_failed"))
            .await
            .unwrap_err();
        assert!(
            !err.is_verification_failure(),
            "handler errors must map to 5xx"
        );
    }

    #[tokio::test]
    async fn end_to_end_verify_and_dispatch() {
        let secret = "whsec_e2e";
        let payload = serde_json::json!({
            "id": "evt_9", "object": "event", "type": "checkout.session.completed",
            "created": 1_700_000_000,
            "data": {"object": {"id": "cs_1"}}, "livemode": false
        })
        .to_string();
        let header = crate::signature::tests_helper_sign(payload.as_bytes(), secret);

        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let router =
            WebhookRouter::new(secret).on(EventType::CheckoutSessionCompleted, move |_e| {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            });

        router.handle(payload.as_bytes(), &header).await.unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        let err = router.handle(b"tampered", &header).await.unwrap_err();
        assert!(err.is_verification_failure());
    }
}
