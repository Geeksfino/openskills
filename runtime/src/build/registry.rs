use crate::build::plugin::{BuildPlugin, PluginInfo};
use crate::build::plugins;
use std::sync::Arc;

pub struct PluginRegistry {
    plugins: Vec<Arc<dyn BuildPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        let mut registry = Self { plugins: Vec::new() };
        registry.register_builtin_plugins();
        registry
    }

    fn register_builtin_plugins(&mut self) {
        #[cfg(feature = "plugin-javy")]
        self.register(Arc::new(plugins::javy::JavyBuildPlugin::new()));
        #[cfg(feature = "plugin-quickjs")]
        self.register(Arc::new(plugins::quickjs::QuickJsBuildPlugin::new()));
        #[cfg(feature = "plugin-assemblyscript")]
        self.register(Arc::new(plugins::assemblyscript::AssemblyScriptBuildPlugin::new()));
    }

    pub fn register(&mut self, plugin: Arc<dyn BuildPlugin>) {
        self.plugins.push(plugin);
    }

    pub fn find(&self, name: &str) -> Option<Arc<dyn BuildPlugin>> {
        self.plugins
            .iter()
            .find(|plugin| plugin.name() == name)
            .cloned()
    }

    pub fn find_by_extension(&self, extension: &str) -> Vec<Arc<dyn BuildPlugin>> {
        self.plugins
            .iter()
            .filter(|plugin| plugin.supported_extensions().contains(&extension))
            .cloned()
            .collect()
    }

    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins
            .iter()
            .map(|plugin| PluginInfo {
                name: plugin.name().to_string(),
                description: plugin.description().to_string(),
                available: plugin.is_available().unwrap_or(false),
                extensions: plugin
                    .supported_extensions()
                    .iter()
                    .map(|ext| ext.to_string())
                    .collect(),
            })
            .collect()
    }
}
