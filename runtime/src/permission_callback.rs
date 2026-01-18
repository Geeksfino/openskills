//! Permission callback system for ask-before-act security model.
//!
//! This module provides a callback-based permission system that allows
//! runtime users to approve or deny potentially dangerous operations
//! before they execute.

use crate::errors::OpenSkillError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Risk level for permission requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Low risk: Read operations
    Low,
    /// Medium risk: Write operations
    Medium,
    /// High risk: Bash, network, destructive operations
    High,
}

/// Permission request context for user approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// Skill requesting permission.
    pub skill_id: String,
    /// Tool/capability being requested.
    pub tool: String,
    /// Human-readable description of what will happen.
    pub description: String,
    /// Risk level: low, medium, high.
    pub risk_level: RiskLevel,
    /// Additional context (e.g., file path, command).
    pub context: HashMap<String, String>,
}

/// User's response to a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionResponse {
    /// Allow this operation once.
    AllowOnce,
    /// Allow all operations of this type for this skill.
    AllowAlways,
    /// Deny this operation.
    Deny,
}

/// Callback trait for requesting user permissions.
///
/// Implement this trait to provide custom permission prompts (CLI, GUI, etc.).
pub trait PermissionCallback: Send + Sync {
    /// Request permission for a potentially dangerous operation.
    ///
    /// Returns:
    /// - `Ok(PermissionResponse)` if user made a decision
    /// - `Err(OpenSkillError)` if permission system failed
    fn request_permission(
        &self,
        request: &PermissionRequest,
    ) -> Result<PermissionResponse, OpenSkillError>;
}

/// Permission manager that tracks approvals and denials.
pub struct PermissionManager {
    callback: Option<Arc<dyn PermissionCallback>>,
    // Track "allow always" grants: (skill_id, tool) -> granted
    always_allowed: Arc<Mutex<HashMap<(String, String), bool>>>,
    // Audit log of permission requests
    audit_log: Arc<Mutex<Vec<PermissionAuditEntry>>>,
}

impl std::fmt::Debug for PermissionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermissionManager")
            .field("has_callback", &self.callback.is_some())
            .field("always_allowed_count", &self.always_allowed.lock().unwrap().len())
            .field("audit_log_count", &self.audit_log.lock().unwrap().len())
            .finish()
    }
}

/// Audit entry for a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionAuditEntry {
    pub timestamp: u64,
    pub skill_id: String,
    pub tool: String,
    pub response: PermissionResponse,
}

impl PermissionManager {
    /// Create a new permission manager with no callback (auto-allow).
    pub fn new() -> Self {
        Self {
            callback: None,
            always_allowed: Arc::new(Mutex::new(HashMap::new())),
            audit_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a permission manager with a callback.
    pub fn with_callback(callback: Arc<dyn PermissionCallback>) -> Self {
        Self {
            callback: Some(callback),
            always_allowed: Arc::new(Mutex::new(HashMap::new())),
            audit_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Check if permission is granted for this operation.
    pub fn check_permission(
        &self,
        skill_id: &str,
        tool: &str,
        description: String,
        risk_level: RiskLevel,
        context: HashMap<String, String>,
    ) -> Result<bool, OpenSkillError> {
        // Check if previously granted "allow always"
        let key = (skill_id.to_string(), tool.to_string());
        {
            let always_allowed = self.always_allowed.lock().unwrap();
            if let Some(&granted) = always_allowed.get(&key) {
                return Ok(granted);
            }
        }

        // No callback means auto-allow (for backward compatibility)
        let Some(ref callback) = self.callback else {
            return Ok(true);
        };

        // Request permission
        let request = PermissionRequest {
            skill_id: skill_id.to_string(),
            tool: tool.to_string(),
            description,
            risk_level,
            context,
        };

        let response = callback.request_permission(&request)?;

        // Record audit
        self.record_permission_audit(skill_id, tool, response.clone());

        match response {
            PermissionResponse::AllowOnce => Ok(true),
            PermissionResponse::AllowAlways => {
                // Grant permanently for this (skill, tool) pair
                let mut always_allowed = self.always_allowed.lock().unwrap();
                always_allowed.insert(key, true);
                Ok(true)
            }
            PermissionResponse::Deny => Ok(false),
        }
    }

    fn record_permission_audit(&self, skill_id: &str, tool: &str, response: PermissionResponse) {
        let entry = PermissionAuditEntry {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            skill_id: skill_id.to_string(),
            tool: tool.to_string(),
            response,
        };

        let mut audit_log = self.audit_log.lock().unwrap();
        audit_log.push(entry);
    }

    /// Get permission audit log.
    pub fn get_audit_log(&self) -> Vec<PermissionAuditEntry> {
        let audit_log = self.audit_log.lock().unwrap();
        audit_log.clone()
    }

    /// Reset all "allow always" grants (for testing or security).
    pub fn reset_grants(&self) {
        let mut always_allowed = self.always_allowed.lock().unwrap();
        always_allowed.clear();
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Deny-all callback for testing or strict security.
pub struct DenyAllCallback;

impl PermissionCallback for DenyAllCallback {
    fn request_permission(
        &self,
        _request: &PermissionRequest,
    ) -> Result<PermissionResponse, OpenSkillError> {
        Ok(PermissionResponse::Deny)
    }
}

/// CLI callback for terminal-based permission prompts.
pub struct CliPermissionCallback;

impl PermissionCallback for CliPermissionCallback {
    fn request_permission(
        &self,
        request: &PermissionRequest,
    ) -> Result<PermissionResponse, OpenSkillError> {
        use std::io::{self, Write};

        println!("\n⚠️  Permission Required");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Skill:       {}", request.skill_id);
        println!("Tool:        {}", request.tool);
        println!("Risk Level:  {:?}", request.risk_level);
        println!("Description: {}", request.description);

        if !request.context.is_empty() {
            println!("\nContext:");
            for (key, value) in &request.context {
                println!("  {}: {}", key, value);
            }
        }

        println!("\nChoices:");
        println!("  [1] Allow once");
        println!("  [2] Allow always (for this skill + tool)");
        println!("  [3] Deny");
        print!("\nYour choice (1-3): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(|e| {
            OpenSkillError::PermissionDenied(format!("Failed to read input: {e}"))
        })?;

        match input.trim() {
            "1" => Ok(PermissionResponse::AllowOnce),
            "2" => Ok(PermissionResponse::AllowAlways),
            "3" | "" => Ok(PermissionResponse::Deny),
            _ => {
                println!("Invalid choice. Denying by default.");
                Ok(PermissionResponse::Deny)
            }
        }
    }
}

/// Helper function to determine if a tool is risky and needs permission.
pub fn is_risky_tool(tool: &str) -> bool {
    matches!(
        tool,
        "Write" | "Edit" | "MultiEdit" | "Bash" | "Terminal" | "WebSearch" | "Fetch"
    )
}

/// Get risk level for a tool.
pub fn get_risk_level(tool: &str) -> RiskLevel {
    match tool {
        "Read" | "Grep" | "Glob" | "LS" => RiskLevel::Low,
        "Write" | "Edit" | "MultiEdit" => RiskLevel::Medium,
        "Bash" | "Terminal" | "WebSearch" | "Fetch" => RiskLevel::High,
        _ => RiskLevel::Medium,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_manager_auto_allow() {
        let manager = PermissionManager::new();
        let granted = manager
            .check_permission(
                "test-skill",
                "Write",
                "Write file".to_string(),
                RiskLevel::Medium,
                HashMap::new(),
            )
            .unwrap();
        assert!(granted); // Auto-allows when no callback
    }

    #[test]
    fn test_permission_manager_deny_all() {
        let manager = PermissionManager::with_callback(Arc::new(DenyAllCallback));
        let granted = manager
            .check_permission(
                "test-skill",
                "Write",
                "Write file".to_string(),
                RiskLevel::Medium,
                HashMap::new(),
            )
            .unwrap();
        assert!(!granted);
    }

    #[test]
    fn test_permission_manager_allow_always() {
        struct AllowAlwaysCallback;
        impl PermissionCallback for AllowAlwaysCallback {
            fn request_permission(
                &self,
                _request: &PermissionRequest,
            ) -> Result<PermissionResponse, OpenSkillError> {
                Ok(PermissionResponse::AllowAlways)
            }
        }

        let manager = PermissionManager::with_callback(Arc::new(AllowAlwaysCallback));

        // First request
        let granted1 = manager
            .check_permission(
                "test-skill",
                "Write",
                "Write file".to_string(),
                RiskLevel::Medium,
                HashMap::new(),
            )
            .unwrap();
        assert!(granted1);

        // Second request should be auto-approved
        let granted2 = manager
            .check_permission(
                "test-skill",
                "Write",
                "Write file".to_string(),
                RiskLevel::Medium,
                HashMap::new(),
            )
            .unwrap();
        assert!(granted2);

        // Check audit log
        let audit = manager.get_audit_log();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].skill_id, "test-skill");
        assert_eq!(audit[0].tool, "Write");
    }

    #[test]
    fn test_is_risky_tool() {
        assert!(!is_risky_tool("Read"));
        assert!(is_risky_tool("Write"));
        assert!(is_risky_tool("Bash"));
        assert!(is_risky_tool("WebSearch"));
    }

    #[test]
    fn test_get_risk_level() {
        assert_eq!(get_risk_level("Read"), RiskLevel::Low);
        assert_eq!(get_risk_level("Write"), RiskLevel::Medium);
        assert_eq!(get_risk_level("Bash"), RiskLevel::High);
    }
}
