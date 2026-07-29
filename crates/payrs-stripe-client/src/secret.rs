//! Secret API key handling: never printed, never serialized.

use std::fmt;

/// A Stripe secret API key (`sk_live_…`, `sk_test_…`, or restricted `rk_…`).
///
/// * `Debug`/`Display` print a **redacted** form (`sk_test_***`) so keys can
///   never leak through logs, panics, or error chains.
/// * The type intentionally implements neither `Serialize` nor
///   `Deserialize`.
///
/// ```
/// use payrs_stripe_client::SecretKey;
/// let key = SecretKey::new("sk_test_abcdef123456");
/// assert_eq!(format!("{key:?}"), "SecretKey(sk_test_***)");
/// ```
#[derive(Clone)]
pub struct SecretKey(String);

impl SecretKey {
    /// Wrap a secret key string.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Access the raw key. Only the transport layer should call this.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The non-secret prefix used for redacted display (`sk_test`, `sk_live`,
    /// `rk_live`, …), or `"key"` if the shape is unrecognized.
    #[must_use]
    pub fn redacted_prefix(&self) -> &str {
        let mut underscores = 0usize;
        for (idx, byte) in self.0.bytes().enumerate() {
            if byte == b'_' {
                underscores += 1;
                if underscores == 2 {
                    return &self.0[..idx];
                }
            }
        }
        "key"
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretKey({}_***)", self.redacted_prefix())
    }
}

impl fmt::Display for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_***", self.redacted_prefix())
    }
}

impl From<String> for SecretKey {
    fn from(key: String) -> Self {
        Self::new(key)
    }
}

impl From<&str> for SecretKey {
    fn from(key: &str) -> Self {
        Self::new(key)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_leaks() {
        let key = SecretKey::new("sk_live_SUPERSECRET");
        let printed = format!("{key:?} {key}");
        assert!(!printed.contains("SUPERSECRET"));
        assert_eq!(printed, "SecretKey(sk_live_***) sk_live_***");
    }

    #[test]
    fn weird_shapes_still_redact() {
        let key = SecretKey::new("not-a-stripe-key");
        assert_eq!(format!("{key}"), "key_***");
    }
}
