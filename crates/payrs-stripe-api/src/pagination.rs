//! Cursor pagination over Stripe v1 list endpoints.
//!
//! Every generated list operation exposes `.paginate()`, which turns the
//! request into a [`Paginator`]: pull pages with [`Paginator::next_page`] or
//! drain everything (bounded) with [`Paginator::collect_all`]. The cursor
//! (`starting_after`) is threaded automatically from each page's last item.

use std::marker::PhantomData;

use payrs_stripe_client::{Client, Error, Method, RequestSpec};
use payrs_stripe_types::List;

/// Pages through a v1 list endpoint using `starting_after` cursors.
///
/// ```no_run
/// # async fn run(client: &payrs_stripe_client::Client) -> Result<(), payrs_stripe_client::Error> {
/// use payrs_stripe_api::v1::customers::GetCustomers;
///
/// // Page by page:
/// let mut pager = GetCustomers::new().limit(100).paginate();
/// while let Some(page) = pager.next_page(client).await? {
///     for customer in &page.data {
///         println!("{:?}", customer.email);
///     }
/// }
///
/// // Or drain (bounded — never accidentally load a million rows):
/// let all = GetCustomers::new().limit(100).paginate()
///     .collect_all(client, 10_000)
///     .await?;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub struct Paginator<T> {
    path: String,
    params: serde_json::Map<String, serde_json::Value>,
    exhausted: bool,
    _item: PhantomData<fn() -> T>,
}

impl<T: serde::de::DeserializeOwned> Paginator<T> {
    pub(crate) fn new(path: String, params: serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            path,
            params,
            exhausted: false,
            _item: PhantomData,
        }
    }

    /// Fetch the next page, or `None` once the listing is exhausted.
    ///
    /// # Errors
    /// See [`payrs_stripe_client::Error`]; pagination stops on the first
    /// error (calling again retries the same page).
    pub async fn next_page(&mut self, client: &Client) -> Result<Option<List<T>>, Error> {
        if self.exhausted {
            return Ok(None);
        }
        let mut spec = RequestSpec::new(Method::Get, self.path.clone());
        spec.raw_query = crate::encode_params(&self.params)?;
        let raw: serde_json::Value = client.execute(spec).await?;

        let has_more = raw
            .get("has_more")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let last_id = raw
            .get("data")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.last())
            .and_then(|item| item.get("id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);

        let page: List<T> = serde_json::from_value(raw)
            .map_err(|e| Error::deserialization("<list page>", e, None))?;

        match (has_more, last_id) {
            (true, Some(id)) => {
                self.params
                    .insert("starting_after".to_owned(), serde_json::Value::String(id));
            }
            // No cursor available (or no more pages): stop cleanly.
            _ => self.exhausted = true,
        }
        Ok(Some(page))
    }

    /// Fetch every remaining item, stopping at `max_items` as a safety bound
    /// against unbounded memory on huge listings.
    ///
    /// # Errors
    /// See [`Paginator::next_page`].
    pub async fn collect_all(mut self, client: &Client, max_items: usize) -> Result<Vec<T>, Error> {
        let mut items = Vec::new();
        while let Some(page) = self.next_page(client).await? {
            let empty_page = page.data.is_empty();
            items.extend(page.data);
            if items.len() >= max_items || empty_page {
                break;
            }
        }
        items.truncate(max_items);
        Ok(items)
    }
}
