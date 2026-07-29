//! Configuration-surface and pagination behavioral tests.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::similar_names
)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use payrs_stripe::api::v1;
use payrs_stripe::{Client, ClientBuilder, HttpTransport};
use payrs_stripe_client::transport::{Request, Response, TransportError, TransportFuture};

struct MockTransport {
    responses: Mutex<Vec<Response>>,
    seen: Mutex<Vec<Request>>,
}

impl MockTransport {
    fn new(mut responses: Vec<Response>) -> Arc<Self> {
        responses.reverse();
        Arc::new(Self {
            responses: Mutex::new(responses),
            seen: Mutex::new(Vec::new()),
        })
    }
    fn requests(&self) -> Vec<Request> {
        self.seen.lock().unwrap().clone()
    }
}

impl HttpTransport for MockTransport {
    fn execute(&self, request: Request) -> TransportFuture<'_> {
        self.seen.lock().unwrap().push(request);
        let response = self.responses.lock().unwrap().pop();
        Box::pin(async move {
            response.ok_or_else(|| {
                TransportError::new(payrs_stripe::TransportErrorKind::Other, "script exhausted")
            })
        })
    }
}

fn ok(body: &str) -> Response {
    Response::new(200, HashMap::new(), body.as_bytes().to_vec())
}

// ------------------------------------------------------------ configuration

#[test]
fn from_env_var_reads_custom_key_name_and_reports_missing() {
    // Custom variable name (multi-account pattern).
    std::env::set_var("PAYRS_TEST_KEY_ACME", "sk_test_acme");
    assert!(Client::from_env_var("PAYRS_TEST_KEY_ACME").is_ok());
    std::env::remove_var("PAYRS_TEST_KEY_ACME");

    // Missing variable: actionable error naming the variable.
    let err = Client::from_env_var("PAYRS_TEST_KEY_DOES_NOT_EXIST").unwrap_err();
    assert!(
        err.to_string().contains("PAYRS_TEST_KEY_DOES_NOT_EXIST"),
        "{err}"
    );

    // Empty variable is rejected too.
    std::env::set_var("PAYRS_TEST_KEY_EMPTY", "   ");
    assert!(Client::from_env_var("PAYRS_TEST_KEY_EMPTY").is_err());
    std::env::remove_var("PAYRS_TEST_KEY_EMPTY");
}

#[tokio::test]
async fn builder_from_env_honors_api_base_and_explicit_overrides_win() {
    std::env::set_var("STRIPE_SECRET_KEY", "sk_test_env");
    std::env::set_var("STRIPE_API_BASE", "http://localhost:12111");

    // Env-provided base is used…
    let transport = MockTransport::new(vec![ok("{}"), ok("{}")]);
    let client = ClientBuilder::from_env()
        .unwrap()
        .transport(Arc::clone(&transport) as Arc<dyn HttpTransport>)
        .build()
        .unwrap();
    let _: serde_json::Value = client
        .request(payrs_stripe::Method::Get, "/v1/balance")
        .send()
        .await
        .unwrap();
    assert!(transport.requests()[0]
        .url
        .starts_with("http://localhost:12111/"));

    // …but an explicit builder call beats the environment.
    let client = ClientBuilder::from_env()
        .unwrap()
        .api_base("https://api.stripe.com")
        .transport(Arc::clone(&transport) as Arc<dyn HttpTransport>)
        .build()
        .unwrap();
    let _: serde_json::Value = client
        .request(payrs_stripe::Method::Get, "/v1/balance")
        .send()
        .await
        .unwrap();
    assert!(transport.requests()[1]
        .url
        .starts_with("https://api.stripe.com/"));

    std::env::remove_var("STRIPE_SECRET_KEY");
    std::env::remove_var("STRIPE_API_BASE");
}

#[test]
fn webhook_router_from_env_var() {
    std::env::set_var("PAYRS_TEST_WHSEC", "whsec_from_env");
    assert!(payrs_stripe::webhooks::WebhookRouter::from_env_var("PAYRS_TEST_WHSEC").is_ok());
    std::env::remove_var("PAYRS_TEST_WHSEC");

    let err = payrs_stripe::webhooks::WebhookRouter::from_env_var("PAYRS_TEST_WHSEC_MISSING")
        .unwrap_err();
    assert!(err.contains("PAYRS_TEST_WHSEC_MISSING"), "{err}");
}

// -------------------------------------------------------------- pagination

#[tokio::test]
async fn paginator_threads_starting_after_across_pages() {
    let transport = MockTransport::new(vec![
        ok(
            r#"{"object": "list", "url": "/v1/customers", "has_more": true,
              "data": [{"id": "cus_1"}, {"id": "cus_2"}]}"#,
        ),
        ok(
            r#"{"object": "list", "url": "/v1/customers", "has_more": false,
              "data": [{"id": "cus_3"}]}"#,
        ),
    ]);
    let client = Client::builder("sk_test_x")
        .transport(Arc::clone(&transport) as Arc<dyn HttpTransport>)
        .build()
        .unwrap();

    let mut pager = v1::customers::GetCustomers::new().limit(2).paginate();

    let page1 = pager.next_page(&client).await.unwrap().unwrap();
    assert_eq!(page1.len(), 2);
    let page2 = pager.next_page(&client).await.unwrap().unwrap();
    assert_eq!(page2.data[0].id.as_deref(), Some("cus_3"));
    assert!(
        pager.next_page(&client).await.unwrap().is_none(),
        "exhausted"
    );

    let urls: Vec<String> = transport.requests().iter().map(|r| r.url.clone()).collect();
    assert_eq!(urls[0], "https://api.stripe.com/v1/customers?limit=2");
    assert_eq!(
        urls[1], "https://api.stripe.com/v1/customers?limit=2&starting_after=cus_2",
        "cursor must come from the last item of the previous page"
    );
    assert_eq!(urls.len(), 2, "no request after exhaustion");
}

#[tokio::test]
async fn collect_all_respects_max_items_bound() {
    let transport = MockTransport::new(vec![
        ok(r#"{"object": "list", "has_more": true,
              "data": [{"id": "cus_1"}, {"id": "cus_2"}]}"#),
        ok(r#"{"object": "list", "has_more": true,
              "data": [{"id": "cus_3"}, {"id": "cus_4"}]}"#),
    ]);
    let client = Client::builder("sk_test_x")
        .transport(Arc::clone(&transport) as Arc<dyn HttpTransport>)
        .build()
        .unwrap();

    let items: Vec<payrs_stripe::models::Customer> = v1::customers::GetCustomers::new()
        .paginate()
        .collect_all(&client, 3)
        .await
        .unwrap();

    assert_eq!(items.len(), 3, "bounded at max_items");
    assert_eq!(
        transport.requests().len(),
        2,
        "stops fetching once bound is hit"
    );
}
