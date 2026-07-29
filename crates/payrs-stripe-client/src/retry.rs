//! Retry policy: behavioral parity with Stripe's official SDKs.
//!
//! * Retry on connection errors/timeouts, HTTP 409 (lock contention), 429,
//!   500, and 503.
//! * `Stripe-Should-Retry: true|false` **overrides** the status-based rule.
//! * Backoff is exponential with **full jitter**, capped; a `Retry-After`
//!   header (seconds) is honored as a floor.
//! * Retries are only safe because the client reuses one idempotency key
//!   across all attempts of a mutating request.

use std::time::Duration;

use crate::transport::Response;

/// Configuration for automatic retries.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RetryPolicy {
    max_retries: u32,
    base_delay: Duration,
    max_delay: Duration,
}

impl Default for RetryPolicy {
    /// Defaults matching official Stripe SDKs: up to 3 retries (4 attempts),
    /// 500 ms base, 8 s cap.
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(8),
        }
    }
}

impl RetryPolicy {
    /// Disable retries entirely.
    #[must_use]
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    /// Set the maximum number of retries (attempts = retries + 1).
    #[must_use]
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Set the base delay for the first backoff interval.
    #[must_use]
    pub fn base_delay(mut self, d: Duration) -> Self {
        self.base_delay = d;
        self
    }

    /// Set the maximum backoff delay.
    #[must_use]
    pub fn max_delay(mut self, d: Duration) -> Self {
        self.max_delay = d;
        self
    }

    /// The configured retry budget.
    #[must_use]
    pub fn retries(&self) -> u32 {
        self.max_retries
    }

    /// Decide whether a *response* (an HTTP answer was received) should be
    /// retried, given how many retries have already happened.
    #[must_use]
    pub fn should_retry_response(&self, response: &Response, retries_so_far: u32) -> bool {
        if retries_so_far >= self.max_retries {
            return false;
        }
        // Stripe's explicit signal wins in both directions.
        match response.header("stripe-should-retry") {
            Some("true") => return true,
            Some("false") => return false,
            _ => {}
        }
        matches!(response.status, 409 | 429 | 500 | 503)
    }

    /// Whether a transport failure should be retried.
    #[must_use]
    pub fn should_retry_transport(
        &self,
        error: &crate::transport::TransportError,
        retries_so_far: u32,
    ) -> bool {
        retries_so_far < self.max_retries && error.is_retryable()
    }

    /// Compute the sleep before retry number `retries_so_far + 1`.
    ///
    /// Exponential growth with full jitter: `rand(0, min(base·2^n, max))`,
    /// floored by `Retry-After` when the server sent one.
    #[must_use]
    pub fn backoff(&self, retries_so_far: u32, retry_after: Option<Duration>) -> Duration {
        let exp = self
            .base_delay
            .saturating_mul(2u32.saturating_pow(retries_so_far))
            .min(self.max_delay);
        let jittered = exp.mul_f64(fastrand::f64());
        match retry_after {
            Some(floor) => jittered.max(floor).min(self.max_delay.max(floor)),
            None => jittered,
        }
    }
}

/// Parse a `Retry-After` header value (seconds form only; Stripe uses it for
/// rate limits).
#[must_use]
pub(crate) fn parse_retry_after(value: Option<&str>) -> Option<Duration> {
    value
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn resp(status: u16, headers: &[(&str, &str)]) -> Response {
        Response {
            status,
            headers: headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<HashMap<_, _>>(),
            body: Vec::new(),
        }
    }

    #[test]
    fn retries_retryable_statuses_only() {
        let policy = RetryPolicy::default();
        for status in [409u16, 429, 500, 503] {
            assert!(
                policy.should_retry_response(&resp(status, &[]), 0),
                "{status}"
            );
        }
        for status in [200u16, 400, 401, 402, 404, 422] {
            assert!(
                !policy.should_retry_response(&resp(status, &[]), 0),
                "{status}"
            );
        }
    }

    #[test]
    fn stripe_should_retry_header_overrides() {
        let policy = RetryPolicy::default();
        // Header says retry a normally-final 400.
        assert!(policy.should_retry_response(&resp(400, &[("stripe-should-retry", "true")]), 0));
        // Header forbids retrying a normally-retryable 500.
        assert!(!policy.should_retry_response(&resp(500, &[("stripe-should-retry", "false")]), 0));
    }

    #[test]
    fn respects_retry_budget() {
        let policy = RetryPolicy::default().max_retries(2);
        assert!(policy.should_retry_response(&resp(500, &[]), 1));
        assert!(!policy.should_retry_response(&resp(500, &[]), 2));
    }

    #[test]
    fn backoff_is_bounded_and_honors_retry_after() {
        let policy = RetryPolicy::default()
            .base_delay(Duration::from_millis(100))
            .max_delay(Duration::from_secs(2));
        for n in 0..10 {
            assert!(policy.backoff(n, None) <= Duration::from_secs(2));
        }
        let floor = Duration::from_secs(1);
        assert!(policy.backoff(0, Some(floor)) >= floor);
    }
}
