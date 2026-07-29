//! Stripe's cursor-paginated list envelope.

use serde::{Deserialize, Serialize};

/// A page of results from a Stripe list endpoint.
///
/// The Phase-1 pagination engine turns consecutive pages into a
/// `futures::Stream`; this type is the raw envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct List<T> {
    /// The items on this page.
    pub data: Vec<T>,
    /// Whether another page exists after this one.
    pub has_more: bool,
    /// The URL that produced this page (used to continue pagination).
    #[serde(default)]
    pub url: Option<String>,
}

impl<T> List<T> {
    /// Number of items on this page.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether this page is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
