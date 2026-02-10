//! Host policy for the OpenSkills permission model (Layer 2).
//!
//! The host policy sits between skill declarations (Layer 1: SKILL.md allowed-tools)
//! and sandbox enforcement (Layer 3: WASM/seatbelt). It lets the host developer
//! control which tools are actually granted to skills.
//!
//! Resolution algorithm (first match wins):
//! 1. Tool in deny_overrides  → DENIED
//! 2. Tool in allow_overrides → APPROVED
//! 3. trust_skill_allowed_tools AND tool in skill's allowed-tools → APPROVED
//! 4. fallback = allow → APPROVED, deny → DENIED, prompt → delegate to callback

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Fallback behavior for tools not covered by overrides or skill pre-approvals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fallback {
    Allow,
    Deny,
    Prompt,
}

impl Default for Fallback {
    fn default() -> Self {
        Fallback::Deny
    }
}

/// Result of the host policy resolution for a single tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolDecision {
    /// Tool is approved by host policy.
    Approved,
    /// Tool is denied by host policy.
    Denied,
    /// Tool needs interactive approval (fallback = prompt).
    Prompt,
}

/// Permissions configuration for the host policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsConfig {
    #[serde(default = "default_trust")]
    pub trust_skill_allowed_tools: bool,

    #[serde(default)]
    pub fallback: Fallback,

    #[serde(default)]
    pub deny: Vec<String>,

    #[serde(default)]
    pub allow: Vec<String>,
}

fn default_trust() -> bool {
    true
}

impl Default for PermissionsConfig {
    fn default() -> Self {
        Self {
            trust_skill_allowed_tools: true,
            fallback: Fallback::Deny,
            deny: Vec::new(),
            allow: Vec::new(),
        }
    }
}

/// Host policy controlling which tools skills are granted.
///
/// Set programmatically via `OpenSkillRuntime::with_host_policy()` builder
/// or `OpenSkillRuntime::set_host_policy()`. Defaults to trust=true, fallback=deny.
#[derive(Debug, Clone)]
pub struct HostPolicy {
    pub trust_skill_allowed_tools: bool,
    pub fallback: Fallback,
    deny_overrides: HashSet<String>,
    allow_overrides: HashSet<String>,
}

impl Default for HostPolicy {
    fn default() -> Self {
        Self {
            trust_skill_allowed_tools: true,
            fallback: Fallback::Deny,
            deny_overrides: HashSet::new(),
            allow_overrides: HashSet::new(),
        }
    }
}

impl HostPolicy {
    /// Build a HostPolicy from a parsed permissions config.
    pub fn from_config(config: PermissionsConfig) -> Self {
        Self {
            trust_skill_allowed_tools: config.trust_skill_allowed_tools,
            fallback: config.fallback,
            deny_overrides: config.deny.into_iter().collect(),
            allow_overrides: config.allow.into_iter().collect(),
        }
    }

    /// Resolve whether a tool is approved, denied, or needs prompting.
    ///
    /// Implements the resolution algorithm from docs/permissions.md:
    /// 1. deny_overrides → DENIED
    /// 2. allow_overrides → APPROVED
    /// 3. trust + tool in skill's allowed-tools → APPROVED
    ///    (empty allowed-tools = nothing pre-approved, per Claude spec)
    /// 4. fallback
    pub fn resolve_tool(&self, tool: &str, skill_allowed_tools: &[String]) -> ToolDecision {
        // Step 1: deny overrides always win
        if self.deny_overrides.contains(tool) {
            return ToolDecision::Denied;
        }

        // Step 2: allow overrides
        if self.allow_overrides.contains(tool) {
            return ToolDecision::Approved;
        }

        // Step 3: trust skill's allowed-tools declaration
        // Per Claude spec: empty allowed-tools means no tools are pre-approved.
        if self.trust_skill_allowed_tools && skill_allowed_tools.iter().any(|t| t == tool) {
            return ToolDecision::Approved;
        }

        // Step 4: fallback
        match self.fallback {
            Fallback::Allow => ToolDecision::Approved,
            Fallback::Deny => ToolDecision::Denied,
            Fallback::Prompt => ToolDecision::Prompt,
        }
    }

    /// Get the set of allow overrides (for resolve_skill_permissions in lib.rs).
    pub fn allow_overrides(&self) -> &HashSet<String> {
        &self.allow_overrides
    }

    /// Get the set of deny overrides.
    pub fn deny_overrides(&self) -> &HashSet<String> {
        &self.deny_overrides
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(trust: bool, fallback: Fallback, deny: Vec<&str>, allow: Vec<&str>) -> HostPolicy {
        HostPolicy::from_config(PermissionsConfig {
            trust_skill_allowed_tools: trust,
            fallback,
            deny: deny.into_iter().map(String::from).collect(),
            allow: allow.into_iter().map(String::from).collect(),
        })
    }

    fn tools(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn deny_override_wins_over_everything() {
        // Tool in both deny and allow → denied (step 1 beats step 2)
        let p = policy(true, Fallback::Allow, vec!["Bash"], vec!["Bash"]);
        assert_eq!(p.resolve_tool("Bash", &tools(&["Bash"])), ToolDecision::Denied);
    }

    #[test]
    fn allow_override_wins_over_fallback_deny() {
        let p = policy(true, Fallback::Deny, vec![], vec!["Write"]);
        assert_eq!(p.resolve_tool("Write", &[]), ToolDecision::Approved);
    }

    #[test]
    fn trust_approves_skill_allowed_tools() {
        let p = policy(true, Fallback::Deny, vec![], vec![]);
        assert_eq!(p.resolve_tool("Read", &tools(&["Read", "Grep"])), ToolDecision::Approved);
    }

    #[test]
    fn trust_false_skips_step3() {
        let p = policy(false, Fallback::Deny, vec![], vec![]);
        // Even though Read is in allowed-tools, trust=false means step 3 is skipped
        assert_eq!(p.resolve_tool("Read", &tools(&["Read"])), ToolDecision::Denied);
    }

    #[test]
    fn empty_allowed_tools_nothing_preapproved() {
        // Per Claude spec: empty allowed-tools = no tools pre-approved
        let p = policy(true, Fallback::Deny, vec![], vec![]);
        assert_eq!(p.resolve_tool("Read", &[]), ToolDecision::Denied);
    }

    #[test]
    fn fallback_allow() {
        let p = policy(true, Fallback::Allow, vec![], vec![]);
        assert_eq!(p.resolve_tool("Unknown", &tools(&["Read"])), ToolDecision::Approved);
    }

    #[test]
    fn fallback_deny() {
        let p = policy(true, Fallback::Deny, vec![], vec![]);
        assert_eq!(p.resolve_tool("Unknown", &tools(&["Read"])), ToolDecision::Denied);
    }

    #[test]
    fn fallback_prompt() {
        let p = policy(true, Fallback::Prompt, vec![], vec![]);
        assert_eq!(p.resolve_tool("Unknown", &tools(&["Read"])), ToolDecision::Prompt);
    }

    #[test]
    fn deny_override_blocks_skill_declared_tool() {
        let p = policy(true, Fallback::Allow, vec!["Bash"], vec![]);
        // Bash is in skill's allowed-tools but denied by host
        assert_eq!(p.resolve_tool("Bash", &tools(&["Read", "Bash"])), ToolDecision::Denied);
    }

    #[test]
    fn default_policy() {
        let p = HostPolicy::default();
        // Default: trust=true, fallback=deny, no overrides
        assert_eq!(p.resolve_tool("Read", &tools(&["Read"])), ToolDecision::Approved);
        assert_eq!(p.resolve_tool("Write", &tools(&["Read"])), ToolDecision::Denied);
    }

}
