#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/bindings/ts"

if ! command -v npm >/dev/null 2>&1; then
  echo "npm is required to build TS bindings" >&2
  exit 1
fi

echo "Building TypeScript bindings (napi-rs)..."
npm install
npm run build
