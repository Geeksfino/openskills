use crate::errors::OpenSkillError;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct BuildConfigFile {
    pub build: Option<BuildConfigSection>,
}

#[derive(Debug, Default, Deserialize)]
pub struct BuildConfigSection {
    pub plugin: Option<String>,
    #[serde(default)]
    pub plugin_options: HashMap<String, String>,
}

impl BuildConfigFile {
    pub fn load(skill_dir: &Path) -> Result<Self, OpenSkillError> {
        let candidates = [
            skill_dir.join(".openskills.toml"),
            skill_dir.join("openskills.toml"),
        ];
        for candidate in candidates {
            if candidate.exists() {
                let content = std::fs::read_to_string(&candidate).map_err(|e| {
                    OpenSkillError::BuildError(format!(
                        "Failed to read build config {}: {}",
                        candidate.display(),
                        e
                    ))
                })?;
                let parsed: BuildConfigFile = toml::from_str(&content).map_err(|e| {
                    OpenSkillError::BuildError(format!(
                        "Failed to parse build config {}: {}",
                        candidate.display(),
                        e
                    ))
                })?;
                return Ok(parsed);
            }
        }
        Ok(Self::default())
    }

    pub fn plugin_ref(&self) -> Option<&String> {
        self.build.as_ref().and_then(|section| section.plugin.as_ref())
    }

    pub fn plugin_options(&self) -> HashMap<String, String> {
        self.build
            .as_ref()
            .map(|section| section.plugin_options.clone())
            .unwrap_or_default()
    }
}
