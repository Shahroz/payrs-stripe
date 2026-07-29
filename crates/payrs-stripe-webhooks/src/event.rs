//! Typed webhook events: snapshot events (v1 payloads) and thin events (v2).

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use payrs_stripe_types::Timestamp;

macro_rules! event_types {
    ($($(#[$doc:meta])* $variant:ident => $name:literal),+ $(,)?) => {
        /// A webhook event type (`payment_intent.succeeded`, …).
        ///
        /// Well-known types are first-class variants; anything newer than
        /// this SDK release round-trips via [`EventType::Other`], so parsing
        /// never fails on new Stripe event types.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum EventType {
            $($(#[$doc])* $variant,)+
            /// Any event type not (yet) a first-class variant.
            Other(SmolStr),
        }

        impl EventType {
            /// The dotted event name as sent by Stripe.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $name,)+
                    Self::Other(name) => name,
                }
            }

            fn from_name(name: &str) -> Self {
                match name {
                    $($name => Self::$variant,)+
                    other => Self::Other(SmolStr::new(other)),
                }
            }
        }
    };
}

event_types! {
    /// `account.updated`
    AccountUpdated => "account.updated",
    /// `account.application.authorized`
    AccountApplicationAuthorized => "account.application.authorized",
    /// `account.application.deauthorized`
    AccountApplicationDeauthorized => "account.application.deauthorized",
    /// `balance.available`
    BalanceAvailable => "balance.available",
    /// `charge.captured`
    ChargeCaptured => "charge.captured",
    /// `charge.dispute.created`
    ChargeDisputeCreated => "charge.dispute.created",
    /// `charge.dispute.closed`
    ChargeDisputeClosed => "charge.dispute.closed",
    /// `charge.failed`
    ChargeFailed => "charge.failed",
    /// `charge.refunded`
    ChargeRefunded => "charge.refunded",
    /// `charge.succeeded`
    ChargeSucceeded => "charge.succeeded",
    /// `checkout.session.async_payment_failed`
    CheckoutSessionAsyncPaymentFailed => "checkout.session.async_payment_failed",
    /// `checkout.session.async_payment_succeeded`
    CheckoutSessionAsyncPaymentSucceeded => "checkout.session.async_payment_succeeded",
    /// `checkout.session.completed`
    CheckoutSessionCompleted => "checkout.session.completed",
    /// `checkout.session.expired`
    CheckoutSessionExpired => "checkout.session.expired",
    /// `customer.created`
    CustomerCreated => "customer.created",
    /// `customer.deleted`
    CustomerDeleted => "customer.deleted",
    /// `customer.updated`
    CustomerUpdated => "customer.updated",
    /// `customer.discount.created`
    CustomerDiscountCreated => "customer.discount.created",
    /// `customer.source.created`
    CustomerSourceCreated => "customer.source.created",
    /// `customer.subscription.created`
    CustomerSubscriptionCreated => "customer.subscription.created",
    /// `customer.subscription.deleted`
    CustomerSubscriptionDeleted => "customer.subscription.deleted",
    /// `customer.subscription.paused`
    CustomerSubscriptionPaused => "customer.subscription.paused",
    /// `customer.subscription.resumed`
    CustomerSubscriptionResumed => "customer.subscription.resumed",
    /// `customer.subscription.trial_will_end`
    CustomerSubscriptionTrialWillEnd => "customer.subscription.trial_will_end",
    /// `customer.subscription.updated`
    CustomerSubscriptionUpdated => "customer.subscription.updated",
    /// `identity.verification_session.verified`
    IdentityVerificationSessionVerified => "identity.verification_session.verified",
    /// `invoice.created`
    InvoiceCreated => "invoice.created",
    /// `invoice.finalized`
    InvoiceFinalized => "invoice.finalized",
    /// `invoice.finalization_failed`
    InvoiceFinalizationFailed => "invoice.finalization_failed",
    /// `invoice.paid`
    InvoicePaid => "invoice.paid",
    /// `invoice.payment_action_required`
    InvoicePaymentActionRequired => "invoice.payment_action_required",
    /// `invoice.payment_failed`
    InvoicePaymentFailed => "invoice.payment_failed",
    /// `invoice.payment_succeeded`
    InvoicePaymentSucceeded => "invoice.payment_succeeded",
    /// `invoice.upcoming`
    InvoiceUpcoming => "invoice.upcoming",
    /// `invoice.voided`
    InvoiceVoided => "invoice.voided",
    /// `issuing_authorization.request`
    IssuingAuthorizationRequest => "issuing_authorization.request",
    /// `payment_intent.amount_capturable_updated`
    PaymentIntentAmountCapturableUpdated => "payment_intent.amount_capturable_updated",
    /// `payment_intent.canceled`
    PaymentIntentCanceled => "payment_intent.canceled",
    /// `payment_intent.created`
    PaymentIntentCreated => "payment_intent.created",
    /// `payment_intent.partially_funded`
    PaymentIntentPartiallyFunded => "payment_intent.partially_funded",
    /// `payment_intent.payment_failed`
    PaymentIntentPaymentFailed => "payment_intent.payment_failed",
    /// `payment_intent.processing`
    PaymentIntentProcessing => "payment_intent.processing",
    /// `payment_intent.requires_action`
    PaymentIntentRequiresAction => "payment_intent.requires_action",
    /// `payment_intent.succeeded`
    PaymentIntentSucceeded => "payment_intent.succeeded",
    /// `payment_method.attached`
    PaymentMethodAttached => "payment_method.attached",
    /// `payment_method.detached`
    PaymentMethodDetached => "payment_method.detached",
    /// `payout.failed`
    PayoutFailed => "payout.failed",
    /// `payout.paid`
    PayoutPaid => "payout.paid",
    /// `price.created`
    PriceCreated => "price.created",
    /// `price.updated`
    PriceUpdated => "price.updated",
    /// `product.created`
    ProductCreated => "product.created",
    /// `product.updated`
    ProductUpdated => "product.updated",
    /// `radar.early_fraud_warning.created`
    RadarEarlyFraudWarningCreated => "radar.early_fraud_warning.created",
    /// `refund.created`
    RefundCreated => "refund.created",
    /// `refund.failed`
    RefundFailed => "refund.failed",
    /// `refund.updated`
    RefundUpdated => "refund.updated",
    /// `setup_intent.canceled`
    SetupIntentCanceled => "setup_intent.canceled",
    /// `setup_intent.created`
    SetupIntentCreated => "setup_intent.created",
    /// `setup_intent.setup_failed`
    SetupIntentSetupFailed => "setup_intent.setup_failed",
    /// `setup_intent.succeeded`
    SetupIntentSucceeded => "setup_intent.succeeded",
    /// `subscription_schedule.canceled`
    SubscriptionScheduleCanceled => "subscription_schedule.canceled",
    /// `subscription_schedule.created`
    SubscriptionScheduleCreated => "subscription_schedule.created",
    /// `subscription_schedule.updated`
    SubscriptionScheduleUpdated => "subscription_schedule.updated",
    /// `ping` (sent by `stripe trigger` and test deliveries)
    Ping => "ping",
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for EventType {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_name(s))
    }
}

impl Serialize for EventType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EventType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = SmolStr::deserialize(d)?;
        Ok(Self::from_name(&name))
    }
}

/// The `data` envelope of a snapshot event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EventData {
    /// The API object the event describes, as raw JSON. Deserialize into
    /// your own type or a generated model with [`Event::deserialize_object`].
    pub object: serde_json::Value,
    /// For `*.updated` events: the attribute values before the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_attributes: Option<serde_json::Value>,
}

/// Metadata about the API request that caused the event, if any.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EventRequest {
    /// The request ID (`req_…`), if the event resulted from an API call.
    pub id: Option<String>,
    /// The idempotency key of the originating request, if any.
    pub idempotency_key: Option<String>,
}

/// A snapshot webhook event — the default payload of v1-style endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Event {
    /// Event identifier (`evt_…`).
    pub id: String,
    /// The event type, strongly typed with an `Other` catch-all.
    #[serde(rename = "type")]
    pub event_type: EventType,
    /// The Stripe API version used to render `data.object`.
    #[serde(default)]
    pub api_version: Option<String>,
    /// When the event was created.
    pub created: Timestamp,
    /// The event payload.
    pub data: EventData,
    /// Whether this is a live-mode event.
    #[serde(default)]
    pub livemode: bool,
    /// Number of webhooks yet to be delivered for this event.
    #[serde(default)]
    pub pending_webhooks: Option<i64>,
    /// The originating API request, if any.
    #[serde(default)]
    pub request: Option<EventRequest>,
    /// For Connect: the account that originated the event.
    #[serde(default)]
    pub account: Option<String>,
}

impl Event {
    /// Deserialize `data.object` into a concrete type (your own struct or a
    /// generated model from `payrs-stripe-api`).
    ///
    /// # Errors
    /// The underlying `serde_json` error if the object doesn't match `T`.
    pub fn deserialize_object<T: serde::de::DeserializeOwned>(
        &self,
    ) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.data.object.clone())
    }
}

/// The object a v2 thin event points at.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RelatedObject {
    /// The related object's ID.
    pub id: String,
    /// The related object's type (e.g. `v2.core.account`).
    #[serde(rename = "type")]
    pub object_type: String,
    /// API URL to fetch the current state of the object.
    pub url: String,
}

/// A v2 **thin** event: a pointer to fresh state rather than a snapshot.
///
/// Fetch the current object via [`RelatedObject::url`] using the raw client
/// — thin payloads intentionally carry no stale object data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ThinEvent {
    /// Event identifier (`evt_…`).
    pub id: String,
    /// The event type (e.g. `v1.billing.meter.error_report_triggered`).
    #[serde(rename = "type")]
    pub event_type: String,
    /// When the event was created (RFC 3339 in v2 payloads).
    pub created: String,
    /// The object this event is about, if any.
    #[serde(default)]
    pub related_object: Option<RelatedObject>,
    /// Livemode flag.
    #[serde(default)]
    pub livemode: Option<bool>,
    /// For Connect: the originating account.
    #[serde(default)]
    pub context: Option<String>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parses_snapshot_event_with_unknown_type() {
        let json = r#"{
            "id": "evt_1", "object": "event", "type": "totally.new.event",
            "api_version": "2026-06-24.dahlia", "created": 1700000000,
            "data": {"object": {"id": "pi_1", "amount": 1999}},
            "livemode": false, "pending_webhooks": 1,
            "request": {"id": "req_1", "idempotency_key": "k"}
        }"#;
        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(
            event.event_type,
            EventType::Other("totally.new.event".into())
        );
        assert_eq!(event.data.object["amount"], 1999);
    }

    #[test]
    fn known_types_round_trip() {
        let t: EventType = "payment_intent.succeeded".parse().unwrap();
        assert_eq!(t, EventType::PaymentIntentSucceeded);
        assert_eq!(
            serde_json::to_string(&t).unwrap(),
            "\"payment_intent.succeeded\""
        );
    }

    #[test]
    fn parses_thin_event() {
        let json = r#"{
            "id": "evt_2", "object": "v2.core.event",
            "type": "v1.billing.meter.error_report_triggered",
            "created": "2026-07-01T00:00:00.000Z",
            "related_object": {"id": "mtr_1", "type": "billing.meter",
                               "url": "/v1/billing/meters/mtr_1"}
        }"#;
        let event: ThinEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.related_object.unwrap().id, "mtr_1");
    }
}
