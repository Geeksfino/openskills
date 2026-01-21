use crate::errors::OpenSkillError;
use std::collections::HashMap;
use std::path::Path;

/// Build backend plugin interface.
pub trait BuildPlugin: Send + Sync {
    /// Unique plugin identifier (e.g., "javy").
    fn name(&self) -> &str;
    /// Human-readable description.
    fn description(&self) -> &str;
    /// Supported source file extensions (without dot).
    fn supported_extensions(&self) -> &[&str];
    /// Check if the plugin is available on this machine.
    fn is_available(&self) -> Result<bool, OpenSkillError>;
    /// Compile source file to a WASM component output.
    fn compile(
        &self,
        source_file: &Path,
        output_wasm: &Path,
        config: &PluginConfig,
    ) -> Result<(), OpenSkillError>;
    /// Requirements for this plugin, used for error messaging.
    fn requirements(&self) -> Vec<String>;
}

/// Plugin-specific configuration.
#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub verbose: bool,
    pub force: bool,
    pub custom: HashMap<String, String>,
}

/// Display information for a plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
    pub available: bool,
    pub extensions: Vec<String>,
}
