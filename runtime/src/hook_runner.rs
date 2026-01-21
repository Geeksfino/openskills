//! Hook execution pipeline for skill lifecycle events.
//!
//! Implements PreToolUse, PostToolUse, and Stop hooks with matcher support,
//! sandboxed execution, and timeout handling.

use crate::errors::OpenSkillError;
use crate::executor::{run_sandboxed_command, CommandPermissions, CommandResult};
use crate::manifest::{HookEntry, HooksConfig};
use glob::Pattern;
use std::path::PathBuf;

/// Hook event types that can trigger hook execution.
#[derive(Debug, Clone)]
pub enum HookEvent {
    /// Before a tool is used.
    PreToolUse {
        /// Name of the tool being used.
        tool_name: String,
        /// Input to the tool (JSON string).
        tool_input: String,
    },
    /// After a tool is used.
    PostToolUse {
        /// Name of the tool that was used.
        tool_name: String,
        /// Output from the tool (JSON string).
        tool_output: String,
    },
    /// When the skill execution stops.
    Stop {
        /// Reason for stopping.
        reason: String,
    },
}

/// Hook runner that executes matching hooks for skill lifecycle events.
pub struct HookRunner {
    hooks: HooksConfig,
    skill_root: PathBuf,
}

impl HookRunner {
    /// Create a new hook runner.
    pub fn new(hooks: HooksConfig, skill_root: PathBuf) -> Self {
        Self { hooks, skill_root }
    }

    /// Execute matching hooks for an event.
    ///
    /// Returns a vector of command results, one for each matching hook that was executed.
    pub fn execute(&self, event: &HookEvent) -> Result<Vec<CommandResult>, OpenSkillError> {
        let entries = self.matching_hooks(event);
        let mut results = Vec::new();

        for entry in entries {
            let cwd = entry
                .cwd
                .as_ref()
                .map(|c| self.skill_root.join(c))
                .unwrap_or_else(|| self.skill_root.clone());

            let timeout_ms = entry.timeout_ms.unwrap_or(30000);
            let perms = CommandPermissions {
                read_paths: vec![self.skill_root.clone()],
                timeout_ms,
                ..Default::default()
            };

            let result = run_sandboxed_command(&entry.command, &cwd, perms)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Find hooks that match the given event.
    fn matching_hooks(&self, event: &HookEvent) -> Vec<&HookEntry> {
        let (entries, tool_name) = match event {
            HookEvent::PreToolUse { tool_name, .. } => {
                (self.hooks.pre_tool_use.as_ref(), Some(tool_name))
            }
            HookEvent::PostToolUse { tool_name, .. } => {
                (self.hooks.post_tool_use.as_ref(), Some(tool_name))
            }
            HookEvent::Stop { .. } => (self.hooks.stop.as_ref(), None),
        };

        entries
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| {
                        match (&entry.matcher, tool_name) {
                            (None, _) => true, // No matcher = match all
                            (Some(pattern), Some(name)) => {
                                Pattern::new(pattern)
                                    .map(|p| p.matches(name))
                                    .unwrap_or(false)
                            }
                            (Some(_), None) => true, // Stop events match all
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
