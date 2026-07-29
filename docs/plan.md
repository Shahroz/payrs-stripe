# Stripe SDK for Rust — Architecture, Engineering & Delivery Plan

**Working name:** `payrs-stripe` (final name TBD — see §9.1 on crates.io naming)
**Status:** Draft v1.0 · July 2026
**Author:** Shahroz (Principal Engineer)
**Target registry:** crates.io (official Rust package manager registry)

---

## 1. Problem Statement & Requirements

### 1.1 Problem

Stripe ships official server-side SDKs for Ruby, Python, PHP, Java, Node, Go, and .NET — but **not Rust**. The Rust ecosystem relies on community crates, primarily `async-stripe` (the de-facto standard, generated from Stripe's OpenAPI spec) and smaller efforts like `stripe-sdk`. Gaps and pain points that motivate a new library:

- `async-stripe`'s 1.0 rewrite is still in alpha/RC with expected breaking changes, and its legacy 0.x line is pinned to older API versions.
- The Stripe API surface is enormous; naive full-surface bindings produce multi-minute compile times and 70 MB binaries (a problem async-stripe's rewrite explicitly targets).
- Developers building AI agents and business automation (our primary audience) need a **small, ergonomic, batteries-included** client for the ~20% of Stripe used 95% of the time (PaymentIntents, Customers, Subscriptions, Checkout, Webhooks, Products/Prices, Invoices, Connect basics) — with first-class idempotency, retries, and webhook verification, plus an escape hatch for everything else.

### 1.2 Functional requirements

| ID | Requirement |
|----|-------------|
| F1 | Async client (Tokio) for Stripe REST API v1, with typed request builders and typed responses |
| F2 | Coverage tiers: **Tier 1 (hand-polished)** — PaymentIntents, SetupIntents, Customers, PaymentMethods, Products, Prices, Subscriptions, Invoices, Checkout Sessions, Refunds, Events, Webhook endpoints; **Tier 2 (generated)** — remaining resources; **Tier 3 (escape hatch)** — raw request API for any endpoint/version |
| F3 | Webhook signature verification (`Stripe-Signature`, HMAC-SHA256, tolerance window) and typed event deserialization |
| F4 | Automatic idempotency keys (UUIDv4) on all mutating requests, overridable per request |
| F5 | Automatic retries with exponential backoff + jitter for network errors, 409/429/5xx, honoring `Stripe-Should-Retry` and `Retry-After` |
| F6 | Auto-pagination: `Stream`-based iteration over list endpoints (cursor-based `starting_after`) |
| F7 | Expandable fields modeled as a type-safe `Expandable<T>` (Id or Object) |
| F8 | Stripe-Account (Connect), Stripe-Version, and Stripe-Context header support per client and per request |
| F9 | Structured error type mirroring Stripe's error envelope (`type`, `code`, `decline_code`, `param`, `doc_url`, `request_id`) |
| F10 | Test-mode friendliness: base URL override for `stripe-mock` / sandboxes; no global state |

### 1.3 Non-functional requirements

- **Compile time:** a "hello Stripe" binary using Tier 1 features builds clean in < 60s on a laptop; incremental < 10s.
- **Binary size:** contribution < 5 MB stripped for Tier 1 usage.
- **MSRV:** Rust 1.78+ (documented; bumps allowed in minor releases, ≥ 6 months old policy).
- **Reliability:** timeouts on every request (default 30s connect+read, configurable); zero panics in library code paths (`#![deny(clippy::unwrap_used, clippy::expect_used)]` in lib code).
- **Security:** rustls default; secret keys wrapped in a `SecretString`-style type that redacts in `Debug`; never logged.
- **API stability:** SemVer strictly; `#[non_exhaustive]` on all Stripe-shaped structs/enums so Stripe additive changes are non-breaking.
- **Docs:** 100% public-item rustdoc coverage enforced (`#![deny(missing_docs)]`), doctests compile in CI, mdBook guide site.

### 1.4 Constraints & assumptions

- Team: 1–2 engineers initially → design must minimize ongoing maintenance (codegen for the long tail, hand-written only where ergonomics pay off).
- Stripe API version pinned per SDK release (like official SDKs): each SDK major targets one Stripe version line (current: `2026-06-24.dahlia`); users shouldn't override the version on strongly-typed calls (types would drift), but the raw client may.
- v1 API namespace first; `/v2` namespace (thin events, JSON bodies, new idempotency semantics) is a Phase 3 concern behind a `v2` feature.
- Not in scope for 1.0: Terminal, Issuing UIs, file uploads multipart (Phase 2), OAuth for Connect standard accounts (Phase 2).

---

## 2. Landscape & Positioning (research summary)

| Library | Model | Strengths | Weaknesses / our differentiation |
|---|---|---|---|
| `async-stripe` (arlyon) | Full codegen from OpenAPI, weekly CI regen, split into `stripe-core`, `stripe-billing`, etc.; serde for ser, miniserde for deser | Complete coverage, actively maintained, dominant | 1.0 still stabilizing; huge type surface; ergonomics driven by spec shape; miniserde limits custom deserialization |
| `stripe-sdk` (Finite Field) | build.rs codegen per operationId | Simple, small | Operation-shaped (not resource-shaped) API; thin ergonomics |
| Official Stripe SDKs (Go/Java/…) | Codegen + curated helpers, SemVer, API-version-pinned releases | Gold standard for behavior (retries, idempotency, telemetry) | No Rust offering — we mirror their *behavioral contract* |

**Positioning:** not "another full mirror of the spec" — a **curated, ergonomics-first SDK** with hand-designed Tier 1 modules, generated Tier 2, and a raw escape hatch; behavioral parity with official Stripe SDKs (retries, idempotency, telemetry headers, version pinning).

---

## 3. High-Level Architecture

### 3.1 Cargo workspace layout

```
stripe-rust/                          (git repo, cargo workspace)
├── Cargo.toml                        (workspace, shared lints & deps)
├── crates/
│   ├── payrs-stripe/                 ← facade crate users depend on (re-exports)
│   ├── payrs-stripe-client/          ← transport: auth, retries, idempotency,
│   │                                    pagination engine, errors, config
│   ├── payrs-stripe-types/           ← shared primitives: Currency, Timestamp,
│   │                                    Metadata, Expandable<T>, ids (CustomerId…),
│   │                                    List<T>, ApiVersion
│   ├── payrs-stripe-core/            ← Tier 1: payments, customers, refunds
│   ├── payrs-stripe-billing/         ← Tier 1: subscriptions, invoices, prices
│   ├── payrs-stripe-checkout/        ← Tier 1: checkout sessions, payment links
│   ├── payrs-stripe-webhooks/        ← signature verify + typed Event
│   ├── payrs-stripe-connect/         ← Tier 2 (generated): accounts, transfers
│   ├── payrs-stripe-misc/            ← Tier 2 (generated): long tail
│   └── payrs-stripe-codegen/         ← internal tool (NOT published):
│                                        OpenAPI spec → Tier 2 code
├── examples/                         (runnable, one per major flow)
├── docs/                             (mdBook guide)
├── tests/                            (integration tests vs stripe-mock)
└── xtask/                            (cargo xtask: regen, msrv check, release)
```

### 3.2 Request flow

```
user code
   │  CreatePaymentIntent::new(amount, currency).customer(id).build()
   ▼
typed request builder (per-resource crate)
   │  → RequestSpec { method, path, form/query params, headers }
   ▼
payrs-stripe-client::Client
   │  1. attach auth (Bearer secret key)          [redacted type]
   │  2. attach Stripe-Version (pinned const)
   │  3. attach Idempotency-Key (uuid v4 if mutating & unset)
   │  4. attach Stripe-Account / user headers
   │  5. serialize params (form-encoded for v1)
   ▼
HTTP layer (reqwest + rustls by default; trait-abstracted)
   │  timeout → send → response
   ▼
retry policy (backoff + jitter; 429/409-lock/5xx/conn errors;
              honors Stripe-Should-Retry / Retry-After; max 3 by default)
   ▼
deserialize (serde_json) → Result<T, StripeError>
   │  error path: parse Stripe error envelope + request-id
   ▼
user gets typed value / typed error
```

### 3.3 Public API shape (target ergonomics)

```rust
use payrs_stripe::{Client, Currency};
use payrs_stripe::payment_intent::CreatePaymentIntent;

#[tokio::main]
async fn main() -> Result<(), payrs_stripe::Error> {
    // Reads STRIPE_SECRET_KEY; explicit `Client::new(key)` also available.
    let client = Client::from_env()?;

    let pi = CreatePaymentIntent::new(1999, Currency::USD)
        .customer("cus_123")
        .automatic_payment_methods(true)
        .metadata([("order_id", "ord_789")])
        .send(&client)
        .await?;

    println!("client_secret = {}", pi.client_secret.expose());

    // Auto-pagination as a Stream
    use futures_util::TryStreamExt;
    let mut charges = payrs_stripe::charge::ListCharges::new()
        .customer("cus_123")
        .stream(&client);
    while let Some(charge) = charges.try_next().await? {
        println!("{} {}", charge.id, charge.amount);
    }
    Ok(())
}
```

Escape hatch (Tier 3):

```rust
let value: serde_json::Value = client
    .request(Method::POST, "/v1/terminal/readers")
    .form(&[("registration_code", "puppies-plug-could")])
    .idempotency_key("my-key")
    .send_json()
    .await?;
```

Webhooks (framework-agnostic, with optional `axum` feature providing an extractor):

```rust
let event = payrs_stripe::webhooks::Webhook::construct_event(
    payload_bytes, signature_header, endpoint_secret,
)?; // verifies HMAC-SHA256 within default 5-min tolerance

match event.data {
    EventData::PaymentIntentSucceeded(pi) => fulfill(pi).await?,
    EventData::Unknown { type_, raw } => tracing::warn!(%type_, "unhandled"),
    _ => {}
}
```

---

## 4. Component Deep Dives

### 4.1 `payrs-stripe-client` (transport core)

**Config (builder):**

```rust
let client = Client::builder("sk_test_…")        // key stored as SecretKey (Debug-redacted)
    .api_base("https://api.stripe.com")           // override for stripe-mock/sandbox
    .stripe_account("acct_123")                   // Connect: acts-on-behalf-of
    .timeout(Duration::from_secs(30))
    .retry_policy(RetryPolicy::default().max_retries(3))
    .app_info("my-agent-platform", Some("1.2.0"), Some("https://example.com"))
    .build()?;
```

- `Client` is `Clone + Send + Sync` (cheap `Arc` internals) — one client per process, safely shared across tasks.
- **Telemetry parity with official SDKs:** `User-Agent` + `X-Stripe-Client-User-Agent` JSON blob (lang, lang_version, os, publisher, app_info). Opt-out flag for privacy-sensitive deployments.
- **HTTP abstraction:** a small `HttpTransport` trait (object-safe: `fn execute(Request) -> BoxFuture<Response>`); default impl on `reqwest`. Keeps the door open for hyper-only, `wasm` (fetch), or user-injected middleware/mock transports without a breaking change.

**Retry policy (behavioral contract copied from official SDKs):**

- Retry on: connection errors, timeouts, HTTP 409 (lock contention), 429, 500, 503 — and any response with header `Stripe-Should-Retry: true`; never when `Stripe-Should-Retry: false`.
- Backoff: `min(base * 2^attempt, max)` with full jitter (base 500ms, max 8s); honor `Retry-After` when present.
- Retries are safe **because** every mutating request carries an idempotency key generated *before* the first attempt (key reused across retries — this is the whole point).

**Idempotency:**

- POST/DELETE requests: auto-generate UUIDv4 `Idempotency-Key` unless user supplies one.
- Exposed on every builder: `.idempotency_key(k)`. Documented guidance: derive keys from your own business identifiers (e.g., `order_{id}_capture`) for cross-process safety; keys ≤ 255 chars; avoid PII in keys; Stripe prunes keys after ~24h.

**Errors (single non_exhaustive enum):**

```rust
#[non_exhaustive]
pub enum Error {
    Api(Box<ApiError>),        // Stripe error envelope: type, code, decline_code,
                               // message, param, doc_url + http status + request_id
    Network(TransportError),   // connect/timeout/tls
    Deserialization { source: serde_json::Error, request_id: Option<RequestId> },
    SignatureVerification(WebhookError),   // webhooks only
    Config(ConfigError),
}
```

- `ApiError::request_id` always captured — the #1 thing Stripe support asks for.
- Helper predicates: `err.is_card_declined()`, `err.is_rate_limited()`, `err.is_idempotency_conflict()`.
- Implements `std::error::Error`; `Display` never prints secrets or full payloads.

### 4.2 `payrs-stripe-types` (shared vocabulary)

- **Typed IDs:** `CustomerId`, `PaymentIntentId`, … as newtypes over `SmolStr` with `FromStr` validating the prefix (`cus_`, `pi_`). Prevents the classic "passed a price id where a product id goes" bug at compile time. All request builders accept `impl Into<CustomerId>` so `&str` still works.
- **Money:** amounts are `i64` minor units (Stripe's model) — no floats, ever. `Currency` is a `#[non_exhaustive]` enum of ISO-4217 codes with `Other(SmolStr)` fallback so new currencies never break deserialization.
- **`Expandable<T>`:**

```rust
#[serde(untagged)]
pub enum Expandable<T: Object> {
    Id(T::Id),
    Object(Box<T>),
}
// helpers: .id() -> &T::Id ; .as_object() -> Option<&T>
```

- **`Timestamp`** newtype (unix seconds) with optional `chrono`/`time` conversion features (both off by default to keep the dep tree lean).
- **`Metadata`** = `BTreeMap<String, String>` (deterministic ordering for tests).
- **`List<T>`** `{ data, has_more, url }` + internal cursor for the pagination engine.
- All response structs: `#[non_exhaustive]`, `Debug, Clone, PartialEq, Serialize, Deserialize`; unknown enum variants captured via `Other(String)` catch-alls — **an SDK must never fail to deserialize because Stripe added a value** (this is the top real-world failure mode of typed Stripe clients).

### 4.3 Tier 1 resource crates (hand-written on generated skeletons)

Per resource, the pattern is:

```
payment_intent/
├── types.rs      // PaymentIntent, PaymentIntentStatus, … (generated, reviewed)
├── create.rs     // CreatePaymentIntent builder  (hand-tuned: required args in new())
├── update.rs / retrieve.rs / list.rs / capture.rs / cancel.rs / confirm.rs
```

Builder rules (Rust API Guidelines-aligned):
- Required parameters are positional in `new()`; everything optional is a chainable method.
- Enums for every closed vocabulary (`CaptureMethod::Automatic`), never stringly-typed.
- `send(&client) -> impl Future<Output = Result<T, Error>>` — builders are inert until sent (easy to construct in tests).
- Every builder method has a rustdoc line lifted from Stripe's own field description (attribution: generated from the OpenAPI spec, which Stripe publishes for this purpose).

### 4.4 Codegen pipeline (`payrs-stripe-codegen`, internal)

- Input: Stripe's official OpenAPI spec (`stripe/openapi` repo), vendored + pinned by commit for reproducible builds.
- Output: Tier 2 crates fully generated; Tier 1 crates get generated `types.rs` + generated builder *skeletons* that are committed and then hand-polished (codegen writes to a `generated/` dir; a diff report flags upstream changes touching hand-edited files).
- Run via `cargo xtask regen`; CI job runs monthly against the pinned Stripe version line (we do **not** chase weekly spec changes — we release SDK minors aligned to Stripe's monthly backward-compatible releases, and SDK majors for Stripe's yearly breaking releases, mirroring official SDK policy).
- Generated code is committed (not build.rs codegen) → users get fast builds, docs.rs works, `cargo vendor` works, and diffs are reviewable.

### 4.5 Webhooks (`payrs-stripe-webhooks`)

- Constant-time HMAC-SHA256 comparison (`subtle` crate) of `v1` signatures; timestamp tolerance default 300s (configurable); explicit errors: `MissingHeader`, `BadFormat`, `TimestampOutOfTolerance`, `SignatureMismatch`.
- `Event` with `#[serde(tag)]`-style dispatch into typed payloads for Tier 1 events + `Unknown { type_, raw: Box<RawValue> }` for everything else — never drops data.
- Optional integrations behind features: `axum` (extractor rejecting bad signatures with 400), `actix`, `lambda` helpers.
- Docs emphasize: verify **raw body bytes** (framework body-transformation is the #1 webhook bug), return 2xx fast, process async.

---

## 5. Key Decisions (ADR summaries)

### ADR-001 — Curated tiers vs full codegen mirror
- **Options:** (a) full codegen like async-stripe; (b) fully hand-written like early stripe-rust; (c) hybrid tiers.
- **Decision:** (c). Hand-written ergonomics where developers live (payments/billing/checkout/webhooks); codegen for the long tail; raw client for gaps.
- **Consequences:** + best-in-class DX where it matters, small builds; − Tier 1 requires manual review each Stripe major; mitigated by diff-report tooling. Rejected (a): duplicates async-stripe with no differentiation; rejected (b): unmaintainable surface.

### ADR-002 — serde everywhere (no miniserde)
- async-stripe adopted miniserde to cut compile times of its *full-surface* type set. Our surface is an order of magnitude smaller (tiered), so serde's flexibility (untagged enums for `Expandable`, `RawValue` passthrough, custom deserializers) wins. Compile-time risk is controlled by crate splitting + feature flags per resource group.

### ADR-003 — reqwest+rustls default behind an `HttpTransport` trait
- **Options:** hard-code reqwest; hyper directly; generic over transport trait.
- **Decision:** trait with reqwest/rustls default (`native-tls` behind a feature). Two-way-door: users inject transports (proxies, mocking, middleware) without us maintaining N runtime feature combos on day 1. Tokio-only for 1.0 (async-std is EOL-adjacent in 2026); the trait keeps other runtimes possible later.

### ADR-004 — Pin Stripe API version per SDK major
- Follow official SDK policy: each SDK release sends a fixed `Stripe-Version` matching its types; monthly Stripe releases → SDK minor; yearly breaking Stripe release (acacia→basil→clover→dahlia cadence) → SDK major. Raw escape hatch may override the header (documented as "types not guaranteed").

### ADR-005 — Committed generated code (no build.rs generation)
- build.rs codegen (as stripe-sdk does) hurts docs.rs, IDE experience, build determinism, and audit-ability. Committing generated code trades repo size for reviewable diffs and fast user builds. One-way-door acceptable.

### ADR-006 — Facade crate + granular crates + feature flags
- Users depend on `payrs-stripe` with features (`core`, `billing`, `checkout`, `webhooks`, `connect`, `full`); the facade re-exports sub-crates. Gives both "one dependency" simplicity and pay-for-what-you-use compile times. Default features: `core`, `webhooks`, `rustls`.

---

## 6. Cross-Cutting Concerns

### 6.1 Security
- Secret keys in a `SecretKey` newtype: `Debug`/`Display` print `sk_test_***`; zeroize-on-drop behind `zeroize` feature; never serialized.
- rustls by default; TLS 1.2+ enforced; certificate verification never disableable via public API.
- No logging of request/response bodies at info level; `tracing` instrumentation (feature `tracing`) logs method, path, status, request-id, latency, retry count — bodies only at `trace` with an explicit opt-in flag, with card-adjacent fields redacted.
- Webhook verification is constant-time; docs push endpoint secrets to env/secret managers.
- `cargo deny` (licenses + advisories) and `cargo audit` in CI; dependency count budget (< 25 transitive for default features) reviewed per PR.
- No `unsafe` (`#![forbid(unsafe_code)]` in all published crates).

### 6.2 Observability
- `tracing` spans per request: `stripe.request{method, path, idempotency_key(hash), attempt}`; users bridge to OpenTelemetry themselves.
- `Response` metadata surfaced: `request_id()`, `stripe_version()`, rate-limit headers — usable for alerting.

### 6.3 Testing strategy
| Layer | Tooling | Gate |
|---|---|---|
| Unit (serialization, builders, error mapping, signature math) | `cargo test`, `insta` snapshots for form-encoding | every PR |
| Contract | `stripe-mock` (Stripe's official mock server, spec-driven) in CI via docker service | every PR |
| Wire-format fixtures | recorded real responses (test mode) as JSON fixtures, deserialization round-trips | every PR |
| Live smoke | tiny test-mode suite (create/confirm PI with test cards, webhook CLI trigger) behind `STRIPE_TEST_KEY` secret | nightly + pre-release |
| Property tests | `proptest` on: unknown-enum-variant tolerance, Expandable id/object, pagination cursoring | every PR |
| MSRV / features | `cargo hack --feature-powerset check` + MSRV job | every PR |
| Docs | doctests + `cargo doc` warnings-as-errors + link checker | every PR |

### 6.4 CI/CD (GitHub Actions)
- Matrix: stable, beta, MSRV × linux/macos/windows; clippy `-D warnings` with the workspace lint table; `rustfmt --check`; `cargo semver-checks` on release PRs to catch accidental breaking changes; `cargo public-api` diff posted as PR comment.
- Release automation: `release-plz` (changelog from conventional commits, version bump PRs, tag → publish with **crates.io Trusted Publishing / OIDC** — no long-lived tokens in CI).

---

## 7. Rust Coding Standards (enforced, not aspirational)

Aligned with the official **Rust API Guidelines** checklist; the notable commitments:

- **Naming (C-CASE, C-WORD-ORDER):** builders `CreatePaymentIntent`, methods `snake_case`, conversions `as_/to_/into_` semantics respected.
- **Interoperability (C-COMMON-TRAITS):** all public types derive `Debug, Clone`; data types add `PartialEq, Serialize, Deserialize`; IDs add `Eq, Hash, Ord, Display, FromStr`.
- **Future-proofing (C-STRUCT-PRIVATE, C-NEWTYPE):** `#[non_exhaustive]` everywhere Stripe controls the vocabulary; no public fields on client/config types.
- **Error handling:** no `unwrap`/`expect`/`panic!` in library code (clippy-denied); all fallible APIs return `Result<_, Error>`; errors are `Send + Sync + 'static`.
- **Docs (C-CRATE-DOC, C-EXAMPLE, C-FAILURE):** every public item documented; every module has a runnable example; every fallible fn documents its error cases; crate root has a 60-second quickstart.
- **Workspace lint table** (single source of truth):

```toml
[workspace.lints.rust]
missing_docs = "deny"
unsafe_code = "forbid"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
missing_errors_doc = "warn"
pedantic = { level = "warn", priority = -1 }
```

- `rustfmt.toml` committed (default style, pinned edition 2024); `deny.toml` for licenses (MIT/Apache-2.0/BSD allowlist).

---

## 8. Documentation & Developer Experience Plan

1. **README.md** (repo + crates.io front page): badges (CI, crates.io, docs.rs, MSRV, license), 30-second install + quickstart, feature-flag table, comparison note vs async-stripe (honest: "want 100% surface today → use async-stripe"), link to guide.
2. **docs.rs rustdoc:** crate-level tour; `#[doc(cfg(feature = "…"))]` so feature-gated items are labeled; all Stripe field docs carried onto struct fields.
3. **mdBook guide** (GitHub Pages): Getting Started → Payments (PaymentIntent lifecycle) → Subscriptions & Billing → Checkout → **Webhooks (with axum/actix/lambda recipes)** → Connect → Error handling & retries → Idempotency patterns for agents/automation → Testing with stripe-mock & test clocks → Migrating from async-stripe → Cookbook (SCA/3DS, saving cards, usage-based billing, refunds & disputes).
4. **examples/** — one compiling binary per guide chapter, run in CI against stripe-mock.
5. **Developer guidelines (CONTRIBUTING.md):** dev setup (`cargo xtask setup` boots stripe-mock via docker), codegen workflow, conventional commits, PR checklist (tests, docs, semver-checks green), review SLAs, RELEASING.md runbook.
6. **Governance files:** LICENSE (dual MIT/Apache-2.0 — Rust ecosystem norm), CODE_OF_CONDUCT.md, SECURITY.md (private vulnerability reporting via GitHub advisories, 90-day disclosure), issue/PR templates, MIGRATION.md per major.

---

## 9. Packaging & crates.io Publishing Plan

### 9.1 Naming & metadata
- Must not squat or confuse with `stripe`/`async-stripe`; candidates: `payrs-stripe`, `stripely`, `ferrostripe`. Check availability + trademark hygiene (Stripe's marks: name must not imply official status; README carries "not affiliated with Stripe, Inc.").
- `Cargo.toml` completeness: `description`, `keywords = ["stripe","payments","api","billing","webhooks"]`, `categories = ["api-bindings","web-programming"]`, `repository`, `documentation`, `readme`, `license = "MIT OR Apache-2.0"`, `rust-version` (MSRV), `include = [...]` to keep packages lean (< 1 MB each), `[package.metadata.docs.rs] all-features = true`.

### 9.2 Versioning policy
- Pre-1.0 line (`0.x`) during Phases 1–2; **1.0 only after**: 3 months of 0.x soak, `cargo semver-checks` clean, public-api review, ≥ 2 external production adopters.
- SemVer mapping (mirrors Stripe's official SDK policy): Stripe monthly (backward-compatible) release → SDK **minor**; Stripe yearly breaking release (e.g., dahlia → next) → SDK **major**; bug fixes → **patch**. MSRV bumps allowed in minors, documented in changelog.
- All workspace crates version-locked and released together (lockstep) to avoid cross-crate compatibility matrices.

### 9.3 Release runbook (automated via release-plz + xtask)
1. release-plz PR: version bumps + CHANGELOG (Keep-a-Changelog format).
2. CI green incl. semver-checks, feature-powerset, MSRV, stripe-mock suite, live smoke.
3. Merge → tag → Trusted Publishing (OIDC) publishes crates in dependency order (`types` → `client` → resources → facade) with `cargo publish` verification builds.
4. GitHub Release with notes; docs.rs build verified; announce (This Week in Rust, r/rust, changelog RSS).
5. Yank policy: only for security/soundness or broken builds; never for API regret.

---

## 10. Rollout Plan (phases)

| Phase | Scope | Exit criteria | Est. effort |
|---|---|---|---|
| **0 — Foundations** (wk 1–2) | Workspace, lints, CI, `client` crate (auth, retries, idempotency, errors, raw escape hatch), `types` crate, stripe-mock harness | Raw client can create a Customer against stripe-mock; CI matrix green | 2 wk |
| **1 — Payments MVP** (wk 3–6) | Tier 1: Customers, PaymentIntents, SetupIntents, PaymentMethods, Refunds; pagination streams; webhooks crate + axum extractor; examples + guide chapters 1–3 | `0.1.0` published; end-to-end demo app (checkout → webhook → fulfillment) | 4 wk |
| **2 — Billing & Checkout** (wk 7–11) | Products, Prices, Subscriptions, Invoices, Checkout Sessions, Customer Portal; test-clock testing docs; file uploads; codegen pipeline producing Tier 2 `connect` + `misc` | `0.3.0`; migration guide from async-stripe; ≥ 3 external users | 5 wk |
| **3 — Hardening → 1.0** (wk 12–18) | Connect polish, tracing feature, property tests, public-api freeze, semver-checks gate, docs completeness audit, perf/binary-size report, security review | `1.0.0` on crates.io targeting `2026-06-24.dahlia` line | 6 wk |
| **4 — Post-1.0** | `/v2` namespace + thin events (feature `v2`), OAuth/Connect standard, WASM transport, opentelemetry helper crate | driven by adoption | ongoing |

---

## 11. Risks & Open Questions

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Stripe yearly breaking release lands mid-development | High | Med | Version-pinned spec; ADR-004 policy makes it a planned major, not a fire drill |
| Deserialization breaks on new enum values in prod | Med | High | `Other(String)` catch-alls + property tests + fixture regen job (this is non-negotiable) |
| Competing with async-stripe splits ecosystem goodwill | Med | Med | Honest positioning, migration guide both directions, contribute fixes upstream where shared (e.g., spec bugs) |
| Solo-maintainer bus factor | High | High | Codegen for long tail, ruthless scope tiers, automation-first releases, recruit 1–2 co-maintainers by Phase 2 |
| Compile-time creep as Tier 2 grows | Med | Med | Per-resource feature flags, `cargo build --timings` budget in CI, facade defaults stay lean |
| crates.io name conflict / trademark complaint | Low | Med | Legal-safe naming (§9.1), explicit non-affiliation notice |

**Open questions (need decisions before Phase 1 ends):**
1. Final crate name (reserve on crates.io immediately once chosen — publish a `0.0.1` placeholder with README).
2. Do we expose a blocking client wrapper (`blocking` feature, tokio `block_on`) for scripts/CLIs? (Leaning yes, Phase 2.)
3. `chrono` vs `time` vs both for timestamp conversions? (Leaning: both, optional features.)
4. Should Tier 1 include Tax and Billing Meters (usage-based billing is big for AI products)? (Leaning yes for Meters given your AI-agent business focus.)

---

## Appendix A — Behavioral parity checklist vs official Stripe SDKs

- [x] Pinned `Stripe-Version` per release
- [x] Auto idempotency keys on POST, reused across retries
- [x] Retry on 409/429/5xx + `Stripe-Should-Retry`, exponential backoff + jitter
- [x] `X-Stripe-Client-User-Agent` telemetry (opt-out)
- [x] `Stripe-Account` header support (Connect)
- [x] Webhook signature verification w/ tolerance
- [x] Expandable fields, auto-pagination, request-id surfacing
- [ ] `/v2` namespace + thin events (Phase 4)
- [ ] File uploads (multipart) (Phase 2)

## Appendix B — Reference stack

`tokio` · `reqwest` (rustls) · `serde`/`serde_json` (+ `serde_path_to_error` for debuggable failures) · `serde_qs`/custom form encoder for Stripe's nested `a[b][0][c]` style · `uuid` v4 · `hmac`+`sha2`+`subtle` · `smol_str` · `futures-core/util` (streams) · `thiserror` · optional: `tracing`, `zeroize`, `chrono`/`time`, `axum`/`actix` glue.

---

## Implementation status (reconciled 2026-07)

| Plan phase | Item | Status |
|---|---|---|
| 0 | Workspace, lints, CI, transport core (auth/retries/idempotency/errors), raw escape hatch, stripe-mock harness | ✅ done |
| 1 | Typed operations & models | ✅ done — superseded by full codegen: 1,431 models, 587 ops, 76 sections |
| 1 | Pagination engine | ✅ done — `Paginator` with cursor threading; `.paginate()` on all 127 list endpoints |
| 2 | Codegen pipeline from official OpenAPI spec | ✅ done — `codegen/generate.py` + vendored spec + `docs/coverage.md` report |
| 2 | v2 namespace | ✅ done — JSON bodies, `Stripe-Context`, per-request version override, typed v2 core, thin events |
| 3 | Webhooks | ✅ done — constant-time verification, typed events, async `WebhookRouter`, env config |
| 3 | Dev-controllable configuration | ✅ done — `from_env` / `from_env_var` / `ClientBuilder::from_env` + explicit overrides |
| 4 | Release automation | ✅ done — `release-plz.toml`, tag-triggered Trusted-Publishing workflow; dry-run publish green |
| post-1.0 | Typed `Expandable<T>` in generated models (currently `serde_json::Value`) | 🔜 roadmap |
| post-1.0 | `tracing` instrumentation + telemetry opt-out | 🔜 roadmap |
