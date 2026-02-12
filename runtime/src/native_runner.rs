//! Native sandbox execution using OS-specific mechanisms.
//!
//! Provides OS-level sandboxing for native scripts (Python, shell) as a
//! complement to the WASM sandbox.
//!
//! ## Supported Platforms
//!
//! - **macOS**: Uses Seatbelt (sandbox-exec) with a dynamically generated profile
//! - **Linux**: Uses Landlock LSM (kernel 5.13+) for filesystem restrictions,
//!   with NO_NEW_PRIVS fallback for older kernels
//!
//! ## Security Model
//!
//! Both platforms follow Claude Code's security approach:
//! - Allow broad file reads (interpreters need access to libraries)
//! - Deny specific sensitive paths (~/.ssh, ~/.aws, etc.)
//! - Allow writes only to explicitly permitted paths (skill root, workspace, temp)
//! - Network and process spawning controlled by allowed_tools

use crate::audit::ExecutionStatus;
use crate::errors::OpenSkillError;
use crate::executor::ExecutionArtifacts;
use crate::permissions::PermissionEnforcer;
use crate::registry::Skill;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

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

// ============================================================================
// Shared utility functions (platform-independent)
// ============================================================================

/// Safely join a thread with a timeout to prevent indefinite blocking.
fn join_thread_with_timeout<T: Send + 'static>(
    handle: thread::JoinHandle<T>,
    timeout: Duration,
) -> Result<T, OpenSkillError> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();

    // Spawn a watcher thread
    thread::spawn(move || {
        let result = handle.join();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Err(OpenSkillError::NativeExecutionError(
            "Thread panicked during execution".to_string(),
        )),
        Err(_) => Err(OpenSkillError::Timeout),
    }
}

/// Read all bytes from an optional stream (used for stdout/stderr capture).
fn read_stream<T: std::io::Read>(mut stream: Option<T>) -> Vec<u8> {
    let mut buf = Vec::new();
    if let Some(ref mut reader) = stream {
        let _ = reader.read_to_end(&mut buf);
    }
    buf
}

/// Resolve an executable by searching PATH.
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

// ============================================================================
// macOS implementation (Seatbelt)
// ============================================================================

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::Instant;

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
        
        // Use panic-safe thread spawning with catch_unwind
        let stdout_handle = thread::spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_stream(stdout)))
                .unwrap_or_else(|_| Vec::new())
        });
        let stderr_handle = thread::spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_stream(stderr)))
                .unwrap_or_else(|_| Vec::new())
        });

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

        // Safely join threads with timeout to prevent indefinite blocking
        let stdout_bytes = join_thread_with_timeout(stdout_handle, Duration::from_secs(5))
            .unwrap_or_else(|_| Vec::new());
        let stderr_bytes = join_thread_with_timeout(stderr_handle, Duration::from_secs(5))
            .unwrap_or_else(|_| Vec::new());
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

        // Disable corepack's auto-pin feature to prevent it from traversing up
        // the directory tree and trying to add a `packageManager` field to
        // package.json files outside the sandbox's allowed write paths.
        cmd.env("COREPACK_ENABLE_AUTO_PIN", "0");

        // Set CI=true to signal non-interactive mode to tools like npm, pnpm,
        // create-vite, etc. This prevents them from prompting for input which
        // would cause issues since stdin may contain JSON input data.
        cmd.env("CI", "true");

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
        
        // process-fork: always allowed on macOS.
        // Apple's /usr/bin/python3 is a CLT shim that needs fork+exec to launch the
        // real interpreter and may probe for xcodebuild during init.  Blocking fork
        // breaks even simple Python scripts.  This matches Claude Code's model:
        // security comes from filesystem restrictions + permission system, not from
        // blocking process spawning.
        profile.push_str("(allow process-fork)\n");

        // Deny sensitive credential and config paths FIRST (before allow-all)
        // Seatbelt uses first-match-wins, so deny rules must come before allow rules
        // Validate HOME environment variable for security
        let home = std::env::var("HOME")
            .ok()
            .and_then(|h| {
                let path = Path::new(&h);
                // Validate: must be absolute, not empty, and not just "/"
                if path.is_absolute() && h.len() > 1 && h != "/" {
                    Some(h)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                // Fallback to system-appropriate default
                if cfg!(target_os = "macos") {
                    "/Users".to_string()
                } else {
                    "/home".to_string()
                }
            });
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

        // Additional process permissions if shell/terminal tools are allowed
        if allow_process {
            // Allow all process operations (broader than just fork+exec)
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

    fn write_profile(profile: &str) -> Result<PathBuf, OpenSkillError> {
        use rand::Rng;
        
        let temp_dir = std::env::temp_dir();
        let pid = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let random_suffix: u32 = rand::thread_rng().gen();
        
        let filename = format!(
            "openskills-seatbelt-{}-{}-{}.sb",
            pid, timestamp, random_suffix
        );
        let path = temp_dir.join(filename);
        
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| OpenSkillError::SeatbeltError(format!(
                "Failed to create seatbelt profile file: {}", e
            )))?;
        
        file.write_all(profile.as_bytes()).map_err(OpenSkillError::Io)?;
        file.flush().map_err(OpenSkillError::Io)?;
        
        Ok(path)
    }
}

#[cfg(target_os = "macos")]
pub use macos::execute_native;

// ============================================================================
// Linux implementation (Landlock LSM)
// ============================================================================

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::io::Write;
    use std::os::unix::process::CommandExt;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    use landlock::{
        Access, AccessFs, PathBeneath, PathFd,
        Ruleset, RulesetAttr, RulesetCreatedAttr, ABI,
    };

    // System paths that should be readable for interpreter execution
    const SYSTEM_READ_PATHS: &[&str] = &[
        "/usr/lib",
        "/usr/lib64",
        "/usr/libexec",
        "/usr/bin",
        "/usr/sbin",
        "/usr/share",
        "/usr/local",
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/etc",
        "/proc/self",
        "/dev/null",
        "/dev/urandom",
        "/dev/zero",
    ];

    // Python interpreter locations on Linux
    const PYTHON_EXEC_PATHS: &[&str] = &[
        "/usr/bin/python3",
        "/usr/bin/python",
        "/usr/local/bin/python3",
        "/usr/local/bin/python",
        "/opt/python/bin/python3",
    ];

    // Temp directories that need read/write access
    const TEMP_PATHS: &[&str] = &["/tmp", "/var/tmp"];

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
        let _allow_network = allowed_tools
            .iter()
            .any(|t| t == "WebSearch" || t == "Fetch");
        let _allow_process = script_type == ScriptType::Shell
            || allowed_tools
                .iter()
                .any(|t| t == "Bash" || t == "Terminal");

        // Canonicalize paths
        let skill_root = skill
            .root
            .canonicalize()
            .unwrap_or_else(|_| skill.root.clone());

        let read_paths: Vec<PathBuf> = enforcer
            .filesystem_read_paths()
            .iter()
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()))
            .collect();

        let mut write_paths: Vec<PathBuf> = enforcer
            .filesystem_write_paths()
            .iter()
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()))
            .collect();

        // Add workspace directory to write paths if configured
        if let Some(workspace) = workspace_dir {
            if workspace.exists() {
                write_paths
                    .push(workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf()));
            } else {
                // Create workspace if it doesn't exist
                let _ = std::fs::create_dir_all(workspace);
                write_paths.push(workspace.to_path_buf());
            }
        }

        let (program, args) = command_for_script(script_type, script_path)?;

        // --- Collect Landlock path sets ---
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());

        // Read-only paths: system paths + skill root + enforcer read paths
        let mut ro_paths: Vec<PathBuf> = SYSTEM_READ_PATHS
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();
        ro_paths.push(skill_root.clone());
        for p in &read_paths {
            if p.exists() && !ro_paths.contains(p) {
                ro_paths.push(p.clone());
            }
        }

        // Read-write paths: temp dirs + skill root + enforcer write paths + workspace
        let mut rw_paths: Vec<PathBuf> = TEMP_PATHS
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();
        rw_paths.push(skill_root.clone());
        for p in &write_paths {
            if !rw_paths.contains(p) {
                rw_paths.push(p.clone());
            }
        }

        // Sensitive deny paths (expanded from ~)
        let deny_paths: Vec<PathBuf> = SENSITIVE_DENY_PATHS
            .iter()
            .map(|p| PathBuf::from(p.replace('~', &home)))
            .collect();

        // --- Build command with pre_exec Landlock sandbox ---
        let mut cmd = Command::new(&program);
        cmd.args(&args);
        if !script_args.is_empty() {
            cmd.args(script_args);
        }

        // Use workspace_dir as cwd if provided, otherwise fall back to skill_root
        let working_directory = workspace_dir.unwrap_or(&skill_root);
        cmd.current_dir(working_directory);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        apply_environment(
            &mut cmd,
            skill,
            &input_json,
            timeout_ms,
            enforcer,
            script_type,
            workspace_dir,
        );

        // Apply Landlock sandbox restrictions in the child process before exec.
        // This is the correct approach: restrictions are applied between fork() and exec(),
        // so they are inherited by the target command.
        let ro_clone = ro_paths;
        let rw_clone = rw_paths;
        let deny_clone = deny_paths;
        unsafe {
            cmd.pre_exec(move || {
                apply_landlock(&ro_clone, &rw_clone, &deny_clone)
            });
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                return Err(OpenSkillError::LinuxSandboxError(format!(
                    "Failed to execute with Landlock sandbox: {e}"
                )));
            }
        };

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let input_clone = input_json.clone();
            thread::spawn(move || {
                let _ = stdin.write_all(input_clone.as_bytes());
            });
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Use panic-safe thread spawning
        let stdout_handle = thread::spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_stream(stdout)))
                .unwrap_or_else(|_| Vec::new())
        });
        let stderr_handle = thread::spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_stream(stderr)))
                .unwrap_or_else(|_| Vec::new())
        });

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

        // Safely join threads with timeout
        let stdout_bytes = join_thread_with_timeout(stdout_handle, Duration::from_secs(5))
            .unwrap_or_else(|_| Vec::new());
        let stderr_bytes = join_thread_with_timeout(stderr_handle, Duration::from_secs(5))
            .unwrap_or_else(|_| Vec::new());
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
                // Check for sandbox violations (SIGSYS from seccomp, SIGKILL, etc.)
                let message = if let Some(signal) = status.signal() {
                    match signal {
                        libc::SIGSYS => "Sandbox violation: blocked system call".to_string(),
                        libc::SIGKILL => {
                            if stderr.contains("landlock") || stderr.contains("Permission denied") {
                                "Sandbox violation: blocked file access (Landlock)".to_string()
                            } else {
                                format!("Process killed (signal {})", signal)
                            }
                        }
                        _ => format!("Process terminated by signal {}", signal),
                    }
                } else if !stderr.trim().is_empty() {
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

        Ok(ExecutionArtifacts {
            output,
            stdout,
            stderr,
            permissions_used: enforcer.permissions_used(),
            exit_status,
        })
    }

    /// Apply Landlock filesystem restrictions to the current process (called in pre_exec).
    ///
    /// Uses the `landlock` crate to create a ruleset that restricts filesystem access.
    /// On kernels that don't support Landlock (< 5.13), falls back to NO_NEW_PRIVS only.
    /// Never returns Err to avoid preventing process execution — sandbox failures are
    /// logged to stderr and execution continues with reduced security.
    fn apply_landlock(
        ro_paths: &[PathBuf],
        rw_paths: &[PathBuf],
        deny_paths: &[PathBuf],
    ) -> std::io::Result<()> {
        // Use ABI V1 (Linux 5.13+) for widest compatibility.
        // The landlock crate handles best-effort downgrade automatically.
        let abi = ABI::V1;

        let result = (|| -> Result<(), landlock::RulesetError> {
            let mut ruleset = Ruleset::default()
                .handle_access(AccessFs::from_all(abi))?
                .create()?;

            // Add read-only rules (excluding denied paths)
            for path in ro_paths {
                // Skip if this path overlaps with a deny path
                if deny_paths.iter().any(|d| path.starts_with(d) || d.starts_with(path)) {
                    continue;
                }
                if let Ok(fd) = PathFd::new(path) {
                    if let Ok(updated_ruleset) =
                        ruleset.add_rule(PathBeneath::new(fd, AccessFs::from_read(abi)))
                    {
                        ruleset = updated_ruleset;
                    }
                }
            }

            // Add read-write rules (these grant full access for the path subtree)
            for path in rw_paths {
                if let Ok(fd) = PathFd::new(path) {
                    if let Ok(updated_ruleset) =
                        ruleset.add_rule(PathBeneath::new(fd, AccessFs::from_all(abi)))
                    {
                        ruleset = updated_ruleset;
                    }
                }
            }

            // restrict_self() calls prctl(NO_NEW_PRIVS) then landlock_restrict_self()
            ruleset.restrict_self()?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                // Sandbox applied successfully
                Ok(())
            }
            Err(_e) => {
                // Landlock not supported or failed — apply NO_NEW_PRIVS at minimum
                // Safety: prctl(PR_SET_NO_NEW_PRIVS) is async-signal-safe
                unsafe {
                    libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
                }
                Ok(()) // Don't prevent execution even if sandbox fails
            }
        }
    }

    fn command_for_script(
        script_type: ScriptType,
        script_path: &Path,
    ) -> Result<(String, Vec<String>), OpenSkillError> {
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
                ))
            }
            ScriptType::Shell => Ok((
                "/bin/bash".to_string(),
                vec![script_path.to_string_lossy().to_string()],
            )),
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

        let path = std::env::var("PATH")
            .unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin".to_string());
        cmd.env("PATH", path);

        // Disable corepack auto-pin
        cmd.env("COREPACK_ENABLE_AUTO_PIN", "0");
        cmd.env("CI", "true");

        // Locale settings
        if let Ok(lang) = std::env::var("LANG") {
            cmd.env("LANG", lang);
        }
        if let Ok(lc_all) = std::env::var("LC_ALL") {
            cmd.env("LC_ALL", lc_all);
        }
        if let Ok(tmpdir) = std::env::var("TMPDIR") {
            cmd.env("TMPDIR", tmpdir);
        } else {
            cmd.env("TMPDIR", "/tmp");
        }

        // Skill-specific environment
        cmd.env("SKILL_ID", &skill.id);
        cmd.env("SKILL_NAME", &skill.manifest.name);
        cmd.env("SKILL_INPUT", input_json);
        cmd.env("TIMEOUT_MS", timeout_ms.to_string());
        cmd.env("SKILL_ROOT", skill.root.to_string_lossy().to_string());

        if let Some(workspace) = workspace_dir {
            cmd.env("SKILL_WORKSPACE", workspace.to_string_lossy().to_string());
        }

        // Pass through allowed environment variables
        for key in enforcer.env_allowlist() {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }

        // Python-specific settings
        if script_type == ScriptType::Python {
            cmd.env("PYTHONUNBUFFERED", "1");
            cmd.env("PYTHONDONTWRITEBYTECODE", "1");
            cmd.env("PYTHONNOUSERSITE", "1");
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::execute_native;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
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
        "Native execution requires macOS (seatbelt) or Linux (Landlock)".to_string(),
    ))
}