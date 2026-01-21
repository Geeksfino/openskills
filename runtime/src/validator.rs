//! Validation for Claude Skills.

use crate::errors::OpenSkillError;
use crate::manifest::{constraints, SkillManifest};
use crate::registry::Skill;
use crate::skill_parser::parse_skill_md;
use serde::Serialize;
use std::fs;
use std::path::Path;

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

    // No leading hyphen
    if name.starts_with('-') {
        return Err(OpenSkillError::InvalidManifest(
            "Skill name cannot start with a hyphen".to_string(),
        ));
    }

    // No trailing hyphen
    if name.ends_with('-') {
        return Err(OpenSkillError::InvalidManifest(
            "Skill name cannot end with a hyphen".to_string(),
        ));
    }

    // No consecutive hyphens
    if name.contains("--") {
        return Err(OpenSkillError::InvalidManifest(
            "Skill name cannot contain consecutive hyphens".to_string(),
        ));
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

/// Validation results for a skill directory.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub stats: Option<ValidationStats>,
}

/// Stats collected during validation.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationStats {
    pub name: String,
    pub name_len: usize,
    pub description_len: usize,
    pub instructions_len: usize,
    pub has_wasm: bool,
}

/// Token analysis report for a skill.
#[derive(Debug, Clone, Serialize)]
pub struct TokenAnalysis {
    pub path: String,
    pub name_len: usize,
    pub description_len: usize,
    pub instructions_len: usize,
    pub tier1_tokens: usize,
    pub tier2_tokens: usize,
    pub total_tokens: usize,
    pub error: Option<String>,
}

/// Validate a skill directory by reading and parsing SKILL.md.
pub fn validate_skill_path(path: &Path) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let skill_md = path.join("SKILL.md");
    if !skill_md.exists() {
        errors.push("SKILL.md not found".to_string());
        return ValidationResult {
            errors,
            warnings,
            stats: None,
        };
    }

    let content = match fs::read_to_string(&skill_md) {
        Ok(c) => c,
        Err(err) => {
            errors.push(format!("Failed to read SKILL.md: {}", err));
            return ValidationResult {
                errors,
                warnings,
                stats: None,
            };
        }
    };

    let parsed = match parse_skill_md(&content) {
        Ok(parsed) => parsed,
        Err(err) => {
            errors.push(format!("Invalid SKILL.md: {}", err));
            return ValidationResult {
                errors,
                warnings,
                stats: None,
            };
        }
    };

    let name_len = parsed.manifest.name.len();
    if name_len == 0 || name_len > constraints::MAX_NAME_LENGTH {
        errors.push(format!(
            "Skill name must be 1-{} characters",
            constraints::MAX_NAME_LENGTH
        ));
    }

    if let Err(err) = validate_name(&parsed.manifest.name) {
        errors.push(err.to_string());
    }

    let description_len = parsed.manifest.description.len();
    if let Err(err) = validate_description(&parsed.manifest.description) {
        errors.push(err.to_string());
    }

    let instructions_len = parsed.instructions.len();
    if instructions_len == 0 {
        warnings.push("Instructions are empty".to_string());
    } else if instructions_len > 10000 {
        warnings.push("Instructions are long; consider moving details to resources".to_string());
    }

    if description_len > 500 {
        warnings.push("Description is long; consider shortening for better discovery".to_string());
    }

    let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !dir_name.is_empty() && dir_name != parsed.manifest.name {
        errors.push(format!(
            "Skill directory '{}' does not match manifest name '{}'",
            dir_name, parsed.manifest.name
        ));
    }

    let has_wasm = find_wasm_module(path);
    let stats = ValidationStats {
        name: parsed.manifest.name,
        name_len,
        description_len,
        instructions_len,
        has_wasm,
    };

    ValidationResult {
        errors,
        warnings,
        stats: Some(stats),
    }
}

/// Analyze token usage for a skill directory.
pub fn analyze_skill_tokens(path: &Path) -> TokenAnalysis {
    let mut analysis = TokenAnalysis {
        path: path.to_string_lossy().to_string(),
        name_len: 0,
        description_len: 0,
        instructions_len: 0,
        tier1_tokens: 0,
        tier2_tokens: 0,
        total_tokens: 0,
        error: None,
    };

    let skill_md = path.join("SKILL.md");
    let content = match fs::read_to_string(&skill_md) {
        Ok(c) => c,
        Err(err) => {
            analysis.error = Some(format!("Failed to read SKILL.md: {}", err));
            return analysis;
        }
    };

    let parsed = match parse_skill_md(&content) {
        Ok(parsed) => parsed,
        Err(err) => {
            analysis.error = Some(format!("Invalid SKILL.md: {}", err));
            return analysis;
        }
    };

    analysis.name_len = parsed.manifest.name.len();
    analysis.description_len = parsed.manifest.description.len();
    analysis.instructions_len = parsed.instructions.len();

    // Rough estimate: 1 token ~= 4 chars. Add small overhead for YAML keys.
    let tier1_chars = analysis.name_len + analysis.description_len + 50;
    analysis.tier1_tokens = tier1_chars / 4;
    analysis.tier2_tokens = analysis.instructions_len / 4;
    analysis.total_tokens = analysis.tier1_tokens + analysis.tier2_tokens;

    analysis
}

fn find_wasm_module(skill_dir: &Path) -> bool {
    let candidates = [
        "skill.wasm",
        "wasm/skill.wasm",
        "module.wasm",
        "main.wasm",
    ];

    if candidates
        .iter()
        .any(|candidate| skill_dir.join(candidate).exists())
    {
        return true;
    }

    if let Ok(entries) = fs::read_dir(skill_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "wasm").unwrap_or(false) {
                return true;
            }
        }
    }

    false
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
    fn test_validate_name_leading_hyphen_rejected() {
        assert!(validate_name("-invalid").is_err());
    }

    #[test]
    fn test_validate_name_trailing_hyphen_rejected() {
        assert!(validate_name("invalid-").is_err());
    }

    #[test]
    fn test_validate_name_consecutive_hyphens_rejected() {
        assert!(validate_name("in--valid").is_err());
    }

    #[test]
    fn test_validate_name_reserved() {
        assert!(validate_name("claude").is_err());
        assert!(validate_name("anthropic").is_err());
    }

    #[test]
    fn test_validate_description_max_length() {
        let max_desc = "x".repeat(1024);
        assert!(validate_description(&max_desc).is_ok());
        
        let too_long = "x".repeat(1025);
        assert!(validate_description(&too_long).is_err());
    }

    #[test]
    fn test_validate_name_max_length() {
        let max_name = "x".repeat(64);
        assert!(validate_name(&max_name).is_ok());
        
        let too_long = "x".repeat(65);
        assert!(validate_name(&too_long).is_err());
    }

    #[test]
    fn test_validate_description_special_characters() {
        // Description can contain special characters (unlike name)
        assert!(validate_description("Test skill with special chars: !@#$%^&*()").is_ok());
        assert!(validate_description("Test with unicode: 测试技能").is_ok());
    }

    #[test]
    fn test_validate_description_xml_tags() {
        assert!(validate_description("Normal description").is_ok());
        assert!(validate_description("Description with <script>").is_err());
        assert!(validate_description("Description with </tag>").is_err());
    }

    #[test]
    fn test_validate_context_invalid_value() {
        use crate::manifest::SkillManifest;
        let manifest = SkillManifest {
            name: "test-skill".to_string(),
            description: "Test".to_string(),
            context: Some("invalid".to_string()),
            allowed_tools: None,
            model: None,
            agent: None,
            hooks: None,
            user_invocable: None,
            license: None,
            compatibility: None,
            metadata: None,
        };
        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn test_validate_context_valid_fork() {
        use crate::manifest::SkillManifest;
        let manifest = SkillManifest {
            name: "test-skill".to_string(),
            description: "Test".to_string(),
            context: Some("fork".to_string()),
            allowed_tools: None,
            model: None,
            agent: None,
            hooks: None,
            user_invocable: None,
            license: None,
            compatibility: None,
            metadata: None,
        };
        assert!(validate_manifest(&manifest).is_ok());
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
