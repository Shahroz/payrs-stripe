//! Domain lifecycle tests: full checkout flow, the invoice lifecycle, and
//! transaction/charge/refund operations — exercised through the generated
//! builders against a scripted mock transport (no network).
//!
//! These pin the wire contract per domain: paths, methods, form encoding,
//! and typed model/list responses.

#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use payrs_stripe::api::v1;
use payrs_stripe::{Client, HttpTransport};
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

fn ok(body: &str) -> Response {
    Response::new(200, HashMap::new(), body.as_bytes().to_vec())
}

fn client(t: Arc<MockTransport>) -> Client {
    Client::builder("sk_test_x").transport(t).build().unwrap()
}

fn body_of(req: &Request) -> String {
    String::from_utf8(req.body.clone().unwrap_or_default()).unwrap()
}

// ---------------------------------------------------------------- checkout

#[tokio::test]
async fn checkout_full_flow_create_retrieve_line_items_expire() {
    let transport = MockTransport::new(vec![
        ok(
            r#"{"id": "cs_1", "object": "checkout.session", "mode": "payment",
              "url": "https://checkout.stripe.com/c/pay/cs_1", "status": "open",
              "amount_total": 3998, "currency": "usd"}"#,
        ),
        ok(r#"{"id": "cs_1", "object": "checkout.session", "status": "open"}"#),
        ok(
            r#"{"object": "list", "has_more": false, "url": "/v1/checkout/sessions/cs_1/line_items",
              "data": [{"id": "li_1", "object": "item", "quantity": 2, "amount_total": 3998}]}"#,
        ),
        ok(r#"{"id": "cs_1", "object": "checkout.session", "status": "expired"}"#),
    ]);
    let c = client(Arc::clone(&transport));

    // 1. Create a session: nested line_items, success/cancel URLs, metadata.
    let session: payrs_stripe::models::CheckoutSession = v1::checkout::PostCheckoutSessions::new()
        .mode("payment")
        .success_url("https://shop.example.com/success?sid={CHECKOUT_SESSION_ID}")
        .cancel_url("https://shop.example.com/cancel")
        .customer_email("ada@example.com")
        .client_reference_id("order_42")
        .param(
            "line_items",
            serde_json::json!([{
                "price_data": {
                    "currency": "usd",
                    "unit_amount": 1999,
                    "product_data": {"name": "Widget"}
                },
                "quantity": 2
            }]),
        )
        .param("metadata", serde_json::json!({"order_id": "ord_42"}))
        .send(&c)
        .await
        .unwrap();
    assert_eq!(session.id.as_deref(), Some("cs_1"));
    assert!(session
        .url
        .as_deref()
        .unwrap()
        .starts_with("https://checkout.stripe.com/"));

    // 2. Retrieve it.
    let _fetched: payrs_stripe::models::CheckoutSession =
        v1::checkout::GetCheckoutSessionsSession::new("cs_1")
            .send(&c)
            .await
            .unwrap();

    // 3. List its line items (typed List<Item>).
    let items: payrs_stripe::List<payrs_stripe::models::Item> =
        v1::checkout::GetCheckoutSessionsSessionLineItems::new("cs_1")
            .send(&c)
            .await
            .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items.data[0].quantity, Some(2));

    // 4. Expire it.
    let expired: payrs_stripe::models::CheckoutSession =
        v1::checkout::PostCheckoutSessionsSessionExpire::new("cs_1")
            .send(&c)
            .await
            .unwrap();
    assert_eq!(expired.status.as_deref(), Some("expired"));

    // Wire assertions across the whole flow.
    let reqs = transport.requests();
    assert_eq!(reqs[0].method, "POST");
    assert_eq!(reqs[0].url, "https://api.stripe.com/v1/checkout/sessions");
    let create_body = body_of(&reqs[0]);
    assert!(create_body.contains("mode=payment"), "{create_body}");
    assert!(
        create_body.contains("line_items[0][price_data][unit_amount]=1999"),
        "{create_body}"
    );
    assert!(
        create_body.contains("line_items[0][price_data][product_data][name]=Widget"),
        "{create_body}"
    );
    assert!(
        create_body.contains("metadata[order_id]=ord_42"),
        "{create_body}"
    );
    // {CHECKOUT_SESSION_ID} placeholder must survive percent-encoding round-trip
    assert!(
        create_body.contains("%7BCHECKOUT_SESSION_ID%7D"),
        "{create_body}"
    );

    assert_eq!(
        reqs[1].url,
        "https://api.stripe.com/v1/checkout/sessions/cs_1"
    );
    assert_eq!(
        reqs[2].url,
        "https://api.stripe.com/v1/checkout/sessions/cs_1/line_items"
    );
    assert_eq!(
        reqs[3].url,
        "https://api.stripe.com/v1/checkout/sessions/cs_1/expire"
    );
    // Every mutating call in the flow carried an idempotency key.
    for req in [&reqs[0], &reqs[3]] {
        assert!(
            req.headers
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("idempotency-key")),
            "missing idempotency key on {}",
            req.url
        );
    }
}

// ---------------------------------------------------------------- invoices

#[tokio::test]
async fn invoice_lifecycle_item_create_finalize_pay_send_void() {
    let transport = MockTransport::new(vec![
        ok(r#"{"id": "ii_1", "object": "invoiceitem", "amount": 5000}"#),
        ok(r#"{"id": "in_1", "object": "invoice", "status": "draft", "customer": "cus_9"}"#),
        ok(r#"{"id": "in_1", "object": "invoice", "status": "open", "number": "INV-0001"}"#),
        ok(r#"{"id": "in_1", "object": "invoice", "status": "paid", "amount_paid": 5000}"#),
        ok(r#"{"id": "in_1", "object": "invoice", "status": "paid"}"#),
        ok(r#"{"id": "in_2", "object": "invoice", "status": "void"}"#),
        ok(
            r#"{"object": "list", "has_more": false, "url": "/v1/invoices",
              "data": [{"id": "in_1", "status": "paid"}]}"#,
        ),
    ]);
    let c = client(Arc::clone(&transport));

    // 1. Add a pending invoice item to the customer.
    let item: payrs_stripe::models::Invoiceitem = v1::invoiceitems::PostInvoiceitems::new()
        .customer("cus_9")
        .amount(5000)
        .currency("usd")
        .description("Consulting — July")
        .send(&c)
        .await
        .unwrap();
    assert_eq!(item.amount, Some(5000));

    // 2. Create a draft invoice, auto-advance off (we drive it manually).
    let invoice: payrs_stripe::models::Invoice = v1::invoices::PostInvoices::new()
        .customer("cus_9")
        .auto_advance(false)
        .collection_method("charge_automatically")
        .send(&c)
        .await
        .unwrap();
    assert_eq!(invoice.status.as_deref(), Some("draft"));

    // 3. Finalize → open.
    let finalized: payrs_stripe::models::Invoice =
        v1::invoices::PostInvoicesInvoiceFinalize::new("in_1")
            .send(&c)
            .await
            .unwrap();
    assert_eq!(finalized.status.as_deref(), Some("open"));
    assert_eq!(finalized.number.as_deref(), Some("INV-0001"));

    // 4. Pay → paid.
    let paid: payrs_stripe::models::Invoice = v1::invoices::PostInvoicesInvoicePay::new("in_1")
        .send(&c)
        .await
        .unwrap();
    assert_eq!(paid.amount_paid, Some(5000));

    // 5. Send the invoice email; 6. void another; 7. list.
    let _sent: payrs_stripe::models::Invoice = v1::invoices::PostInvoicesInvoiceSend::new("in_1")
        .send(&c)
        .await
        .unwrap();
    let voided: payrs_stripe::models::Invoice = v1::invoices::PostInvoicesInvoiceVoid::new("in_2")
        .send(&c)
        .await
        .unwrap();
    assert_eq!(voided.status.as_deref(), Some("void"));
    let list: payrs_stripe::List<payrs_stripe::models::Invoice> = v1::invoices::GetInvoices::new()
        .customer("cus_9")
        .status("paid")
        .limit(10)
        .send(&c)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);

    let reqs = transport.requests();
    let urls: Vec<&str> = reqs.iter().map(|r| r.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://api.stripe.com/v1/invoiceitems",
            "https://api.stripe.com/v1/invoices",
            "https://api.stripe.com/v1/invoices/in_1/finalize",
            "https://api.stripe.com/v1/invoices/in_1/pay",
            "https://api.stripe.com/v1/invoices/in_1/send",
            "https://api.stripe.com/v1/invoices/in_2/void",
            "https://api.stripe.com/v1/invoices?customer=cus_9&limit=10&status=paid",
        ]
    );
    let item_body = body_of(&reqs[0]);
    assert!(item_body.contains("customer=cus_9"), "{item_body}");
    assert!(item_body.contains("amount=5000"), "{item_body}");
    let invoice_body = body_of(&reqs[1]);
    assert!(
        invoice_body.contains("auto_advance=false"),
        "{invoice_body}"
    );
}

// ------------------------------------------------- transactions & payments

#[tokio::test]
async fn transactions_balance_customer_charge_refund_capture() {
    let transport = MockTransport::new(vec![
        ok(
            r#"{"object": "list", "has_more": true, "url": "/v1/balance_transactions",
              "data": [{"id": "txn_1", "object": "balance_transaction", "amount": 1999,
                        "net": 1911, "fee": 88, "type": "charge", "currency": "usd"}]}"#,
        ),
        ok(r#"{"id": "txn_1", "object": "balance_transaction", "amount": 1999, "fee": 88}"#),
        ok(r#"{"id": "cbtxn_1", "object": "customer_balance_transaction", "amount": -500}"#),
        ok(
            r#"{"id": "pi_1", "object": "payment_intent", "status": "requires_capture",
              "amount": 1999, "amount_capturable": 1999}"#,
        ),
        ok(
            r#"{"id": "pi_1", "object": "payment_intent", "status": "succeeded",
              "amount_received": 1500}"#,
        ),
        ok(
            r#"{"id": "re_1", "object": "refund", "amount": 500, "status": "succeeded",
              "charge": "ch_1"}"#,
        ),
    ]);
    let c = client(Arc::clone(&transport));

    // Balance transactions: typed list + retrieve (your ledger).
    let txns: payrs_stripe::List<payrs_stripe::models::BalanceTransaction> =
        v1::balance_transactions::GetBalanceTransactions::new()
            .type_("charge")
            .limit(1)
            .send(&c)
            .await
            .unwrap();
    assert_eq!(txns.data[0].net, Some(1911));
    assert!(txns.has_more);

    let txn: payrs_stripe::models::BalanceTransaction =
        v1::balance_transactions::GetBalanceTransactionsId::new("txn_1")
            .send(&c)
            .await
            .unwrap();
    assert_eq!(txn.fee, Some(88));

    // Customer balance transaction (credit the customer 5.00).
    let credit: payrs_stripe::models::CustomerBalanceTransaction =
        v1::customers::PostCustomersCustomerBalanceTransactions::new("cus_9", -500, "usd")
            .description("goodwill credit")
            .send(&c)
            .await
            .unwrap();
    assert_eq!(credit.amount, Some(-500));

    // Auth-then-capture: manual-capture intent, partial capture.
    let intent: payrs_stripe::models::PaymentIntent =
        v1::payment_intents::PostPaymentIntents::new(1999, "usd")
            .capture_method("manual")
            .customer("cus_9")
            .send(&c)
            .await
            .unwrap();
    assert_eq!(intent.status.as_deref(), Some("requires_capture"));

    let captured: payrs_stripe::models::PaymentIntent =
        v1::payment_intents::PostPaymentIntentsIntentCapture::new("pi_1")
            .amount_to_capture(1500)
            .send(&c)
            .await
            .unwrap();
    assert_eq!(captured.amount_received, Some(1500));

    // Partial refund by charge.
    let refund: payrs_stripe::models::Refund = v1::refunds::PostRefunds::new()
        .charge("ch_1")
        .amount(500)
        .reason("requested_by_customer")
        .send(&c)
        .await
        .unwrap();
    assert_eq!(refund.status.as_deref(), Some("succeeded"));

    let reqs = transport.requests();
    assert_eq!(
        reqs[0].url, "https://api.stripe.com/v1/balance_transactions?limit=1&type=charge",
        "reserved-word param `type` must hit the wire unrenamed"
    );
    assert_eq!(
        reqs[2].url,
        "https://api.stripe.com/v1/customers/cus_9/balance_transactions"
    );
    let credit_body = body_of(&reqs[2]);
    assert!(credit_body.contains("amount=-500"), "{credit_body}");
    let capture_body = body_of(&reqs[4]);
    assert!(
        capture_body.contains("amount_to_capture=1500"),
        "{capture_body}"
    );
    let refund_body = body_of(&reqs[5]);
    assert!(refund_body.contains("charge=ch_1"), "{refund_body}");
    assert!(
        refund_body.contains("reason=requested_by_customer"),
        "{refund_body}"
    );
}
