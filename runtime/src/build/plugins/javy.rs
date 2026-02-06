#[cfg(feature = "plugin-javy")]
mod javy_impl {
    use crate::build::plugin::{BuildPlugin, PluginConfig};
    use crate::errors::OpenSkillError;
    // rust-analyzer may show false positive here when feature is disabled
    // This is safe - the entire module is conditionally compiled
    #[allow(unused_imports)]
    use javy_codegen::{Generator, JS, LinkingKind, Plugin, SourceEmbedding};
    use std::path::{Path, PathBuf};

    pub struct JavyBuildPlugin;

    impl JavyBuildPlugin {
        pub fn new() -> Self {
            Self
        }

        fn locate_plugin_path(&self, config: &PluginConfig) -> Option<PathBuf> {
            let custom_path = config
                .custom
                .get("plugin_path")
                .map(|value| {
                    // Expand ~ in custom paths
                    if value.starts_with("~/") {
                        dirs::home_dir()
                            .map(|home| home.join(&value[2..]))
                            .unwrap_or_else(|| PathBuf::from(value))
                    } else {
                        PathBuf::from(value)
                    }
                });
            custom_path
                .or_else(|| {
                    std::env::var("JAVY_PLUGIN_PATH").ok().map(|path| {
                        // Expand ~ in environment variable paths
                        if path.starts_with("~/") {
                            dirs::home_dir()
                                .map(|home| home.join(&path[2..]))
                                .unwrap_or_else(|| PathBuf::from(&path))
                        } else {
                            PathBuf::from(path)
                        }
                    })
                })
                .or_else(|| {
                    let mut candidates = vec![
                        PathBuf::from("plugin_wizened.wasm"),
                        PathBuf::from("plugin.wasm"),
                        PathBuf::from("../javy/target/wasm32-wasip1/release/plugin_wizened.wasm"),
                        PathBuf::from("../javy/target/wasm32-wasip1/release/plugin.wasm"),
                    ];
                    // Add home directory paths with proper expansion
                    if let Some(home) = dirs::home_dir() {
                        candidates.push(home.join(".cargo/bin/plugin_wizened.wasm"));
                        candidates.push(home.join(".cargo/bin/plugin.wasm"));
                    }
                    candidates.iter().find(|p| p.exists()).cloned()
                })
        }

        fn load_plugin(&self, config: &PluginConfig) -> Result<Plugin, OpenSkillError> {
            let plugin_path = self.locate_plugin_path(config);
            match plugin_path {
                Some(path) => Plugin::new_from_path(&path).map_err(|e| {
                    OpenSkillError::BuildError(format!(
                        "Failed to load javy plugin from {}: {}",
                        path.display(),
                        e
                    ))
                }),
                None => Err(OpenSkillError::BuildError(
                    "javy plugin not found. javy-codegen requires a plugin.wasm file.\n\
                    Options:\n  \
                    1. Set JAVY_PLUGIN_PATH environment variable to point to plugin.wasm\n  \
                    2. Place plugin_wizened.wasm in the current directory\n  \
                    3. Run scripts/build_javy_plugin.sh (recommended)\n  \
                    4. Build the plugin: git clone https://github.com/bytecodealliance/javy.git && \
                       cd javy && cargo build --release --target wasm32-wasip1 -p javy-plugin"
                        .to_string(),
                )),
            }
        }
    }

    impl BuildPlugin for JavyBuildPlugin {
        fn name(&self) -> &str {
            "javy"
        }

        fn description(&self) -> &str {
            "Javy/QuickJS compiler via javy-codegen"
        }

        fn supported_extensions(&self) -> &[&str] {
            &["js", "ts"]
        }

        fn is_available(&self) -> Result<bool, OpenSkillError> {
            let config = PluginConfig {
                verbose: false,
                force: false,
                custom: std::collections::HashMap::new(),
            };
            Ok(self.locate_plugin_path(&config).is_some())
        }

        fn compile(
            &self,
            source_file: &Path,
            output_wasm: &Path,
            config: &PluginConfig,
        ) -> Result<(), OpenSkillError> {
            let _ = config.force;

            if let Some(parent) = output_wasm.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    OpenSkillError::BuildError(format!("Failed to create output directory: {}", e))
                })?;
            }

            if config.verbose {
                eprintln!(
                    "Compiling {} to {} using javy",
                    source_file.display(),
                    output_wasm.display()
                );
            }

            let js = JS::from_file(source_file).map_err(|e| {
                OpenSkillError::BuildError(format!(
                    "Failed to read JavaScript file {}: {}",
                    source_file.display(),
                    e
                ))
            })?;

            let plugin = self.load_plugin(config)?;

            let mut generator = Generator::new(plugin);
            generator
                .source_embedding(SourceEmbedding::Compressed)
                .linking(LinkingKind::Static);

            let wasm = generator.generate(&js).map_err(|e| {
                OpenSkillError::BuildError(format!("Failed to generate WASM from JavaScript: {}", e))
            })?;

            std::fs::write(output_wasm, wasm).map_err(|e| {
                OpenSkillError::BuildError(format!(
                    "Failed to write WASM output to {}: {}",
                    output_wasm.display(),
                    e
                ))
            })?;

            Ok(())
        }

        fn requirements(&self) -> Vec<String> {
            vec![
                "Set JAVY_PLUGIN_PATH or place plugin_wizened.wasm in the current directory".to_string(),
                "Build plugin via scripts/build_javy_plugin.sh".to_string(),
            ]
        }
    }
}

#[cfg(feature = "plugin-javy")]
pub use javy_impl::*;
