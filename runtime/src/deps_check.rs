//! Dependency checking for skills (OpenClaw-compatible requires.bins / requires.env
//! and optional package-level metadata).

use crate::manifest::SkillRequires;
use std::path::Path;
use std::process::Command;

/// Check if a binary exists in PATH.
pub fn check_binary_in_path(name: &str) -> bool {
    let (cmd, args) = if cfg!(target_os = "windows") {
        ("where", vec![name])
    } else {
        ("which", vec![name])
    };
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if an environment variable is set and non-empty.
pub fn check_env_set(key: &str) -> bool {
    std::env::var(key).is_ok_and(|v| !v.trim().is_empty())
}

/// Current platform identifier for requires.platforms matching.
fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}

/// Best-effort check if a Python package (import name) is importable.
const PYTHON_IMPORT_CHECK_SCRIPT: &str = "import importlib.util, sys\n\
try:\n\
    spec = importlib.util.find_spec(sys.argv[1])\n\
except (ImportError, ModuleNotFoundError, ValueError):\n\
    spec = None\n\
raise SystemExit(0 if spec is not None else 1)";

fn check_python_package(interpreter: &Path, import_name: &str) -> bool {
    let status = Command::new(interpreter)
        .args(["-c", PYTHON_IMPORT_CHECK_SCRIPT, import_name])
        .output();
    matches!(status, Ok(ref o) if o.status.success())
}

/// Result of checking skill requirements.
#[derive(Debug, Clone, Default)]
pub struct MissingDependencies {
    pub bins: Vec<String>,
    pub env: Vec<String>,
    /// Declared Python packages that could not be imported (when interpreter was available).
    pub missing_python_packages: Vec<String>,
    /// Declared Python packages not verified (no interpreter provided; host can resolve).
    pub unverified_python_packages: Vec<String>,
    /// Declared Node packages not verified (runtime does not run node checks; host can resolve).
    pub unverified_node_packages: Vec<String>,
    /// Declared Rust crates not verified (metadata only; host can resolve).
    pub unverified_rust_crates: Vec<String>,
    /// Declared system packages not verified (metadata only; host can resolve).
    pub unverified_system_packages: Vec<String>,
    /// Set when requires.platforms is non-empty and current OS is not in the list.
    pub platform_mismatch: Option<String>,
}

impl MissingDependencies {
    pub fn is_empty(&self) -> bool {
        self.bins.is_empty()
            && self.env.is_empty()
            && self.missing_python_packages.is_empty()
            && self.unverified_python_packages.is_empty()
            && self.unverified_node_packages.is_empty()
            && self.unverified_rust_crates.is_empty()
            && self.unverified_system_packages.is_empty()
            && self.platform_mismatch.is_none()
    }
}

/// Check skill requires and return missing bins, env, and (when interpreter provided) package diagnostics.
///
/// * `requires` - from skill manifest
/// * `python_interpreter` - if set, Python package imports are checked; otherwise declared python_packages are reported as unverified
pub fn check_requires(
    requires: Option<&SkillRequires>,
    python_interpreter: Option<&Path>,
) -> MissingDependencies {
    let mut missing = MissingDependencies::default();
    let Some(requires) = requires else {
        return missing;
    };
    for bin in &requires.bins {
        if !check_binary_in_path(bin) {
            missing.bins.push(bin.clone());
        }
    }
    for key in &requires.env {
        if !check_env_set(key) {
            missing.env.push(key.clone());
        }
    }
    if !requires.platforms.is_empty() {
        let current = current_platform();
        let matches = requires
            .platforms
            .iter()
            .any(|s| s.to_lowercase() == current);
        if !matches {
            missing.platform_mismatch = Some(format!(
                "skill requires one of [{}], current platform is {}",
                requires.platforms.join(", "),
                current
            ));
        }
    }
    if let Some(interpreter) = python_interpreter {
        for pkg in &requires.python_packages {
            if !check_python_package(interpreter, pkg) {
                missing.missing_python_packages.push(pkg.clone());
            }
        }
    } else if !requires.python_packages.is_empty() {
        missing.unverified_python_packages.extend(requires.python_packages.clone());
    }
    if !requires.node_packages.is_empty() {
        missing.unverified_node_packages.extend(requires.node_packages.clone());
    }
    if !requires.rust_crates.is_empty() {
        missing.unverified_rust_crates.extend(requires.rust_crates.clone());
    }
    if !requires.system_packages.is_empty() {
        missing.unverified_system_packages.extend(requires.system_packages.clone());
    }
    missing
}

/// Legacy: check only bins and env (no interpreter, no package diagnostics).
#[allow(dead_code)]
pub fn check_requires_bins_env_only(requires: Option<&SkillRequires>) -> MissingDependencies {
    check_requires(requires, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_os = "windows"))]
    use std::fs;
    #[cfg(not(target_os = "windows"))]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(not(target_os = "windows"))]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_check_env_set_empty() {
        // Unset or empty env should be false
        std::env::remove_var("__OPENSKILLS_TEST_EMPTY__");
        assert!(!check_env_set("__OPENSKILLS_TEST_EMPTY__"));
    }

    #[test]
    fn test_check_requires_none() {
        let m = check_requires(None, None);
        assert!(m.is_empty());
    }

    #[test]
    fn test_check_requires_empty() {
        let r = SkillRequires::default();
        let m = check_requires(Some(&r), None);
        assert!(m.is_empty());
    }

    #[test]
    fn test_check_binary_in_path_common() {
        // Should have at least one of these in PATH in CI/local
        let has_any = check_binary_in_path("true")
            || check_binary_in_path("false")
            || check_binary_in_path("echo");
        assert!(has_any);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_check_python_package_treats_manifest_value_as_data() {
        let unique = format!(
            "openskills-deps-check-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let temp_dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&temp_dir).unwrap();

        let interpreter = temp_dir.join("fake-python.sh");
        let code_capture = temp_dir.join("code.txt");
        let arg_capture = temp_dir.join("arg.txt");
        let malicious = "yaml; __import__('os').system('touch /tmp/openskills-test')";
        let script = format!(
            "#!/bin/sh\nprintf '%s' \"$2\" > \"{}\"\nprintf '%s' \"$3\" > \"{}\"\n[ \"$3\" = \"{}\" ]\n",
            code_capture.display(),
            arg_capture.display(),
            malicious
        );
        fs::write(&interpreter, script).unwrap();

        let mut perms = fs::metadata(&interpreter).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&interpreter, perms).unwrap();

        assert!(check_python_package(&interpreter, malicious));
        assert_eq!(fs::read_to_string(&arg_capture).unwrap(), malicious);
        assert!(!fs::read_to_string(&code_capture).unwrap().contains(malicious));

        fs::remove_dir_all(temp_dir).unwrap();
    }
}
