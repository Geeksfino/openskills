//! Build tool for compiling TypeScript/JavaScript skills to WASM components.
//!
//! Supports:
//! - TypeScript (.ts) → transpile to JS → compile to WASM
//! - JavaScript (.js) → compile to WASM
//!
//! Uses javy-codegen library for JS→WASM compilation.

use crate::errors::OpenSkillError;
use std::path::{Path, PathBuf};
use std::process::Command;
use javy_codegen::{Generator, JS, LinkingKind, Plugin, SourceEmbedding};

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
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            skill_dir: PathBuf::from("."),
            source_file: None,
            output_file: None,
            force: false,
            verbose: false,
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

// Note: javy-codegen is now a library dependency, so no need to check for CLI installation

/// Transpile TypeScript to JavaScript.
pub fn transpile_typescript(
    ts_file: &Path,
    output_js: &Path,
    verbose: bool,
) -> Result<(), OpenSkillError> {
    // Try esbuild first (faster)
    if Command::new("npx")
        .args(["-y", "esbuild", "--version"])
        .output()
        .is_ok()
    {
        if verbose {
            eprintln!("Using esbuild for TypeScript transpilation");
        }

        let status = Command::new("npx")
            .args([
                "-y",
                "esbuild",
                &ts_file.to_string_lossy().to_string(),
                "--bundle",
                "--format=esm",
                "--target=es2020",
                &format!("--outfile={}", output_js.display()),
            ])
            .status()
            .map_err(|e| {
                OpenSkillError::BuildError(format!("Failed to run esbuild: {}", e))
            })?;

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
                OpenSkillError::BuildError(format!(
                    "Failed to create tsconfig.json: {}",
                    e
                ))
            })?;
        }

        let status = Command::new("tsc")
            .arg(ts_file.to_string_lossy().to_string())
            .arg("--outDir")
            .arg(output_js.parent().unwrap().to_string_lossy().to_string())
            .status()
            .map_err(|e| {
                OpenSkillError::BuildError(format!("Failed to run tsc: {}", e))
            })?;

        if !status.success() {
            return Err(OpenSkillError::BuildError(
                "TypeScript transpilation failed with tsc".to_string(),
            ));
        }

        // tsc outputs to the same directory with .js extension
        let tsc_output = ts_file.with_extension("js");
        if tsc_output.exists() && tsc_output != output_js {
            std::fs::rename(&tsc_output, output_js).map_err(|e| {
                OpenSkillError::BuildError(format!(
                    "Failed to move tsc output: {}",
                    e
                ))
            })?;
        }

        return Ok(());
    }

    Err(OpenSkillError::BuildError(
        "No TypeScript compiler found. Install one of:\n  - npm install -g typescript (tsc)\n  - npm install -g esbuild (esbuild)".to_string(),
    ))
}

/// Compile JavaScript to WASM component using javy-codegen library.
pub fn compile_js_to_wasm(
    js_file: &Path,
    output_wasm: &Path,
    verbose: bool,
) -> Result<(), OpenSkillError> {
    // Ensure output directory exists
    if let Some(parent) = output_wasm.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            OpenSkillError::BuildError(format!(
                "Failed to create output directory: {}",
                e
            ))
        })?;
    }

    if verbose {
        eprintln!("Compiling {} to {}", js_file.display(), output_wasm.display());
    }

    // Read JavaScript source
    let js = JS::from_file(js_file).map_err(|e| {
        OpenSkillError::BuildError(format!(
            "Failed to read JavaScript file {}: {}",
            js_file.display(),
            e
        ))
    })?;

    // Load the javy plugin
    // The plugin can be provided via JAVY_PLUGIN_PATH environment variable,
    // or we'll try to find it in common locations
    let plugin = {
        let plugin_path = std::env::var("JAVY_PLUGIN_PATH")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                // Try common locations
                let candidates = [
                    PathBuf::from("plugin_wizened.wasm"),
                    PathBuf::from("plugin.wasm"),
                    PathBuf::from("../javy/target/wasm32-wasip1/release/plugin_wizened.wasm"),
                    PathBuf::from("../javy/target/wasm32-wasip1/release/plugin.wasm"),
                    PathBuf::from("~/.cargo/bin/plugin_wizened.wasm"),
                    PathBuf::from("~/.cargo/bin/plugin.wasm"),
                ];
                candidates.iter().find(|p| p.exists()).cloned()
            });

        match plugin_path {
            Some(path) => Plugin::new_from_path(&path).map_err(|e| {
                OpenSkillError::BuildError(format!(
                    "Failed to load javy plugin from {}: {}",
                    path.display(), e
                ))
            })?,
            None => {
                return Err(OpenSkillError::BuildError(
                    "javy plugin not found. javy-codegen requires a plugin.wasm file.\n\
                    Options:\n  \
                    1. Set JAVY_PLUGIN_PATH environment variable to point to plugin.wasm\n  \
                    2. Place plugin_wizened.wasm in the current directory\n  \
                    3. Run scripts/build_javy_plugin.sh (recommended)\n  \
                    4. Build the plugin: git clone https://github.com/bytecodealliance/javy.git && \
                       cd javy && cargo build --release --target wasm32-wasip1 -p javy-plugin".to_string(),
                ));
            }
        }
    };

    // Create generator with default configuration
    let mut generator = Generator::new(plugin);
    generator
        .source_embedding(SourceEmbedding::Compressed)
        .linking(LinkingKind::Static);

    // Generate WASM (synchronous operation)
    let wasm = generator.generate(&js).map_err(|e| {
        OpenSkillError::BuildError(format!(
            "Failed to generate WASM from JavaScript: {}",
            e
        ))
    })?;

    // Write WASM to output file
    std::fs::write(output_wasm, wasm).map_err(|e| {
        OpenSkillError::BuildError(format!(
            "Failed to write WASM output to {}: {}",
            output_wasm.display(),
            e
        ))
    })?;

    Ok(())
}

/// Build a skill from source to WASM.
pub fn build_skill(config: BuildConfig) -> Result<PathBuf, OpenSkillError> {
    let skill_dir = config.skill_dir.canonicalize().map_err(|e| {
        OpenSkillError::BuildError(format!(
            "Failed to resolve skill directory: {}",
            e
        ))
    })?;

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
    let output_wasm = config.output_file.unwrap_or_else(|| {
        skill_dir.join("wasm/skill.wasm")
    });

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
                    eprintln!(
                        "Skipping build: {} is up to date",
                        output_wasm.display()
                    );
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

    // Compile JS to WASM
    if config.verbose {
        eprintln!("Compiling to WASM: {}", output_wasm.display());
    }
    compile_js_to_wasm(&js_file, &output_wasm, config.verbose)?;

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
        std::fs::write(skill_dir.join("src/index.ts"), "console.log('hello');")
            .unwrap();
        assert!(detect_source_file(skill_dir).is_ok());
    }
}