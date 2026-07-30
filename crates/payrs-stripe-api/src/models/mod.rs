//! Stripe API object models — one module per API domain.
//!
//! 1431 models across 76 modules, generated from the
//! official `OpenAPI` specification by `codegen/generate.py`.
//! **Do not edit by hand.**
//!
//! Every type is also re-exported at this level, so both paths work:
//!
//! ```
//! // Both paths name the same type:
//! let _: Option<payrs_stripe_api::models::Customer> = None;
//! let _: Option<payrs_stripe_api::models::customers::Customer> = None;
//! ```
//!
//! Forward-compatibility rules: every field is `Option` (Stripe omits
//! fields by context and adds fields over time), unknown JSON fields
//! are ignored, and expandable/polymorphic fields are
//! `serde_json::Value` in this codegen generation.
#![allow(clippy::wildcard_imports)]

pub mod account_links;
pub mod account_sessions;
pub mod accounts;
pub mod apple_pay;
pub mod application_fees;
pub mod apps;
pub mod balance;
pub mod balance_settings;
pub mod balance_transactions;
pub mod billing;
pub mod billing_portal;
pub mod charges;
pub mod checkout;
pub mod climate;
pub mod common;
pub mod confirmation_tokens;
pub mod country_specs;
pub mod coupons;
pub mod credit_notes;
pub mod customer_sessions;
pub mod customers;
pub mod deleted_test_helpers;
pub mod disputes;
pub mod entitlements;
pub mod ephemeral_keys;
pub mod events;
pub mod exchange_rates;
pub mod external_accounts;
pub mod file_links;
pub mod files;
pub mod financial_connections;
pub mod forwarding;
pub mod identity;
pub mod invoice_payments;
pub mod invoice_rendering_templates;
pub mod invoiceitems;
pub mod invoices;
pub mod issuing;
pub mod linked_accounts;
pub mod mandates;
pub mod payment_attempt_records;
pub mod payment_intents;
pub mod payment_links;
pub mod payment_method_configurations;
pub mod payment_method_domains;
pub mod payment_methods;
pub mod payment_records;
pub mod payouts;
pub mod plans;
pub mod prices;
pub mod products;
pub mod promotion_codes;
pub mod quotes;
pub mod radar;
pub mod refunds;
pub mod reporting;
pub mod reviews;
pub mod setup_attempts;
pub mod setup_intents;
pub mod shipping_rates;
pub mod sigma;
pub mod sources;
pub mod subscription_items;
pub mod subscription_schedules;
pub mod subscriptions;
pub mod tax;
pub mod tax_codes;
pub mod tax_ids;
pub mod tax_rates;
pub mod terminal;
pub mod test_helpers;
pub mod tokens;
pub mod topups;
pub mod transfers;
pub mod treasury;
pub mod webhook_endpoints;

// Flat re-exports preserve the pre-grouping import paths.
pub use account_links::*;
pub use account_sessions::*;
pub use accounts::*;
pub use apple_pay::*;
pub use application_fees::*;
pub use apps::*;
pub use balance::*;
pub use balance_settings::*;
pub use balance_transactions::*;
pub use billing::*;
pub use billing_portal::*;
pub use charges::*;
pub use checkout::*;
pub use climate::*;
pub use common::*;
pub use confirmation_tokens::*;
pub use country_specs::*;
pub use coupons::*;
pub use credit_notes::*;
pub use customer_sessions::*;
pub use customers::*;
pub use deleted_test_helpers::*;
pub use disputes::*;
pub use entitlements::*;
pub use ephemeral_keys::*;
pub use events::*;
pub use exchange_rates::*;
pub use external_accounts::*;
pub use file_links::*;
pub use files::*;
pub use financial_connections::*;
pub use forwarding::*;
pub use identity::*;
pub use invoice_payments::*;
pub use invoice_rendering_templates::*;
pub use invoiceitems::*;
pub use invoices::*;
pub use issuing::*;
pub use linked_accounts::*;
pub use mandates::*;
pub use payment_attempt_records::*;
pub use payment_intents::*;
pub use payment_links::*;
pub use payment_method_configurations::*;
pub use payment_method_domains::*;
pub use payment_methods::*;
pub use payment_records::*;
pub use payouts::*;
pub use plans::*;
pub use prices::*;
pub use products::*;
pub use promotion_codes::*;
pub use quotes::*;
pub use radar::*;
pub use refunds::*;
pub use reporting::*;
pub use reviews::*;
pub use setup_attempts::*;
pub use setup_intents::*;
pub use shipping_rates::*;
pub use sigma::*;
pub use sources::*;
pub use subscription_items::*;
pub use subscription_schedules::*;
pub use subscriptions::*;
pub use tax::*;
pub use tax_codes::*;
pub use tax_ids::*;
pub use tax_rates::*;
pub use terminal::*;
pub use test_helpers::*;
pub use tokens::*;
pub use topups::*;
pub use transfers::*;
pub use treasury::*;
pub use webhook_endpoints::*;
