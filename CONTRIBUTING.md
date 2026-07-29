# Contributing

## Setup

- Rust ≥ 1.89 (MSRV, driven by smol_str + icu deps)
- **Rust 1.91 for linting** — CI pins clippy/rustfmt/rustdoc to this version
  (`LINT_TOOLCHAIN` in `.github/workflows/ci.yml`) so lint results are
  reproducible. New clippy releases add lints; pinning means a toolchain bump
  is a deliberate PR, not a surprise red build on an unrelated change.
  Locally: `rustup toolchain install 1.91` then `cargo +1.91 clippy …`.
- Docker (for stripe-mock contract tests)

```bash
cargo test --workspace            # unit + behavioral tests (no network)
./scripts/stripe-mock.sh          # in another terminal
cargo test -- --ignored           # contract tests against stripe-mock
cargo +1.91 clippy --workspace --all-targets --all-features -- -D warnings
cargo +1.91 fmt --all
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
