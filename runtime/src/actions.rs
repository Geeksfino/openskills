//! Action/capability model: list, resolve, validate input, and build argv for skill actions.

use crate::errors::OpenSkillError;
use crate::manifest::{ActionInputSchema, SkillAction};
use crate::registry::SkillRegistry;
use serde_json::Value;
use std::collections::HashSet;

/// Descriptor for an action as returned by list_skill_actions (includes skill_id).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillActionDescriptor {
    /// Skill that provides this action.
    pub skill_id: String,
    /// Stable action id (e.g. "scaffold.create").
    pub action_id: String,
    /// Capability tags (e.g. ["skill.scaffold"]).
    pub capabilities: Vec<String>,
    /// Human-readable description if set.
    pub description: Option<String>,
    /// Whether the action declares an input schema.
    pub has_input_schema: bool,
}

/// List all actions from all skills in the registry.
pub fn list_skill_actions(registry: &SkillRegistry) -> Vec<SkillActionDescriptor> {
    let mut out = Vec::new();
    for meta in registry.all() {
        let Some(actions) = &meta.manifest.actions else {
            continue;
        };
        for action in actions {
            out.push(SkillActionDescriptor {
                skill_id: meta.id.clone(),
                action_id: action.id.clone(),
                capabilities: action.capabilities.clone(),
                description: action.description.clone(),
                has_input_schema: action.input.is_some(),
            });
        }
    }
    out
}

/// Find a (skill_id, action) that provides the given capability.
pub fn find_action_by_capability<'a>(
    registry: &'a SkillRegistry,
    capability: &str,
) -> Option<(String, &'a SkillAction)> {
    for meta in registry.all() {
        let Some(actions) = &meta.manifest.actions else {
            continue;
        };
        for action in actions {
            if action.capabilities.iter().any(|c| c == capability) {
                return Some((meta.id.clone(), action));
            }
        }
    }
    None
}

/// Find a (skill_id, action) by action id (first match wins).
pub fn find_action_by_id<'a>(
    registry: &'a SkillRegistry,
    action_id: &str,
) -> Option<(String, &'a SkillAction)> {
    for meta in registry.all() {
        let Some(actions) = &meta.manifest.actions else {
            continue;
        };
        for action in actions {
            if action.id == action_id {
                return Some((meta.id.clone(), action));
            }
        }
    }
    None
}

/// Validate input object against schema: required keys present, no extra keys.
pub fn validate_action_input(
    input: &Value,
    schema: &ActionInputSchema,
) -> Result<(), OpenSkillError> {
    let obj = input
        .as_object()
        .ok_or_else(|| OpenSkillError::InvalidActionInput("input must be a JSON object".to_string()))?;
    let keys: Vec<String> = obj.keys().cloned().collect();
    for req in &schema.required {
        if !keys.contains(req) {
            return Err(OpenSkillError::InvalidActionInput(format!(
                "missing required key: {}",
                req
            )));
        }
    }
    let allowed: HashSet<&String> = schema
        .required
        .iter()
        .chain(schema.optional.iter())
        .collect();
    for k in &keys {
        if !allowed.contains(k) {
            return Err(OpenSkillError::InvalidActionInput(format!(
                "unexpected key: {}",
                k
            )));
        }
    }
    Ok(())
}

/// Build argv for a script action from validated input.
/// Convention: first required key = first positional arg, remaining required and optional as --key value.
pub fn build_script_args(
    input: &Value,
    schema: &ActionInputSchema,
) -> Result<Vec<String>, OpenSkillError> {
    let obj = input
        .as_object()
        .ok_or_else(|| OpenSkillError::InvalidActionInput("input must be a JSON object".to_string()))?;
    let mut args = Vec::new();
    for (i, key) in schema.required.iter().enumerate() {
        let v = obj.get(key).ok_or_else(|| {
            OpenSkillError::InvalidActionInput(format!("missing required key: {}", key))
        })?;
        let s = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
        if i == 0 {
            args.push(s);
        } else {
            args.push(format!("--{}", key.replace('_', "-")));
            args.push(s);
        }
    }
    for key in &schema.optional {
        if let Some(v) = obj.get(key) {
            let s = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
            args.push(format!("--{}", key.replace('_', "-")));
            args.push(s);
        }
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ActionInputSchema;

    #[test]
    fn test_validate_action_input_ok() {
        let schema = ActionInputSchema {
            required: vec!["skill_name".to_string(), "path".to_string()],
            optional: vec!["resources".to_string()],
        };
        let input = serde_json::json!({
            "skill_name": "my-skill",
            "path": "/tmp/skills",
            "resources": "a,b"
        });
        assert!(validate_action_input(&input, &schema).is_ok());
    }

    #[test]
    fn test_validate_action_input_missing_required() {
        let schema = ActionInputSchema {
            required: vec!["skill_name".to_string(), "path".to_string()],
            optional: vec![],
        };
        let input = serde_json::json!({ "skill_name": "x" });
        assert!(validate_action_input(&input, &schema).is_err());
    }

    #[test]
    fn test_validate_action_input_extra_key() {
        let schema = ActionInputSchema {
            required: vec!["skill_name".to_string()],
            optional: vec![],
        };
        let input = serde_json::json!({ "skill_name": "x", "unknown": "y" });
        assert!(validate_action_input(&input, &schema).is_err());
    }

    #[test]
    fn test_build_script_args() {
        let schema = ActionInputSchema {
            required: vec!["skill_name".to_string(), "path".to_string()],
            optional: vec!["resources".to_string(), "examples".to_string()],
        };
        let input = serde_json::json!({
            "skill_name": "hello-world",
            "path": "skills/public",
            "resources": "refs"
        });
        let args = build_script_args(&input, &schema).unwrap();
        assert_eq!(args[0], "hello-world");
        assert_eq!(args[1], "--path");
        assert_eq!(args[2], "skills/public");
        assert_eq!(args[3], "--resources");
        assert_eq!(args[4], "refs");
    }
}
