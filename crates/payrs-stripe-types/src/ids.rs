//! Prefix-validated, typed Stripe object identifiers.
//!
//! Stripe IDs carry a resource prefix (`cus_`, `pi_`, `sub_`, …). Modeling
//! them as distinct newtypes turns "passed a product ID where a price ID
//! goes" into a compile error instead of a 400 at runtime.
//!
//! * Constructing via [`std::str::FromStr`] **validates** the prefix.
//! * Deserializing from Stripe responses is **lenient** (accepts any string):
//!   an SDK must never fail to read data Stripe actually sent.

use std::fmt;
use std::str::FromStr;

use smol_str::SmolStr;

/// Error returned when parsing a typed ID from a string with the wrong shape.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid Stripe ID: expected prefix `{expected_prefix}`, got `{found}`")]
pub struct IdError {
    /// The prefix required by the ID type, e.g. `cus_`.
    pub expected_prefix: &'static str,
    /// The (truncated) input that failed validation.
    pub found: String,
}

macro_rules! def_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(SmolStr);

        impl $name {
            /// The required prefix for this ID type.
            pub const PREFIX: &'static str = $prefix;

            /// View the ID as a string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if s.starts_with($prefix) && s.len() > $prefix.len() {
                    Ok(Self(SmolStr::new(s)))
                } else {
                    Err(IdError {
                        expected_prefix: $prefix,
                        found: s.chars().take(32).collect(),
                    })
                }
            }
        }

        impl TryFrom<&str> for $name {
            type Error = IdError;
            fn try_from(s: &str) -> Result<Self, Self::Error> {
                s.parse()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.0)
            }
        }

        // Lenient on the wire: never reject data Stripe sent.
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = SmolStr::deserialize(d)?;
                Ok(Self(s))
            }
        }
    };
}

def_id!(
    /// Identifier of a Customer (`cus_…`).
    CustomerId,
    "cus_"
);
def_id!(
    /// Identifier of a `PaymentIntent` (`pi_…`).
    PaymentIntentId,
    "pi_"
);
def_id!(
    /// Identifier of a `SetupIntent` (`seti_…`).
    SetupIntentId,
    "seti_"
);
def_id!(
    /// Identifier of a `PaymentMethod` (`pm_…`).
    PaymentMethodId,
    "pm_"
);
def_id!(
    /// Identifier of a Charge (`ch_…`).
    ChargeId,
    "ch_"
);
def_id!(
    /// Identifier of a Refund (`re_…`).
    RefundId,
    "re_"
);
def_id!(
    /// Identifier of a Product (`prod_…`).
    ProductId,
    "prod_"
);
def_id!(
    /// Identifier of a Price (`price_…`).
    PriceId,
    "price_"
);
def_id!(
    /// Identifier of a Subscription (`sub_…`).
    SubscriptionId,
    "sub_"
);
def_id!(
    /// Identifier of an Invoice (`in_…`).
    InvoiceId,
    "in_"
);
def_id!(
    /// Identifier of an Event (`evt_…`).
    EventId,
    "evt_"
);
def_id!(
    /// Identifier of a connected Account (`acct_…`).
    AccountId,
    "acct_"
);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn from_str_validates_prefix() {
        assert!("cus_123".parse::<CustomerId>().is_ok());
        let err = "price_123".parse::<CustomerId>().unwrap_err();
        assert_eq!(err.expected_prefix, "cus_");
    }

    #[test]
    fn deserialize_is_lenient() {
        // Hypothetical future prefix change must not break reads.
        let id: CustomerId = serde_json::from_str("\"weird_id\"").unwrap();
        assert_eq!(id.as_str(), "weird_id");
    }

    #[test]
    fn debug_and_display() {
        let id: PaymentIntentId = "pi_abc".parse().unwrap();
        assert_eq!(id.to_string(), "pi_abc");
        assert_eq!(format!("{id:?}"), "PaymentIntentId(pi_abc)");
    }
}
