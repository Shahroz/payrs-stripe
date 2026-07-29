//! Repo automation: `cargo xtask <command>`.
//! Phase 0 stub — `regen` (codegen) and `msrv` land in Phase 2.

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "stripe-mock" => {
            eprintln!(
                "Run: ./scripts/stripe-mock.sh (requires docker), then: cargo test -- --ignored"
            );
        }
        _ => {
            eprintln!("usage: cargo xtask <stripe-mock>");
            std::process::exit(2);
        }
    }
}
