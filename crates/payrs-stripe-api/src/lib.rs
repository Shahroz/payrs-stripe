//! Full-surface Stripe API bindings, generated from Stripe's official
//! `OpenAPI` specification (`codegen/generate.py`).
//!
//! * [`models`] — every API object schema as an exported struct (1,431),
//!   grouped into 77 modules by API domain (`models::customers`,
//!   `models::checkout`, `models::treasury`, …) with shared/nested types in
//!   `models::common`. Every type is **also** re-exported flat, so
//!   `models::Customer` and `models::customers::Customer` both resolve.
//!   Forward-compatible by construction (all-`Option` fields, unknown JSON
//!   ignored).
//! * [`v1`] — every v1 operation (587) as a typed builder, grouped by API
//!   section: `v1::customers::PostCustomers::new().name("Ada").send(&client)`.
//! * [`v2`] — hand-written bindings for the v2 core namespace (events,
//!   event destinations); v2 uses JSON bodies, which the client handles via
//!   [`payrs_stripe_client::Body::Json`].
//!
//! Regenerate after updating the vendored spec: `python3 codegen/generate.py`.

// Generated modules are excluded from rustfmt: their layout is owned by
// `codegen/generate.py`, and formatting ~26k lines of generated code on every
// `cargo fmt` is slow and produces noisy diffs on every spec bump. Removing
// these attributes means `cargo fmt --all --check` (the CI lint gate) will
// demand a full reformat of the generated tree.
#[rustfmt::skip]
pub mod models;
#[rustfmt::skip]
pub mod v1;
// v2 is hand-written, so it *is* formatted normally.
pub mod v2;

mod pagination;
pub use pagination::Paginator;

use payrs_stripe_client::Error;

/// Serialize builder params to Stripe's nested form/query encoding.
/// Returns `None` when there are no params.
pub(crate) fn encode_params(
    params: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<String>, Error> {
    if params.is_empty() {
        return Ok(None);
    }
    serde_qs::to_string(params)
        .map(Some)
        .map_err(|e| Error::Config(format!("failed to encode params: {e}")))
}
