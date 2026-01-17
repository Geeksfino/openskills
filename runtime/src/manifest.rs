//! Claude Skills manifest types
//!
//! Conforms to the Claude Code Agent Skills specification:
//! https://code.claude.com/docs/en/skills

use serde::{Deserialize, Serialize};

/// Claude Skill manifest parsed from SKILL.md YAML frontmatter.
///
/// Required fields: `name`, `description`
/// Optional fields: `allowed_tools`, `model`, `context`, `agent`, `hooks`, `user_invocable`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SkillManifest {
    /// Skill name. Must use lowercase letters, numbers, and hyphens only (max 64 characters).
    /// Should match the directory name.
    pub name: String,

    /// What the Skill does and when to use it (max 1024 characters).
    /// Claude uses this to decide when to apply the Skill.
    pub description: String,

    /// Tools Claude can use without asking permission when this Skill is active.
    /// Supports comma-separated values or YAML-style lists.
    #[serde(default)]
    pub allowed_tools: Option<AllowedTools>,

    /// Model to use when this Skill is active (e.g., "claude-sonnet-4-20250514").
    /// Defaults to the conversation's model.
    #[serde(default)]
    pub model: Option<String>,

    /// Set to "fork" to run the Skill in a forked sub-agent context.
    #[serde(default)]
    pub context: Option<String>,

    /// Specify which agent type to use when `context: fork` is set.
    /// (e.g., "Explore", "Plan", "general-purpose", or a custom agent name)
    #[serde(default)]
    pub agent: Option<String>,

    /// Define hooks scoped to this Skill's lifecycle.
    /// Supports PreToolUse, PostToolUse, and Stop events.
    #[serde(default)]
    pub hooks: Option<HooksConfig>,

    /// Controls whether the Skill appears in the slash command menu.
    /// Does not affect the Skill tool or automatic discovery. Defaults to true.
    #[serde(default)]
    pub user_invocable: Option<bool>,
}

/// Allowed tools can be specified as a list or comma-separated string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AllowedTools {
    List(Vec<String>),
    CommaSeparated(String),
}

impl AllowedTools {
    /// Get the list of allowed tools as a Vec<String>.
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            AllowedTools::List(v) => v.clone(),
            AllowedTools::CommaSeparated(s) => s
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
        }
    }
}

/// Hooks configuration for skill lifecycle events.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct HooksConfig {
    #[serde(default)]
    pub pre_tool_use: Option<Vec<HookEntry>>,
    #[serde(default)]
    pub post_tool_use: Option<Vec<HookEntry>>,
    #[serde(default)]
    pub stop: Option<Vec<HookEntry>>,
}

/// A single hook entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    /// Tool matcher (glob pattern or specific tool name).
    #[serde(default)]
    pub matcher: Option<String>,
    /// Command to run.
    pub command: String,
    /// Working directory for the command.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Timeout in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// WASM execution configuration for sandboxed script execution.
/// This extends Claude Skills with WASM-based sandboxing instead of OS-level sandboxing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WasmConfig {
    /// Path to the WASM module relative to the skill directory.
    pub module: Option<String>,
    /// Timeout in milliseconds for WASM execution.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Memory limit in MB.
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u64,
    /// Filesystem permissions for WASM sandbox.
    #[serde(default)]
    pub filesystem: FilesystemPermissions,
    /// Network permissions for WASM sandbox.
    #[serde(default)]
    pub network: NetworkPermissions,
    /// Environment variable permissions.
    #[serde(default)]
    pub env: EnvPermissions,
    /// Deterministic random seed (for reproducibility).
    #[serde(default)]
    pub random_seed: Option<u64>,
}

fn default_timeout_ms() -> u64 {
    30000 // 30 seconds
}

fn default_memory_mb() -> u64 {
    128
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilesystemPermissions {
    /// Paths that can be read.
    #[serde(default)]
    pub read: Vec<String>,
    /// Paths that can be written.
    #[serde(default)]
    pub write: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkPermissions {
    /// Hostnames/domains that can be accessed.
    #[serde(default)]
    pub allow: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvPermissions {
    /// Environment variables that can be read.
    #[serde(default)]
    pub allow: Vec<String>,
}

impl SkillManifest {
    /// Check if this skill should run in a forked context.
    pub fn is_forked(&self) -> bool {
        self.context.as_deref() == Some("fork")
    }

    /// Check if this skill is user-invocable (defaults to true).
    pub fn is_user_invocable(&self) -> bool {
        self.user_invocable.unwrap_or(true)
    }

    /// Get allowed tools as a vector.
    pub fn get_allowed_tools(&self) -> Vec<String> {
        self.allowed_tools
            .as_ref()
            .map(|t| t.to_vec())
            .unwrap_or_default()
    }
}

/// Validation constants from Claude Skills spec.
pub mod constraints {
    /// Maximum length for skill name.
    pub const MAX_NAME_LENGTH: usize = 64;
    /// Maximum length for skill description.
    pub const MAX_DESCRIPTION_LENGTH: usize = 1024;
    /// Valid name pattern: lowercase letters, numbers, hyphens.
    pub const NAME_PATTERN: &str = r"^[a-z0-9-]+$";
}
