//! End-to-end behavioral tests for the generated API surface and v2 support,
//! using an injected mock transport (no network).

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines
)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use payrs_stripe::api::v1;
use payrs_stripe::api::v2;
use payrs_stripe::{Client, HttpTransport, Method};
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
                TransportError::new(
                    payrs_stripe::TransportErrorKind::Other,
                    "mock script exhausted",
                )
            })
        })
    }
}

fn json_response(body: &str) -> Response {
    Response::new(200, HashMap::new(), body.as_bytes().to_vec())
}

fn client(transport: Arc<MockTransport>) -> Client {
    Client::builder("sk_test_x")
        .transport(transport)
        .build()
        .unwrap()
}

#[tokio::test]
async fn generated_post_builds_form_body_and_typed_model() {
    let transport = MockTransport::new(vec![json_response(
        r#"{"id": "cus_9", "object": "customer", "name": "Ada Lovelace",
            "email": "ada@example.com", "created": 1700000000,
            "brand_new_field_from_2027": {"nested": true}}"#,
    )]);
    let c = client(Arc::clone(&transport));

    let customer: payrs_stripe::models::Customer = v1::customers::PostCustomers::new()
        .name("Ada Lovelace")
        .email("ada@example.com")
        .send(&c)
        .await
        .unwrap();

    // Typed model round-trip, tolerant of unknown future fields.
    assert_eq!(customer.id.as_deref(), Some("cus_9"));
    assert_eq!(customer.name.as_deref(), Some("Ada Lovelace"));

    let req = &transport.requests()[0];
    assert_eq!(req.method, "POST");
    assert_eq!(req.url, "https://api.stripe.com/v1/customers");
    let body = String::from_utf8(req.body.clone().unwrap()).unwrap();
    assert!(body.contains("name=Ada+Lovelace"), "{body}");
    assert!(body.contains("email=ada%40example.com"), "{body}");
}

#[tokio::test]
async fn generated_required_args_and_path_params_work() {
    let transport = MockTransport::new(vec![json_response(
        r#"{"id": "pi_1", "object": "payment_intent", "amount": 1999, "currency": "usd"}"#,
    )]);
    let c = client(Arc::clone(&transport));

    // amount + currency are required -> positional in new().
    let intent = v1::payment_intents::PostPaymentIntents::new(1999, "usd")
        .customer("cus_9")
        .send(&c)
        .await
        .unwrap();
    assert_eq!(intent.amount, Some(1999));

    let body = String::from_utf8(transport.requests()[0].body.clone().unwrap()).unwrap();
    assert!(body.contains("amount=1999"), "{body}");
    assert!(body.contains("currency=usd"), "{body}");
    assert!(body.contains("customer=cus_9"), "{body}");

    // Path-param substitution.
    let transport = MockTransport::new(vec![json_response(r#"{"id": "pi_1"}"#)]);
    let c = client(Arc::clone(&transport));
    let _ = v1::payment_intents::GetPaymentIntentsIntent::new("pi_1")
        .send(&c)
        .await
        .unwrap();
    assert_eq!(
        transport.requests()[0].url,
        "https://api.stripe.com/v1/payment_intents/pi_1"
    );
}

#[tokio::test]
async fn generated_list_returns_typed_list_and_encodes_query() {
    let transport = MockTransport::new(vec![json_response(
        r#"{"object": "list", "url": "/v1/customers", "has_more": true,
            "data": [{"id": "cus_1"}, {"id": "cus_2"}]}"#,
    )]);
    let c = client(Arc::clone(&transport));

    let page: payrs_stripe::List<payrs_stripe::models::Customer> =
        v1::customers::GetCustomers::new()
            .limit(2)
            .email("ada@example.com")
            .send(&c)
            .await
            .unwrap();

    assert_eq!(page.len(), 2);
    assert!(page.has_more);
    assert_eq!(page.data[1].id.as_deref(), Some("cus_2"));

    let url = &transport.requests()[0].url;
    assert!(url.contains("limit=2"), "{url}");
    assert!(url.contains("email=ada%40example.com"), "{url}");
}

#[tokio::test]
async fn nested_params_use_stripe_bracket_encoding() {
    let transport = MockTransport::new(vec![json_response(r#"{"id": "cs_1"}"#)]);
    let c = client(Arc::clone(&transport));

    let _ = v1::checkout::PostCheckoutSessions::new()
        .mode("payment")
        .success_url("https://example.com/ok")
        .param(
            "line_items",
            serde_json::json!([{"price": "price_1", "quantity": 2}]),
        )
        .param("metadata", serde_json::json!({"order_id": "ord_42"}))
        .send(&c)
        .await
        .unwrap();

    let body = String::from_utf8(transport.requests()[0].body.clone().unwrap()).unwrap();
    assert!(body.contains("line_items[0][price]=price_1"), "{body}");
    assert!(body.contains("line_items[0][quantity]=2"), "{body}");
    assert!(body.contains("metadata[order_id]=ord_42"), "{body}");
}

#[tokio::test]
async fn v2_requests_send_json_bodies() {
    let transport = MockTransport::new(vec![json_response(
        r#"{"id": "ed_1", "object": "v2.core.event_destination"}"#,
    )]);
    let c = client(Arc::clone(&transport));

    let _ = v2::CreateEventDestination::new(
        "prod hooks",
        "webhook_endpoint",
        "thin",
        vec!["ping".to_owned()],
    )
    .webhook_endpoint_url("https://example.com/hooks")
    .send(&c)
    .await
    .unwrap();

    let req = &transport.requests()[0];
    assert_eq!(req.url, "https://api.stripe.com/v2/core/event_destinations");
    let content_type = req
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str());
    assert_eq!(content_type, Some("application/json"));

    let body: serde_json::Value = serde_json::from_slice(req.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["event_payload"], "thin");
    assert_eq!(body["webhook_endpoint"]["url"], "https://example.com/hooks");
}

#[tokio::test]
async fn raw_escape_hatch_overrides_version_and_context() {
    let transport = MockTransport::new(vec![json_response("{}")]);
    let c = client(Arc::clone(&transport));

    let _: serde_json::Value = c
        .request(Method::Post, "/v2/money_management/payout_methods")
        .json(&serde_json::json!({"x": 1}))
        .unwrap()
        .stripe_version("2025-12-15.preview")
        .stripe_context("acct_child")
        .send_json()
        .await
        .unwrap();

    let req = &transport.requests()[0];
    let get = |name: &str| {
        req.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    };
    assert_eq!(get("Stripe-Version").as_deref(), Some("2025-12-15.preview"));
    assert_eq!(get("Stripe-Context").as_deref(), Some("acct_child"));
    assert!(
        get("Idempotency-Key").is_some(),
        "v2 POSTs get idempotency keys too"
    );
}

#[tokio::test]
async fn preview_feature_version_tags_pass_through_verbatim() {
    const PREVIEW_VERSION: &str = "2026-06-24.dahlia; feature_beta=v3";

    // Stripe has no /v3 REST namespace — the API is v1 + v2. The only place a
    // "v3" legitimately appears is a *preview feature tag* inside the
    // Stripe-Version header, e.g. "2026-06-24.dahlia; feature_beta=v3".
    // Such strings must reach the wire byte-for-byte: the semicolon, the
    // space, and the tag are all significant to Stripe's version parser.
    let transport = MockTransport::new(vec![json_response("{}")]);
    let c = client(Arc::clone(&transport));

    let _: serde_json::Value = c
        .request(Method::Get, "/v1/customers")
        .stripe_version(PREVIEW_VERSION)
        .send_json()
        .await
        .unwrap();

    let sent = transport.requests()[0]
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("stripe-version"))
        .map(|(_, v)| v.clone())
        .expect("Stripe-Version header must be present");
    assert_eq!(sent, PREVIEW_VERSION, "preview tag must not be mangled");
}

/// Model grouping must not break existing imports: every type stays reachable
/// at the flat `models::X` path it had before the split into domain modules,
/// and is additionally reachable at its grouped path.
#[test]
fn model_paths_are_available_flat_and_grouped() {
    // Flat paths (the pre-grouping, published API surface).
    let _: Option<payrs_stripe::models::Customer> = None;
    let _: Option<payrs_stripe::models::PaymentIntent> = None;
    let _: Option<payrs_stripe::models::CheckoutSession> = None;
    let _: Option<payrs_stripe::models::Invoice> = None;
    let _: Option<payrs_stripe::models::BalanceTransaction> = None;

    // Grouped paths (new, additive).
    let _: Option<payrs_stripe::models::customers::Customer> = None;
    let _: Option<payrs_stripe::models::payment_intents::PaymentIntent> = None;
    let _: Option<payrs_stripe::models::checkout::CheckoutSession> = None;
    let _: Option<payrs_stripe::models::invoices::Invoice> = None;
    let _: Option<payrs_stripe::models::treasury::TreasuryFinancialAccount> = None;
    let _: Option<payrs_stripe::models::issuing::IssuingCard> = None;

    // Shared/nested types land in `common`.
    let _: Option<payrs_stripe::models::common::Address> = None;
    let _: Option<payrs_stripe::models::Address> = None;
}
