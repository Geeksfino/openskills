//! Native sandbox execution using macOS seatbelt.
//!
//! Provides OS-level sandboxing for native scripts (Python, shell) as a
//! complement to the WASM sandbox.

use crate::audit::ExecutionStatus;
use crate::errors::OpenSkillError;
use crate::executor::ExecutionArtifacts;
use crate::permissions::PermissionEnforcer;
use crate::registry::Skill;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Supported native script types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptType {
    Python,
    Shell,
}

/// Detect script type based on file extension.
pub fn detect_script_type(path: &Path) -> Result<ScriptType, OpenSkillError> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "py" => Ok(ScriptType::Python),
        "sh" | "bash" => Ok(ScriptType::Shell),
        _ => Err(OpenSkillError::NativeExecutionError(format!(
            "Unsupported script type: {}",
            path.display()
        ))),
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    const SYSTEM_READ_PATHS: &[&str] = &[
        "/System",
        "/usr/lib",
        "/usr/libexec",
        "/usr/bin",
        "/usr/sbin",
        "/usr/share",
        "/usr/local",
        "/bin",
        "/sbin",
        "/opt/homebrew",
        "/Library",
        "/etc",
        "/private/etc",
        "/var/db",
        "/private/var/db",
        "/var",
        "/private/var",
        "/Users",
        "/dev",
    ];

    // Common Python interpreter locations on macOS.
    // We allow these explicitly to avoid sandbox failures for popular installs.
    const PYTHON_EXEC_PATHS: &[&str] = &[
        "/usr/bin/python",
        "/usr/bin/python3",
        "/usr/local/bin/python",
        "/usr/local/bin/python3",
        "/opt/homebrew/bin/python",
        "/opt/homebrew/bin/python3",
        "/Library/Frameworks/Python.framework/Versions/Current/bin/python",
        "/Library/Frameworks/Python.framework/Versions/Current/bin/python3",
    ];

    const TEMP_PATHS: &[&str] = &[
        "/tmp",
        "/private/tmp",
        "/private/var/tmp",
        "/private/var/folders",
    ];

    pub fn execute_native(
        skill: &Skill,
        script_path: &Path,
        script_type: ScriptType,
        input: Value,
        timeout_ms: u64,
        enforcer: &PermissionEnforcer,
        allowed_tools: &[String],
        workspace_dir: Option<&Path>,
        script_args: &[String],
    ) -> Result<ExecutionArtifacts, OpenSkillError> {
        if !script_path.exists() {
            return Err(OpenSkillError::NativeExecutionError(format!(
                "Script not found: {}",
                script_path.display()
            )));
        }

        let input_json = serde_json::to_string(&input)?;
        let allow_network = allowed_tools.iter().any(|t| t == "WebSearch" || t == "Fetch");
        // Only Shell scripts get process permissions by default.
        // Python scripts require explicit Bash/Terminal permission to spawn subprocesses.
        let allow_process = script_type == ScriptType::Shell
            || allowed_tools
                .iter()
                .any(|t| t == "Bash" || t == "Terminal");

        // Canonicalize skill_root first to ensure path consistency
        let skill_root = skill
            .root
            .canonicalize()
            .unwrap_or_else(|_| skill.root.clone());

        // Get paths from enforcer and canonicalize them to match canonicalized skill_root
        let read_paths: Vec<PathBuf> = enforcer
            .filesystem_read_paths()
            .iter()
            .map(|p| {
                p.canonicalize()
                    .unwrap_or_else(|_| p.to_path_buf())
            })
            .collect();
        let mut write_paths: Vec<PathBuf> = enforcer
            .filesystem_write_paths()
            .iter()
            .map(|p| {
                p.canonicalize()
                    .unwrap_or_else(|_| p.to_path_buf())
            })
            .collect();

        // Add workspace directory to write paths if configured
        if let Some(workspace) = workspace_dir {
            if workspace.exists() {
                write_paths.push(workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf()));
            }
        }

        let (program, args, program_path) = command_for_script(script_type, script_path)?;
        // Canonicalize the executable path for the seatbelt profile
        // We need to pass the actual executable path (not its parent) to grant file-map-executable permission
        let exec_path = program_path.as_ref().and_then(|p| {
            p.canonicalize().ok().or_else(|| Some(p.clone()))
        });
        // Also ensure the parent directory is accessible for traversal
        // This is needed even if the executable path itself is granted permission
        let mut read_paths_with_parent = read_paths.clone();
        // Track the parent directory separately if it needs file-map-executable permission
        let exec_parent_path = if let Some(path) = exec_path.as_ref().and_then(|p| p.parent()) {
            let canonicalized_parent = path
                .canonicalize()
                .unwrap_or_else(|_| path.to_path_buf());
            // Only add to read_paths if not already covered by SYSTEM_READ_PATHS
            let is_system_path = SYSTEM_READ_PATHS.iter().any(|&sys_path| {
                canonicalized_parent.starts_with(sys_path)
            });
            if !is_system_path {
                read_paths_with_parent.push(canonicalized_parent.clone());
                Some(canonicalized_parent)
            } else {
                None
            }
        } else {
            None
        };
        let profile = build_seatbelt_profile(
            &skill_root,
            &read_paths_with_parent,
            &write_paths,
            allow_network,
            allow_process,
            exec_path.as_deref(),
            exec_parent_path.as_deref(),
        );

        let profile_path = write_profile(&profile)?;
        let mut cmd = Command::new("sandbox-exec");
        cmd.arg("-f").arg(&profile_path).arg("--").arg(program).args(args);
        // Append user-provided script arguments (e.g., "my-test" for init-artifact.sh)
        if !script_args.is_empty() {
            cmd.args(script_args);
        }
        // Use workspace_dir as cwd if provided, otherwise fall back to skill_root.
        // This ensures scripts that create output files write to the workspace directory.
        // Scripts can still access skill resources via SKILL_ROOT env var.
        let working_directory = workspace_dir.unwrap_or(&skill_root);
        cmd.current_dir(working_directory);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        apply_environment(&mut cmd, skill, &input_json, timeout_ms, enforcer, script_type, workspace_dir);

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let _ = std::fs::remove_file(&profile_path);
                return Err(OpenSkillError::SeatbeltError(format!(
                    "Failed to execute with seatbelt: {e}"
                )));
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            let input_clone = input_json.clone();
            thread::spawn(move || {
                let _ = stdin.write_all(input_clone.as_bytes());
            });
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout_handle = thread::spawn(move || read_stream(stdout));
        let stderr_handle = thread::spawn(move || read_stream(stderr));

        let start = Instant::now();
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(OpenSkillError::Io)? {
                break Some(status);
            }
            if start.elapsed() >= Duration::from_millis(timeout_ms) {
                timed_out = true;
                let _ = child.kill();
                break child.wait().ok();
            }
            thread::sleep(Duration::from_millis(10));
        };

        let stdout_bytes = stdout_handle.join().unwrap_or_default();
        let stderr_bytes = stderr_handle.join().unwrap_or_default();
        let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
        let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

        let (exit_status, output) = if timed_out {
            (
                ExecutionStatus::Timeout,
                serde_json::json!({ "status": "error", "error": "execution timeout" }),
            )
        } else if let Some(status) = status {
            if status.success() {
                let output = if let Ok(json) = serde_json::from_str::<Value>(&stdout) {
                    json
                } else {
                    serde_json::json!({ "status": "success", "output": stdout.trim() })
                };
                (ExecutionStatus::Success, output)
            } else {
                let message = if !stderr.trim().is_empty() {
                    stderr.trim().to_string()
                } else {
                    format!("Process exited with status {}", status)
                };
                (
                    ExecutionStatus::Failed(message.clone()),
                    serde_json::json!({ "status": "error", "error": message }),
                )
            }
        } else {
            (
                ExecutionStatus::Failed("Process failed to start".to_string()),
                serde_json::json!({ "status": "error", "error": "Process failed to start" }),
            )
        };

        let _ = std::fs::remove_file(&profile_path);

        Ok(ExecutionArtifacts {
            output,
            stdout,
            stderr,
            permissions_used: enforcer.permissions_used(),
            exit_status,
        })
    }

    fn command_for_script(
        script_type: ScriptType,
        script_path: &Path,
    ) -> Result<(String, Vec<String>, Option<PathBuf>), OpenSkillError> {
        match script_type {
            ScriptType::Python => {
                let resolved = PYTHON_EXEC_PATHS
                    .iter()
                    .map(PathBuf::from)
                    .find(|candidate| candidate.exists())
                    .or_else(|| resolve_executable("python3"))
                    .ok_or_else(|| {
                        OpenSkillError::NativeExecutionError("python3 not found".to_string())
                    })?;
                Ok((
                    resolved.to_string_lossy().to_string(),
                    vec![script_path.to_string_lossy().to_string()],
                    Some(resolved),
                ))
            }
            ScriptType::Shell => {
                let program = "/bin/bash".to_string();
                let resolved = PathBuf::from(&program);
                Ok((
                    program,
                    vec![script_path.to_string_lossy().to_string()],
                    Some(resolved),
                ))
            }
        }
    }

    fn apply_environment(
        cmd: &mut Command,
        skill: &Skill,
        input_json: &str,
        timeout_ms: u64,
        enforcer: &PermissionEnforcer,
        script_type: ScriptType,
        workspace_dir: Option<&Path>,
    ) {
        cmd.env_clear();

        let path = std::env::var("PATH").unwrap_or_else(|_| {
            "/usr/bin:/bin:/usr/sbin:/sbin".to_string()
        });
        cmd.env("PATH", path);

        if let Ok(lang) = std::env::var("LANG") {
            cmd.env("LANG", lang);
        }
        if let Ok(lc_all) = std::env::var("LC_ALL") {
            cmd.env("LC_ALL", lc_all);
        }
        if let Ok(tmpdir) = std::env::var("TMPDIR") {
            cmd.env("TMPDIR", tmpdir);
        }

        cmd.env("SKILL_ID", &skill.id);
        cmd.env("SKILL_NAME", &skill.manifest.name);
        cmd.env("SKILL_INPUT", input_json);
        cmd.env("TIMEOUT_MS", timeout_ms.to_string());
        cmd.env("SKILL_ROOT", skill.root.to_string_lossy().to_string());

        // Inject workspace directory if configured
        if let Some(workspace) = workspace_dir {
            cmd.env("SKILL_WORKSPACE", workspace.to_string_lossy().to_string());
        }

        for key in enforcer.env_allowlist() {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }

        if script_type == ScriptType::Python {
            cmd.env("PYTHONUNBUFFERED", "1");
            cmd.env("PYTHONDONTWRITEBYTECODE", "1");
            cmd.env("PYTHONNOUSERSITE", "1");
            // Prevent Python from trying to check for Xcode/development tools during initialization.
            // Python (especially when built with Clang) may try to spawn xcodebuild to verify
            // development tools are available. Since the sandbox blocks subprocess spawning
            // (allow_process=false), this would fail. By clearing these environment variables,
            // we prevent Python from knowing where to look for xcodebuild, avoiding the check.
            cmd.env_remove("DEVELOPER_DIR");
            cmd.env_remove("SDKROOT");
            // Clear compiler environment variables to prevent Python from trying to locate
            // or spawn compiler tools
            cmd.env_remove("CC");
            cmd.env_remove("CXX");
            cmd.env_remove("CFLAGS");
            cmd.env_remove("CXXFLAGS");
        }
    }

    // Sensitive paths that should never be readable even with broad file-read access.
    // Following Claude Code's approach: allow broad reads, deny specific sensitive paths.
    const SENSITIVE_DENY_PATHS: &[&str] = &[
        "~/.ssh",
        "~/.gnupg",
        "~/.aws",
        "~/.azure",
        "~/.config/gcloud",
        "~/.kube",
        "~/.docker",
        "~/.npmrc",
        "~/.pypirc",
        "~/.netrc",
        "~/.gitconfig",
        "~/.git-credentials",
        "~/.bashrc",
        "~/.zshrc",
        "~/.profile",
        "~/.bash_profile",
        "~/.zprofile",
    ];

    fn build_seatbelt_profile(
        skill_root: &Path,
        read_paths: &[PathBuf],
        write_paths: &[PathBuf],
        allow_network: bool,
        allow_process: bool,
        _exec_path: Option<&Path>,
        _exec_parent_path: Option<&Path>,
    ) -> String {
        let mut profile = String::new();
        
        // Following Claude Code's model:
        // 1. Start with deny default
        // 2. Allow broad file reads (Python and other interpreters need this)
        // 3. Deny specific sensitive paths
        // 4. Allow writes only to specific paths
        profile.push_str("(version 1)\n(deny default)\n");

        // Core permissions needed for interpreter execution
        // process-exec: needed to execute the interpreter binary itself
        // mach-lookup: needed for process execution on macOS
        // signal: needed for signal handling
        profile.push_str("(allow sysctl-read)\n");
        profile.push_str("(allow process-exec)\n");
        profile.push_str("(allow mach-lookup)\n");
        profile.push_str("(allow signal)\n");
        
        // process-fork: only allow when explicitly permitted (Bash/Terminal tools)
        // This prevents subprocess spawning without explicit permission

        // Deny sensitive credential and config paths FIRST (before allow-all)
        // Seatbelt uses first-match-wins, so deny rules must come before allow rules
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users".to_string());
        for sensitive_path in SENSITIVE_DENY_PATHS {
            let expanded = sensitive_path.replace('~', &home);
            profile.push_str(&format!(
                "(deny file-read* (subpath \"{}\"))\n",
                escape_path(&expanded)
            ));
        }

        // Allow broad file reads - this is essential for Python and other interpreters
        // to access their libraries, modules, and system resources.
        // This comes AFTER deny rules so sensitive paths are protected.
        // Claude Code uses this approach: allow reads broadly, deny writes specifically.
        profile.push_str("(allow file-read*)\n");

        // Allow /dev/null writes (needed for output redirection)
        profile.push_str("(allow file-write* (literal \"/dev/null\"))\n");

        // Allow writes to temp directories
        for temp_path in TEMP_PATHS {
            profile.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                escape_path(temp_path)
            ));
        }

        // Allow writes to skill root directory
        profile.push_str(&format!(
            "(allow file-write* (subpath \"{}\"))\n",
            escape_path(skill_root.to_string_lossy().as_ref())
        ));

        // Allow writes to explicitly configured paths
        for path in write_paths {
            profile.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                escape_path(path.to_string_lossy().as_ref())
            ));
        }

        // Additional read paths (already covered by allow file-read*, but explicit for clarity)
        for path in read_paths {
            profile.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                escape_path(path.to_string_lossy().as_ref())
            ));
        }

        // Process permissions if shell/terminal tools are allowed
        if allow_process {
            // Allow process-fork for subprocess spawning
            profile.push_str("(allow process-fork)\n");
            // Allow all other process operations
            profile.push_str("(allow process*)\n");
        }

        // Network access only if explicitly allowed
        if allow_network {
            profile.push_str("(allow network*)\n");
        }

        profile
    }

    fn escape_path(path: &str) -> String {
        path.replace('"', "\\\"")
    }

    fn resolve_executable(program: &str) -> Option<PathBuf> {
        let program_path = Path::new(program);
        if program_path.is_absolute() {
            return program_path.exists().then(|| program_path.to_path_buf());
        }

        let path_var = std::env::var_os("PATH")?;
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(program);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        None
    }

    fn write_profile(profile: &str) -> Result<PathBuf, OpenSkillError> {
        let mut attempt = 0;
        let temp_dir = std::env::temp_dir();

        loop {
            let filename = format!(
                "openskills-seatbelt-{}-{}.sb",
                std::process::id(),
                attempt
            );
            let path = temp_dir.join(filename);
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path);

            match file {
                Ok(mut file) => {
                    file.write_all(profile.as_bytes()).map_err(OpenSkillError::Io)?;
                    return Ok(path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempt += 1;
                    if attempt > 100 {
                        return Err(OpenSkillError::SeatbeltError(
                            "Failed to create seatbelt profile file".to_string(),
                        ));
                    }
                }
                Err(e) => return Err(OpenSkillError::Io(e)),
            }
        }
    }

    fn read_stream<T: Read>(mut stream: Option<T>) -> Vec<u8> {
        let mut buf = Vec::new();
        if let Some(ref mut reader) = stream {
            let _ = reader.read_to_end(&mut buf);
        }
        buf
    }
}

#[cfg(target_os = "macos")]
pub use macos::execute_native;

#[cfg(not(target_os = "macos"))]
pub fn execute_native(
    _skill: &Skill,
    _script_path: &Path,
    _script_type: ScriptType,
    _input: Value,
    _timeout_ms: u64,
    _enforcer: &PermissionEnforcer,
    _allowed_tools: &[String],
    _workspace_dir: Option<&Path>,
    _script_args: &[String],
) -> Result<ExecutionArtifacts, OpenSkillError> {
    Err(OpenSkillError::UnsupportedPlatform(
        "Native execution requires macOS (seatbelt)".to_string(),
    ))
}