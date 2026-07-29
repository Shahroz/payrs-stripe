//! The raw request escape hatch and the request description shared with the
//! (future) typed resource crates.
//!
//! Stripe's v1 API takes `application/x-www-form-urlencoded` bodies with
//! bracketed nesting (`metadata[order_id]=42`, `items[0][price]=price_x`).
//! Typed builders and the raw hatch both compile down to a [`RequestSpec`],
//! which [`crate::Client`] executes with auth, idempotency, and retries.

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;

/// HTTP methods used by the Stripe v1 API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Method {
    /// Read a resource or list. Never carries an idempotency key.
    Get,
    /// Create or update (Stripe v1 uses POST for both). Idempotent via key.
    Post,
    /// Delete a resource. Idempotent via key.
    Delete,
}

impl Method {
    /// The method as an uppercase static string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Delete => "DELETE",
        }
    }

    /// Whether the client auto-attaches an idempotency key.
    #[must_use]
    pub const fn is_mutating(self) -> bool {
        matches!(self, Self::Post | Self::Delete)
    }
}

/// A request body in one of Stripe's two wire formats.
///
/// The v1 namespace takes `application/x-www-form-urlencoded`; the v2
/// namespace takes `application/json`. Typed builders pick the right one for
/// their endpoint automatically.
#[derive(Debug, Clone)]
pub enum Body {
    /// `application/x-www-form-urlencoded` — Stripe v1.
    Form(String),
    /// `application/json` — Stripe v2.
    Json(String),
}

impl Body {
    /// The `Content-Type` for this body.
    #[must_use]
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Form(_) => "application/x-www-form-urlencoded",
            Self::Json(_) => "application/json",
        }
    }

    /// The serialized bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Form(s) | Self::Json(s) => s.as_bytes(),
        }
    }
}

/// A fully-described Stripe API request, transport-agnostic.
///
/// Produced by [`RequestBuilder`] and by the generated typed builders in
/// `payrs-stripe-api`; consumed by [`Client`]. Works for both the `/v1` and
/// `/v2` namespaces.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RequestSpec {
    /// HTTP method.
    pub method: Method,
    /// Path starting with `/v1/…` or `/v2/…`.
    pub path: String,
    /// Query-string pairs (typically for GET/list requests).
    pub query: Vec<(String, String)>,
    /// A pre-encoded query string (used by generated builders that serialize
    /// nested params via `serde_qs`). Appended after `query` pairs.
    pub raw_query: Option<String>,
    /// Request body (form for v1, JSON for v2).
    pub body: Option<Body>,
    /// Explicit idempotency key; `None` means auto-generate for mutating
    /// methods.
    pub idempotency_key: Option<String>,
    /// Per-request `Stripe-Account` override (Connect).
    pub stripe_account: Option<String>,
    /// Per-request `Stripe-Version` override. Only the raw escape hatch sets
    /// this (e.g. for v2 preview versions); typed builders always use the
    /// pinned version so response shapes match the generated types.
    pub stripe_version: Option<String>,
    /// Per-request `Stripe-Context` (v2: act on a child account/sandbox).
    pub stripe_context: Option<String>,
}

impl RequestSpec {
    /// A bare spec for `method path` with no parameters.
    #[must_use]
    pub fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            raw_query: None,
            body: None,
            idempotency_key: None,
            stripe_account: None,
            stripe_version: None,
            stripe_context: None,
        }
    }
}

/// Builder for arbitrary Stripe API calls — the Tier-3 escape hatch.
///
/// Use this for endpoints the typed surface doesn't cover yet:
///
/// ```no_run
/// # use payrs_stripe_client::{Client, Method};
/// # async fn run(client: &Client) -> Result<(), payrs_stripe_client::Error> {
/// let reader: serde_json::Value = client
///     .request(Method::Post, "/v1/terminal/readers")
///     .form_pairs([("registration_code", "puppies-plug-could")])
///     .idempotency_key("reader-setup-store-42")
///     .send_json()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct RequestBuilder<'a> {
    client: &'a Client,
    spec: RequestSpec,
}

impl<'a> RequestBuilder<'a> {
    pub(crate) fn new(client: &'a Client, method: Method, path: impl Into<String>) -> Self {
        Self {
            client,
            spec: RequestSpec::new(method, path),
        }
    }

    /// Add raw form pairs. Keys may use Stripe's bracket syntax directly
    /// (`metadata[order_id]`). Values are percent-encoded for you.
    #[must_use]
    pub fn form_pairs<K, V>(mut self, pairs: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        let encoded = encode_pairs(pairs);
        match &mut self.spec.body {
            Some(Body::Form(body)) if !body.is_empty() => {
                body.push('&');
                body.push_str(&encoded);
            }
            _ => self.spec.body = Some(Body::Form(encoded)),
        }
        self
    }

    /// Serialize any `Serialize` type into Stripe's nested form encoding
    /// (`a[b][0][c]=…`) as the request body.
    ///
    /// # Errors
    /// Returns [`Error::Config`] if the value cannot be represented as a
    /// query string (e.g. a non-string map key).
    pub fn form<T: Serialize>(mut self, params: &T) -> Result<Self, Error> {
        let body = serde_qs::to_string(params)
            .map_err(|e| Error::Config(format!("failed to form-encode params: {e}")))?;
        self.spec.body = Some(Body::Form(body));
        Ok(self)
    }

    /// Serialize any `Serialize` type as a JSON body — the format of the
    /// Stripe **v2** namespace (`/v2/…` endpoints).
    ///
    /// ```no_run
    /// # use payrs_stripe_client::{Client, Method};
    /// # async fn run(client: &Client) -> Result<(), payrs_stripe_client::Error> {
    /// let dest: serde_json::Value = client
    ///     .request(Method::Post, "/v2/core/event_destinations")
    ///     .json(&serde_json::json!({
    ///         "name": "my destination",
    ///         "type": "webhook_endpoint",
    ///         "event_payload": "thin",
    ///         "enabled_events": ["ping"],
    ///         "webhook_endpoint": {"url": "https://example.com/hooks"},
    ///     }))?
    ///     .send_json()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns [`Error::Config`] if the value cannot be serialized to JSON.
    pub fn json<T: Serialize>(mut self, params: &T) -> Result<Self, Error> {
        let body = serde_json::to_string(params)
            .map_err(|e| Error::Config(format!("failed to JSON-encode params: {e}")))?;
        self.spec.body = Some(Body::Json(body));
        Ok(self)
    }

    /// Override the `Stripe-Version` header for this request only.
    ///
    /// Use for v2 preview versions. **Caution:** with typed responses the
    /// generated types match the SDK's pinned version; overriding is safest
    /// with `send_json`.
    #[must_use]
    pub fn stripe_version(mut self, version: impl Into<String>) -> Self {
        self.spec.stripe_version = Some(version.into());
        self
    }

    /// Set the `Stripe-Context` header (v2: scope the request to a child
    /// account or sandbox).
    #[must_use]
    pub fn stripe_context(mut self, context: impl Into<String>) -> Self {
        self.spec.stripe_context = Some(context.into());
        self
    }

    /// Add a query-string pair (for GET/list endpoints).
    #[must_use]
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.spec.query.push((key.into(), value.into()));
        self
    }

    /// Set an explicit idempotency key (≤ 255 chars; avoid PII).
    ///
    /// Prefer keys derived from your own business identifiers
    /// (`order_{id}_capture`) so retries are safe across process restarts,
    /// not just within one. Unset mutating requests get a `UUIDv4`.
    #[must_use]
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.spec.idempotency_key = Some(key.into());
        self
    }

    /// Act on behalf of a connected account for this request only
    /// (`Stripe-Account` header).
    #[must_use]
    pub fn stripe_account(mut self, account: impl Into<String>) -> Self {
        self.spec.stripe_account = Some(account.into());
        self
    }

    /// Send and deserialize the response into `T`.
    ///
    /// # Errors
    /// [`Error::Api`] for Stripe error responses, [`Error::Network`] for
    /// transport failures after retries, [`Error::Deserialization`] if a 2xx
    /// body doesn't match `T`.
    pub async fn send<T: DeserializeOwned>(self) -> Result<T, Error> {
        self.client.execute(self.spec).await
    }

    /// Send and return the response as loosely-typed JSON.
    ///
    /// # Errors
    /// Same as [`RequestBuilder::send`].
    pub async fn send_json(self) -> Result<serde_json::Value, Error> {
        self.send().await
    }
}

/// Percent-encode pairs as `application/x-www-form-urlencoded`.
///
/// Brackets in *keys* are preserved (`metadata[order_id]`) because Stripe's
/// servers accept them literally and it keeps hand-written pairs readable.
pub(crate) fn encode_pairs<K, V>(pairs: impl IntoIterator<Item = (K, V)>) -> String
where
    K: Into<String>,
    V: Into<String>,
{
    let mut out = String::new();
    for (key, value) in pairs {
        if !out.is_empty() {
            out.push('&');
        }
        encode_component(&mut out, &key.into(), true);
        out.push('=');
        encode_component(&mut out, &value.into(), false);
    }
    out
}

fn encode_component(out: &mut String, raw: &str, is_key: bool) {
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b'[' | b']' if is_key => out.push(byte as char),
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(hex_digit(byte >> 4));
                out.push(hex_digit(byte & 0x0F));
            }
        }
    }
}

fn hex_digit(nibble: u8) -> char {
    char::from_digit(u32::from(nibble), 16)
        .unwrap_or('0')
        .to_ascii_uppercase()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn encodes_pairs_with_brackets_and_specials() {
        let body = encode_pairs([("name", "Ada Lovelace"), ("metadata[order_id]", "ord/42&x")]);
        assert_eq!(body, "name=Ada+Lovelace&metadata[order_id]=ord%2F42%26x");
    }

    #[test]
    fn serde_form_encoding_nests_like_stripe() {
        #[derive(serde::Serialize)]
        struct Params<'a> {
            amount: i64,
            currency: &'a str,
            metadata: std::collections::BTreeMap<&'a str, &'a str>,
        }
        let params = Params {
            amount: 1999,
            currency: "usd",
            metadata: [("order_id", "42")].into_iter().collect(),
        };
        let s = serde_qs::to_string(&params).unwrap();
        assert_eq!(s, "amount=1999&currency=usd&metadata[order_id]=42");
    }
}
