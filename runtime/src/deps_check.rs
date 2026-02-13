//! Dependency checking for skills (OpenClaw-compatible requires.bins / requires.env).

use crate::manifest::SkillRequires;
use std::process::Command;

/// Check if a binary exists in PATH.
pub fn check_binary_in_path(name: &str) -> bool {
    let (cmd, args) = if cfg!(target_os = "windows") {
        ("where", vec![name])
    } else {
        ("which", vec![name])
    };
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if an environment variable is set and non-empty.
pub fn check_env_set(key: &str) -> bool {
    std::env::var(key).is_ok_and(|v| !v.trim().is_empty())
}

/// Result of checking skill requirements.
#[derive(Debug, Clone, Default)]
pub struct MissingDependencies {
    pub bins: Vec<String>,
    pub env: Vec<String>,
}

impl MissingDependencies {
    pub fn is_empty(&self) -> bool {
        self.bins.is_empty() && self.env.is_empty()
    }
}

/// Check skill requires and return missing bins and env vars.
pub fn check_requires(requires: Option<&SkillRequires>) -> MissingDependencies {
    let mut missing = MissingDependencies::default();
    let Some(requires) = requires else {
        return missing;
    };
    for bin in &requires.bins {
        if !check_binary_in_path(bin) {
            missing.bins.push(bin.clone());
        }
    }
    for key in &requires.env {
        if !check_env_set(key) {
            missing.env.push(key.clone());
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_env_set_empty() {
        // Unset or empty env should be false
        std::env::remove_var("__OPENSKILLS_TEST_EMPTY__");
        assert!(!check_env_set("__OPENSKILLS_TEST_EMPTY__"));
    }

    #[test]
    fn test_check_requires_none() {
        let m = check_requires(None);
        assert!(m.is_empty());
    }

    #[test]
    fn test_check_requires_empty() {
        let r = SkillRequires {
            bins: vec![],
            env: vec![],
        };
        let m = check_requires(Some(&r));
        assert!(m.is_empty());
    }

    #[test]
    fn test_check_binary_in_path_common() {
        // Should have at least one of these in PATH in CI/local
        let has_any = check_binary_in_path("true")
            || check_binary_in_path("false")
            || check_binary_in_path("echo");
        assert!(has_any);
    }
}
