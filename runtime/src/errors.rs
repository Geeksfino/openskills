//! Error types for OpenSkills runtime.

use thiserror::Error;

/// OpenSkills runtime error.
#[derive(Error, Debug)]
pub enum OpenSkillError {
    /// Skill not found in registry.
    #[error("skill not found: {0}")]
    SkillNotFound(String),

    /// Invalid SKILL.md manifest.
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    /// Permission denied for operation.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Execution timed out.
    #[error("execution timeout")]
    Timeout,

    /// Tool not allowed by skill configuration.
    #[error("tool not allowed: {0}")]
    ToolNotAllowed(String),

    /// WASM execution failure.
    #[error("wasm execution failed: {0}")]
    WasmError(String),

    /// Native execution failure.
    #[error("native execution failed: {0}")]
    NativeExecutionError(String),

    /// Seatbelt sandbox error.
    #[error("seatbelt error: {0}")]
    SeatbeltError(String),

    /// Unsupported platform for native execution.
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),

    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML parsing error.
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// JSON parsing error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Build/compilation error.
    #[error("build error: {0}")]
    BuildError(String),
}
