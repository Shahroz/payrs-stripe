//! The Stripe [`Client`]: configuration + the request execution loop.

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;

use payrs_stripe_types::ApiVersion;

use crate::error::{ApiError, Error, ErrorEnvelope, RequestId};
use crate::request::{encode_pairs, Method, RequestBuilder, RequestSpec};
use crate::retry::{parse_retry_after, RetryPolicy};
use crate::secret::SecretKey;
use crate::transport::{HttpTransport, Request, ReqwestTransport, Response};

/// Optional application metadata appended to the `User-Agent`, mirroring the
/// `set_app_info` facility of official Stripe SDKs. Recommended for plugins
/// and platforms so Stripe can attribute traffic.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AppInfo {
    /// Application name.
    pub name: String,
    /// Application version, if any.
    pub version: Option<String>,
    /// Application URL, if any.
    pub url: Option<String>,
}

struct Inner {
    transport: Arc<dyn HttpTransport>,
    api_base: String,
    secret: SecretKey,
    stripe_account: Option<String>,
    retry: RetryPolicy,
    user_agent: String,
    client_user_agent: String,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("api_base", &self.api_base)
            .field("secret", &self.secret) // redacted by SecretKey's Debug
            .field("stripe_account", &self.stripe_account)
            .field("retry", &self.retry)
            .finish_non_exhaustive()
    }
}

/// A handle to the Stripe API.
///
/// Cheap to clone (`Arc` internals) and `Send + Sync`: create one per
/// process and share it across tasks.
///
/// ```no_run
/// use payrs_stripe_client::Client;
/// # fn run() -> Result<(), payrs_stripe_client::Error> {
/// let client = Client::new("sk_test_…");
/// let from_env = Client::from_env()?; // STRIPE_SECRET_KEY
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Client {
    inner: Arc<Inner>,
}

impl Client {
    /// Create a client with default configuration.
    ///
    /// Panics never; TLS initialization failures surface on first request.
    #[must_use]
    pub fn new(secret_key: impl Into<SecretKey>) -> Self {
        // Builder with defaults cannot fail except on transport init, which
        // `build_infallible` defers to first use.
        ClientBuilder::new(secret_key).build_infallible()
    }

    /// Create a client from the environment with default variable names:
    /// `STRIPE_SECRET_KEY` (required) and `STRIPE_API_BASE` (optional — e.g.
    /// point at `stripe-mock` in CI without touching code).
    ///
    /// Need overrides on top? Use [`ClientBuilder::from_env`].
    ///
    /// # Errors
    /// [`Error::Config`] if `STRIPE_SECRET_KEY` is unset or empty.
    pub fn from_env() -> Result<Self, Error> {
        ClientBuilder::from_env()?.build()
    }

    /// Create a client reading the secret key from a **custom** environment
    /// variable — for multi-account setups (`STRIPE_KEY_ACME`,
    /// `STRIPE_KEY_UMBRELLA`, …) or org-specific naming conventions.
    ///
    /// # Errors
    /// [`Error::Config`] if the variable is unset or empty.
    pub fn from_env_var(var_name: &str) -> Result<Self, Error> {
        ClientBuilder::from_env_var(var_name)?.build()
    }

    /// Start configuring a client.
    #[must_use]
    pub fn builder(secret_key: impl Into<SecretKey>) -> ClientBuilder {
        ClientBuilder::new(secret_key)
    }

    /// Begin a raw API request — the escape hatch for endpoints without a
    /// typed builder yet. `path` must start with `/` (e.g. `/v1/customers`).
    #[must_use]
    pub fn request(&self, method: Method, path: impl Into<String>) -> RequestBuilder<'_> {
        RequestBuilder::new(self, method, path)
    }

    /// Execute a fully-described request with auth, idempotency, and retries,
    /// deserializing the successful response into `T`.
    ///
    /// # Errors
    /// [`Error::Api`] for Stripe error envelopes, [`Error::Network`] once the
    /// retry budget is exhausted, [`Error::Deserialization`] for 2xx bodies
    /// that don't match `T`.
    pub async fn execute<T: DeserializeOwned>(&self, spec: RequestSpec) -> Result<T, Error> {
        let response = self.execute_raw(spec).await?;
        let request_id = response
            .header("request-id")
            .map(|s| RequestId(s.to_owned()));

        if (200..300).contains(&response.status) {
            let de = &mut serde_json::Deserializer::from_slice(&response.body);
            serde_path_to_error::deserialize(de).map_err(|err| Error::Deserialization {
                path: err.path().to_string(),
                source: err.into_inner(),
                request_id,
            })
        } else {
            Err(Error::Api(Box::new(parse_api_error(&response, request_id))))
        }
    }

    /// The retry loop. One idempotency key is fixed *before* the first
    /// attempt and reused on every retry — that is what makes retrying
    /// mutating requests safe.
    async fn execute_raw(&self, mut spec: RequestSpec) -> Result<Response, Error> {
        if spec.method.is_mutating() && spec.idempotency_key.is_none() {
            spec.idempotency_key = Some(uuid::Uuid::new_v4().to_string());
        }

        let request = self.to_transport_request(&spec);
        let policy = &self.inner.retry;
        let mut retries = 0u32;

        loop {
            let outcome = self.inner.transport.execute(request.clone()).await;

            match outcome {
                Ok(response) => {
                    if policy.should_retry_response(&response, retries) {
                        let retry_after = parse_retry_after(response.header("retry-after"));
                        tokio::time::sleep(policy.backoff(retries, retry_after)).await;
                        retries += 1;
                        continue;
                    }
                    return Ok(response);
                }
                Err(err) => {
                    if policy.should_retry_transport(&err, retries) {
                        tokio::time::sleep(policy.backoff(retries, None)).await;
                        retries += 1;
                        continue;
                    }
                    return Err(Error::Network(err));
                }
            }
        }
    }

    fn to_transport_request(&self, spec: &RequestSpec) -> Request {
        let mut url = format!("{}{}", self.inner.api_base, spec.path);
        let mut query = String::new();
        if !spec.query.is_empty() {
            query.push_str(&encode_pairs(
                spec.query.iter().map(|(k, v)| (k.clone(), v.clone())),
            ));
        }
        if let Some(raw) = &spec.raw_query {
            if !raw.is_empty() {
                if !query.is_empty() {
                    query.push('&');
                }
                query.push_str(raw);
            }
        }
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query);
        }

        let mut headers = vec![
            (
                "Authorization".to_owned(),
                format!("Bearer {}", self.inner.secret.expose()),
            ),
            (
                "Stripe-Version".to_owned(),
                spec.stripe_version
                    .clone()
                    .unwrap_or_else(|| ApiVersion::CURRENT.as_str().to_owned()),
            ),
            ("User-Agent".to_owned(), self.inner.user_agent.clone()),
            (
                "X-Stripe-Client-User-Agent".to_owned(),
                self.inner.client_user_agent.clone(),
            ),
        ];

        if let Some(account) = spec
            .stripe_account
            .as_ref()
            .or(self.inner.stripe_account.as_ref())
        {
            headers.push(("Stripe-Account".to_owned(), account.clone()));
        }
        if let Some(context) = &spec.stripe_context {
            headers.push(("Stripe-Context".to_owned(), context.clone()));
        }
        if let Some(key) = &spec.idempotency_key {
            headers.push(("Idempotency-Key".to_owned(), key.clone()));
        }

        let body = spec.body.as_ref().map(|b| {
            headers.push(("Content-Type".to_owned(), b.content_type().to_owned()));
            b.as_bytes().to_vec()
        });

        Request {
            method: spec.method.as_str(),
            url,
            headers,
            body,
        }
    }
}

fn parse_api_error(response: &Response, request_id: Option<RequestId>) -> ApiError {
    let mut api_error = match serde_json::from_slice::<ErrorEnvelope>(&response.body) {
        Ok(envelope) => envelope.error,
        // Non-JSON error bodies (proxies, outages) still become structured
        // errors instead of a deserialization failure.
        Err(_) => ApiError {
            error_type: None,
            code: None,
            decline_code: None,
            message: Some(String::from_utf8_lossy(&response.body).into_owned()),
            param: None,
            doc_url: None,
            status: 0,
            request_id: None,
        },
    };
    api_error.status = response.status;
    api_error.request_id = request_id;
    api_error
}

/// Configures and builds a [`Client`].
#[derive(Debug)]
pub struct ClientBuilder {
    secret: SecretKey,
    api_base: String,
    stripe_account: Option<String>,
    timeout: Duration,
    retry: RetryPolicy,
    app_info: Option<AppInfo>,
    transport: Option<Arc<dyn HttpTransport>>,
}

impl ClientBuilder {
    /// Start from the environment (default variable names), then chain any
    /// programmatic overrides — explicit builder calls always win over env:
    ///
    /// * `STRIPE_SECRET_KEY` — required
    /// * `STRIPE_API_BASE` — optional base-URL override
    /// * `STRIPE_ACCOUNT` — optional Connect account default
    ///
    /// ```no_run
    /// # use payrs_stripe_client::{ClientBuilder, RetryPolicy};
    /// # fn run() -> Result<(), payrs_stripe_client::Error> {
    /// let client = ClientBuilder::from_env()?
    ///     .retry_policy(RetryPolicy::default().max_retries(5))
    ///     .timeout(std::time::Duration::from_secs(10))
    ///     .build()?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// [`Error::Config`] if `STRIPE_SECRET_KEY` is unset or empty.
    pub fn from_env() -> Result<Self, Error> {
        Self::from_env_var("STRIPE_SECRET_KEY")
    }

    /// Like [`ClientBuilder::from_env`], but the secret key is read from a
    /// custom variable name. `STRIPE_API_BASE` / `STRIPE_ACCOUNT` are still
    /// honored as optional defaults.
    ///
    /// # Errors
    /// [`Error::Config`] if `var_name` is unset or empty.
    pub fn from_env_var(var_name: &str) -> Result<Self, Error> {
        let key = std::env::var(var_name)
            .map_err(|_| Error::Config(format!("environment variable `{var_name}` is not set")))?;
        if key.trim().is_empty() {
            return Err(Error::Config(format!(
                "environment variable `{var_name}` is empty"
            )));
        }
        let mut builder = Self::new(key);
        if let Ok(base) = std::env::var("STRIPE_API_BASE") {
            if !base.trim().is_empty() {
                builder = builder.api_base(base);
            }
        }
        if let Ok(account) = std::env::var("STRIPE_ACCOUNT") {
            if !account.trim().is_empty() {
                builder = builder.stripe_account(account);
            }
        }
        Ok(builder)
    }

    fn new(secret_key: impl Into<SecretKey>) -> Self {
        Self {
            secret: secret_key.into(),
            api_base: "https://api.stripe.com".to_owned(),
            stripe_account: None,
            timeout: Duration::from_secs(30),
            retry: RetryPolicy::default(),
            app_info: None,
            transport: None,
        }
    }

    /// Override the API base URL — point at `stripe-mock`
    /// (`http://localhost:12111`) or a sandbox. No trailing slash.
    #[must_use]
    pub fn api_base(mut self, base: impl Into<String>) -> Self {
        let mut base = base.into();
        while base.ends_with('/') {
            base.pop();
        }
        self.api_base = base;
        self
    }

    /// Act on behalf of a connected account for all requests
    /// (`Stripe-Account`). Per-request overrides win.
    #[must_use]
    pub fn stripe_account(mut self, account: impl Into<String>) -> Self {
        self.stripe_account = Some(account.into());
        self
    }

    /// Total per-attempt request timeout (default 30 s).
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Configure the retry policy (default: 3 retries, backoff + jitter).
    #[must_use]
    pub fn retry_policy(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// Identify your application in the `User-Agent`, as official SDKs'
    /// `set_app_info` does.
    #[must_use]
    pub fn app_info(
        mut self,
        name: impl Into<String>,
        version: Option<&str>,
        url: Option<&str>,
    ) -> Self {
        self.app_info = Some(AppInfo {
            name: name.into(),
            version: version.map(ToOwned::to_owned),
            url: url.map(ToOwned::to_owned),
        });
        self
    }

    /// Inject a custom [`HttpTransport`] (proxies, middleware, mocks).
    #[must_use]
    pub fn transport(mut self, transport: Arc<dyn HttpTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Build the client.
    ///
    /// # Errors
    /// [`Error::Config`] if the default transport's TLS backend fails to
    /// initialize.
    pub fn build(self) -> Result<Client, Error> {
        let transport: Arc<dyn HttpTransport> = match self.transport {
            Some(t) => t,
            None => Arc::new(
                ReqwestTransport::new(self.timeout).map_err(|e| Error::Config(e.to_string()))?,
            ),
        };
        Ok(Client {
            inner: Arc::new(Inner {
                user_agent: build_user_agent(self.app_info.as_ref()),
                client_user_agent: build_client_user_agent(self.app_info.as_ref()),
                transport,
                api_base: self.api_base,
                secret: self.secret,
                stripe_account: self.stripe_account,
                retry: self.retry,
            }),
        })
    }

    /// Like [`ClientBuilder::build`], but transport initialization failures
    /// are deferred to the first request instead of surfacing here. Used by
    /// the infallible `Client::new` convenience constructor.
    fn build_infallible(self) -> Client {
        let timeout = self.timeout;
        let transport: Arc<dyn HttpTransport> = match self.transport {
            Some(t) => t,
            None => match ReqwestTransport::new(timeout) {
                Ok(t) => Arc::new(t),
                Err(init_err) => Arc::new(FailedTransport(init_err.to_string())),
            },
        };
        Client {
            inner: Arc::new(Inner {
                user_agent: build_user_agent(self.app_info.as_ref()),
                client_user_agent: build_client_user_agent(self.app_info.as_ref()),
                transport,
                api_base: self.api_base,
                secret: self.secret,
                stripe_account: self.stripe_account,
                retry: self.retry,
            }),
        }
    }
}

/// Transport that failed to initialize; every request reports the original
/// initialization error.
struct FailedTransport(String);

impl HttpTransport for FailedTransport {
    fn execute(&self, _request: Request) -> crate::transport::TransportFuture<'_> {
        let message = self.0.clone();
        Box::pin(async move {
            Err(crate::transport::TransportError::new(
                crate::transport::TransportErrorKind::Other,
                format!("transport failed to initialize: {message}"),
            ))
        })
    }
}

const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

fn build_user_agent(app_info: Option<&AppInfo>) -> String {
    let mut ua = format!("payrs-stripe/{SDK_VERSION} (Rust)");
    if let Some(info) = app_info {
        ua.push(' ');
        ua.push_str(&format_app_info(info));
    }
    ua
}

/// The JSON diagnostics blob official Stripe SDKs send; opt-out and richer
/// fields (rustc version) land with the `tracing` work in Phase 3.
fn build_client_user_agent(app_info: Option<&AppInfo>) -> String {
    let mut value = serde_json::json!({
        "bindings_version": SDK_VERSION,
        "lang": "rust",
        "publisher": "payrs",
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    });
    if let Some(info) = app_info {
        value["application"] = serde_json::json!({
            "name": info.name,
            "version": info.version,
            "url": info.url,
        });
    }
    value.to_string()
}

fn format_app_info(info: &AppInfo) -> String {
    let mut s = info.name.clone();
    if let Some(v) = &info.version {
        s.push('/');
        s.push_str(v);
    }
    if let Some(u) = &info.url {
        s.push_str(" (");
        s.push_str(u);
        s.push(')');
    }
    s
}
