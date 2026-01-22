//! OpenSkills Runtime - Claude Skills compatible runtime with WASM sandbox.
//!
//! This runtime implements the Claude Code Agent Skills specification:
//! https://code.claude.com/docs/en/skills
//!
//! Key differences from Claude Code's native implementation:
//! - Uses WASM/WASI sandbox by default, with native seatbelt support on macOS
//! - Provides capability-based security through WASI or OS-level sandboxing
//! - Cross-platform consistent behavior for WASM execution
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
//! │                    Execution Sandbox                             │
//! │  - WASM/WASI 0.3 (component model)                               │
//! │  - Native seatbelt sandbox (macOS)                               │
//! │  - Permission enforcement from allowed-tools                     │
//! │  - Audit logging                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

mod audit;
mod build;
mod context;
mod errors;
mod executor;
mod hook_runner;
mod manifest;
mod skill_session;
mod native_runner;
mod permission_callback;
mod permissions;
mod registry;
mod skill_parser;
mod validator;
mod wasm_runner;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Generate a unique session ID for workspace isolation.
fn generate_session_id() -> String {
    use std::time::SystemTime;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("session-{}", timestamp)
}

/// Get the default workspace root directory.
fn get_default_workspace_root() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("openskills")
        .join("workspace")
}

use audit::{AuditRecord, AuditSink, NoopAuditSink};
use errors::OpenSkillError;
use executor::{
    execute_skill, read_skill_file, run_skill_target, list_skill_files,
    ExecutionOptions as ExecOpts,
};
use permission_callback::PermissionManager;
use registry::{Skill, SkillRegistry};
use serde_json::Value;
use std::sync::Arc;
use validator::validate_skill;

// Re-exports for public API
pub use audit::{AuditRecord as RuntimeAuditRecord, ExecutionStatus as RuntimeExecutionStatus};
pub use build::{build_skill, BuildConfig, list_build_plugins};
pub use errors::OpenSkillError as RuntimeError;
pub use manifest::{constraints, HooksConfig, SkillManifest, WasmConfig};
pub use context::{ContextOutput, ExecutionContext, OutputType};
pub use skill_session::SkillExecutionSession;
pub use permission_callback::{
    CliPermissionCallback, DenyAllCallback, PermissionAuditEntry, PermissionCallback,
    PermissionRequest, PermissionResponse, RiskLevel, get_risk_level, is_risky_tool,
};
pub use skill_parser::parse_skill_md;
pub use registry::{SkillDescriptor, SkillLocation};
pub use validator::{analyze_skill_tokens, validate_skill_path, TokenAnalysis, ValidationResult, ValidationStats};

// Re-export execution target types for public API
pub use executor::{ExecutionTarget, TargetExecutionOptions};

// Re-export sandboxed command execution API
pub use executor::{CommandPermissions, CommandResult, run_sandboxed_command};

// Re-export hook execution API
pub use hook_runner::{HookEvent, HookRunner};

/// Runtime configuration for skill discovery.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Custom skill directories to scan (in addition to or instead of standard locations).
    pub custom_directories: Vec<PathBuf>,
    /// Whether to discover skills from standard locations (~/.claude/skills/, .claude/skills/, nested).
    pub use_standard_locations: bool,
    /// Project root for relative path resolution.
    pub project_root: Option<PathBuf>,
    /// Workspace directory for skill I/O operations.
    /// If not set, defaults to ~/.cache/openskills/workspace/{session_id}/
    pub workspace_dir: Option<PathBuf>,
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
    /// Workspace directory for skill I/O operations.
    workspace_dir: Option<PathBuf>,
    /// Session ID for unique workspace paths.
    session_id: String,
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
            workspace_dir: None,
            session_id: generate_session_id(),
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
            workspace_dir: config.workspace_dir,
            session_id: generate_session_id(),
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
            workspace_dir: None,
            session_id: generate_session_id(),
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
            workspace_dir: None,
            session_id: generate_session_id(),
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

    // ==================== Workspace Management ====================

    /// Set a custom workspace directory for skill I/O operations.
    ///
    /// The workspace directory is a sandboxed location where skills can read/write
    /// files. This is automatically accessible in WASM and native sandboxes.
    ///
    /// Environment variable `SKILL_WORKSPACE` is injected into skill execution
    /// pointing to this directory.
    pub fn with_workspace_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.workspace_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Get the current workspace directory.
    ///
    /// Returns the configured workspace directory, or generates a default one
    /// at `~/.cache/openskills/workspace/{session_id}/`.
    ///
    /// The directory is created if it doesn't exist.
    pub fn get_workspace_dir(&self) -> Result<PathBuf, OpenSkillError> {
        let dir = self.workspace_dir.clone().unwrap_or_else(|| {
            get_default_workspace_root().join(&self.session_id)
        });

        // Ensure the directory exists
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }

        Ok(dir)
    }

    /// Get the session ID for this runtime instance.
    pub fn get_session_id(&self) -> &str {
        &self.session_id
    }

    /// Clean up the workspace directory.
    ///
    /// This removes all files and subdirectories in the workspace.
    /// Use with caution - this is destructive.
    pub fn cleanup_workspace(&self) -> Result<(), OpenSkillError> {
        let dir = self.get_workspace_dir()?;
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    // ==================== End Workspace Management ====================

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

    /// Get a complete skill-agnostic system prompt for agents.
    ///
    /// This returns a generic system prompt that teaches the agent how to use
    /// Claude Skills without any skill-specific knowledge. The agent:
    /// - Knows what skills are available (name + description)
    /// - Knows how to activate a skill to get its full instructions
    /// - Follows the instructions in SKILL.md without prior knowledge
    ///
    /// This is the recommended way to integrate skills into an agent.
    pub fn get_agent_system_prompt(&self) -> String {
        let skills: Vec<SkillDescriptor> = self
            .list_skills()
            .into_iter()
            .filter(|skill| skill.user_invocable)
            .collect();

        if skills.is_empty() {
            return String::from("No skills are currently available.");
        }

        let mut prompt = String::from(r#"You have access to Claude Skills that provide specialized capabilities.

## Available Skills

"#);

        for skill in &skills {
            prompt.push_str(&format!("- **{}**: {}\n", skill.id, skill.description));
        }

        prompt.push_str(r#"
## How to Use Skills

When a user's request matches a skill's capabilities:

1. **Activate the skill**: Call `activate_skill(skill_id)` to load the full SKILL.md instructions
2. **Read the instructions carefully**: The SKILL.md contains everything you need to know
3. **Follow the instructions exactly**: Execute the steps as described in SKILL.md
4. **Use helper files if referenced**: Call `read_skill_file(skill_id, path)` to read referenced docs (e.g., `docx-js.md`)
5. **Run scripts or WASM modules as instructed**: 
   - **CRITICAL**: If a skill provides a WASM module or script to perform a task, you MUST use `run_skill_script()` to execute it. Do NOT manually recreate the functionality or create files manually.
   - For scripts/WASM in the skill directory: Call `run_skill_script(skill_id, path, args?, input?)`
     - The sandbox type is auto-detected from file extension: `.wasm` → WASM sandbox, `.py/.sh` → native sandbox
     - For WASM modules, pass JSON input: `run_skill_script("skill-id", "wasm/skill.wasm", null, '{"action": "..."}')`
     - Example: If skill-creator has `wasm/skill.wasm` for creating skills, use: `run_skill_script("skill-creator", "wasm/skill.wasm", null, '{"action": "init_skill", "skill_name": "my-skill", "path": "skills/public"}')`
   - For scripts you generate in the workspace: Use `run_sandboxed_bash()` to execute them

## Important

- Each skill's SKILL.md contains all the knowledge you need - do NOT assume prior knowledge about any skill
- The instructions may reference helper files within the skill directory - read them when needed
- The instructions may tell you to run scripts or WASM modules - these are sandboxed for security
- The sandbox type is transparent: WASM files use WASM sandbox, Python/shell scripts use native OS sandbox
- Output files are written to the workspace directory (available as SKILL_WORKSPACE environment variable)

## Code Generation and Execution

When a skill instructs you to create a JavaScript/TypeScript file:

1. **Follow the skill's documentation**: Skills may use CommonJS (`require()`) or ES modules (`import`). Follow the examples in the skill's documentation (e.g., `docx-js.md`).
   - **CommonJS** (most Claude Skills use this): `const { Document, Packer } = require('docx'); const fs = require('fs');`
   - **ES modules**: `import { Document, Packer } from 'docx'; import fs from 'fs';`
   - When in doubt, use CommonJS as it's the most common format for Claude Skills

2. **Write to workspace directory**: Use the workspace directory for output files:
   - Access via `process.env.SKILL_WORKSPACE` or use the configured workspace path
   - Example: `fs.writeFileSync(path.join(process.env.SKILL_WORKSPACE || './output', 'document.docx'), buffer)`

3. **Execute generated scripts**: After creating a script file, you MUST execute it:
   - For CommonJS JavaScript files: Use `run_sandboxed_bash('node path/to/script.js', working_dir, allow_process=true)`
   - For ES module JavaScript files: Use `run_sandboxed_bash('node --input-type=module path/to/script.js', working_dir, allow_process=true)` or use `.mjs` extension
   - For TypeScript files: Use `run_sandboxed_bash('npx tsx path/to/script.ts', working_dir, allow_process=true)`
   - **CRITICAL**: Always set `allow_process=true` when executing scripts (npx, node, etc.)
   - Set `working_dir` to the workspace directory where the script is located

4. **Complete the workflow**: After executing a script that generates files:
   - Use `list_workspace_files(pattern: "*.docx")` to find generated files
   - Use `get_file_info(path)` to get file details
   - Mention the file in your response

## File Output and Delivery

When you generate files (documents, images, etc.):

1. **Files are written to the workspace directory** (available as SKILL_WORKSPACE environment variable)
2. **After generating files**, use `list_workspace_files()` to discover what was created
3. **Use `get_file_info(path)`** to get file details (size, type, MIME type) for your response
4. **Mention files in your response** so the user knows what was created
5. **Include file paths and types** in your final response

Example response:
"I've created a Word document for you: 'output/document.docx' (45 KB, Word document)"

## Available Tools

- `list_skills()` - List all available skills
- `activate_skill(skill_id)` - Load full SKILL.md instructions for a skill
- `read_skill_file(skill_id, path)` - Read a file from a skill directory
- `list_skill_files(skill_id, subdir?, recursive?)` - List files in a skill directory
- `run_skill_script(skill_id, path, args?, input?)` - Run a script or WASM module from a skill (auto-detects sandbox type from extension)
- `run_sandboxed_bash(command, working_dir?, allow_process?)` - Run a sandboxed bash command (set allow_process=true for script execution)
- `write_file(path, content)` - Write a file to the workspace
- `read_file(path)` - Read a file from the workspace
- `list_workspace_files(subdir?, recursive?, pattern?)` - List files in the workspace directory
- `get_file_info(path)` - Get file information (size, type, MIME type)
"#);

        prompt
    }

    /// Activate a skill by ID (load full SKILL.md content).
    ///
    /// This implements the "activation" step of progressive disclosure:
    /// the full instructions are only loaded when the skill is activated.
    ///
    /// # Fork Context Behavior
    ///
    /// **Important**: This function does NOT create a fork context, even if the skill
    /// has `context: fork` in its manifest. Fork is created later during execution.
    ///
    /// - Skill instructions are returned to the **main conversation context**
    /// - LLM reads and comprehends instructions in main context
    /// - Fork is only created when `start_skill_session()` or `execute_skill_with_context()`
    ///   is called, isolating execution outputs (tool calls, errors, debug logs)
    ///
    /// This ensures skill instructions are part of the main conversation for comprehension,
    /// while execution noise is isolated in the fork context.
    pub fn activate_skill(&self, skill_id: &str) -> Result<LoadedSkill, OpenSkillError> {
        // Lazy load: get full skill content (including instructions) from registry
        let skill = self.registry.load_full_skill(skill_id)?;

        validate_skill(&skill)?;

        Ok(LoadedSkill::from(&skill))
    }

    /// Start a skill execution session for instruction-based workflows.
    ///
    /// This is primarily used when the agent will handle tool calls directly
    /// and needs to respect `context: fork` semantics. If the skill is forked,
    /// the returned session includes an isolated context for recording tool
    /// calls, intermediate outputs, and results.
    ///
    /// # Fork Context Behavior
    ///
    /// **Important**: Fork context is created **after** skill activation, not before.
    ///
    /// 1. **Activation Phase** (happens before this call):
    ///    - `activate_skill()` loads full SKILL.md instructions
    ///    - Instructions are returned to main conversation context
    ///    - LLM reads/comprehends instructions in main context
    ///
    /// 2. **Execution Phase** (this function):
    ///    - Fork is created here if skill has `context: fork`
    ///    - Tool calls during execution are recorded in fork context
    ///    - Intermediate outputs (errors, debug logs) stay in fork
    ///
    /// 3. **Summary Return** (via `finish_skill_session()`):
    ///    - Only final summary/results are returned to main context
    ///    - Prevents context pollution from trial-and-error
    ///
    /// This design ensures skill instructions are part of the main conversation
    /// (for comprehension), while execution noise is isolated in the fork.
    pub fn start_skill_session(
        &mut self,
        skill_id: &str,
        input: Option<Value>,
        parent_context: Option<&ExecutionContext>,
    ) -> Result<SkillExecutionSession, OpenSkillError> {
        if self.registry.is_empty() {
            self.discover_skills()?;
        }

        // Load full skill (with instructions) for session
        // Note: This happens in main context - instructions are already
        // available to the agent via activate_skill() if called earlier
        let skill = self.registry.load_full_skill(skill_id)?;

        validate_skill(&skill)?;

        // Fork is created HERE, after skill is loaded
        // This isolates execution outputs, not instruction comprehension
        let is_forked = skill.manifest.is_forked();
        let context = if is_forked {
            let base_context = parent_context.cloned().unwrap_or_else(ExecutionContext::new);
            Some(base_context.fork())
        } else {
            None
        };

        Ok(SkillExecutionSession::new(
            LoadedSkill::from(&skill),
            is_forked,
            input.unwrap_or(Value::Null),
            context,
        ))
    }

    /// Finish a skill execution session and return an ExecutionResult.
    ///
    /// If the session is forked, only the summary is returned to the caller and
    /// intermediate outputs remain isolated in the forked context.
    pub fn finish_skill_session(
        &mut self,
        mut session: SkillExecutionSession,
        output: Value,
        stdout: String,
        stderr: String,
        exit_status: audit::ExecutionStatus,
    ) -> Result<ExecutionResult, OpenSkillError> {
        let duration_ms = session.elapsed_ms();

        // Capture outputs in forked context if applicable
        if session.is_forked() {
            session.record_stdout_if_present(&stdout);
            session.record_stderr_if_present(&stderr);
            session.record_result(&output);
        }

        let audit = AuditRecord {
            skill_id: session.skill().id.clone(),
            version: "1.0".to_string(),
            input_hash: audit::hash_json(session.input()),
            output_hash: audit::hash_json(&output),
            start_time_ms: session.start_epoch_ms(),
            duration_ms,
            permissions_used: session.permissions_used().to_vec(),
            exit_status,
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        };

        self.audit_sink.record(&audit);

        if session.is_forked() {
            let summary = session.summarize_fork();
            Ok(ExecutionResult {
                output: serde_json::json!({
                    "summary": summary,
                    "context_id": session.context_id().unwrap_or(""),
                    "is_forked": true
                }),
                stdout: summary.clone(),
                stderr: String::new(),
                audit,
            })
        } else {
            Ok(ExecutionResult {
                output,
                stdout,
                stderr,
                audit,
            })
        }
    }

    /// Check permission for a tool call for a given skill.
    ///
    /// This is intended for agent-managed tool execution. It enforces allowed-tools
    /// and triggers ask-before-act for risky tools.
    pub fn check_tool_permission(
        &self,
        skill_id: &str,
        tool: &str,
        description: Option<String>,
        context: std::collections::HashMap<String, String>,
    ) -> Result<bool, OpenSkillError> {
        // Only need metadata for permission checking
        let metadata = self
            .registry
            .get(skill_id)
            .ok_or_else(|| OpenSkillError::SkillNotFound(skill_id.to_string()))?;

        let allowed = metadata.manifest.get_allowed_tools();
        if !allowed.is_empty() && !allowed.iter().any(|t| t == tool) {
            return Err(OpenSkillError::PermissionDenied(format!(
                "Tool {} is not allowed for skill {}",
                tool, skill_id
            )));
        }

        if is_risky_tool(tool) {
            let description = description.unwrap_or_else(|| format!("Execute {} operations", tool));
            return self.permission_manager.check_permission(
                skill_id,
                tool,
                description,
                get_risk_level(tool),
                context,
            );
        }

        Ok(true)
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

        // Load full skill (with instructions) for execution
        let skill = self.registry.load_full_skill(skill_id)?;

        validate_skill(&skill)?;

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
            workspace_dir: self.get_workspace_dir().ok(),
        };

        let execution = execute_skill(&skill, exec_options)?;
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
        // Only need metadata for tool checking
        let metadata = self
            .registry
            .get(skill_id)
            .ok_or_else(|| OpenSkillError::SkillNotFound(skill_id.to_string()))?;

        let allowed = metadata.manifest.get_allowed_tools();
        
        // Empty list means all tools allowed
        if allowed.is_empty() {
            return Ok(true);
        }

        Ok(allowed.iter().any(|t| t == tool))
    }

    /// Run a specific target (script/WASM) within a skill.
    ///
    /// This is designed for Claude Skills where SKILL.md instructions tell
    /// the agent which script to run (e.g., "run python ooxml/scripts/unpack.py").
    ///
    /// # Arguments
    ///
    /// * `skill_id` - The skill containing the target
    /// * `target` - What to execute (Script, Wasm, or Auto)
    /// * `options` - Execution options (timeout, input, etc.)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Run a specific Python script within the docx skill
    /// let result = runtime.run_skill_target(
    ///     "docx",
    ///     ExecutionTarget::Script {
    ///         path: "ooxml/scripts/unpack.py".to_string(),
    ///         args: vec!["document.docx".to_string(), "output/".to_string()],
    ///     },
    ///     TargetExecutionOptions {
    ///         timeout_ms: Some(30000),
    ///         ..Default::default()
    ///     },
    /// )?;
    /// ```
    pub fn run_skill_target(
        &mut self,
        skill_id: &str,
        target: ExecutionTarget,
        timeout_ms: Option<u64>,
        input: Option<Value>,
    ) -> Result<ExecutionResult, OpenSkillError> {
        // Ensure registry is loaded
        if self.registry.is_empty() {
            self.discover_skills()?;
        }

        // Load full skill (with instructions) for target execution
        let skill = self.registry.load_full_skill(skill_id)?;

        validate_skill(&skill)?;

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

        let options = TargetExecutionOptions {
            target,
            timeout_ms,
            input,
            workspace_dir: self.get_workspace_dir().ok(),
            ..Default::default()
        };

        let execution = run_skill_target(&skill, options)?;
        let duration_ms = start.elapsed().as_millis() as u64;

        let audit = AuditRecord {
            skill_id: skill.id.clone(),
            version: "1.0".to_string(),
            input_hash: "".to_string(),
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

    /// Read a file from a skill directory.
    ///
    /// This allows agents to read helper files (like `docx-js.md`) that skills
    /// reference in their SKILL.md instructions.
    ///
    /// # Security
    ///
    /// The path must be within the skill directory. Attempts to escape via
    /// `..` or symlinks are rejected.
    pub fn read_skill_file(&self, skill_id: &str, relative_path: &str) -> Result<String, OpenSkillError> {
        // Only need metadata (root path) for file reading
        let metadata = self
            .registry
            .get(skill_id)
            .ok_or_else(|| OpenSkillError::SkillNotFound(skill_id.to_string()))?;

        read_skill_file(&metadata.root, relative_path)
    }

    /// List files in a skill directory (or subdirectory).
    ///
    /// Returns relative paths from the skill root.
    ///
    /// # Arguments
    ///
    /// * `skill_id` - The skill to list files from
    /// * `subdir` - Optional subdirectory (e.g., "scripts", "ooxml/scripts")
    /// * `recursive` - Whether to list recursively
    pub fn list_skill_files(
        &self,
        skill_id: &str,
        subdir: Option<&str>,
        recursive: bool,
    ) -> Result<Vec<String>, OpenSkillError> {
        // Only need metadata (root path) for file listing
        let metadata = self
            .registry
            .get(skill_id)
            .ok_or_else(|| OpenSkillError::SkillNotFound(skill_id.to_string()))?;

        list_skill_files(&metadata.root, subdir, recursive)
    }

    /// Get the root directory path for a skill.
    ///
    /// This is useful for agents that need to construct absolute paths
    /// for skill resources.
    pub fn get_skill_root(&self, skill_id: &str) -> Result<PathBuf, OpenSkillError> {
        // Only need metadata (root path)
        let metadata = self
            .registry
            .get(skill_id)
            .ok_or_else(|| OpenSkillError::SkillNotFound(skill_id.to_string()))?;

        Ok(metadata.root.clone())
    }

    /// Execute hooks for a skill event.
    ///
    /// This runs matching hooks (PreToolUse, PostToolUse, or Stop) for a skill.
    /// Hooks are executed in a sandboxed environment with the skill's root directory
    /// as the working directory (or the hook's specified cwd if provided).
    ///
    /// # Arguments
    ///
    /// * `skill_id` - The skill to execute hooks for
    /// * `event` - The hook event to trigger
    ///
    /// # Returns
    ///
    /// A vector of command results, one for each matching hook that was executed.
    pub fn execute_hooks(
        &self,
        skill_id: &str,
        event: HookEvent,
    ) -> Result<Vec<CommandResult>, OpenSkillError> {
        let skill = self.activate_skill(skill_id)?;

        if let Some(hooks) = skill.manifest.hooks {
            let runner = HookRunner::new(hooks, self.get_skill_root(skill_id)?);
            runner.execute(&event)
        } else {
            Ok(Vec::new())
        }
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
