//! OpenSkills Runtime - Claude Skills compatible runtime with WASM sandbox.
//!
//! This runtime implements the Claude Code Agent Skills specification:
//! https://code.claude.com/docs/en/skills
//!
//! Key differences from Claude Code's native implementation:
//! - Uses WASM/WASI sandbox instead of OS-level sandboxing (seatbelt/seccomp)
//! - Provides capability-based security through WASI
//! - Cross-platform consistent behavior
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Claude Skills Loader                        │
//! │  - Discovers SKILL.md in ~/.claude/skills, .claude/skills, etc  │
//! │  - Parses YAML frontmatter (name, description, allowed-tools…)  │
//! │  - Exposes progressive disclosure API                            │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     WASM Execution Sandbox                       │
//! │  - Wasmtime + WASI (filesystem, env, network capabilities)       │
//! │  - Permission enforcement from allowed-tools                     │
//! │  - Audit logging                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

mod audit;
mod errors;
mod executor;
mod manifest;
mod permissions;
mod registry;
mod skill_parser;
mod validator;
mod wasm_runner;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use audit::{AuditRecord, AuditSink, NoopAuditSink};
use errors::OpenSkillError;
use executor::{execute_skill, ExecutionOptions as ExecOpts};
use registry::{Skill, SkillRegistry};
use serde_json::Value;
use validator::validate_skill;

// Re-exports for public API
pub use audit::{AuditRecord as RuntimeAuditRecord, ExecutionStatus as RuntimeExecutionStatus};
pub use errors::OpenSkillError as RuntimeError;
pub use manifest::{HooksConfig, SkillManifest, WasmConfig};
pub use registry::{SkillDescriptor, SkillLocation};

/// Runtime configuration for skill discovery.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Custom skill directories to scan (in addition to or instead of standard locations).
    pub custom_directories: Vec<PathBuf>,
    /// Whether to discover skills from standard locations (~/.claude/skills/, .claude/skills/, nested).
    pub use_standard_locations: bool,
    /// Project root for relative path resolution.
    pub project_root: Option<PathBuf>,
}

/// Execution options for skill invocation.
#[derive(Debug, Clone, Default)]
pub struct ExecutionOptions {
    /// Override timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Override memory limit in MB.
    pub memory_mb: Option<u64>,
    /// Input data for WASM execution.
    pub input: Option<Value>,
}

/// Execution result returned to callers.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Output from execution.
    pub output: Value,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Audit record for this execution.
    pub audit: AuditRecord,
}

/// Loaded skill with full content (for activation).
#[derive(Debug, Clone)]
pub struct LoadedSkill {
    /// Skill ID.
    pub id: String,
    /// Skill manifest.
    pub manifest: SkillManifest,
    /// Full instructions (Markdown body from SKILL.md).
    pub instructions: String,
    /// Location where skill was discovered.
    pub location: SkillLocation,
}

impl From<&Skill> for LoadedSkill {
    fn from(skill: &Skill) -> Self {
        Self {
            id: skill.id.clone(),
            manifest: skill.manifest.clone(),
            instructions: skill.instructions.clone(),
            location: skill.location.clone(),
        }
    }
}

/// OpenSkills runtime - Claude Skills compatible with WASM sandbox.
pub struct OpenSkillRuntime {
    registry: SkillRegistry,
    audit_sink: Box<dyn AuditSink + Send + Sync>,
    custom_directories: Vec<PathBuf>,
    use_standard_locations: bool,
}

impl OpenSkillRuntime {
    /// Create a new runtime that discovers skills from standard locations.
    ///
    /// Standard locations:
    /// - `~/.claude/skills/` (personal skills)
    /// - `.claude/skills/` (project skills)
    /// - Nested `.claude/skills/` directories (monorepo support)
    pub fn new() -> Self {
        Self {
            registry: SkillRegistry::new(),
            audit_sink: Box::new(NoopAuditSink {}),
            custom_directories: Vec::new(),
            use_standard_locations: true,
        }
    }

    /// Create a runtime from a configuration.
    pub fn from_config(config: RuntimeConfig) -> Self {
        let mut registry = SkillRegistry::new();
        if let Some(root) = &config.project_root {
            registry = registry.with_project_root(root);
        }
        Self {
            registry,
            audit_sink: Box::new(NoopAuditSink {}),
            custom_directories: config.custom_directories,
            use_standard_locations: config.use_standard_locations,
        }
    }

    /// Create a runtime with a specific project root.
    pub fn with_project_root<P: AsRef<Path>>(root: P) -> Self {
        Self {
            registry: SkillRegistry::new().with_project_root(root),
            audit_sink: Box::new(NoopAuditSink {}),
            custom_directories: Vec::new(),
            use_standard_locations: true,
        }
    }

    /// Create a runtime that only scans a specific directory.
    pub fn from_directory<P: AsRef<Path>>(dir: P) -> Self {
        let mut registry = SkillRegistry::new();
        let _ = registry.scan_explicit(dir);
        Self {
            registry,
            audit_sink: Box::new(NoopAuditSink {}),
            custom_directories: Vec::new(),
            use_standard_locations: false,
        }
    }

    /// Add a custom skill directory to scan.
    ///
    /// This method can be called multiple times to add multiple directories.
    /// Skills from later directories override earlier ones if IDs conflict.
    pub fn with_custom_directory<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.custom_directories.push(dir.as_ref().to_path_buf());
        self
    }

    /// Add multiple custom skill directories to scan.
    ///
    /// Skills from later directories override earlier ones if IDs conflict.
    pub fn with_custom_directories<P: AsRef<Path>>(mut self, dirs: Vec<P>) -> Self {
        self.custom_directories.extend(
            dirs.into_iter().map(|d| d.as_ref().to_path_buf())
        );
        self
    }

    /// Enable or disable discovery from standard locations.
    ///
    /// Standard locations are:
    /// - `~/.claude/skills/` (personal skills)
    /// - `.claude/skills/` (project skills)
    /// - Nested `.claude/skills/` directories (monorepo support)
    pub fn with_standard_locations(mut self, enable: bool) -> Self {
        self.use_standard_locations = enable;
        self
    }

    /// Set a custom audit sink.
    pub fn with_audit_sink(mut self, sink: Box<dyn AuditSink + Send + Sync>) -> Self {
        self.audit_sink = sink;
        self
    }

    /// Discover and load skills from configured locations.
    ///
    /// This scans:
    /// - Standard locations (if `use_standard_locations` is true):
    ///   - `~/.claude/skills/` (personal skills)
    ///   - `.claude/skills/` (project skills)
    ///   - Nested `.claude/skills/` directories (monorepo support)
    /// - Custom directories (if any were configured via `with_custom_directory` or `with_custom_directories`)
    ///
    /// Returns skill descriptors (name + description only) for progressive disclosure.
    /// Skills from later directories override earlier ones if IDs conflict.
    pub fn discover_skills(&mut self) -> Result<Vec<SkillDescriptor>, OpenSkillError> {
        // Scan standard locations if enabled
        if self.use_standard_locations {
            self.registry.discover()?;
        }

        // Scan custom directories
        for dir in &self.custom_directories {
            self.registry.scan_explicit(dir)?;
        }

        Ok(self.registry.list())
    }

    /// Load skills from a specific directory (for testing or custom paths).
    pub fn load_from_directory<P: AsRef<Path>>(
        &mut self,
        dir: P,
    ) -> Result<Vec<SkillDescriptor>, OpenSkillError> {
        self.registry.scan_explicit(dir)?;
        Ok(self.registry.list())
    }

    /// List all discovered skills (progressive disclosure - descriptors only).
    pub fn list_skills(&self) -> Vec<SkillDescriptor> {
        self.registry.list()
    }

    /// Activate a skill by ID (load full SKILL.md content).
    ///
    /// This implements the "activation" step of progressive disclosure:
    /// the full instructions are only loaded when the skill is activated.
    pub fn activate_skill(&self, skill_id: &str) -> Result<LoadedSkill, OpenSkillError> {
        let skill = self
            .registry
            .get(skill_id)
            .ok_or_else(|| OpenSkillError::SkillNotFound(skill_id.to_string()))?;

        validate_skill(skill)?;

        Ok(LoadedSkill::from(skill))
    }

    /// Execute a skill's WASM module in sandbox.
    ///
    /// Note: Most Claude Skills are instructional (Claude follows the instructions).
    /// WASM execution is for skills that include sandboxed script execution.
    pub fn execute_skill(
        &mut self,
        skill_id: &str,
        options: ExecutionOptions,
    ) -> Result<ExecutionResult, OpenSkillError> {
        // Ensure registry is loaded
        if self.registry.is_empty() {
            // Use discover_skills to load from all configured locations
            self.discover_skills()?;
        }

        let skill = self
            .registry
            .get(skill_id)
            .ok_or_else(|| OpenSkillError::SkillNotFound(skill_id.to_string()))?;

        validate_skill(skill)?;

        let start = Instant::now();
        let start_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_millis() as u64;

        let exec_options = ExecOpts {
            timeout_ms: options.timeout_ms,
            memory_mb: options.memory_mb,
            input: options.input.clone(),
            wasm_module: None,
        };

        let execution = execute_skill(skill, exec_options)?;
        let duration_ms = start.elapsed().as_millis() as u64;

        let audit = AuditRecord {
            skill_id: skill.id.clone(),
            version: "1.0".to_string(), // Claude Skills don't have version in manifest
            input_hash: audit::hash_json(&options.input.clone().unwrap_or(Value::Null)),
            output_hash: audit::hash_json(&execution.output),
            start_time_ms: start_epoch,
            duration_ms,
            permissions_used: execution.permissions_used.clone(),
            exit_status: execution.exit_status.clone(),
            stdout: execution.stdout.clone(),
            stderr: execution.stderr.clone(),
        };

        self.audit_sink.record(&audit);

        Ok(ExecutionResult {
            output: execution.output,
            stdout: execution.stdout,
            stderr: execution.stderr,
            audit,
        })
    }

    /// Check if a tool is allowed for a skill.
    pub fn is_tool_allowed(&self, skill_id: &str, tool: &str) -> Result<bool, OpenSkillError> {
        let skill = self
            .registry
            .get(skill_id)
            .ok_or_else(|| OpenSkillError::SkillNotFound(skill_id.to_string()))?;

        let allowed = skill.manifest.get_allowed_tools();
        
        // Empty list means all tools allowed
        if allowed.is_empty() {
            return Ok(true);
        }

        Ok(allowed.iter().any(|t| t == tool))
    }
}

impl Default for OpenSkillRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_creation() {
        let runtime = OpenSkillRuntime::new();
        assert!(runtime.list_skills().is_empty());
    }

    #[test]
    fn test_runtime_from_nonexistent_directory() {
        let runtime = OpenSkillRuntime::from_directory("/nonexistent/path");
        assert!(runtime.list_skills().is_empty());
    }
}
