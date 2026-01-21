//! AssemblyScript build plugin using asc + wasm-tools component conversion.

use crate::build::plugin::{BuildPlugin, PluginConfig};
use crate::build::plugins::adapter::{check_tool, find_adapter, generate_requirements};
use crate::errors::OpenSkillError;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AssemblyScriptBuildPlugin;

impl AssemblyScriptBuildPlugin {
    pub fn new() -> Self {
        Self
    }

    fn temp_core_wasm_path(&self, output_wasm: &Path) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let pid = std::process::id();
        let mut path = std::env::temp_dir();
        let stem = output_wasm
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("skill");
        path.push(format!("{}_as_core_{}_{}.wasm", stem, pid, ts));
        path
    }

    fn run_command(
        &self,
        label: &str,
        mut command: Command,
        verbose: bool,
    ) -> Result<(), OpenSkillError> {
        if verbose {
            eprintln!("Running: {:?}", command);
        }
        let status = command.status().map_err(|e| {
            OpenSkillError::BuildError(format!("Failed to run {}: {}", label, e))
        })?;
        if !status.success() {
            return Err(OpenSkillError::BuildError(format!(
                "{} failed with exit code {:?}",
                label,
                status.code()
            )));
        }
        Ok(())
    }
}

impl BuildPlugin for AssemblyScriptBuildPlugin {
    fn name(&self) -> &str {
        "assemblyscript"
    }

    fn description(&self) -> &str {
        "AssemblyScript compiler (asc) + wasm-tools component conversion"
    }

    fn supported_extensions(&self) -> &[&str] {
        &["ts", "as"]
    }

    fn is_available(&self) -> Result<bool, OpenSkillError> {
        // Check if required tools are installed
        let asc_ok = check_tool("asc");
        let wasm_tools_ok = check_tool("wasm-tools");
        Ok(asc_ok && wasm_tools_ok)
    }

    fn compile(
        &self,
        source_file: &Path,
        output_wasm: &Path,
        config: &PluginConfig,
    ) -> Result<(), OpenSkillError> {
        // Find adapter with auto-detection
        let adapter_path = find_adapter(config)?;

        let asc_path = config
            .custom
            .get("asc_path")
            .map(|v| v.as_str())
            .unwrap_or("asc");
        let asc_args = config
            .custom
            .get("asc_args")
            .map(|v| v.as_str())
            .unwrap_or("");
        let wasm_tools_path = config
            .custom
            .get("wasm_tools_path")
            .map(|v| v.as_str())
            .unwrap_or("wasm-tools");

        if let Some(parent) = output_wasm.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                OpenSkillError::BuildError(format!("Failed to create output directory: {}", e))
            })?;
        }

        let core_wasm = self.temp_core_wasm_path(output_wasm);
        if config.verbose {
            eprintln!(
                "AssemblyScript: compiling {} to core module {}",
                source_file.display(),
                core_wasm.display()
            );
        }

        // Step 1: Compile AssemblyScript to core WASM module
        let mut asc_cmd = Command::new(asc_path);
        asc_cmd.arg(source_file).arg("--outFile").arg(&core_wasm);
        if !asc_args.is_empty() {
            for part in asc_args.split_whitespace() {
                asc_cmd.arg(part);
            }
        }
        self.run_command("asc", asc_cmd, config.verbose)?;

        if config.verbose {
            eprintln!(
                "AssemblyScript: converting core module to WASI 0.3 component at {}",
                output_wasm.display()
            );
        }

        // Step 2: Convert core module to WASI 0.3 component
        let mut wasm_tools_cmd = Command::new(wasm_tools_path);
        wasm_tools_cmd
            .arg("component")
            .arg("new")
            .arg(&core_wasm)
            .arg("--adapt")
            .arg(format!(
                "wasi_snapshot_preview1={}",
                adapter_path.display()
            ))
            .arg("-o")
            .arg(output_wasm);
        self.run_command("wasm-tools component new", wasm_tools_cmd, config.verbose)?;

        // Clean up intermediate file
        let _ = std::fs::remove_file(&core_wasm);

        if config.verbose {
            eprintln!(
                "AssemblyScript: build complete: {}",
                output_wasm.display()
            );
        }

        Ok(())
    }

    fn requirements(&self) -> Vec<String> {
        generate_requirements(&["asc", "wasm-tools"], true)
    }
}
