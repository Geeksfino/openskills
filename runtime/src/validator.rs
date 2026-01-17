//! Validation for Claude Skills.

use crate::errors::OpenSkillError;
use crate::manifest::{constraints, SkillManifest};
use crate::registry::Skill;

/// Validate a skill's manifest against Claude Skills spec constraints.
pub fn validate_skill(skill: &Skill) -> Result<(), OpenSkillError> {
    validate_manifest(&skill.manifest)?;
    
    // Validate directory name matches manifest name
    if skill.id != skill.manifest.name {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Directory name '{}' must match skill name '{}'",
            skill.id, skill.manifest.name
        )));
    }

    Ok(())
}

/// Validate a skill manifest.
pub fn validate_manifest(manifest: &SkillManifest) -> Result<(), OpenSkillError> {
    // Validate name
    validate_name(&manifest.name)?;
    
    // Validate description
    validate_description(&manifest.description)?;
    
    // Validate context value if present
    if let Some(ref ctx) = manifest.context {
        if ctx != "fork" {
            return Err(OpenSkillError::InvalidManifest(format!(
                "Invalid context value '{}', must be 'fork' or absent",
                ctx
            )));
        }
    }
    
    Ok(())
}

/// Validate skill name according to Claude Skills spec.
pub fn validate_name(name: &str) -> Result<(), OpenSkillError> {
    if name.is_empty() {
        return Err(OpenSkillError::InvalidManifest(
            "Skill name is required".to_string(),
        ));
    }

    if name.len() > constraints::MAX_NAME_LENGTH {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Skill name exceeds {} characters",
            constraints::MAX_NAME_LENGTH
        )));
    }

    // Must be lowercase letters, numbers, hyphens only
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Skill name '{}' must contain only lowercase letters, numbers, and hyphens",
            name
        )));
    }

    // Cannot contain XML-like tags
    if name.contains('<') || name.contains('>') {
        return Err(OpenSkillError::InvalidManifest(
            "Skill name cannot contain XML tags".to_string(),
        ));
    }

    // Reserved words check (example subset)
    let reserved = ["anthropic", "claude", "skill", "system"];
    if reserved.contains(&name) {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Skill name '{}' is a reserved word",
            name
        )));
    }

    Ok(())
}

/// Validate skill description according to Claude Skills spec.
pub fn validate_description(description: &str) -> Result<(), OpenSkillError> {
    if description.is_empty() {
        return Err(OpenSkillError::InvalidManifest(
            "Skill description is required".to_string(),
        ));
    }

    if description.len() > constraints::MAX_DESCRIPTION_LENGTH {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Skill description exceeds {} characters",
            constraints::MAX_DESCRIPTION_LENGTH
        )));
    }

    // Cannot contain XML-like tags
    if description.contains('<') || description.contains('>') {
        return Err(OpenSkillError::InvalidManifest(
            "Skill description cannot contain XML tags".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name_valid() {
        assert!(validate_name("my-skill").is_ok());
        assert!(validate_name("skill123").is_ok());
        assert!(validate_name("a").is_ok());
    }

    #[test]
    fn test_validate_name_invalid() {
        assert!(validate_name("").is_err());
        assert!(validate_name("My-Skill").is_err()); // uppercase
        assert!(validate_name("my_skill").is_err()); // underscore
        assert!(validate_name("my skill").is_err()); // space
        assert!(validate_name("<script>").is_err()); // XML
    }

    #[test]
    fn test_validate_name_reserved() {
        assert!(validate_name("claude").is_err());
        assert!(validate_name("anthropic").is_err());
    }

    #[test]
    fn test_validate_description_valid() {
        assert!(validate_description("A helpful skill.").is_ok());
    }

    #[test]
    fn test_validate_description_invalid() {
        assert!(validate_description("").is_err());
        assert!(validate_description("<script>bad</script>").is_err());
    }
}
