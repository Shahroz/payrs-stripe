//! Stripe's time representation: Unix epoch seconds.

use serde::{Deserialize, Serialize};

/// A point in time as Unix-epoch seconds (Stripe's wire format).
///
/// Conversions to `chrono`/`time` types will land behind optional features;
/// the core type stays dependency-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(pub i64);

impl Timestamp {
    /// The raw epoch-seconds value.
    #[must_use]
    pub const fn as_secs(self) -> i64 {
        self.0
    }
}

impl From<i64> for Timestamp {
    fn from(secs: i64) -> Self {
        Self(secs)
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
