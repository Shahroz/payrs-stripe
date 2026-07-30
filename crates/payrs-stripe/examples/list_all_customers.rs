//! Page through every customer with the cursor paginator.
//!
//! ```bash
//! STRIPE_SECRET_KEY=sk_test_… cargo run -p payrs-stripe --example list_all_customers
//! ```

use payrs_stripe::api::v1::customers::GetCustomers;
use payrs_stripe::{Client, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let client = Client::from_env()?;

    // Page-by-page keeps memory constant no matter how large the account is.
    let mut pager = GetCustomers::new().limit(100).paginate();
    let mut total = 0usize;

    while let Some(page) = pager.next_page(&client).await? {
        for customer in &page.data {
            println!(
                "{:<20} {}",
                customer.id.as_deref().unwrap_or("-"),
                customer.email.as_deref().unwrap_or("(no email)")
            );
        }
        total += page.data.len();
    }
    println!("\n{total} customer(s)");

    // When you do want everything in memory, state an explicit ceiling so a
    // huge account cannot exhaust it:
    let first_500: Vec<payrs_stripe::models::Customer> = GetCustomers::new()
        .paginate()
        .collect_all(&client, 500)
        .await?;
    println!("collect_all returned {}", first_500.len());

    Ok(())
}
