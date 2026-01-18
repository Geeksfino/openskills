//! Claude Skills registry with standard discovery paths.
//!
//! Discovers skills from:
//! - `~/.claude/skills/` (personal skills)
//! - `.claude/skills/` (project skills)
//! - Nested `.claude/skills/` directories (monorepo support)

use crate::errors::OpenSkillError;
use crate::manifest::SkillManifest;
use crate::skill_parser::parse_skill_md;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A loaded Claude Skill.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Skill ID (directory name, must match manifest name).
    pub id: String,
    /// Root directory of the skill.
    pub root: PathBuf,
    /// Parsed manifest from SKILL.md frontmatter.
    pub manifest: SkillManifest,
    /// Markdown instructions from SKILL.md body.
    pub instructions: String,
    /// Location type (personal, project, nested).
    pub location: SkillLocation,
}

/// Where the skill was discovered from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillLocation {
    /// Personal skills from ~/.claude/skills/
    Personal,
    /// Project skills from .claude/skills/
    Project,
    /// Nested skills from subdirectory .claude/skills/
    Nested,
    /// Custom/explicit path
    Custom,
}

impl fmt::Display for SkillLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillLocation::Personal => write!(f, "personal"),
            SkillLocation::Project => write!(f, "project"),
            SkillLocation::Nested => write!(f, "nested"),
            SkillLocation::Custom => write!(f, "custom"),
        }
    }
}

/// Skill descriptor for listing (progressive disclosure - only name/description).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDescriptor {
    pub id: String,
    pub description: String,
    pub location: SkillLocation,
    pub user_invocable: bool,
}

/// Registry of discovered Claude Skills.
pub struct SkillRegistry {
    /// All discovered skills, keyed by ID.
    skills: HashMap<String, Skill>,
    /// Project root for relative path resolution.
    project_root: Option<PathBuf>,
}

impl SkillRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            project_root: None,
        }
    }

    /// Set the project root for relative path resolution.
    pub fn with_project_root<P: AsRef<Path>>(mut self, root: P) -> Self {
        self.project_root = Some(root.as_ref().to_path_buf());
        self
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Discover and load skills from all standard locations.
    ///
    /// Discovery order (later entries override earlier):
    /// 1. Personal: ~/.claude/skills/
    /// 2. Project: .claude/skills/ (relative to project_root or cwd)
    /// 3. Nested: any .claude/skills/ in subdirectories
    pub fn discover(&mut self) -> Result<(), OpenSkillError> {
        // 1. Personal skills
        if let Some(home) = dirs::home_dir() {
            let personal_path = home.join(".claude").join("skills");
            if personal_path.exists() {
                self.scan_directory(&personal_path, SkillLocation::Personal)?;
            }
        }

        // 2. Project skills
        let project_root = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        let project_skills = project_root.join(".claude").join("skills");
        if project_skills.exists() {
            self.scan_directory(&project_skills, SkillLocation::Project)?;
        }

        // 3. Nested skills (monorepo support)
        self.discover_nested(&project_root)?;

        Ok(())
    }

    /// Discover skills from nested .claude/skills/ directories.
    fn discover_nested(&mut self, root: &Path) -> Result<(), OpenSkillError> {
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories (except .claude), node_modules, target, etc.
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') && name != ".claude" {
                    return false;
                }
                !matches!(name.as_ref(), "node_modules" | "target" | "vendor" | ".git")
            })
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().is_dir() {
                let path = entry.path();
                // Check if this is a .claude/skills directory
                if path.ends_with(".claude/skills") {
                    // Skip if it's the project root .claude/skills (already scanned)
                    let project_skills = root.join(".claude").join("skills");
                    if path != project_skills {
                        self.scan_directory(path, SkillLocation::Nested)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Scan a directory for skills.
    fn scan_directory(&mut self, dir: &Path, location: SkillLocation) -> Result<(), OpenSkillError> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()), // Directory not readable, skip
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let id = match path.file_name().and_then(|v| v.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Check for SKILL.md
            let skill_md_path = path.join("SKILL.md");
            if !skill_md_path.exists() {
                continue;
            }

            // Load and parse the skill
            match self.load_skill(&id, &path, &skill_md_path, location.clone()) {
                Ok(skill) => {
                    self.skills.insert(id, skill);
                }
                Err(e) => {
                    // Log warning but continue scanning
                    eprintln!("Warning: failed to load skill '{}': {}", id, e);
                }
            }
        }

        Ok(())
    }

    /// Load a skill from a SKILL.md file.
    fn load_skill(
        &self,
        id: &str,
        root: &Path,
        skill_md_path: &Path,
        location: SkillLocation,
    ) -> Result<Skill, OpenSkillError> {
        let content = fs::read_to_string(skill_md_path)?;
        let parsed = parse_skill_md(&content)?;

        // Validate skill ID matches name
        validate_skill_id(id, &parsed.manifest)?;

        Ok(Skill {
            id: id.to_string(),
            root: root.to_path_buf(),
            manifest: parsed.manifest,
            instructions: parsed.instructions,
            location,
        })
    }

    /// Load skills from an explicit directory (for testing or custom paths).
    pub fn scan_explicit<P: AsRef<Path>>(&mut self, dir: P) -> Result<(), OpenSkillError> {
        self.scan_directory(dir.as_ref(), SkillLocation::Custom)
    }

    /// Get a skill by ID.
    pub fn get(&self, id: &str) -> Option<&Skill> {
        self.skills.get(id)
    }

    /// List all skills (progressive disclosure - only descriptors).
    pub fn list(&self) -> Vec<SkillDescriptor> {
        self.skills
            .values()
            .map(|s| SkillDescriptor {
                id: s.id.clone(),
                description: s.manifest.description.clone(),
                location: s.location.clone(),
                user_invocable: s.manifest.is_user_invocable(),
            })
            .collect()
    }

    /// Get all skills.
    #[allow(dead_code)] // May be useful for future API extensions
    pub fn all(&self) -> impl Iterator<Item = &Skill> {
        self.skills.values()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate that the skill ID conforms to Claude Skills spec.
fn validate_skill_id(id: &str, manifest: &SkillManifest) -> Result<(), OpenSkillError> {
    use crate::manifest::constraints::*;

    // ID must match manifest name
    if manifest.name != id {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Skill directory '{}' does not match manifest name '{}'",
            id, manifest.name
        )));
    }

    // Validate name format
    if id.is_empty() || id.len() > MAX_NAME_LENGTH {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Skill name must be 1-{} characters, got {}",
            MAX_NAME_LENGTH,
            id.len()
        )));
    }

    // Validate name characters (lowercase, numbers, hyphens)
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Skill name '{}' must contain only lowercase letters, numbers, and hyphens",
            id
        )));
    }

    // Validate description
    if manifest.description.is_empty() {
        return Err(OpenSkillError::InvalidManifest(
            "Skill description is required".to_string(),
        ));
    }

    if manifest.description.len() > MAX_DESCRIPTION_LENGTH {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Skill description exceeds {} characters",
            MAX_DESCRIPTION_LENGTH
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_skill_id_valid() {
        let manifest = SkillManifest {
            name: "my-skill".to_string(),
            description: "A valid skill".to_string(),
            allowed_tools: None,
            model: None,
            context: None,
            agent: None,
            hooks: None,
            user_invocable: None,
        };
        assert!(validate_skill_id("my-skill", &manifest).is_ok());
    }

    #[test]
    fn test_validate_skill_id_mismatch() {
        let manifest = SkillManifest {
            name: "other-name".to_string(),
            description: "A valid skill".to_string(),
            allowed_tools: None,
            model: None,
            context: None,
            agent: None,
            hooks: None,
            user_invocable: None,
        };
        assert!(validate_skill_id("my-skill", &manifest).is_err());
    }

    #[test]
    fn test_validate_skill_id_invalid_chars() {
        let manifest = SkillManifest {
            name: "My_Skill".to_string(),
            description: "A valid skill".to_string(),
            allowed_tools: None,
            model: None,
            context: None,
            agent: None,
            hooks: None,
            user_invocable: None,
        };
        assert!(validate_skill_id("My_Skill", &manifest).is_err());
    }
}
