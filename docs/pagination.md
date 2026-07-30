# Pagination

Stripe list endpoints return one page at a time (100 items maximum). Every list
builder exposes `.paginate()`, which threads the `starting_after` cursor for
you.

## Page by page

Best for streaming work — constant memory regardless of account size.

```rust,no_run
# use payrs_stripe::{Client, Error};
use payrs_stripe::api::v1::customers::GetCustomers;

# async fn run(client: &Client) -> Result<(), Error> {
let mut pager = GetCustomers::new().limit(100).paginate();

while let Some(page) = pager.next_page(client).await? {
    for customer in &page.data {
        println!("{:?}", customer.email);
    }
}
# Ok(())
# }
```

`next_page` returns `None` once the listing is exhausted, and issues no further
requests after that.

## Bounded drain

When you genuinely want everything in memory, state a ceiling:

```rust,no_run
# use payrs_stripe::{Client, Error};
use payrs_stripe::api::v1::invoices::GetInvoices;

# async fn run(client: &Client) -> Result<(), Error> {
let invoices: Vec<payrs_stripe::models::Invoice> = GetInvoices::new()
    .status("open")
    .paginate()
    .collect_all(client, 10_000)
    .await?;
# Ok(())
# }
```

The `max_items` argument is deliberately required. An account with a million
customers should not be able to exhaust your process memory because a listing
had no bound.

## Filters apply to every page

Filters set before `.paginate()` are carried through the whole traversal:

```rust,ignore
GetInvoices::new()
    .customer("cus_123")
    .status("paid")
    .limit(100)
    .paginate()
```

## Errors mid-traversal

If a page fails, `next_page` returns the error and the paginator keeps its
position — calling it again retries the same page. Underlying HTTP retries
still apply per request, so a transient blip is usually handled before you see
it.

## One page only

Skip `.paginate()` and call `.send()` for a single page:

```rust,ignore
let page = GetCustomers::new().limit(10).send(&client).await?;
page.has_more   // is there another page?
```
