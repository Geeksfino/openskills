#!/usr/bin/env bash
set -euo pipefail

# OpenSkills Build Tools Setup
# Downloads and configures dependencies for quickjs and assemblyscript plugins.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE_DIR="${HOME}/.cache/openskills"
ADAPTER_VERSION="${ADAPTER_VERSION:-25.0.1}"
ADAPTER_URL="https://github.com/bytecodealliance/wasmtime/releases/download/v${ADAPTER_VERSION}/wasi_snapshot_preview1.command.wasm"

echo "======================================"
echo "OpenSkills Build Tools Setup"
echo "======================================"
echo ""

# Create cache directory
mkdir -p "${CACHE_DIR}"
echo "Cache directory: ${CACHE_DIR}"
echo ""

# Download WASI preview1 adapter if missing
ADAPTER_PATH="${CACHE_DIR}/wasi_preview1_adapter.wasm"
if [ ! -f "${ADAPTER_PATH}" ]; then
    echo "Downloading WASI preview1 adapter (v${ADAPTER_VERSION})..."
    if command -v curl >/dev/null 2>&1; then
        curl -L -o "${ADAPTER_PATH}" "${ADAPTER_URL}"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "${ADAPTER_PATH}" "${ADAPTER_URL}"
    else
        echo "❌ Error: curl or wget required to download adapter" >&2
        exit 1
    fi
    echo "✅ Adapter downloaded: ${ADAPTER_PATH}"
else
    echo "✅ Adapter already exists: ${ADAPTER_PATH}"
fi

echo ""
echo "Checking build tools..."
echo ""

# Install/check for javy CLI
JAVY_STATUS="❌"
JAVY_PATH=""
if command -v javy >/dev/null 2>&1; then
    JAVY_STATUS="✅"
    JAVY_PATH="$(which javy)"
    echo "✅ javy CLI found: ${JAVY_PATH}"
else
    echo "📦 Installing javy CLI..."
    
    # Detect OS and architecture
    OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
    ARCH="$(uname -m)"
    
    # Map architecture names to javy binary naming
    case "${ARCH}" in
        x86_64|amd64) JAVY_ARCH="x86_64" ;;
        arm64|aarch64) JAVY_ARCH="arm" ;;
        *) JAVY_ARCH="unknown" ;;
    esac
    
    # Map OS names
    case "${OS}" in
        darwin) JAVY_OS="macos" ;;
        linux) JAVY_OS="linux" ;;
        *) JAVY_OS="unknown" ;;
    esac
    
    JAVY_VERSION="8.0.0"
    JAVY_BIN_DIR="${HOME}/.local/bin"
    mkdir -p "${JAVY_BIN_DIR}"
    JAVY_BINARY_PATH="${JAVY_BIN_DIR}/javy"
    
    # Try to download pre-built binary first (much faster)
    if [ "${JAVY_OS}" != "unknown" ] && [ "${JAVY_ARCH}" != "unknown" ]; then
        JAVY_BINARY_NAME="javy-${JAVY_ARCH}-${JAVY_OS}-v${JAVY_VERSION}.gz"
        JAVY_URL="https://github.com/bytecodealliance/javy/releases/download/v${JAVY_VERSION}/${JAVY_BINARY_NAME}"
        
        echo "   Downloading pre-built binary for ${OS}-${ARCH}..."
        DOWNLOAD_SUCCESS=false
        
        if command -v curl >/dev/null 2>&1; then
            if curl -L -f -o "${JAVY_BINARY_PATH}.gz" "${JAVY_URL}" 2>/dev/null; then
                DOWNLOAD_SUCCESS=true
            fi
        elif command -v wget >/dev/null 2>&1; then
            if wget -O "${JAVY_BINARY_PATH}.gz" "${JAVY_URL}" 2>/dev/null; then
                DOWNLOAD_SUCCESS=true
            fi
        fi
        
        if [ "${DOWNLOAD_SUCCESS}" = "true" ]; then
            # Decompress the gzipped binary
            if command -v gunzip >/dev/null 2>&1; then
                gunzip -f "${JAVY_BINARY_PATH}.gz" || {
                    echo "   Failed to decompress binary, falling back to source build..."
                    rm -f "${JAVY_BINARY_PATH}.gz"
                    DOWNLOAD_SUCCESS=false
                }
            else
                echo "   gunzip not found, falling back to source build..."
                rm -f "${JAVY_BINARY_PATH}.gz"
                DOWNLOAD_SUCCESS=false
            fi
            
            if [ "${DOWNLOAD_SUCCESS}" = "true" ] && [ -f "${JAVY_BINARY_PATH}" ]; then
                chmod +x "${JAVY_BINARY_PATH}"
                # Add to PATH if not already there
                if [[ ":$PATH:" != *":${JAVY_BIN_DIR}:"* ]]; then
                    export PATH="${JAVY_BIN_DIR}:${PATH}"
                fi
                if command -v javy >/dev/null 2>&1; then
                    JAVY_STATUS="✅"
                    JAVY_PATH="$(which javy)"
                    echo "✅ javy CLI installed from binary: ${JAVY_PATH}"
                fi
            fi
        fi
    fi
    
    # Fallback to building from source if binary download failed
    if [ "${JAVY_STATUS}" != "✅" ]; then
        echo "   Pre-built binary not available, building from source (this may take a few minutes)..."
        JAVY_REPO="/tmp/javy"
        if [ ! -d "${JAVY_REPO}" ]; then
            echo "   Cloning javy repository..."
            git clone https://github.com/bytecodealliance/javy.git "${JAVY_REPO}" || {
                echo "❌ Failed to clone javy repository" >&2
                exit 1
            }
        fi
        echo "   Building javy CLI..."
        (cd "${JAVY_REPO}" && cargo install --path crates/cli) || {
            echo "❌ Failed to install javy CLI" >&2
            exit 1
        }
        if command -v javy >/dev/null 2>&1; then
            JAVY_STATUS="✅"
            JAVY_PATH="$(which javy)"
            echo "✅ javy CLI installed from source: ${JAVY_PATH}"
        else
            echo "❌ javy CLI installation completed but not found in PATH" >&2
            exit 1
        fi
    fi
fi

# Install/check for wasm-tools
WASM_TOOLS_STATUS="❌"
WASM_TOOLS_PATH=""
if command -v wasm-tools >/dev/null 2>&1; then
    WASM_TOOLS_STATUS="✅"
    WASM_TOOLS_PATH="$(which wasm-tools)"
    echo "✅ wasm-tools found: ${WASM_TOOLS_PATH}"
else
    echo "📦 Installing wasm-tools..."
    cargo install wasm-tools || {
        echo "❌ Failed to install wasm-tools" >&2
        exit 1
    }
    if command -v wasm-tools >/dev/null 2>&1; then
        WASM_TOOLS_STATUS="✅"
        WASM_TOOLS_PATH="$(which wasm-tools)"
        echo "✅ wasm-tools installed: ${WASM_TOOLS_PATH}"
    else
        echo "❌ wasm-tools installation completed but not found in PATH" >&2
        exit 1
    fi
fi

# Check for AssemblyScript (optional, only check, don't auto-install)
ASC_STATUS="❌"
ASC_PATH=""
if command -v asc >/dev/null 2>&1; then
    ASC_STATUS="✅"
    ASC_PATH="$(which asc)"
    echo "✅ AssemblyScript (asc) found: ${ASC_PATH}"
else
    echo "ℹ️  AssemblyScript (asc) not found (optional)"
    echo "   Install manually: npm install -g assemblyscript"
fi

echo ""
echo "======================================"
echo "Setup Summary"
echo "======================================"
echo ""
echo "Adapter:     ${ADAPTER_PATH}"
echo "javy:        ${JAVY_STATUS} ${JAVY_PATH}"
echo "wasm-tools:  ${WASM_TOOLS_STATUS} ${WASM_TOOLS_PATH}"
echo "asc:         ${ASC_STATUS} ${ASC_PATH}"
echo ""

# Generate config file example
CONFIG_EXAMPLE="${CACHE_DIR}/openskills.toml.example"
cat > "${CONFIG_EXAMPLE}" << EOF
# Example .openskills.toml configuration
# Copy this to your skill directory as .openskills.toml

[build]
# Choose: "javy", "quickjs", or "assemblyscript"
plugin = "quickjs"

[build.plugin_options]
# Path to WASI preview1 adapter (required for quickjs/assemblyscript)
adapter_path = "${ADAPTER_PATH}"
# Optional: override tool paths if not on PATH
# javy_path = "${JAVY_PATH:-/path/to/javy}"
# wasm_tools_path = "${WASM_TOOLS_PATH:-/path/to/wasm-tools}"
# asc_path = "${ASC_PATH:-/path/to/asc}"
EOF

echo "Example config saved: ${CONFIG_EXAMPLE}"
echo ""
echo "Quick Start:"
echo "  1. Copy config to skill: cp ${CONFIG_EXAMPLE} my-skill/.openskills.toml"
echo "  2. Build skill: openskills build my-skill"
echo ""
echo "Or use CLI flags:"
echo "  openskills build --plugin quickjs --plugin-option adapter_path=${ADAPTER_PATH}"
echo ""

# Set environment variable hint
echo "Or set environment variable (add to ~/.bashrc or ~/.zshrc):"
echo "  export WASI_ADAPTER_PATH=${ADAPTER_PATH}"
echo ""
