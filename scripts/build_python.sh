#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/bindings/python"

# Try to find maturin in common locations
MATURIN=""
if command -v maturin >/dev/null 2>&1; then
  MATURIN="maturin"
elif [ -f ~/.local/bin/maturin ]; then
  MATURIN="$HOME/.local/bin/maturin"
elif python3 -m maturin --version >/dev/null 2>&1; then
  MATURIN="python3 -m maturin"
else
  echo "maturin is required to build Python bindings" >&2
  echo "Install: pip install --user maturin" >&2
  exit 1
fi

echo "Building Python bindings (PyO3 + maturin)..."
# Use ABI3 forward compatibility for Python 3.14+
export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
$MATURIN build --release
