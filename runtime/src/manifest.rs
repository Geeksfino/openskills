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

    /// SPDX license identifier (e.g., "Apache-2.0", "MIT").
    #[serde(default)]
    pub license: Option<String>,

    /// Compatibility requirements (min/max version, platform, etc.).
    #[serde(default)]
    pub compatibility: Option<CompatibilityConfig>,

    /// Additional metadata (author, repository, keywords).
    #[serde(default)]
    pub metadata: Option<SkillMetadataInfo>,

    /// OpenClaw-compatible dependency requirements (bins in PATH, env vars set).
    #[serde(default)]
    pub requires: Option<SkillRequires>,

    /// OpenSkills action/capability descriptors (machine-readable actions this skill provides).
    #[serde(default)]
    pub actions: Option<Vec<SkillAction>>,
}

/// Machine-readable action descriptor (OpenSkills extension).
///
/// Declares a stable action id, capability tags, target (script or WASM), and input contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SkillAction {
    /// Stable action id (e.g. "scaffold.create").
    pub id: String,
    /// Semantic capability tags (e.g. ["skill.scaffold"]). Used for capability-based resolution.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Human-readable description (optional).
    #[serde(default)]
    pub description: Option<String>,
    /// Target to execute (script path or WASM path within the skill).
    pub target: ActionTarget,
    /// Input contract for validation and argv building.
    #[serde(default)]
    pub input: Option<ActionInputSchema>,
    /// Optional notes for embedding systems.
    #[serde(default)]
    pub notes: Option<String>,
}

/// How to run the action (script or WASM).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionTarget {
    /// Run a script (e.g. scripts/init_skill.py). Args built from action input.
    Script { path: String },
    /// Run a WASM module. Input passed as JSON.
    Wasm { path: String },
}

/// Minimal input contract: required and optional keys. Extra keys are rejected.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ActionInputSchema {
    /// Required top-level keys (all must be present).
    #[serde(default)]
    pub required: Vec<String>,
    /// Optional keys (may be present).
    #[serde(default)]
    pub optional: Vec<String>,
}

/// OpenClaw-compatible dependency requirements (requires.bins, requires.env in SKILL.md).
/// Extended with optional package-level dependency metadata for embedding systems to
/// resolve or preflight; OpenSkills does not install packages, only reports what is declared.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct SkillRequires {
    /// Required binaries (must be in PATH).
    #[serde(default)]
    pub bins: Vec<String>,
    /// Required environment variables (must be set and non-empty).
    #[serde(default)]
    pub env: Vec<String>,
    /// Declarative Python package names (e.g. pip installable). Metadata only; host provisions.
    #[serde(default)]
    pub python_packages: Vec<String>,
    /// Declarative Node package names or CLI names (e.g. npm installable). Metadata only.
    #[serde(default)]
    pub node_packages: Vec<String>,
    /// Declarative Rust crate or CLI names. Metadata only.
    #[serde(default)]
    pub rust_crates: Vec<String>,
    /// Declarative system package names (e.g. apt, dnf, brew). Metadata only.
    #[serde(default)]
    pub system_packages: Vec<String>,
    /// Platforms where this skill is applicable (e.g. ["linux", "macos"]). Optional constraint.
    #[serde(default)]
    pub platforms: Vec<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            module: None,
            timeout_ms: default_timeout_ms(),
            memory_mb: default_memory_mb(),
            filesystem: FilesystemPermissions::default(),
            network: NetworkPermissions::default(),
            env: EnvPermissions::default(),
            random_seed: None,
        }
    }
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
    fn test_wasm_config_default_timeout_and_memory() {
        let config = WasmConfig::default();
        assert_eq!(config.timeout_ms, 30_000, "WasmConfig::default() must use 30s timeout");
        assert_eq!(config.memory_mb, 128, "WasmConfig::default() must use 128 MB");
    }

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
    fn test_parse_requires_field() {
        let yaml = r#"name: git-workflow
description: Git operations
requires:
  bins:
    - git
  env:
    - GITHUB_TOKEN"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.requires.is_some());
        let r = manifest.requires.unwrap();
        assert_eq!(r.bins, vec!["git"]);
        assert_eq!(r.env, vec!["GITHUB_TOKEN"]);
        assert!(r.python_packages.is_empty());
        assert!(r.platforms.is_empty());
    }

    #[test]
    fn test_parse_requires_package_metadata() {
        let yaml = r#"name: data-toolkit
description: Data processing
requires:
  bins: [python3]
  python-packages: [pyyaml, pandas]
  node-packages: [typescript]
  platforms: [linux, macos]"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        let r = manifest.requires.unwrap();
        assert_eq!(r.bins, vec!["python3"]);
        assert_eq!(r.python_packages, vec!["pyyaml", "pandas"]);
        assert_eq!(r.node_packages, vec!["typescript"]);
        assert_eq!(r.platforms, vec!["linux", "macos"]);
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

    #[test]
    fn test_parse_actions_field() {
        let yaml = r#"name: skill-creator
description: Creates new skills
actions:
  - id: scaffold.create
    capabilities: [skill.scaffold]
    description: Create a new skill from template
    target:
      type: script
      path: scripts/init_skill.py
    input:
      required: [skill_name, path]
      optional: [resources, examples]
"#;
        let manifest: SkillManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.actions.is_some());
        let actions = manifest.actions.unwrap();
        assert_eq!(actions.len(), 1);
        let a = &actions[0];
        assert_eq!(a.id, "scaffold.create");
        assert_eq!(a.capabilities, vec!["skill.scaffold"]);
        assert_eq!(a.description.as_deref(), Some("Create a new skill from template"));
        match &a.target {
            ActionTarget::Script { path } => assert_eq!(path, "scripts/init_skill.py"),
            _ => panic!("expected script target"),
        }
        let input = a.input.as_ref().unwrap();
        assert_eq!(input.required, vec!["skill_name", "path"]);
        assert_eq!(input.optional, vec!["resources", "examples"]);
    }
}
