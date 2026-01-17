#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/bindings/python"

if ! command -v maturin >/dev/null 2>&1; then
  echo "maturin is required to build Python bindings" >&2
  echo "Install: pip install maturin" >&2
  exit 1
fi

echo "Building Python bindings (PyO3 + maturin)..."
maturin build --release
