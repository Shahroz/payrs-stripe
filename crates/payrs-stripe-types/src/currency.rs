//! ISO-4217 currency codes as used by Stripe (lowercase on the wire).
//!
//! Amounts throughout the SDK are `i64` **minor units** (cents, pence, …) —
//! exactly Stripe's model. Never floats.

use smol_str::SmolStr;

macro_rules! currencies {
    ($($(#[$doc:meta])* $variant:ident => $code:literal),+ $(,)?) => {
        /// A currency code.
        ///
        /// Common currencies are first-class variants; anything else (including
        /// codes Stripe adds after this SDK release) round-trips losslessly via
        /// [`Currency::Other`]. Deserialization therefore never fails.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum Currency {
            $($(#[$doc])* $variant,)+
            /// Any currency code not (yet) a first-class variant.
            Other(SmolStr),
        }

        impl Currency {
            /// The lowercase ISO-4217 code as sent to Stripe.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $code,)+
                    Self::Other(code) => code,
                }
            }

            fn from_code(code: &str) -> Self {
                match code {
                    $($code => Self::$variant,)+
                    other => Self::Other(SmolStr::new(other)),
                }
            }
        }
    };
}

currencies! {
    /// United States dollar.
    Usd => "usd",
    /// Euro.
    Eur => "eur",
    /// Pound sterling.
    Gbp => "gbp",
    /// Japanese yen (zero-decimal currency).
    Jpy => "jpy",
    /// Canadian dollar.
    Cad => "cad",
    /// Australian dollar.
    Aud => "aud",
    /// Swiss franc.
    Chf => "chf",
    /// Chinese yuan.
    Cny => "cny",
    /// Indian rupee.
    Inr => "inr",
    /// Pakistani rupee.
    Pkr => "pkr",
    /// UAE dirham.
    Aed => "aed",
    /// Singapore dollar.
    Sgd => "sgd",
    /// Hong Kong dollar.
    Hkd => "hkd",
    /// Swedish krona.
    Sek => "sek",
    /// Norwegian krone.
    Nok => "nok",
    /// Danish krone.
    Dkk => "dkk",
    /// Polish złoty.
    Pln => "pln",
    /// Brazilian real.
    Brl => "brl",
    /// Mexican peso.
    Mxn => "mxn",
    /// New Zealand dollar.
    Nzd => "nzd",
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Currency {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_code(&s.to_ascii_lowercase()))
    }
}

impl serde::Serialize for Currency {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Currency {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let code = SmolStr::deserialize(d)?;
        Ok(Self::from_code(&code))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_known_and_unknown() {
        let usd: Currency = serde_json::from_str("\"usd\"").unwrap();
        assert_eq!(usd, Currency::Usd);

        let novel: Currency = serde_json::from_str("\"xyz\"").unwrap();
        assert_eq!(novel, Currency::Other("xyz".into()));
        assert_eq!(serde_json::to_string(&novel).unwrap(), "\"xyz\"");
    }
}
