//! OS sandbox enforcement mode for native script and command execution.
//!
//! Hosts that provide an outer sandbox boundary (e.g. FinSAFE) can set
//! [`SandboxMode::Disabled`] so OpenSkills does not apply a second inner layer.

/// Whether OpenSkills applies its built-in OS sandbox (Landlock / Seatbelt).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SandboxMode {
    /// Apply Landlock (Linux) or Seatbelt (macOS). Default for all consumers.
    #[default]
    Enforce,
    /// Run natively without OpenSkills OS sandboxing; host owns the boundary.
    Disabled,
}

impl SandboxMode {
    pub fn as_audit_str(self) -> &'static str {
        match self {
            Self::Enforce => "enforce",
            Self::Disabled => "disabled",
        }
    }
}
