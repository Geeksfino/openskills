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
mod build;
mod context;
mod errors;
mod executor;
mod manifest;
mod permission_callback;
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
use permission_callback::PermissionManager;
use registry::{Skill, SkillRegistry};
use serde_json::Value;
use std::sync::Arc;
use validator::validate_skill;

// Re-exports for public API
pub use audit::{AuditRecord as RuntimeAuditRecord, ExecutionStatus as RuntimeExecutionStatus};
pub use build::{build_skill, BuildConfig};
pub use errors::OpenSkillError as RuntimeError;
pub use manifest::{constraints, HooksConfig, SkillManifest, WasmConfig};
pub use context::{ContextOutput, ExecutionContext, OutputType};
pub use permission_callback::{
    CliPermissionCallback, DenyAllCallback, PermissionAuditEntry, PermissionCallback,
    PermissionRequest, PermissionResponse, RiskLevel, get_risk_level, is_risky_tool,
};
pub use skill_parser::parse_skill_md;
pub use registry::{SkillDescriptor, SkillLocation};
pub use validator::{analyze_skill_tokens, validate_skill_path, TokenAnalysis, ValidationResult, ValidationStats};

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
    permission_manager: PermissionManager,
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
            permission_manager: PermissionManager::new(),
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
            permission_manager: PermissionManager::new(),
            custom_directories: config.custom_directories,
            use_standard_locations: config.use_standard_locations,
        }
    }

    /// Create a runtime with a specific project root.
    pub fn with_project_root<P: AsRef<Path>>(root: P) -> Self {
        Self {
            registry: SkillRegistry::new().with_project_root(root),
            audit_sink: Box::new(NoopAuditSink {}),
            permission_manager: PermissionManager::new(),
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
            permission_manager: PermissionManager::new(),
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

    /// Enable interactive permission system with a callback.
    ///
    /// The callback will be invoked when skills attempt to use risky tools
    /// (Write, Bash, WebSearch, etc.) to request user approval.
    ///
    /// # Example
    ///
    /// ```rust
    /// use openskills_runtime::{OpenSkillRuntime, CliPermissionCallback};
    /// use std::sync::Arc;
    ///
    /// let runtime = OpenSkillRuntime::new()
    ///     .with_permission_callback(Arc::new(CliPermissionCallback));
    /// ```
    pub fn with_permission_callback(mut self, callback: Arc<dyn PermissionCallback>) -> Self {
        self.permission_manager = PermissionManager::with_callback(callback);
        self
    }

    /// Enable strict permissions mode (all risky operations denied by default).
    ///
    /// This is useful for testing or high-security environments.
    pub fn with_strict_permissions(mut self) -> Self {
        use permission_callback::DenyAllCallback;
        self.permission_manager = PermissionManager::with_callback(Arc::new(DenyAllCallback));
        self
    }

    /// Get permission audit log.
    ///
    /// Returns a list of all permission requests and their outcomes.
    pub fn get_permission_audit(&self) -> Vec<PermissionAuditEntry> {
        self.permission_manager.get_audit_log()
    }

    /// Reset all "allow always" permission grants.
    ///
    /// This clears all permanent permission grants that were previously
    /// approved with "allow always". Useful for:
    /// - Security: Revoke all permanent grants when security policy changes
    /// - Testing: Reset grants between test cases
    /// - Runtime: Clear grants when switching to a different security context
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use openskills_runtime::{ExecutionOptions, OpenSkillRuntime};
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut runtime = OpenSkillRuntime::new();
    ///     let options = ExecutionOptions::default();
    ///
    ///     // User previously granted "allow always" for Write operations
    ///     let _ = runtime.execute_skill("my-skill", options)?;
    ///
    ///     // Later, revoke all permanent grants
    ///     runtime.reset_permission_grants();
    ///
    ///     // Next Write operation will require permission again
    ///     Ok(())
    /// }
    /// ```
    pub fn reset_permission_grants(&self) {
        self.permission_manager.reset_grants();
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

    /// Validate a skill directory by reading and parsing SKILL.md.
    pub fn validate_skill_directory<P: AsRef<Path>>(path: P) -> ValidationResult {
        validate_skill_path(path.as_ref())
    }

    /// Analyze token usage for a skill directory.
    pub fn analyze_skill_directory<P: AsRef<Path>>(path: P) -> TokenAnalysis {
        analyze_skill_tokens(path.as_ref())
    }

    /// Format available skill metadata for system prompt injection.
    ///
    /// Returns a human-readable list of skills intended to be appended to the
    /// system prompt so the model knows what it can invoke. Only user-invocable
    /// skills are included by default.
    pub fn get_system_prompt_metadata(&self) -> String {
        let skills: Vec<SkillDescriptor> = self
            .list_skills()
            .into_iter()
            .filter(|skill| skill.user_invocable)
            .collect();

        if skills.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("You have access to the following skills:\n\n");
        for skill in skills {
            prompt.push_str(&format!("- {}: {}\n", skill.id, skill.description));
        }
        prompt.push_str(
            "\nTo use a skill, activate it when the user's request matches the skill's purpose.",
        );
        prompt
    }

    /// Format available skill metadata as JSON for structured system prompts.
    ///
    /// Returns a JSON string like:
    /// {"skills":[{"id":"code-review","description":"...","location":"project","user_invocable":true}]}
    pub fn get_system_prompt_metadata_json(&self) -> Result<String, OpenSkillError> {
        let skills: Vec<SkillDescriptor> = self
            .list_skills()
            .into_iter()
            .filter(|skill| skill.user_invocable)
            .collect();

        if skills.is_empty() {
            return Ok(String::new());
        }

        let payload = serde_json::json!({
            "skills": skills.iter().map(|skill| {
                serde_json::json!({
                    "id": skill.id,
                    "description": skill.description,
                    "location": skill.location,
                    "user_invocable": skill.user_invocable,
                })
            }).collect::<Vec<_>>()
        });

        Ok(payload.to_string())
    }

    /// Get a compact, one-line summary of available skills.
    ///
    /// Example: "Skills: code-review, test-generator (2 total)"
    pub fn get_system_prompt_summary(&self) -> String {
        let skill_names: Vec<String> = self
            .list_skills()
            .into_iter()
            .filter(|skill| skill.user_invocable)
            .map(|skill| skill.id)
            .collect();

        if skill_names.is_empty() {
            return String::new();
        }

        format!(
            "Skills: {} ({} total)",
            skill_names.join(", "),
            skill_names.len()
        )
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
    ///
    /// If a permission callback is configured, risky operations (Write, Bash, etc.)
    /// will require user approval before execution.
    ///
    /// If the skill has `context: fork`, execution happens in an isolated context
    /// and only a summary is returned. Use `execute_skill_with_context` for explicit
    /// context management.
    pub fn execute_skill(
        &mut self,
        skill_id: &str,
        options: ExecutionOptions,
    ) -> Result<ExecutionResult, OpenSkillError> {
        use context::ExecutionContext;
        let main_context = ExecutionContext::new();
        self.execute_skill_with_context(skill_id, options, &main_context)
    }

    /// Execute a skill with explicit context management.
    ///
    /// If the skill has `context: fork`, execution happens in an isolated
    /// forked context where intermediate outputs are captured separately.
    /// Only the summary is returned to the parent context, preventing context pollution.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use openskills_runtime::{ExecutionContext, ExecutionOptions, OpenSkillRuntime};
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut runtime = OpenSkillRuntime::new();
    ///     let main_context = ExecutionContext::new();
    ///
    ///     // Execute skill that forks context
    ///     let _result = runtime.execute_skill_with_context(
    ///         "my-skill",
    ///         ExecutionOptions::default(),
    ///         &main_context,
    ///     )?;
    ///
    ///     // For forked skills, result.output contains only the summary
    ///     Ok(())
    /// }
    /// ```
    pub fn execute_skill_with_context(
        &mut self,
        skill_id: &str,
        options: ExecutionOptions,
        parent_context: &ExecutionContext,
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

        // Check if skill should run in forked context
        let is_forked = skill.manifest.is_forked();
        let mut fork_context = if is_forked {
            Some(parent_context.fork())
        } else {
            None
        };

        // Check permissions for risky tools before execution
        let allowed_tools = skill.manifest.get_allowed_tools();
        use permission_callback::{get_risk_level, is_risky_tool};
        use std::collections::HashMap;

        for tool in &allowed_tools {
            if is_risky_tool(tool) {
                let granted = self.permission_manager.check_permission(
                    skill_id,
                    tool,
                    format!("Execute {} operations", tool),
                    get_risk_level(tool),
                    HashMap::new(),
                )?;

                if !granted {
                    return Err(OpenSkillError::PermissionDenied(format!(
                        "User denied {} permission for skill {}",
                        tool, skill_id
                    )));
                }
            }
        }

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

        // Capture outputs in forked context if applicable
        if let Some(ref mut fork) = fork_context {
            use context::OutputType;
            // Record stdout, stderr, and result in the forked context
            if !execution.stdout.is_empty() {
                fork.record_output(OutputType::Stdout, execution.stdout.clone());
            }
            if !execution.stderr.is_empty() {
                fork.record_output(OutputType::Stderr, execution.stderr.clone());
            }
            // Record the final result
            if let Some(result_str) = execution.output.as_str() {
                fork.record_output(OutputType::Result, result_str.to_string());
            } else {
                // Serialize JSON output to string
                let result_str = serde_json::to_string(&execution.output)
                    .unwrap_or_else(|_| "{}".to_string());
                fork.record_output(OutputType::Result, result_str);
            }
        }

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

        // For forked contexts, return only the summary
        if let Some(mut fork) = fork_context {
            let summary = fork.summarize();
            
            Ok(ExecutionResult {
                output: serde_json::json!({
                    "summary": summary,
                    "context_id": fork.id(),
                    "is_forked": true
                }),
                stdout: summary.clone(),
                stderr: String::new(), // Stderr is captured in fork, not returned
                audit,
            })
        } else {
            // Normal execution - return full outputs
            Ok(ExecutionResult {
                output: execution.output,
                stdout: execution.stdout,
                stderr: execution.stderr,
                audit,
            })
        }
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

    #[test]
    fn test_system_prompt_helpers_empty() {
        let runtime = OpenSkillRuntime::new();
        assert_eq!(runtime.get_system_prompt_metadata(), "");
        assert_eq!(runtime.get_system_prompt_summary(), "");
        assert_eq!(
            runtime.get_system_prompt_metadata_json().unwrap_or_default(),
            ""
        );
    }
}
