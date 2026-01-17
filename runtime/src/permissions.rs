//! Permission enforcement for Claude Skills with WASM sandbox.
//!
//! Maps Claude Skills' `allowed-tools` to WASI capability grants.

use crate::errors::OpenSkillError;
use crate::manifest::WasmConfig;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use url::Url;

/// Permission enforcer for WASM sandbox execution.
pub struct PermissionEnforcer {
    /// Allowed tools from skill manifest.
    allowed_tools: HashSet<String>,
    /// WASM-specific configuration.
    wasm_config: WasmConfig,
    /// Skill root directory.
    skill_root: PathBuf,
}

impl PermissionEnforcer {
    /// Create a new permission enforcer.
    pub fn new(
        allowed_tools: Vec<String>,
        wasm_config: WasmConfig,
        skill_root: PathBuf,
    ) -> Self {
        Self {
            allowed_tools: allowed_tools.into_iter().collect(),
            wasm_config,
            skill_root,
        }
    }

    /// Create with default WASM config.
    pub fn with_defaults(allowed_tools: Vec<String>, skill_root: PathBuf) -> Self {
        Self::new(allowed_tools, WasmConfig::default(), skill_root)
    }

    /// Check if a tool is allowed.
    pub fn is_tool_allowed(&self, tool: &str) -> bool {
        // Empty allowed_tools means all tools are allowed (no restriction)
        if self.allowed_tools.is_empty() {
            return true;
        }
        self.allowed_tools.contains(tool)
    }

    /// Get filesystem read paths for WASI.
    pub fn filesystem_read_paths(&self) -> Vec<PathBuf> {
        self.wasm_config
            .filesystem
            .read
            .iter()
            .map(|p| self.resolve_path(p))
            .collect()
    }

    /// Get filesystem write paths for WASI.
    pub fn filesystem_write_paths(&self) -> Vec<PathBuf> {
        self.wasm_config
            .filesystem
            .write
            .iter()
            .map(|p| self.resolve_path(p))
            .collect()
    }

    /// Check if network access to a URL is allowed.
    pub fn is_network_allowed(&self, url: &str) -> Result<bool, OpenSkillError> {
        // No network hosts allowed = no network access
        if self.wasm_config.network.allow.is_empty() {
            return Ok(false);
        }

        let parsed = Url::parse(url)
            .map_err(|e| OpenSkillError::PermissionDenied(format!("Invalid URL: {e}")))?;

        let host = parsed.host_str().unwrap_or_default();

        Ok(self.wasm_config.network.allow.iter().any(|allowed| {
            host == allowed || host.ends_with(&format!(".{}", allowed))
        }))
    }

    /// Get environment variable allowlist for WASI.
    pub fn env_allowlist(&self) -> &[String] {
        &self.wasm_config.env.allow
    }

    /// Get the timeout in milliseconds.
    pub fn timeout_ms(&self) -> u64 {
        self.wasm_config.timeout_ms
    }

    /// Get the memory limit in MB.
    pub fn memory_mb(&self) -> u64 {
        self.wasm_config.memory_mb
    }

    /// Get deterministic random seed if configured.
    pub fn random_seed(&self) -> Option<u64> {
        self.wasm_config.random_seed
    }

    /// Get list of permissions being used (for audit).
    pub fn permissions_used(&self) -> Vec<String> {
        let mut used = Vec::new();

        for tool in &self.allowed_tools {
            used.push(format!("tool:{}", tool));
        }

        for p in &self.wasm_config.filesystem.read {
            used.push(format!("filesystem:read:{}", p));
        }

        for p in &self.wasm_config.filesystem.write {
            used.push(format!("filesystem:write:{}", p));
        }

        for n in &self.wasm_config.network.allow {
            used.push(format!("network:allow:{}", n));
        }

        for e in &self.wasm_config.env.allow {
            used.push(format!("env:allow:{}", e));
        }

        if let Some(seed) = self.wasm_config.random_seed {
            used.push(format!("random_seed:{}", seed));
        }

        used
    }

    /// Resolve a path relative to the skill root.
    fn resolve_path(&self, input: &str) -> PathBuf {
        let p = Path::new(input);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.skill_root.join(p)
        }
    }
}

/// Map Claude Skills tool names to WASI capabilities.
/// 
/// This provides a mapping from high-level tool names (like "Read", "Write", "Bash")
/// to WASI capability grants.
pub fn map_tools_to_capabilities(tools: &[String]) -> WasmConfig {
    let mut config = WasmConfig::default();

    for tool in tools {
        match tool.as_str() {
            "Read" | "Grep" | "Glob" | "LS" => {
                // Read access to current directory
                if !config.filesystem.read.contains(&".".to_string()) {
                    config.filesystem.read.push(".".to_string());
                }
            }
            "Write" | "Edit" | "MultiEdit" => {
                // Write access to current directory
                if !config.filesystem.write.contains(&".".to_string()) {
                    config.filesystem.write.push(".".to_string());
                }
            }
            "Bash" | "Terminal" => {
                // Full filesystem access for shell commands
                if !config.filesystem.read.contains(&".".to_string()) {
                    config.filesystem.read.push(".".to_string());
                }
                if !config.filesystem.write.contains(&".".to_string()) {
                    config.filesystem.write.push(".".to_string());
                }
            }
            "WebSearch" | "Fetch" => {
                // Network access (all hosts for simplicity)
                if !config.network.allow.contains(&"*".to_string()) {
                    config.network.allow.push("*".to_string());
                }
            }
            _ => {
                // Unknown tool, no special capabilities
            }
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_allowed_empty_list() {
        let enforcer = PermissionEnforcer::with_defaults(vec![], PathBuf::from("."));
        // Empty list means all tools allowed
        assert!(enforcer.is_tool_allowed("Read"));
        assert!(enforcer.is_tool_allowed("Write"));
    }

    #[test]
    fn test_tool_allowed_restricted() {
        let enforcer = PermissionEnforcer::with_defaults(
            vec!["Read".to_string(), "Grep".to_string()],
            PathBuf::from("."),
        );
        assert!(enforcer.is_tool_allowed("Read"));
        assert!(enforcer.is_tool_allowed("Grep"));
        assert!(!enforcer.is_tool_allowed("Write"));
    }

    #[test]
    fn test_network_allowed() {
        let mut config = WasmConfig::default();
        config.network.allow = vec!["api.example.com".to_string()];
        
        let enforcer = PermissionEnforcer::new(vec![], config, PathBuf::from("."));
        
        assert!(enforcer.is_network_allowed("https://api.example.com/v1").unwrap());
        assert!(enforcer.is_network_allowed("https://sub.api.example.com/v1").unwrap());
        assert!(!enforcer.is_network_allowed("https://other.com").unwrap());
    }

    #[test]
    fn test_map_tools_read() {
        let config = map_tools_to_capabilities(&["Read".to_string(), "Grep".to_string()]);
        assert!(config.filesystem.read.contains(&".".to_string()));
        assert!(config.filesystem.write.is_empty());
    }

    #[test]
    fn test_map_tools_write() {
        let config = map_tools_to_capabilities(&["Write".to_string()]);
        assert!(config.filesystem.write.contains(&".".to_string()));
    }
}
