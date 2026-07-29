//! Typed bindings for the Stripe **v2** core namespace.
//!
//! v2 endpoints take JSON bodies and RFC 3339 timestamps, paginate with
//! `page` tokens, and emit **thin events**. The public `OpenAPI` spec doesn't
//! yet include `/v2`, so this module is hand-written against the documented
//! GA surface; any other v2 endpoint is reachable via the raw client:
//! `client.request(Method::Post, "/v2/…").json(&body)?.send_json()`.

use payrs_stripe_client::{Body, Client, Error, Method, RequestSpec};

/// A page of v2 list results (`data` + `next_page_url`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct V2Page<T> {
    /// Items on this page.
    pub data: Vec<T>,
    /// URL of the next page, if any.
    #[serde(default)]
    pub next_page_url: Option<String>,
    /// URL of the previous page, if any.
    #[serde(default)]
    pub previous_page_url: Option<String>,
}

/// `GET /v2/core/events` — list events for a v2 event destination.
#[derive(Debug, Clone)]
pub struct ListEvents {
    object_id: String,
    limit: Option<i64>,
    page: Option<String>,
}

impl ListEvents {
    /// Events are listed for one related `object_id` (required by the API).
    #[must_use]
    pub fn new(object_id: impl Into<String>) -> Self {
        Self {
            object_id: object_id.into(),
            limit: None,
            page: None,
        }
    }

    /// Maximum number of events per page.
    #[must_use]
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Opaque page token from a previous response.
    #[must_use]
    pub fn page(mut self, page: impl Into<String>) -> Self {
        self.page = Some(page.into());
        self
    }

    /// Send the request.
    ///
    /// # Errors
    /// See [`payrs_stripe_client::Error`].
    pub async fn send(self, client: &Client) -> Result<V2Page<serde_json::Value>, Error> {
        let mut spec = RequestSpec::new(Method::Get, "/v2/core/events");
        spec.query.push(("object_id".to_owned(), self.object_id));
        if let Some(limit) = self.limit {
            spec.query.push(("limit".to_owned(), limit.to_string()));
        }
        if let Some(page) = self.page {
            spec.query.push(("page".to_owned(), page));
        }
        client.execute(spec).await
    }
}

/// `GET /v2/core/events/{id}` — retrieve one event (use after receiving a
/// thin webhook payload to pull full details).
#[derive(Debug, Clone)]
pub struct RetrieveEvent {
    id: String,
}

impl RetrieveEvent {
    /// Retrieve the event with this ID (`evt_…`).
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// Send the request.
    ///
    /// # Errors
    /// See [`payrs_stripe_client::Error`].
    pub async fn send(self, client: &Client) -> Result<serde_json::Value, Error> {
        let spec = RequestSpec::new(Method::Get, format!("/v2/core/events/{}", self.id));
        client.execute(spec).await
    }
}

/// `POST /v2/core/event_destinations` — create an event destination
/// (webhook endpoint or Amazon `EventBridge`) with snapshot or thin payloads.
#[derive(Debug, Clone)]
pub struct CreateEventDestination {
    body: serde_json::Map<String, serde_json::Value>,
}

impl CreateEventDestination {
    /// Required: a display `name`, destination `type`
    /// (`webhook_endpoint` / `amazon_eventbridge`), `event_payload`
    /// (`thin`/`snapshot`), and the events to enable.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        destination_type: impl Into<String>,
        event_payload: impl Into<String>,
        enabled_events: Vec<String>,
    ) -> Self {
        let mut body = serde_json::Map::new();
        body.insert("name".to_owned(), serde_json::Value::String(name.into()));
        body.insert(
            "type".to_owned(),
            serde_json::Value::String(destination_type.into()),
        );
        body.insert(
            "event_payload".to_owned(),
            serde_json::Value::String(event_payload.into()),
        );
        body.insert(
            "enabled_events".to_owned(),
            serde_json::Value::Array(
                enabled_events
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
        Self { body }
    }

    /// For `webhook_endpoint` destinations: the delivery URL.
    #[must_use]
    pub fn webhook_endpoint_url(mut self, url: impl Into<String>) -> Self {
        self.body.insert(
            "webhook_endpoint".to_owned(),
            serde_json::json!({ "url": url.into() }),
        );
        self
    }

    /// Set any field by name (JSON body escape hatch).
    #[must_use]
    pub fn field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.body.insert(key.into(), value);
        self
    }

    /// Send the request.
    ///
    /// # Errors
    /// [`Error::Config`] if the body fails to serialize; otherwise see
    /// [`payrs_stripe_client::Error`].
    pub async fn send(self, client: &Client) -> Result<serde_json::Value, Error> {
        let mut spec = RequestSpec::new(Method::Post, "/v2/core/event_destinations");
        let json = serde_json::to_string(&self.body)
            .map_err(|e| Error::Config(format!("failed to encode v2 body: {e}")))?;
        spec.body = Some(Body::Json(json));
        client.execute(spec).await
    }
}
