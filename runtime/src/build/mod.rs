//! Build tool for compiling TypeScript/JavaScript skills to WASM components.
//!
//! Supports:
//! - TypeScript (.ts) → transpile to JS → compile to WASM
//! - JavaScript (.js) → compile to WASM
//!
//! Build backends are plugin-based so developers can choose compilers.

use crate::errors::OpenSkillError;
use crate::build::config::BuildConfigFile;
use crate::build::plugin::PluginConfig;
use crate::build::registry::PluginRegistry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod config;
pub mod plugin;
pub mod plugins;
pub mod registry;

/// List all available build plugins.
pub fn list_build_plugins() -> Vec<plugin::PluginInfo> {
    PluginRegistry::new().list()
}

/// Build configuration for skill compilation.
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// Skill directory path.
    pub skill_dir: PathBuf,
    /// Source file path (auto-detected if None).
    pub source_file: Option<PathBuf>,
    /// Output WASM file path (default: wasm/skill.wasm).
    pub output_file: Option<PathBuf>,
    /// Force rebuild even if output exists.
    pub force: bool,
    /// Verbose output.
    pub verbose: bool,
    /// Build plugin to use (None = auto-detect).
    pub plugin: Option<String>,
    /// Plugin-specific configuration.
    pub plugin_config: HashMap<String, String>,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            skill_dir: PathBuf::from("."),
            source_file: None,
            output_file: None,
            force: false,
            verbose: false,
            plugin: None,
            plugin_config: HashMap::new(),
        }
    }
}

/// Detect source file in skill directory.
pub fn detect_source_file(skill_dir: &Path) -> Result<PathBuf, OpenSkillError> {
    let candidates = [
        skill_dir.join("src/index.ts"),
        skill_dir.join("src/index.js"),
        skill_dir.join("index.ts"),
        skill_dir.join("index.js"),
        skill_dir.join("src/main.ts"),
        skill_dir.join("src/main.js"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    Err(OpenSkillError::BuildError(format!(
        "No source file found in {}. Expected one of: src/index.ts, src/index.js, index.ts, index.js",
        skill_dir.display()
    )))
}

/// Transpile TypeScript to JavaScript.
pub fn transpile_typescript(
    ts_file: &Path,
    output_js: &Path,
    verbose: bool,
) -> Result<(), OpenSkillError> {
    // Validate input paths to prevent command injection
    if !ts_file.exists() {
        return Err(OpenSkillError::BuildError(format!(
            "TypeScript file not found: {}",
            ts_file.display()
        )));
    }
    
    // Sanitize paths for command line use
    let ts_file_str = ts_file.to_str().ok_or_else(|| {
        OpenSkillError::BuildError("Invalid TypeScript file path encoding".to_string())
    })?;
    let output_js_str = output_js.to_str().ok_or_else(|| {
        OpenSkillError::BuildError("Invalid output file path encoding".to_string())
    })?;

    // Try esbuild first (faster)
    if Command::new("npx")
        .args(["-y", "esbuild", "--version"])
        .output()
        .is_ok()
    {
        if verbose {
            eprintln!("Using esbuild for TypeScript transpilation");
        }

        // Use Command with separate arguments instead of shell string to prevent injection
        let status = Command::new("npx")
            .arg("-y")
            .arg("esbuild")
            .arg(ts_file_str)
            .arg("--bundle")
            .arg("--format=esm")
            .arg("--target=es2020")
            .arg("--outfile")
            .arg(output_js_str)
            .status()
            .map_err(|e| OpenSkillError::BuildError(format!("Failed to run esbuild: {}", e)))?;

        if !status.success() {
            return Err(OpenSkillError::BuildError(
                "TypeScript transpilation failed with esbuild".to_string(),
            ));
        }

        return Ok(());
    }

    // Fallback to tsc
    if Command::new("tsc").arg("--version").output().is_ok() {
        if verbose {
            eprintln!("Using tsc for TypeScript transpilation");
        }

        // Create temporary tsconfig.json if needed
        let tsconfig = ts_file.parent().unwrap().join("tsconfig.json");
        if !tsconfig.exists() {
            let default_tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "node",
    "esModuleInterop": true,
    "skipLibCheck": true,
    "strict": true
  }
}"#;
            std::fs::write(&tsconfig, default_tsconfig).map_err(|e| {
                OpenSkillError::BuildError(format!("Failed to create tsconfig.json: {}", e))
            })?;
        }

        let status = Command::new("tsc")
            .arg(ts_file.to_string_lossy().to_string())
            .arg("--outDir")
            .arg(output_js.parent().unwrap().to_string_lossy().to_string())
            .status()
            .map_err(|e| OpenSkillError::BuildError(format!("Failed to run tsc: {}", e)))?;

        if !status.success() {
            return Err(OpenSkillError::BuildError(
                "TypeScript transpilation failed with tsc".to_string(),
            ));
        }

        // tsc outputs to the same directory with .js extension
        let tsc_output = ts_file.with_extension("js");
        if tsc_output.exists() && tsc_output != output_js {
            std::fs::rename(&tsc_output, output_js).map_err(|e| {
                OpenSkillError::BuildError(format!("Failed to move tsc output: {}", e))
            })?;
        }

        return Ok(());
    }

    Err(OpenSkillError::BuildError(
        "No TypeScript compiler found. Install one of:\n  - npm install -g typescript (tsc)\n  - npm install -g esbuild (esbuild)".to_string(),
    ))
}

/// Build a skill from source to WASM using the selected plugin.
pub fn build_skill(config: BuildConfig) -> Result<PathBuf, OpenSkillError> {
    let skill_dir = config.skill_dir.canonicalize().map_err(|e| {
        OpenSkillError::BuildError(format!("Failed to resolve skill directory: {}", e))
    })?;
    let file_config = BuildConfigFile::load(&skill_dir)?;

    // Detect source file
    let source_file = match config.source_file {
        Some(f) => f,
        None => detect_source_file(&skill_dir)?,
    };

    if !source_file.exists() {
        return Err(OpenSkillError::BuildError(format!(
            "Source file not found: {}",
            source_file.display()
        )));
    }

    // Determine output path
    let output_wasm = config
        .output_file
        .unwrap_or_else(|| skill_dir.join("wasm/skill.wasm"));

    // Check if rebuild is needed
    if !config.force && output_wasm.exists() {
        let source_mtime = std::fs::metadata(&source_file)
            .and_then(|m| m.modified())
            .ok();
        let output_mtime = std::fs::metadata(&output_wasm)
            .and_then(|m| m.modified())
            .ok();

        if let (Some(src), Some(out)) = (source_mtime, output_mtime) {
            if src <= out {
                if config.verbose {
                    eprintln!("Skipping build: {} is up to date", output_wasm.display());
                }
                return Ok(output_wasm);
            }
        }
    }

    // Determine if we need TypeScript transpilation
    let js_file = if source_file.extension().and_then(|s| s.to_str()) == Some("ts") {
        if config.verbose {
            eprintln!("Transpiling TypeScript: {}", source_file.display());
        }

        // Create temporary JS file in same directory
        let temp_js = source_file.with_extension("js");
        transpile_typescript(&source_file, &temp_js, config.verbose)?;

        temp_js
    } else {
        source_file.clone()
    };

    // Resolve build plugin
    let registry = PluginRegistry::new();
    let selected_plugin = match config.plugin.as_ref().or(file_config.plugin_ref()) {
        Some(name) => registry.find(name).ok_or_else(|| {
            let available = registry
                .list()
                .into_iter()
                .map(|info| info.name)
                .collect::<Vec<_>>()
                .join(", ");
            OpenSkillError::BuildError(format!(
                "Build plugin '{}' not found. Available: {}",
                name,
                if available.is_empty() { "(none)".to_string() } else { available }
            ))
        })?,
        None => {
            let ext = js_file
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("js");
            let candidates = registry.find_by_extension(ext);
            candidates
                .into_iter()
                .find(|plugin| plugin.is_available().unwrap_or(false))
                .ok_or_else(|| {
                    OpenSkillError::BuildError(format!(
                        "No available build plugin found for .{} sources",
                        ext
                    ))
                })?
        }
    };

    if !selected_plugin.is_available()? {
        let requirements = selected_plugin
            .requirements()
            .into_iter()
            .map(|req| format!("  - {}", req))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(OpenSkillError::BuildError(format!(
            "Build plugin '{}' is not available.\nRequirements:\n{}",
            selected_plugin.name(),
            requirements
        )));
    }

    // Compile JS to WASM using plugin
    if config.verbose {
        eprintln!(
            "Compiling to WASM with plugin '{}': {}",
            selected_plugin.name(),
            output_wasm.display()
        );
    }

    let plugin_config = PluginConfig {
        verbose: config.verbose,
        force: config.force,
        custom: {
            let mut merged = file_config.plugin_options();
            merged.extend(config.plugin_config.clone());
            merged
        },
    };

    selected_plugin.compile(&js_file, &output_wasm, &plugin_config)?;

    // Clean up temporary JS file if it was created from TS
    if source_file.extension().and_then(|s| s.to_str()) == Some("ts") {
        if js_file.exists() && js_file != source_file {
            let _ = std::fs::remove_file(&js_file);
        }
    }

    if config.verbose {
        eprintln!("Build successful: {}", output_wasm.display());
    }

    Ok(output_wasm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_source_file() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path();

        // No source file
        assert!(detect_source_file(skill_dir).is_err());

        // Create src/index.ts
        std::fs::create_dir_all(skill_dir.join("src")).unwrap();
        std::fs::write(skill_dir.join("src/index.ts"), "console.log('hello');").unwrap();
        assert!(detect_source_file(skill_dir).is_ok());
    }
}
