//! SKILL.md parser for Claude Skills format.
//!
//! Parses SKILL.md files with YAML frontmatter between `---` markers
//! followed by Markdown instructions.

use crate::errors::OpenSkillError;
use crate::manifest::constraints::MAX_DESCRIPTION_LENGTH;
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
/// Tolerant parsing: if frontmatter is missing or malformed, returns a default
/// manifest with empty name/description (the caller is expected to fill them in
/// from directory name and body text).
pub fn parse_skill_md(content: &str) -> Result<ParsedSkillMd, OpenSkillError> {
    let content = content.trim();

    if !content.starts_with("---") {
        return Ok(ParsedSkillMd {
            manifest: SkillManifest::default(),
            instructions: content.to_string(),
        });
    }

    let after_first = &content[3..];
    let closing_idx = match after_first.find("\n---") {
        Some(idx) => idx,
        None => {
            return Ok(ParsedSkillMd {
                manifest: SkillManifest::default(),
                instructions: content.to_string(),
            });
        }
    };

    let yaml_content = &after_first[..closing_idx].trim();
    let rest_start = closing_idx + 4;
    let instructions = if rest_start < after_first.len() {
        after_first[rest_start..].trim().to_string()
    } else {
        String::new()
    };

    let manifest: SkillManifest = match serde_yaml::from_str(yaml_content) {
        Ok(m) => m,
        Err(_) => parse_frontmatter_fallback(yaml_content),
    };

    Ok(ParsedSkillMd {
        manifest,
        instructions,
    })
}

/// Parse only the YAML frontmatter from SKILL.md content, discarding body.
/// Used for discovery phase to minimize memory usage.
///
/// Tolerant: returns a default manifest when frontmatter is missing or broken,
/// so the caller can fill in defaults from the directory name and body.
pub fn parse_frontmatter_only(content: &str) -> Result<SkillManifest, OpenSkillError> {
    let content = content.trim();

    if !content.starts_with("---") {
        return Ok(SkillManifest::default());
    }

    let after_first = &content[3..];
    let closing_idx = match after_first.find("\n---") {
        Some(idx) => idx,
        None => return Ok(SkillManifest::default()),
    };

    let yaml_content = &after_first[..closing_idx].trim();

    let manifest: SkillManifest = match serde_yaml::from_str(yaml_content) {
        Ok(m) => m,
        Err(_) => parse_frontmatter_fallback(yaml_content),
    };

    Ok(manifest)
}

/// Line-by-line `key: value` fallback when YAML parsing fails (inspired by Hermes Agent).
///
/// Only reads `name` and `description`; other frontmatter keys are ignored (no full YAML).
fn parse_frontmatter_fallback(yaml_content: &str) -> SkillManifest {
    let mut manifest = SkillManifest::default();
    for line in yaml_content.lines() {
        let line = line.trim();
        if let Some(idx) = line.find(':') {
            let key = line[..idx].trim();
            let value = line[idx + 1..].trim().trim_matches('"').trim_matches('\'');
            match key {
                "name" => manifest.name = value.to_string(),
                "description" => manifest.description = value.to_string(),
                _ => {}
            }
        }
    }
    manifest
}

/// Extract a description from the Markdown body when frontmatter lacks one.
/// Returns the first non-empty, non-heading line, truncated to 1024 chars.
pub fn extract_description_from_body(content: &str) -> Option<String> {
    let body = if content.trim().starts_with("---") {
        let after_first = &content.trim()[3..];
        match after_first.find("\n---") {
            Some(idx) => {
                let rest_start = idx + 4;
                if rest_start < after_first.len() {
                    &after_first[rest_start..]
                } else {
                    ""
                }
            }
            None => "",
        }
    } else {
        content
    };

    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && trimmed != "---"
        {
            return Some(truncate_to_max_description_bytes(trimmed));
        }
    }
    None
}

/// Truncate to [`MAX_DESCRIPTION_LENGTH`] bytes without splitting a UTF-8 codepoint.
fn truncate_to_max_description_bytes(s: &str) -> String {
    if s.len() <= MAX_DESCRIPTION_LENGTH {
        return s.to_string();
    }
    let mut end = MAX_DESCRIPTION_LENGTH;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
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
    fn test_parse_skill_md_missing_frontmatter_tolerant() {
        let content = "# Just markdown\n\nNo frontmatter here.";
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.manifest.name, "");
        assert!(parsed.instructions.contains("Just markdown"));
    }

    #[test]
    fn test_parse_skill_md_unclosed_frontmatter_tolerant() {
        let content = "---\nname: broken\n\nNo closing delimiter.";
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.manifest.name, "");
        assert!(parsed.instructions.contains("---"));
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
    fn test_parse_invalid_yaml_fallback() {
        let content = r#"---
name: test-skill
description: [invalid yaml
---
"#;
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.manifest.name, "test-skill");
    }

    #[test]
    fn test_parse_missing_name_field_tolerant() {
        let content = r#"---
description: A test skill without name
---
"#;
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.manifest.name, "");
        assert_eq!(parsed.manifest.description, "A test skill without name");
    }

    #[test]
    fn test_parse_missing_description_field_tolerant() {
        let content = r#"---
name: test-skill
---
"#;
        let parsed = parse_skill_md(content).unwrap();
        assert_eq!(parsed.manifest.name, "test-skill");
        assert_eq!(parsed.manifest.description, "");
    }

    #[test]
    fn test_extract_description_from_body() {
        let content = r#"---
name: test-skill
---

# My Skill

This skill does something useful.
"#;
        let desc = extract_description_from_body(content);
        assert_eq!(desc, Some("This skill does something useful.".to_string()));
    }

    #[test]
    fn test_extract_description_from_body_no_frontmatter() {
        let content = "# Title\n\nA paragraph of text.";
        let desc = extract_description_from_body(content);
        assert_eq!(desc, Some("A paragraph of text.".to_string()));
    }

    #[test]
    fn test_extract_description_from_body_unclosed_frontmatter() {
        let content = "---\nname: broken\n\nNo closing delimiter.";
        let desc = extract_description_from_body(content);
        assert_ne!(desc.as_deref(), Some("---"));
        // Malformed frontmatter: no usable body slice, so we do not invent a description.
        assert!(desc.is_none());
    }

    #[test]
    fn test_extract_description_from_body_empty() {
        let content = "# Title only\n";
        let desc = extract_description_from_body(content);
        assert_eq!(desc, None);
    }

    #[test]
    fn test_extract_description_truncates_utf8_safe() {
        use crate::manifest::constraints::MAX_DESCRIPTION_LENGTH;
        let filler = "à".repeat(700);
        assert!(filler.len() > MAX_DESCRIPTION_LENGTH);
        let content = format!("---\nname: t\n---\n\n{filler}");
        let desc = extract_description_from_body(&content).expect("desc");
        assert!(desc.len() <= MAX_DESCRIPTION_LENGTH);
        assert!(desc.is_char_boundary(desc.len()));
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
