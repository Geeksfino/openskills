//! Skill execution with WASM sandbox and native sandbox support.
//!
//! Executes skill scripts in a WASM sandbox or, on supported platforms,
//! in a native OS-level sandbox (seatbelt/seccomp).

use crate::audit::ExecutionStatus;
use crate::errors::OpenSkillError;
use crate::native_runner::{detect_script_type, execute_native, ScriptType};
use crate::permissions::{map_tools_to_capabilities, PermissionEnforcer};
use crate::registry::Skill;
use crate::wasm_runner::execute_wasm;
use serde_json::Value;
use std::path::PathBuf;

/// Artifacts from skill execution.
pub struct ExecutionArtifacts {
    /// Output from the execution (for WASM: JSON result).
    pub output: Value,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Permissions that were used.
    pub permissions_used: Vec<String>,
    /// Exit status.
    pub exit_status: ExecutionStatus,
}

/// Options for skill execution.
#[derive(Debug, Clone, Default)]
pub struct ExecutionOptions {
    /// Override timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Override memory limit in MB.
    pub memory_mb: Option<u64>,
    /// Input data for WASM execution.
    pub input: Option<Value>,
    /// WASM module path override (relative to skill root).
    pub wasm_module: Option<String>,
}

#[derive(Debug)]
enum ExecutionMode {
    Wasm { wasm_module: String },
    Native { script_path: PathBuf, script_type: ScriptType },
}

/// Execute a skill's WASM module or native script in a sandbox.
///
/// For Claude Skills compatibility:
/// - Skills are primarily instructional (Claude follows the instructions)
/// - Script execution is sandboxed (WASM or native seatbelt/seccomp)
pub fn execute_skill(
    skill: &Skill,
    options: ExecutionOptions,
) -> Result<ExecutionArtifacts, OpenSkillError> {
    // Map allowed-tools to WASM capabilities
    let allowed_tools = skill.manifest.get_allowed_tools();
    let mut wasm_config = map_tools_to_capabilities(&allowed_tools);

    // Apply option overrides
    if let Some(timeout) = options.timeout_ms {
        wasm_config.timeout_ms = timeout;
    }
    if let Some(memory) = options.memory_mb {
        wasm_config.memory_mb = memory;
    }

    let enforcer = PermissionEnforcer::new(
        allowed_tools.clone(),
        wasm_config.clone(),
        skill.root.clone(),
    );

    // Prepare input
    let input = options.input.unwrap_or_else(|| {
        serde_json::json!({
            "skill_id": skill.id,
            "instructions": skill.instructions
        })
    });

    match detect_execution_mode(&skill.root, options.wasm_module)? {
        ExecutionMode::Wasm { wasm_module } => {
            let wasm_path = skill.root.join(&wasm_module);
            if !wasm_path.exists() {
                return Err(OpenSkillError::WasmError(format!(
                    "WASM module not found: {}",
                    wasm_path.display()
                )));
            }
            execute_wasm(
                skill,
                &wasm_module,
                input,
                wasm_config.timeout_ms,
                &enforcer,
            )
        }
        ExecutionMode::Native {
            script_path,
            script_type,
        } => execute_native(
            skill,
            &script_path,
            script_type,
            input,
            wasm_config.timeout_ms,
            &enforcer,
            &allowed_tools,
        ),
    }
}

fn detect_execution_mode(
    skill_root: &PathBuf,
    wasm_override: Option<String>,
) -> Result<ExecutionMode, OpenSkillError> {
    if let Some(wasm_module) = wasm_override {
        return Ok(ExecutionMode::Wasm { wasm_module });
    }

    if let Some(wasm_module) = find_wasm_module(skill_root) {
        return Ok(ExecutionMode::Wasm { wasm_module });
    }

    if let Some(script_path) = find_native_script(skill_root) {
        let script_type = detect_script_type(&script_path)?;
        return Ok(ExecutionMode::Native {
            script_path,
            script_type,
        });
    }

    Err(OpenSkillError::NativeExecutionError(
        "No executable artifact found (expected .wasm, .py, or .sh)".to_string(),
    ))
}

/// Find a WASM module in the skill directory.
fn find_wasm_module(skill_root: &PathBuf) -> Option<String> {
    // Look for common patterns
    let candidates = [
        "skill.wasm",
        "wasm/skill.wasm",
        "module.wasm",
        "main.wasm",
    ];

    for candidate in candidates {
        if skill_root.join(candidate).exists() {
            return Some(candidate.to_string());
        }
    }

    // Look for any .wasm file
    if let Ok(entries) = std::fs::read_dir(skill_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "wasm").unwrap_or(false) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    return Some(name.to_string());
                }
            }
        }
    }

    None
}

/// Find a native script in the skill directory.
fn find_native_script(skill_root: &PathBuf) -> Option<PathBuf> {
    let candidates = [
        "script.py",
        "main.py",
        "src/main.py",
        "index.py",
        "src/index.py",
        "script.sh",
        "main.sh",
        "src/main.sh",
        "index.sh",
        "src/index.sh",
        "script.bash",
        "main.bash",
        "src/main.bash",
        "index.bash",
        "src/index.bash",
    ];

    for candidate in candidates {
        let path = skill_root.join(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    for dir in [skill_root.to_path_buf(), skill_root.join("src"), skill_root.join("scripts")] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        let ext = ext.to_ascii_lowercase();
                        if ext == "py" || ext == "sh" || ext == "bash" {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_find_wasm_module_none() {
        let path = PathBuf::from("/nonexistent");
        assert!(find_wasm_module(&path).is_none());
    }

    #[test]
    fn test_find_native_script_none() {
        let path = PathBuf::from("/nonexistent");
        assert!(find_native_script(&path).is_none());
    }

    #[test]
    fn test_find_native_script_detects_python() {
        let temp = TempDir::new().unwrap();
        let skill_root = temp.path();
        let script_path = skill_root.join("script.py");
        std::fs::write(&script_path, "print('ok')").unwrap();

        let found = find_native_script(&skill_root.to_path_buf());
        assert_eq!(found.unwrap(), script_path);
    }
}
