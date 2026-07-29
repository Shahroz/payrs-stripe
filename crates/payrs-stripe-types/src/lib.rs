//! Shared primitive types used across the `payrs-stripe` SDK.
//!
//! This crate defines the vocabulary shared by every resource crate:
//!
//! * [`ids`] — prefix-validated, typed object identifiers such as [`ids::CustomerId`].
//! * [`Currency`] — ISO-4217 currency codes as a forward-compatible enum.
//! * [`Timestamp`] — Unix-epoch seconds, Stripe's time representation.
//! * [`Expandable`] — a field that is either an ID or the expanded object.
//! * [`List`] — Stripe's cursor-paginated list envelope.
//! * [`Metadata`] — the `metadata` key/value map present on most objects.
//! * [`ApiVersion`] — the pinned `Stripe-Version` this SDK line targets.
//!
//! Design rules applied throughout (see the architecture plan, §4.2):
//!
//! * Money is always `i64` minor units — never floating point.
//! * Every enum owned by Stripe has an `Other` catch-all so new API values
//!   can never break deserialization.
//! * Response-shaped structs are `#[non_exhaustive]`; Stripe adding fields is
//!   a non-breaking event for downstream code.

pub mod currency;
pub mod expandable;
pub mod ids;
pub mod list;
pub mod timestamp;

pub use currency::Currency;
pub use expandable::{Expandable, Object};
pub use ids::IdError;
pub use list::List;
pub use timestamp::Timestamp;

/// The `metadata` map attached to most Stripe objects.
///
/// A `BTreeMap` is used (rather than `HashMap`) so serialization order is
/// deterministic, which keeps snapshot tests and request signatures stable.
pub type Metadata = std::collections::BTreeMap<String, String>;

/// The Stripe API version (`Stripe-Version` header) this SDK line is pinned to.
///
/// Following official Stripe SDK policy, typed requests always send this
/// pinned version so response shapes match the generated types. Only the raw
/// escape-hatch client may override it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiVersion(&'static str);

impl ApiVersion {
    /// The version pinned for this release line.
    pub const CURRENT: ApiVersion = ApiVersion("2026-06-24.dahlia");

    /// The version string as sent in the `Stripe-Version` header.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
