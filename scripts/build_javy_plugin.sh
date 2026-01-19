#!/usr/bin/env bash
set -euo pipefail

JAVY_DIR="${1:-/tmp/javy}"
PLUGIN_OUT="${2:-}"

if [ ! -d "${JAVY_DIR}/.git" ]; then
  echo "Cloning javy repo into ${JAVY_DIR}..."
  git clone --depth 1 https://github.com/bytecodealliance/javy.git "${JAVY_DIR}"
else
  echo "Using existing javy repo at ${JAVY_DIR}"
fi

echo "Ensuring wasm32-wasip1 target is installed..."
rustup target add wasm32-wasip1

echo "Building javy plugin..."
(cd "${JAVY_DIR}" && cargo build --release --target wasm32-wasip1 -p javy-plugin)

PLUGIN_PATH="${JAVY_DIR}/target/wasm32-wasip1/release/plugin.wasm"
PLUGIN_WIZENED_PATH="${JAVY_DIR}/target/wasm32-wasip1/release/plugin_wizened.wasm"

if [ ! -f "${PLUGIN_PATH}" ]; then
  echo "Error: plugin.wasm not found at ${PLUGIN_PATH}" >&2
  exit 1
fi

echo "Initializing plugin (wizening)..."
(cd "${JAVY_DIR}" && cargo run -p javy-cli -- init-plugin "${PLUGIN_PATH}" --out "${PLUGIN_WIZENED_PATH}")

if [ ! -f "${PLUGIN_WIZENED_PATH}" ]; then
  echo "Error: plugin_wizened.wasm not found at ${PLUGIN_WIZENED_PATH}" >&2
  exit 1
fi

if [ -n "${PLUGIN_OUT}" ]; then
  echo "Copying plugin to ${PLUGIN_OUT}..."
  cp "${PLUGIN_WIZENED_PATH}" "${PLUGIN_OUT}"
fi

echo "Plugin ready:"
echo "  ${PLUGIN_WIZENED_PATH}"
echo ""
echo "Export this for OpenSkills:"
echo "  export JAVY_PLUGIN_PATH=${PLUGIN_WIZENED_PATH}"
