//! SKILL.md parser for Claude Skills format.
//!
//! Parses SKILL.md files with YAML frontmatter between `---` markers
//! followed by Markdown instructions.

use crate::errors::OpenSkillError;
use crate::manifest::SkillManifest;

/// Parsed SKILL.md file.
#[derive(Debug, Clone)]
pub struct ParsedSkillMd {
    /// Parsed YAML frontmatter.
    pub manifest: SkillManifest,
    /// Markdown body (instructions).
    pub instructions: String,
}

/// Parse a SKILL.md file content into manifest and instructions.
///
/// The file format is:
/// ```text
/// ---
/// name: my-skill
/// description: What the skill does and when to use it.
/// allowed-tools: Read, Write
/// ---
///
/// # Instructions
///
/// Markdown content here...
/// ```
pub fn parse_skill_md(content: &str) -> Result<ParsedSkillMd, OpenSkillError> {
    let content = content.trim();

    // Must start with ---
    if !content.starts_with("---") {
        return Err(OpenSkillError::InvalidManifest(
            "SKILL.md must start with YAML frontmatter (---)".to_string(),
        ));
    }

    // Find the closing ---
    let after_first = &content[3..];
    let closing_idx = after_first.find("\n---").ok_or_else(|| {
        OpenSkillError::InvalidManifest(
            "SKILL.md frontmatter not properly closed (missing ---)".to_string(),
        )
    })?;

    let yaml_content = &after_first[..closing_idx].trim();
    let rest_start = closing_idx + 4; // skip \n---
    let instructions = if rest_start < after_first.len() {
        after_first[rest_start..].trim().to_string()
    } else {
        String::new()
    };

    // Parse YAML frontmatter
    let manifest: SkillManifest = serde_yaml::from_str(yaml_content).map_err(|e| {
        OpenSkillError::InvalidManifest(format!("Invalid YAML frontmatter: {e}"))
    })?;

    Ok(ParsedSkillMd {
        manifest,
        instructions,
    })
}

/// Parse only the YAML frontmatter from SKILL.md content, discarding body.
/// Used for discovery phase to minimize memory usage.
///
/// This function extracts and parses only the YAML frontmatter between `---` markers,
/// without reading the Markdown body. This enables progressive disclosure where
/// only metadata is loaded at discovery time.
pub fn parse_frontmatter_only(content: &str) -> Result<SkillManifest, OpenSkillError> {
    let content = content.trim();

    // Must start with ---
    if !content.starts_with("---") {
        return Err(OpenSkillError::InvalidManifest(
            "SKILL.md must start with YAML frontmatter (---)".to_string(),
        ));
    }

    // Find the closing ---
    let after_first = &content[3..];
    let closing_idx = after_first.find("\n---").ok_or_else(|| {
        OpenSkillError::InvalidManifest(
            "SKILL.md frontmatter not properly closed (missing ---)".to_string(),
        )
    })?;

    let yaml_content = &after_first[..closing_idx].trim();

    // Parse YAML frontmatter only
    let manifest: SkillManifest = serde_yaml::from_str(yaml_content).map_err(|e| {
        OpenSkillError::InvalidManifest(format!("Invalid YAML frontmatter: {e}"))
    })?;

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_skill_md() {
        let content = r#"---
name: test-skill
description: A test skill for unit testing.
---

# Instructions

Follow these steps...
"#;
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.manifest.name, "test-skill");
        assert_eq!(parsed.manifest.description, "A test skill for unit testing.");
        assert!(parsed.instructions.contains("# Instructions"));
    }

    #[test]
    fn test_parse_skill_md_with_allowed_tools() {
        let content = r#"---
name: code-review
description: Reviews code for best practices.
allowed-tools: Read, Grep, Glob
---

Review the code.
"#;
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.manifest.name, "code-review");
        let tools = parsed.manifest.get_allowed_tools();
        assert_eq!(tools, vec!["Read", "Grep", "Glob"]);
    }

    #[test]
    fn test_parse_skill_md_with_list_tools() {
        let content = r#"---
name: code-review
description: Reviews code for best practices.
allowed-tools:
  - Read
  - Write
  - Bash
---

Review the code.
"#;
        let parsed = parse_skill_md(content).unwrap();
        let tools = parsed.manifest.get_allowed_tools();
        assert_eq!(tools, vec!["Read", "Write", "Bash"]);
    }

    #[test]
    fn test_parse_skill_md_with_context_fork() {
        let content = r#"---
name: explorer
description: Explores the codebase.
context: fork
agent: Explore
---

Explore systematically.
"#;
        let parsed = parse_skill_md(content).unwrap();
        assert!(parsed.manifest.is_forked());
        assert_eq!(parsed.manifest.agent, Some("Explore".to_string()));
    }

    #[test]
    fn test_parse_skill_md_missing_frontmatter() {
        let content = "# Just markdown\n\nNo frontmatter here.";
        let result = parse_skill_md(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_skill_md_unclosed_frontmatter() {
        let content = "---\nname: broken\n\nNo closing delimiter.";
        let result = parse_skill_md(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_frontmatter_only() {
        let content = r#"---
name: test-skill
description: A test skill.
allowed-tools: Read, Write
---
# Instructions

This is the body that should be ignored.
"#;
        let manifest = parse_frontmatter_only(content).unwrap();
        assert_eq!(manifest.name, "test-skill");
        assert_eq!(manifest.description, "A test skill.");
        // Verify body is not parsed
        let tools = manifest.get_allowed_tools();
        assert_eq!(tools, vec!["Read", "Write"]);
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let content = r#"---
name: test-skill
description: [invalid yaml
---
"#;
        let result = parse_skill_md(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_name_field() {
        let content = r#"---
description: A test skill without name
---
"#;
        let result = parse_skill_md(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_description_field() {
        let content = r#"---
name: test-skill
---
"#;
        let result = parse_skill_md(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_body() {
        let content = r#"---
name: test-skill
description: A test skill.
---
"#;
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.instructions.trim(), "");
    }

    #[test]
    fn test_parse_only_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill.
---
"#;
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.manifest.name, "test-skill");
        assert_eq!(parsed.instructions.trim(), "");
    }
}
