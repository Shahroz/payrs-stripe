# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

While the version is `0.x`, a minor bump may contain breaking changes.

## [Unreleased]

### Added
- Per-crate documentation, developer guides under `docs/`, and runnable
  examples under `crates/payrs-stripe/examples/`.
- Models are grouped into 77 domain modules (`models::checkout`,
  `models::treasury`, …). Every type remains re-exported flat, so
  `models::Customer` resolves exactly as before.
- Test pinning both flat and grouped model import paths.
- Test pinning byte-for-byte passthrough of preview `Stripe-Version` tags
  (for example `2026-06-24.dahlia; feature_beta=v3`).

### Changed
- CI lints on a pinned toolchain, so a new clippy release cannot turn an
  unrelated change red. `-D warnings` is scoped to the lint job.

## [0.1.0] — first public release

### Added
- Transport core: retries with backoff and jitter, automatic idempotency keys
  reused across retries, structured errors carrying `Request-Id`, redacted
  secrets, pluggable `HttpTransport`.
- Full Stripe v1 surface generated from the official OpenAPI specification:
  1,431 models and 587 typed operations across 76 sections.
- v2 namespace support: JSON bodies, `Stripe-Context`, per-request version
  override, typed v2 core endpoints, thin events.
- Webhooks: constant-time signature verification with replay protection and
  secret rotation, typed events, async `WebhookRouter`.
- Cursor pagination on all 127 list endpoints.
- Configuration from code, from the environment, or a hybrid of both.

[Unreleased]: https://github.com/Shahroz/payrs-stripe/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Shahroz/payrs-stripe/releases/tag/v0.1.0
