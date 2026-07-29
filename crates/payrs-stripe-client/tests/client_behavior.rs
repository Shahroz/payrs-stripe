//! Behavioral contract tests using an injected mock transport.
//! These pin down the guarantees the SDK advertises: idempotency-key reuse
//! across retries, retry classification, error-envelope parsing, and header
//! construction.

#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines
)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use payrs_stripe_client::{
    Client, Error, HttpTransport, Method, Request, Response, RetryPolicy, TransportError,
    TransportErrorKind,
};

/// Scripted transport: pops the next outcome per call, records every request.
struct MockTransport {
    outcomes: Mutex<Vec<Result<Response, TransportError>>>,
    seen: Mutex<Vec<Request>>,
}

impl MockTransport {
    fn new(mut outcomes: Vec<Result<Response, TransportError>>) -> Arc<Self> {
        outcomes.reverse(); // pop() returns them in submission order
        Arc::new(Self {
            outcomes: Mutex::new(outcomes),
            seen: Mutex::new(Vec::new()),
        })
    }

    fn requests(&self) -> Vec<Request> {
        self.seen.lock().unwrap().clone()
    }
}

impl HttpTransport for MockTransport {
    fn execute(&self, request: Request) -> payrs_stripe_client::transport::TransportFuture<'_> {
        self.seen.lock().unwrap().push(request);
        let outcome = self.outcomes.lock().unwrap().pop().unwrap_or_else(|| {
            Err(TransportError::new(
                TransportErrorKind::Other,
                "script empty",
            ))
        });
        Box::pin(async move { outcome })
    }
}

fn ok_json(body: &str) -> Response {
    Response::new(
        200,
        HashMap::from([("request-id".to_owned(), "req_ok".to_owned())]),
        body.as_bytes().to_vec(),
    )
}

fn status(code: u16, headers: &[(&str, &str)], body: &str) -> Response {
    Response::new(
        code,
        headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        body.as_bytes().to_vec(),
    )
}

fn client_with(transport: Arc<MockTransport>) -> Client {
    Client::builder("sk_test_secret123")
        .transport(transport)
        .retry_policy(
            RetryPolicy::default()
                .base_delay(Duration::from_millis(1))
                .max_delay(Duration::from_millis(2)),
        )
        .build()
        .unwrap()
}

fn header<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[tokio::test]
async fn post_gets_auto_idempotency_key_reused_across_retries() {
    let transport = MockTransport::new(vec![
        Ok(status(500, &[], "{}")),
        Ok(status(429, &[("retry-after", "0")], "{}")),
        Ok(ok_json(r#"{"id": "cus_1"}"#)),
    ]);
    let client = client_with(Arc::clone(&transport));

    let value: serde_json::Value = client
        .request(Method::Post, "/v1/customers")
        .form_pairs([("name", "Ada")])
        .send()
        .await
        .unwrap();
    assert_eq!(value["id"], "cus_1");

    let seen = transport.requests();
    assert_eq!(seen.len(), 3, "500 and 429 must be retried");

    let keys: Vec<_> = seen
        .iter()
        .map(|r| header(r, "Idempotency-Key").unwrap().to_owned())
        .collect();
    assert!(!keys[0].is_empty());
    assert_eq!(keys[0], keys[1], "key must be identical across retries");
    assert_eq!(keys[1], keys[2]);
}

#[tokio::test]
async fn get_requests_carry_no_idempotency_key() {
    let transport = MockTransport::new(vec![Ok(ok_json(r#"{"object": "balance"}"#))]);
    let client = client_with(Arc::clone(&transport));

    let _: serde_json::Value = client
        .request(Method::Get, "/v1/balance")
        .send()
        .await
        .unwrap();

    let seen = transport.requests();
    assert!(header(&seen[0], "Idempotency-Key").is_none());
}

#[tokio::test]
async fn stripe_should_retry_false_stops_retrying_a_500() {
    let transport = MockTransport::new(vec![Ok(status(
        500,
        &[("stripe-should-retry", "false")],
        r#"{"error": {"type": "api_error", "message": "boom"}}"#,
    ))]);
    let client = client_with(Arc::clone(&transport));

    let err = client
        .request(Method::Post, "/v1/customers")
        .send_json()
        .await
        .unwrap_err();

    assert_eq!(transport.requests().len(), 1, "must not retry");
    let api = err.as_api_error().unwrap();
    assert_eq!(api.status, 500);
}

#[tokio::test]
async fn transport_errors_are_retried_then_surface() {
    let transport = MockTransport::new(vec![
        Err(TransportError::new(TransportErrorKind::Timeout, "t/o")),
        Err(TransportError::new(TransportErrorKind::Connect, "refused")),
        Err(TransportError::new(TransportErrorKind::Timeout, "t/o")),
        Err(TransportError::new(TransportErrorKind::Timeout, "t/o")),
    ]);
    let client = client_with(Arc::clone(&transport));

    let err = client
        .request(Method::Post, "/v1/customers")
        .send_json()
        .await
        .unwrap_err();

    // default budget: 3 retries -> 4 attempts
    assert_eq!(transport.requests().len(), 4);
    assert!(matches!(err, Error::Network(_)));
}

#[tokio::test]
async fn error_envelope_is_parsed_with_request_id() {
    let transport = MockTransport::new(vec![Ok(status(
        402,
        &[("request-id", "req_test_123")],
        r#"{"error": {"type": "card_error", "code": "card_declined",
             "decline_code": "insufficient_funds",
             "message": "Your card has insufficient funds."}}"#,
    ))]);
    let client = client_with(Arc::clone(&transport));

    let err = client
        .request(Method::Post, "/v1/payment_intents")
        .send_json()
        .await
        .unwrap_err();

    let api = err.as_api_error().unwrap();
    assert!(api.is_card_declined());
    assert_eq!(api.status, 402);
    assert_eq!(api.request_id.as_ref().unwrap().0, "req_test_123");
    assert_eq!(api.decline_code.as_deref(), Some("insufficient_funds"));
}

#[tokio::test]
async fn headers_auth_version_account_and_form_body_are_set() {
    let transport = MockTransport::new(vec![Ok(ok_json("{}"))]);
    let client = Client::builder("sk_test_abc")
        .transport(Arc::clone(&transport) as Arc<dyn HttpTransport>)
        .stripe_account("acct_platform")
        .build()
        .unwrap();

    let _: serde_json::Value = client
        .request(Method::Post, "/v1/customers")
        .form_pairs([("metadata[order_id]", "42"), ("name", "Ada Lovelace")])
        .stripe_account("acct_override")
        .send()
        .await
        .unwrap();

    let seen = transport.requests();
    let req = &seen[0];
    assert_eq!(req.url, "https://api.stripe.com/v1/customers");
    assert_eq!(header(req, "Authorization").unwrap(), "Bearer sk_test_abc");
    assert_eq!(header(req, "Stripe-Version").unwrap(), "2026-06-24.dahlia");
    assert_eq!(
        header(req, "Stripe-Account").unwrap(),
        "acct_override",
        "per-request Connect header must win over client default"
    );
    assert_eq!(
        header(req, "Content-Type").unwrap(),
        "application/x-www-form-urlencoded"
    );
    assert_eq!(
        String::from_utf8(req.body.clone().unwrap()).unwrap(),
        "metadata[order_id]=42&name=Ada+Lovelace"
    );
    assert!(header(req, "User-Agent")
        .unwrap()
        .starts_with("payrs-stripe/"));
}

#[tokio::test]
async fn deserialization_errors_report_json_path() {
    #[derive(serde::Deserialize, Debug)]
    struct Expects {
        #[allow(dead_code)]
        amount: i64,
    }
    let transport = MockTransport::new(vec![Ok(ok_json(r#"{"amount": "not a number"}"#))]);
    let client = client_with(transport);

    let err = client
        .request(Method::Get, "/v1/whatever")
        .send::<Expects>()
        .await
        .unwrap_err();

    match err {
        Error::Deserialization {
            path, request_id, ..
        } => {
            assert_eq!(path, "amount");
            assert_eq!(request_id.unwrap().0, "req_ok");
        }
        other => panic!("expected Deserialization error, got {other:?}"),
    }
}
