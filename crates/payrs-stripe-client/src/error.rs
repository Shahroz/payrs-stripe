//! The SDK's single error surface.
//!
//! Every fallible public API returns [`Error`]. API-side failures preserve
//! Stripe's full error envelope plus the `Request-Id` header — the first
//! thing Stripe support asks for.

use serde::Deserialize;
use smol_str_shim::SmolStrShim;

use crate::transport::TransportError;

// Tiny local alias to avoid a hard smol_str dependency in this crate.
mod smol_str_shim {
    pub(crate) type SmolStrShim = String;
}

/// The `Request-Id` header value Stripe attaches to every response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestId(pub String);

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stripe's `error.type` field.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorType {
    /// Failure to connect to Stripe's API (rare in envelopes; kept for parity).
    ApiError,
    /// Card errors: the most common type, safe to show users after mapping.
    CardError,
    /// Idempotency key reused with different parameters.
    IdempotencyError,
    /// Request has invalid parameters.
    InvalidRequestError,
    /// Any error type this SDK release doesn't know yet.
    #[serde(untagged)]
    Other(SmolStrShim),
}

/// Stripe's machine-readable `error.code` (e.g. `card_declined`), kept as a
/// string: the vocabulary is large and grows; matching is by `as_str()`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct ApiErrorCode(pub String);

impl ApiErrorCode {
    /// The raw code string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A structured error returned by the Stripe API (HTTP 4xx/5xx envelope).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ApiError {
    /// The type of error.
    #[serde(rename = "type")]
    pub error_type: Option<ApiErrorType>,
    /// Machine-readable error code, e.g. `card_declined`.
    pub code: Option<ApiErrorCode>,
    /// For card errors: the bank's decline reason, e.g. `insufficient_funds`.
    pub decline_code: Option<String>,
    /// Human-readable message. May be shown to your users for card errors.
    pub message: Option<String>,
    /// The request parameter the error relates to, if any.
    pub param: Option<String>,
    /// Link to relevant Stripe documentation.
    pub doc_url: Option<String>,
    /// HTTP status of the response (not part of the JSON envelope).
    #[serde(skip)]
    pub status: u16,
    /// The `Request-Id` header — quote this when contacting Stripe support.
    #[serde(skip)]
    pub request_id: Option<RequestId>,
}

impl ApiError {
    /// True if this is a card error with code `card_declined`.
    #[must_use]
    pub fn is_card_declined(&self) -> bool {
        self.code
            .as_ref()
            .is_some_and(|c| c.as_str() == "card_declined")
    }

    /// True if Stripe rate-limited the request (HTTP 429).
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        self.status == 429
    }

    /// True if an idempotency key was reused with different parameters.
    #[must_use]
    pub fn is_idempotency_conflict(&self) -> bool {
        matches!(self.error_type, Some(ApiErrorType::IdempotencyError))
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Stripe API error (HTTP {})", self.status)?;
        if let Some(code) = &self.code {
            write!(f, " [{}]", code.as_str())?;
        }
        if let Some(msg) = &self.message {
            write!(f, ": {msg}")?;
        }
        if let Some(rid) = &self.request_id {
            write!(f, " (request-id: {rid})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

/// Wire shape: Stripe nests the error under an `"error"` key.
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorEnvelope {
    pub(crate) error: ApiError,
}

/// Every error the SDK can produce.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Stripe returned an error response (4xx/5xx with an error envelope).
    #[error(transparent)]
    Api(Box<ApiError>),

    /// The request never produced a usable HTTP response
    /// (connect failure, timeout, TLS, …).
    #[error("transport error: {0}")]
    Network(#[from] TransportError),

    /// Stripe returned 2xx but the body didn't match the expected type.
    /// Includes the JSON path that failed, for actionable bug reports.
    #[error("failed to deserialize Stripe response at `{path}`: {source}{}",
        request_id.as_ref().map(|r| format!(" (request-id: {r})")).unwrap_or_default())]
    Deserialization {
        /// JSON path of the failing field (from `serde_path_to_error`).
        path: String,
        /// Underlying serde error.
        #[source]
        source: serde_json::Error,
        /// Request id of the offending response, if present.
        request_id: Option<RequestId>,
    },

    /// Client-side configuration problem (bad base URL, missing env var, …).
    #[error("configuration error: {0}")]
    Config(String),
}

impl Error {
    /// Construct a [`Error::Deserialization`] — for SDK-internal layers
    /// (pagination, generated builders) that post-process JSON responses.
    #[must_use]
    pub fn deserialization(
        path: impl Into<String>,
        source: serde_json::Error,
        request_id: Option<RequestId>,
    ) -> Self {
        Self::Deserialization { path: path.into(), source, request_id }
    }

    /// The Stripe API error, if this is an [`Error::Api`].
    #[must_use]
    pub fn as_api_error(&self) -> Option<&ApiError> {
        match self {
            Self::Api(e) => Some(e),
            _ => None,
        }
    }

    /// The `Request-Id` associated with this error, when one exists.
    #[must_use]
    pub fn request_id(&self) -> Option<&RequestId> {
        match self {
            Self::Api(e) => e.request_id.as_ref(),
            Self::Deserialization { request_id, .. } => request_id.as_ref(),
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parses_stripe_error_envelope() {
        let body = r#"{
            "error": {
                "type": "card_error",
                "code": "card_declined",
                "decline_code": "insufficient_funds",
                "message": "Your card has insufficient funds.",
                "doc_url": "https://stripe.com/docs/error-codes/card-declined"
            }
        }"#;
        let env: ErrorEnvelope = serde_json::from_str(body).unwrap();
        let mut err = env.error;
        err.status = 402;
        assert!(err.is_card_declined());
        assert_eq!(err.error_type, Some(ApiErrorType::CardError));
        assert_eq!(err.decline_code.as_deref(), Some("insufficient_funds"));
    }

    #[test]
    fn unknown_error_type_never_fails() {
        let body = r#"{"error": {"type": "brand_new_error_type", "message": "hi"}}"#;
        let env: ErrorEnvelope = serde_json::from_str(body).unwrap();
        assert_eq!(
            env.error.error_type,
            Some(ApiErrorType::Other("brand_new_error_type".into()))
        );
    }
}
