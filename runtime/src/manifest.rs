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

    /// SPDX license identifier (e.g., "MIT", "Apache-2.0").
    #[serde(default)]
    pub license: Option<String>,

    /// Compatibility requirements (min/max version, platform, etc.).
    #[serde(default)]
    pub compatibility: Option<CompatibilityConfig>,

    /// Additional metadata (author, repository, keywords).
    #[serde(default)]
    pub metadata: Option<SkillMetadataInfo>,
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
    /// Supports comma-separated, space-separated, or YAML list formats.
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            AllowedTools::List(v) => v.clone(),
            AllowedTools::CommaSeparated(s) => {
                // Support both comma-delimited AND space-delimited
                s.split(|c| c == ',' || c == ' ')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            }
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

/// Compatibility configuration for skill requirements.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompatibilityConfig {
    /// Minimum required version.
    #[serde(default)]
    pub min_version: Option<String>,
    /// Maximum supported version.
    #[serde(default)]
    pub max_version: Option<String>,
    /// Supported platforms.
    #[serde(default)]
    pub platforms: Option<Vec<String>>,
}

/// Additional metadata about the skill.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillMetadataInfo {
    /// Author of the skill.
    #[serde(default)]
    pub author: Option<String>,
    /// Repository URL.
    #[serde(default)]
    pub repository: Option<String>,
    /// Keywords for discovery.
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    /// Homepage URL.
    #[serde(default)]
    pub homepage: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_tools_space_delimited() {
        let tools = AllowedTools::CommaSeparated("Read Write Bash".to_string());
        assert_eq!(tools.to_vec(), vec!["Read", "Write", "Bash"]);
    }

    #[test]
    fn test_allowed_tools_comma_delimited() {
        let tools = AllowedTools::CommaSeparated("Read, Write, Bash".to_string());
        assert_eq!(tools.to_vec(), vec!["Read", "Write", "Bash"]);
    }

    #[test]
    fn test_allowed_tools_mixed_delimiters() {
        let tools = AllowedTools::CommaSeparated("Read, Write Bash".to_string());
        assert_eq!(tools.to_vec(), vec!["Read", "Write", "Bash"]);
    }

    #[test]
    fn test_allowed_tools_yaml_list() {
        let tools = AllowedTools::List(vec!["Read".to_string(), "Write".to_string(), "Bash".to_string()]);
        assert_eq!(tools.to_vec(), vec!["Read", "Write", "Bash"]);
    }

    #[test]
    fn test_parse_model_field() {
        let yaml = r#"name: test-skill
description: Test skill
model: claude-sonnet-4-20250514"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.model, Some("claude-sonnet-4-20250514".to_string()));
    }

    #[test]
    fn test_parse_agent_field() {
        let yaml = r#"name: test-skill
description: Test skill
context: fork
agent: Explore"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.agent, Some("Explore".to_string()));
    }

    #[test]
    fn test_user_invocable_defaults_to_true() {
        let yaml = r#"name: test-skill
description: Test skill"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.is_user_invocable());
    }

    #[test]
    fn test_user_invocable_explicit_false() {
        let yaml = r#"name: test-skill
description: Test skill
user-invocable: false"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(!manifest.is_user_invocable());
    }

    #[test]
    fn test_parse_license_field() {
        let yaml = r#"name: test-skill
description: Test skill
license: MIT"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.license, Some("MIT".to_string()));
    }

    #[test]
    fn test_parse_compatibility_field() {
        let yaml = r#"name: test-skill
description: Test skill
compatibility:
  min_version: "1.0.0"
  max_version: "2.0.0"
  platforms:
    - macos
    - linux"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.compatibility.is_some());
        let compat = manifest.compatibility.unwrap();
        assert_eq!(compat.min_version, Some("1.0.0".to_string()));
        assert_eq!(compat.max_version, Some("2.0.0".to_string()));
        assert_eq!(compat.platforms, Some(vec!["macos".to_string(), "linux".to_string()]));
    }

    #[test]
    fn test_parse_metadata_field() {
        let yaml = r#"name: test-skill
description: Test skill
metadata:
  author: "Test Author"
  repository: "https://github.com/test/skill"
  keywords:
    - test
    - skill
  homepage: "https://example.com"
"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.metadata.is_some());
        let meta = manifest.metadata.unwrap();
        assert_eq!(meta.author, Some("Test Author".to_string()));
        assert_eq!(meta.repository, Some("https://github.com/test/skill".to_string()));
        assert_eq!(meta.keywords, Some(vec!["test".to_string(), "skill".to_string()]));
        assert_eq!(meta.homepage, Some("https://example.com".to_string()));
    }
}
