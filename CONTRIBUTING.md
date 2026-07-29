# Contributing

## Setup

- Rust ≥ 1.89 (MSRV, driven by smol_str + icu deps), stable recommended
- Docker (for stripe-mock contract tests)

```bash
cargo test --workspace            # unit + behavioral tests (no network)
./scripts/stripe-mock.sh          # in another terminal
cargo test -- --ignored           # contract tests against stripe-mock
cargo clippy --workspace --all-targets --all-features
cargo fmt --all
```

## Ground rules

- No `unwrap`/`expect`/`panic!` in library code (clippy denies them); tests may allow locally.
- Every public item is documented (`missing_docs` is deny) with error cases noted.
- Response-shaped types are `#[non_exhaustive]`; enums Stripe owns get an `Other` catch-all — deserialization must never fail on data Stripe actually sent.
- One idempotency key per logical request, fixed before the first attempt.
- Conventional commits (`feat:`, `fix:`, `docs:`…) — the changelog is generated.

## PR checklist

- [ ] Tests added/updated (behavioral tests use the mock transport)
- [ ] `cargo clippy`/`fmt`/`doc` clean
- [ ] Public API changes: note SemVer impact in the PR description
