//! Shared adapter utilities for plugins that need WASI preview1 → component conversion.

#![allow(dead_code)] // Functions used by quickjs/assemblyscript plugins when features enabled

use crate::build::plugin::PluginConfig;
use crate::errors::OpenSkillError;
use std::path::PathBuf;
use std::process::Command;

/// Default adapter version to download.
const DEFAULT_ADAPTER_VERSION: &str = "25.0.1";

/// Find the WASI preview1 adapter from various sources.
///
/// Search order:
/// 1. Explicit config option (`adapter_path`)
/// 2. Environment variable (`WASI_ADAPTER_PATH`)
/// 3. Common cache locations
/// 4. Auto-download to cache (if enabled)
/// 5. Return helpful error with setup instructions
pub fn find_adapter(config: &PluginConfig) -> Result<PathBuf, OpenSkillError> {
    // 1. Check explicit config option
    if let Some(path) = config.custom.get("adapter_path") {
        let p = PathBuf::from(expand_tilde(path));
        if p.exists() {
            return Ok(p);
        }
        // Config specified but file doesn't exist - warn but continue searching
    }

    // 2. Check environment variable
    if let Ok(path) = std::env::var("WASI_ADAPTER_PATH") {
        let p = PathBuf::from(expand_tilde(&path));
        if p.exists() {
            return Ok(p);
        }
    }

    // 3. Check common locations
    let candidates = get_common_adapter_locations();
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    // 4. Try to auto-download (unless disabled)
    let auto_download = config
        .custom
        .get("no_auto_download")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if !auto_download {
        if let Ok(path) = download_adapter(config.verbose) {
            return Ok(path);
        }
    }

    // 5. Return helpful error
    Err(adapter_not_found_error())
}

/// Get the default cache path for the adapter.
fn get_cache_adapter_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|c| c.join("openskills/wasi_preview1_adapter.wasm"))
}

/// Download the WASI preview1 adapter to the cache directory.
fn download_adapter(verbose: bool) -> Result<PathBuf, OpenSkillError> {
    let cache_path = get_cache_adapter_path().ok_or_else(|| {
        OpenSkillError::BuildError("Cannot determine cache directory for adapter".to_string())
    })?;

    // Create cache directory if needed
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            OpenSkillError::BuildError(format!("Failed to create cache directory: {}", e))
        })?;
    }

    let version =
        std::env::var("WASI_ADAPTER_VERSION").unwrap_or_else(|_| DEFAULT_ADAPTER_VERSION.to_string());
    let url = format!(
        "https://github.com/bytecodealliance/wasmtime/releases/download/v{}/wasi_snapshot_preview1.command.wasm",
        version
    );

    if verbose {
        eprintln!("Downloading WASI adapter from: {}", url);
        eprintln!("Destination: {}", cache_path.display());
    }

    // Try curl first, then wget
    let result = if check_tool("curl") {
        Command::new("curl")
            .arg("-L")
            .arg("-o")
            .arg(&cache_path)
            .arg(&url)
            .output()
    } else if check_tool("wget") {
        Command::new("wget")
            .arg("-O")
            .arg(&cache_path)
            .arg(&url)
            .output()
    } else {
        return Err(OpenSkillError::BuildError(
            "Neither curl nor wget available to download adapter".to_string(),
        ));
    };

    match result {
        Ok(output) if output.status.success() => {
            if cache_path.exists() {
                if verbose {
                    eprintln!("✅ Adapter downloaded successfully");
                }
                Ok(cache_path)
            } else {
                Err(OpenSkillError::BuildError(
                    "Download appeared to succeed but adapter file not found".to_string(),
                ))
            }
        }
        Ok(output) => Err(OpenSkillError::BuildError(format!(
            "Failed to download adapter: {}",
            String::from_utf8_lossy(&output.stderr)
        ))),
        Err(e) => Err(OpenSkillError::BuildError(format!(
            "Failed to run download command: {}",
            e
        ))),
    }
}

/// Get common locations where the adapter might be found.
fn get_common_adapter_locations() -> Vec<PathBuf> {
    let mut locations = Vec::new();

    // OpenSkills cache directory
    if let Some(cache) = dirs::cache_dir() {
        locations.push(cache.join("openskills/wasi_preview1_adapter.wasm"));
    }

    // Home directory cache
    if let Some(home) = dirs::home_dir() {
        locations.push(home.join(".cache/openskills/wasi_preview1_adapter.wasm"));
        // wasmtime installation locations
        locations.push(home.join(".wasmtime/wasi_snapshot_preview1.command.wasm"));
        locations.push(home.join(".wasmtime/wasi_snapshot_preview1.wasm"));
    }

    // Current directory
    locations.push(PathBuf::from("wasi_preview1_adapter.wasm"));
    locations.push(PathBuf::from("wasi_snapshot_preview1.wasm"));
    locations.push(PathBuf::from("wasi_snapshot_preview1.command.wasm"));

    locations
}

/// Expand ~ to home directory.
fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return path.replacen("~", &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

/// Generate helpful error message when adapter is not found.
fn adapter_not_found_error() -> OpenSkillError {
    let cache_path = dirs::cache_dir()
        .map(|c| c.join("openskills/wasi_preview1_adapter.wasm"))
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "~/.cache/openskills/wasi_preview1_adapter.wasm".to_string());

    OpenSkillError::BuildError(format!(
        r#"WASI preview1 adapter not found.

The adapter is required to convert core WASM modules to WASI 0.3 components.

Quick Setup (recommended):
  ./scripts/setup_build_tools.sh

Manual Setup:
  1. Download the adapter:
     curl -L -o ~/.cache/openskills/wasi_preview1_adapter.wasm \
       https://github.com/bytecodealliance/wasmtime/releases/download/v25.0.1/wasi_snapshot_preview1.command.wasm

  2. Then use one of:
     a) Set environment variable:
        export WASI_ADAPTER_PATH={}

     b) Use CLI option:
        openskills build --plugin-option adapter_path={}

     c) Add to .openskills.toml:
        [build.plugin_options]
        adapter_path = "{}"
"#,
        cache_path, cache_path, cache_path
    ))
}

/// Check if required CLI tools are available.
pub fn check_tool(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--version")
        .output()
        .is_ok()
}

/// Get installation instructions for a tool.
pub fn tool_install_instructions(tool: &str) -> String {
    match tool {
        "javy" => {
            "Install javy CLI:\n  \
             git clone https://github.com/bytecodealliance/javy.git /tmp/javy\n  \
             cd /tmp/javy && cargo install --path crates/cli"
                .to_string()
        }
        "wasm-tools" => "Install wasm-tools:\n  cargo install wasm-tools".to_string(),
        "asc" => "Install AssemblyScript compiler:\n  npm install -g assemblyscript".to_string(),
        _ => format!("Install {}", tool),
    }
}

/// Generate requirements list based on missing tools.
pub fn generate_requirements(tools: &[&str], need_adapter: bool) -> Vec<String> {
    let mut reqs = Vec::new();

    for tool in tools {
        if !check_tool(tool) {
            reqs.push(tool_install_instructions(tool));
        }
    }

    if need_adapter {
        reqs.push(
            "Run ./scripts/setup_build_tools.sh to download the WASI adapter, \
             or provide --plugin-option adapter_path=..."
                .to_string(),
        );
    }

    reqs
}
