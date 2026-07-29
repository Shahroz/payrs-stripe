#!/usr/bin/env bash
# Boots Stripe's official spec-driven mock server for contract tests.
# Usage: ./scripts/stripe-mock.sh   then: cargo test -- --ignored
set -euo pipefail
docker run --rm -it -p 12111-12112:12111-12112 stripe/stripe-mock:latest
