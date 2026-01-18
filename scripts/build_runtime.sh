#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "Building Rust runtime (library + binary)..."
cargo build -p openskills-runtime --release --bins

echo "Built: $ROOT/target/release/openskills"
