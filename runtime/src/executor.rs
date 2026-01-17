//! Skill execution with WASM sandbox.
//!
//! Executes skill scripts in a WASM sandbox instead of native execution
//! with OS-level sandboxing (seatbelt/seccomp).

use crate::audit::ExecutionStatus;
use crate::errors::OpenSkillError;
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

/// Execute a skill's WASM module.
///
/// For Claude Skills compatibility:
/// - Skills are primarily instructional (Claude follows the instructions)
/// - WASM execution is for sandboxed script/tool execution
/// - This replaces OS-level sandboxing with capability-based WASM sandbox
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
        allowed_tools,
        wasm_config.clone(),
        skill.root.clone(),
    );

    // Determine WASM module path
    let wasm_module = options
        .wasm_module
        .or_else(|| find_wasm_module(&skill.root))
        .ok_or_else(|| {
            OpenSkillError::WasmError(
                "No WASM module found in skill directory".to_string(),
            )
        })?;

    let wasm_path = skill.root.join(&wasm_module);
    if !wasm_path.exists() {
        return Err(OpenSkillError::WasmError(format!(
            "WASM module not found: {}",
            wasm_path.display()
        )));
    }

    // Prepare input
    let input = options.input.unwrap_or_else(|| {
        serde_json::json!({
            "skill_id": skill.id,
            "instructions": skill.instructions
        })
    });

    // Execute in WASM sandbox
    execute_wasm(
        skill,
        &wasm_module,
        input,
        wasm_config.timeout_ms,
        &enforcer,
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_wasm_module_none() {
        let path = PathBuf::from("/nonexistent");
        assert!(find_wasm_module(&path).is_none());
    }
}
