//! The HTTP abstraction layer.
//!
//! [`HttpTransport`] is a minimal, object-safe trait between the SDK and the
//! network. The default implementation uses `reqwest` (rustls), but tests
//! inject mocks and advanced users can bring proxies, middleware, or a
//! different HTTP stack — without a breaking change to the SDK (ADR-003).

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// A transport-level request: everything needed to hit the wire.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Request {
    /// HTTP method as an uppercase string (`GET`, `POST`, `DELETE`).
    pub method: &'static str,
    /// Fully-qualified URL.
    pub url: String,
    /// Header name/value pairs. Names are case-insensitive per HTTP.
    pub headers: Vec<(String, String)>,
    /// Optional request body (form-encoded for Stripe v1).
    pub body: Option<Vec<u8>>,
}

/// A transport-level response.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Response {
    /// HTTP status code.
    pub status: u16,
    /// Response headers, lowercased names.
    pub headers: HashMap<String, String>,
    /// Raw response body bytes.
    pub body: Vec<u8>,
}

impl Response {
    /// Construct a response — used by custom [`HttpTransport`]
    /// implementations and tests (the struct is `#[non_exhaustive]`).
    #[must_use]
    pub fn new(status: u16, headers: HashMap<String, String>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// Fetch a header by (case-insensitive) name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

/// Why a request failed before producing a usable HTTP response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransportErrorKind {
    /// DNS/TCP/TLS connection failure.
    Connect,
    /// The configured timeout elapsed.
    Timeout,
    /// Anything else (protocol errors, body read failures, …).
    Other,
}

/// A transport-level failure. Connect/timeout errors are retried by the
/// client because the paired idempotency key makes retries safe.
#[derive(Debug, thiserror::Error)]
#[error("{kind:?} error talking to Stripe: {message}")]
pub struct TransportError {
    /// Classification used by the retry policy.
    pub kind: TransportErrorKind,
    /// Human-readable detail (never contains credentials).
    pub message: String,
}

impl TransportError {
    /// Construct a transport error.
    #[must_use]
    pub fn new(kind: TransportErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Whether the retry policy treats this as retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.kind,
            TransportErrorKind::Connect | TransportErrorKind::Timeout
        )
    }
}

/// Boxed future returned by [`HttpTransport::execute`].
pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Response, TransportError>> + Send + 'a>>;

/// An object-safe async HTTP executor.
///
/// Implementations must be `Send + Sync` — one [`crate::Client`] is shared
/// across tasks. Implementations should enforce their own timeouts and map
/// them to [`TransportErrorKind::Timeout`].
pub trait HttpTransport: Send + Sync + 'static {
    /// Execute one HTTP request. Retries are the *client's* job, not the
    /// transport's — implementations must not retry internally, or
    /// idempotency-key semantics and backoff budgets break.
    fn execute(&self, request: Request) -> TransportFuture<'_>;
}

impl fmt::Debug for dyn HttpTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("dyn HttpTransport")
    }
}

/// The default transport: `reqwest` with rustls.
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl fmt::Debug for ReqwestTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReqwestTransport")
    }
}

impl ReqwestTransport {
    /// Build the default transport with the given total request timeout.
    ///
    /// # Errors
    /// Returns a [`TransportError`] if the underlying TLS backend fails to
    /// initialize.
    pub fn new(timeout: Duration) -> Result<Self, TransportError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| TransportError::new(TransportErrorKind::Other, e.to_string()))?;
        Ok(Self { client })
    }
}

impl HttpTransport for ReqwestTransport {
    fn execute(&self, request: Request) -> TransportFuture<'_> {
        Box::pin(async move {
            let method = reqwest::Method::from_bytes(request.method.as_bytes())
                .map_err(|e| TransportError::new(TransportErrorKind::Other, e.to_string()))?;

            let mut builder = self.client.request(method, &request.url);
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            if let Some(body) = request.body {
                builder = builder.body(body);
            }

            let resp = builder
                .send()
                .await
                .map_err(|e| classify_reqwest_error(&e))?;

            let status = resp.status().as_u16();
            let mut headers = HashMap::new();
            for (name, value) in resp.headers() {
                if let Ok(v) = value.to_str() {
                    headers.insert(name.as_str().to_ascii_lowercase(), v.to_owned());
                }
            }
            let body = resp
                .bytes()
                .await
                .map_err(|e| classify_reqwest_error(&e))?
                .to_vec();

            Ok(Response {
                status,
                headers,
                body,
            })
        })
    }
}

fn classify_reqwest_error(err: &reqwest::Error) -> TransportError {
    let kind = if err.is_timeout() {
        TransportErrorKind::Timeout
    } else if err.is_connect() {
        TransportErrorKind::Connect
    } else {
        TransportErrorKind::Other
    };
    // reqwest error Display does not include request bodies or auth headers.
    TransportError::new(kind, err.to_string())
}
