//! `Stripe-Signature` verification.
//!
//! Scheme: the header carries `t=<unix_ts>,v1=<hex hmac>[,v1=…]`. The signed
//! payload is `"{t}.{raw_body}"`, `MACed` with `HMAC-SHA256` under the endpoint
//! secret (`whsec_…`). Verification must be constant-time (this crate uses
//! `hmac`'s `verify_slice`, which is) and must reject stale timestamps to
//! prevent replay attacks.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::event::{Event, ThinEvent};

/// Default replay tolerance, matching official Stripe SDKs: 5 minutes.
pub const DEFAULT_TOLERANCE: Duration = Duration::from_secs(300);

/// Why signature verification failed.
///
/// The messages are safe to log; they never include the secret or payload.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SignatureError {
    /// The `Stripe-Signature` header was missing or empty.
    #[error("missing Stripe-Signature header")]
    MissingHeader,
    /// The header didn't match the `t=…,v1=…` format.
    #[error("malformed Stripe-Signature header")]
    BadFormat,
    /// The header contained no `v1` signatures.
    #[error("no v1 signatures found in Stripe-Signature header")]
    NoV1Signatures,
    /// The timestamp was outside the allowed tolerance window.
    #[error(
        "timestamp outside the tolerance window ({tolerance_secs}s); possible replay or clock skew"
    )]
    TimestampOutOfTolerance {
        /// The tolerance that was applied, in seconds.
        tolerance_secs: u64,
    },
    /// No candidate signature matched the payload.
    #[error("no signature matched the payload; check the endpoint secret and that the raw body was used")]
    SignatureMismatch,
}

/// Webhook verification entry points.
///
/// Stateless; construct nothing. See [`crate::WebhookRouter`] for the
/// batteries-included dispatch layer.
#[derive(Debug, Clone, Copy)]
pub struct Webhook;

impl Webhook {
    /// Verify a delivery and parse a snapshot [`Event`] (v1-style payloads,
    /// which is what webhook endpoints receive by default).
    ///
    /// * `payload` — the **raw request body bytes**, untransformed.
    /// * `signature_header` — the `Stripe-Signature` header value.
    /// * `secret` — the endpoint's signing secret (`whsec_…`).
    ///
    /// # Errors
    /// [`crate::WebhookError::Signature`] on verification failure,
    /// [`crate::WebhookError::Parse`] if the verified payload isn't a valid
    /// event document.
    pub fn construct_event(
        payload: &[u8],
        signature_header: &str,
        secret: &str,
    ) -> Result<Event, crate::WebhookError> {
        Self::verify_with_tolerance(payload, signature_header, secret, DEFAULT_TOLERANCE)?;
        Ok(serde_json::from_slice(payload)?)
    }

    /// Verify a delivery and parse a v2 **thin** event.
    ///
    /// # Errors
    /// Same as [`Webhook::construct_event`].
    pub fn construct_thin_event(
        payload: &[u8],
        signature_header: &str,
        secret: &str,
    ) -> Result<ThinEvent, crate::WebhookError> {
        Self::verify_with_tolerance(payload, signature_header, secret, DEFAULT_TOLERANCE)?;
        Ok(serde_json::from_slice(payload)?)
    }

    /// Verify only (no parsing), with the default 5-minute tolerance.
    ///
    /// # Errors
    /// A [`SignatureError`] describing the failure.
    pub fn verify(
        payload: &[u8],
        signature_header: &str,
        secret: &str,
    ) -> Result<(), SignatureError> {
        Self::verify_with_tolerance(payload, signature_header, secret, DEFAULT_TOLERANCE)
    }

    /// Verify with a custom tolerance window.
    ///
    /// # Errors
    /// A [`SignatureError`] describing the failure.
    pub fn verify_with_tolerance(
        payload: &[u8],
        signature_header: &str,
        secret: &str,
        tolerance: Duration,
    ) -> Result<(), SignatureError> {
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
        )
        .unwrap_or(i64::MAX);
        verify_at(payload, signature_header, secret, tolerance, now)
    }
}

/// Testable core: verification against an explicit "now".
pub(crate) fn verify_at(
    payload: &[u8],
    signature_header: &str,
    secret: &str,
    tolerance: Duration,
    now_unix: i64,
) -> Result<(), SignatureError> {
    let header = signature_header.trim();
    if header.is_empty() {
        return Err(SignatureError::MissingHeader);
    }

    let mut timestamp: Option<i64> = None;
    let mut candidates: Vec<Vec<u8>> = Vec::new();

    for part in header.split(',') {
        let mut kv = part.trim().splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("t"), Some(v)) => {
                timestamp = Some(v.parse().map_err(|_| SignatureError::BadFormat)?);
            }
            (Some("v1"), Some(v)) => {
                candidates.push(decode_hex(v).ok_or(SignatureError::BadFormat)?);
            }
            // Ignore v0 (legacy) and unknown schemes, per Stripe guidance.
            (Some(_), Some(_)) => {}
            _ => return Err(SignatureError::BadFormat),
        }
    }

    let timestamp = timestamp.ok_or(SignatureError::BadFormat)?;
    if candidates.is_empty() {
        return Err(SignatureError::NoV1Signatures);
    }

    let age = (now_unix - timestamp).unsigned_abs();
    if age > tolerance.as_secs() {
        return Err(SignatureError::TimestampOutOfTolerance {
            tolerance_secs: tolerance.as_secs(),
        });
    }

    // signed_payload = "{t}.{body}"
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| SignatureError::BadFormat)?;
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);

    // `verify_slice` is constant-time; try each candidate (Stripe sends
    // multiple v1 entries during secret rotation).
    for candidate in &candidates {
        if mac.clone().verify_slice(candidate).is_ok() {
            return Ok(());
        }
    }
    Err(SignatureError::SignatureMismatch)
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Test-only helper: produce a valid `Stripe-Signature` header for a payload
/// with the current time (used by router end-to-end tests).
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) fn tests_helper_sign(payload: &[u8], secret: &str) -> String {
    let ts = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs(),
    )
    .unwrap_or(i64::MAX);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(ts.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);
    let sig = to_hex(&mac.finalize().into_bytes());
    format!("t={ts},v1={sig}")
}

#[cfg(test)]
#[allow(clippy::format_collect)]
pub(crate) fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_test_secret";

    pub(crate) fn sign(payload: &[u8], secret: &str, ts: i64) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(ts.to_string().as_bytes());
        mac.update(b".");
        mac.update(payload);
        let sig = super::to_hex(&mac.finalize().into_bytes());
        format!("t={ts},v1={sig}")
    }

    #[test]
    fn valid_signature_passes() {
        let payload = br#"{"id": "evt_1", "type": "ping"}"#;
        let header = sign(payload, SECRET, 1_700_000_000);
        assert!(verify_at(payload, &header, SECRET, DEFAULT_TOLERANCE, 1_700_000_010).is_ok());
    }

    #[test]
    fn tampered_payload_fails() {
        let header = sign(b"original", SECRET, 1_700_000_000);
        let err = verify_at(
            b"tampered",
            &header,
            SECRET,
            DEFAULT_TOLERANCE,
            1_700_000_010,
        )
        .unwrap_err();
        assert_eq!(err, SignatureError::SignatureMismatch);
    }

    #[test]
    fn wrong_secret_fails() {
        let payload = b"payload";
        let header = sign(payload, "whsec_other", 1_700_000_000);
        let err =
            verify_at(payload, &header, SECRET, DEFAULT_TOLERANCE, 1_700_000_010).unwrap_err();
        assert_eq!(err, SignatureError::SignatureMismatch);
    }

    #[test]
    fn stale_timestamp_fails() {
        let payload = b"payload";
        let header = sign(payload, SECRET, 1_700_000_000);
        let err = verify_at(
            payload,
            &header,
            SECRET,
            DEFAULT_TOLERANCE,
            1_700_000_000 + 301,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            SignatureError::TimestampOutOfTolerance { .. }
        ));
    }

    #[test]
    fn rotation_second_v1_candidate_passes() {
        let payload = b"payload";
        let good = sign(payload, SECRET, 1_700_000_000);
        let good_sig = good.split("v1=").nth(1).unwrap();
        let header = format!("t=1700000000,v1={},v1={good_sig}", "ab".repeat(32));
        assert!(verify_at(payload, &header, SECRET, DEFAULT_TOLERANCE, 1_700_000_010).is_ok());
    }

    #[test]
    fn malformed_headers_fail_cleanly() {
        for header in ["", "v1=abcd", "t=notanumber,v1=abcd", "t=1,v1=xyz"] {
            assert!(
                verify_at(b"p", header, SECRET, DEFAULT_TOLERANCE, 1).is_err(),
                "{header}"
            );
        }
    }
}
