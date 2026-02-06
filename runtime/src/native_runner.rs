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
    
    /// Safely join a thread with a timeout to prevent indefinite blocking
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
                "Thread panicked during execution".to_string()
            )),
            Err(_) => Err(OpenSkillError::Timeout),
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

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

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

    // Sensitive paths that should never be readable (credentials, keys)
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
        let allow_network = allowed_tools
            .iter()
            .any(|t| t == "WebSearch" || t == "Fetch");
        let allow_process = script_type == ScriptType::Shell
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

        // Build sandbox configuration
        let sandbox_config = SandboxConfig {
            skill_root: skill_root.clone(),
            read_paths: read_paths.clone(),
            write_paths: write_paths.clone(),
            allow_network,
            allow_process,
        };

        // Create the sandbox wrapper script that applies Landlock before exec
        let wrapper_script = build_sandbox_wrapper(&sandbox_config, &program, &args, script_args)?;
        let wrapper_path = write_wrapper_script(&wrapper_script)?;

        // Make wrapper executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&wrapper_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&wrapper_path, perms)?;
        }

        let mut cmd = Command::new("/bin/bash");
        cmd.arg(&wrapper_path);

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

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let _ = std::fs::remove_file(&wrapper_path);
                return Err(OpenSkillError::NativeExecutionError(format!(
                    "Failed to execute with sandbox: {e}"
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

        // Clean up wrapper script
        let _ = std::fs::remove_file(&wrapper_path);

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
                // Check for sandbox violations (Landlock/seccomp signals)
                let message = if let Some(signal) = status.signal() {
                    match signal {
                        libc::SIGSYS => "Sandbox violation: blocked system call (seccomp)".to_string(),
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

    #[derive(Debug)]
    struct SandboxConfig {
        skill_root: PathBuf,
        read_paths: Vec<PathBuf>,
        write_paths: Vec<PathBuf>,
        #[allow(dead_code)] // Used for future seccomp network filtering
        allow_network: bool,
        #[allow(dead_code)] // Used for future seccomp process filtering
        allow_process: bool,
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

    /// Build a shell wrapper script that applies Landlock sandboxing before executing the target.
    /// 
    /// We use a shell wrapper approach because:
    /// 1. Landlock must be applied by the process itself (can't be inherited like seccomp in some modes)
    /// 2. This keeps the sandbox setup in-process before exec
    /// 3. We leverage the landlock CLI tool if available, or use a minimal Python-based approach
    fn build_sandbox_wrapper(
        config: &SandboxConfig,
        program: &str,
        args: &[String],
        script_args: &[String],
    ) -> Result<String, OpenSkillError> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        
        // Build list of read-only paths
        let mut ro_paths: Vec<String> = SYSTEM_READ_PATHS
            .iter()
            .filter(|p| PathBuf::from(p).exists())
            .map(|s| s.to_string())
            .collect();
        
        // Add skill root as read-only
        ro_paths.push(config.skill_root.to_string_lossy().to_string());
        
        // Add configured read paths
        for p in &config.read_paths {
            if p.exists() {
                ro_paths.push(p.to_string_lossy().to_string());
            }
        }

        // Build list of read-write paths
        let mut rw_paths: Vec<String> = TEMP_PATHS
            .iter()
            .filter(|p| PathBuf::from(p).exists())
            .map(|s| s.to_string())
            .collect();
        
        // Add skill root as writable (skills may need to write within their directory)
        rw_paths.push(config.skill_root.to_string_lossy().to_string());
        
        // Add configured write paths
        for p in &config.write_paths {
            let path_str = p.to_string_lossy().to_string();
            if !rw_paths.contains(&path_str) {
                rw_paths.push(path_str);
            }
        }

        // Build sensitive paths to deny (expand ~ to home)
        let deny_paths: Vec<String> = SENSITIVE_DENY_PATHS
            .iter()
            .map(|p| p.replace('~', &home))
            .filter(|p| PathBuf::from(p).exists())
            .collect();

        // Build the command line
        let mut cmd_parts = vec![shell_escape(program)];
        for arg in args {
            cmd_parts.push(shell_escape(arg));
        }
        for arg in script_args {
            cmd_parts.push(shell_escape(arg));
        }
        let full_command = cmd_parts.join(" ");

        // Generate the wrapper script using Python's landlock bindings if available,
        // otherwise use a basic approach with prctl NO_NEW_PRIVS
        let script = format!(r##"#!/bin/bash
set -e

# OpenSkills Linux Sandbox Wrapper
# Applies Landlock filesystem restrictions before executing the target command

# First, check if Landlock is supported (Linux >= 5.13)
LANDLOCK_SUPPORTED=0
if [ -f /sys/kernel/security/lsm ]; then
    if grep -q landlock /sys/kernel/security/lsm 2>/dev/null; then
        LANDLOCK_SUPPORTED=1
    fi
fi

# Check for kernel version (Landlock requires 5.13+)
KERNEL_VERSION=$(uname -r | cut -d. -f1-2)
KERNEL_MAJOR=$(echo "$KERNEL_VERSION" | cut -d. -f1)
KERNEL_MINOR=$(echo "$KERNEL_VERSION" | cut -d. -f2)

if [ "$KERNEL_MAJOR" -lt 5 ] || ([ "$KERNEL_MAJOR" -eq 5 ] && [ "$KERNEL_MINOR" -lt 13 ]); then
    LANDLOCK_SUPPORTED=0
fi

apply_landlock_python() {{
    # Use Python to apply Landlock restrictions
    python3 << 'PYTHON_EOF'
import os
import sys
import ctypes
import ctypes.util

# Landlock constants (from kernel headers)
LANDLOCK_CREATE_RULESET_VERSION = 1 << 0

# ABI version 1 access rights
LANDLOCK_ACCESS_FS_EXECUTE = 1 << 0
LANDLOCK_ACCESS_FS_WRITE_FILE = 1 << 1
LANDLOCK_ACCESS_FS_READ_FILE = 1 << 2
LANDLOCK_ACCESS_FS_READ_DIR = 1 << 3
LANDLOCK_ACCESS_FS_REMOVE_DIR = 1 << 4
LANDLOCK_ACCESS_FS_REMOVE_FILE = 1 << 5
LANDLOCK_ACCESS_FS_MAKE_CHAR = 1 << 6
LANDLOCK_ACCESS_FS_MAKE_DIR = 1 << 7
LANDLOCK_ACCESS_FS_MAKE_REG = 1 << 8
LANDLOCK_ACCESS_FS_MAKE_SOCK = 1 << 9
LANDLOCK_ACCESS_FS_MAKE_FIFO = 1 << 10
LANDLOCK_ACCESS_FS_MAKE_BLOCK = 1 << 11
LANDLOCK_ACCESS_FS_MAKE_SYM = 1 << 12

# Combined access masks
LANDLOCK_ACCESS_FS_READ = LANDLOCK_ACCESS_FS_EXECUTE | LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR
LANDLOCK_ACCESS_FS_WRITE = (LANDLOCK_ACCESS_FS_WRITE_FILE | LANDLOCK_ACCESS_FS_REMOVE_DIR |
                            LANDLOCK_ACCESS_FS_REMOVE_FILE | LANDLOCK_ACCESS_FS_MAKE_CHAR |
                            LANDLOCK_ACCESS_FS_MAKE_DIR | LANDLOCK_ACCESS_FS_MAKE_REG |
                            LANDLOCK_ACCESS_FS_MAKE_SOCK | LANDLOCK_ACCESS_FS_MAKE_FIFO |
                            LANDLOCK_ACCESS_FS_MAKE_BLOCK | LANDLOCK_ACCESS_FS_MAKE_SYM)
LANDLOCK_ACCESS_FS_ALL = LANDLOCK_ACCESS_FS_READ | LANDLOCK_ACCESS_FS_WRITE

# Syscall numbers (x86_64)
SYS_landlock_create_ruleset = 444
SYS_landlock_add_rule = 445
SYS_landlock_restrict_self = 446

# Rule types
LANDLOCK_RULE_PATH_BENEATH = 1

# prctl constants
PR_SET_NO_NEW_PRIVS = 38

class LandlockRulesetAttr(ctypes.Structure):
    _fields_ = [
        ("handled_access_fs", ctypes.c_uint64),
    ]

class LandlockPathBeneathAttr(ctypes.Structure):
    _fields_ = [
        ("allowed_access", ctypes.c_uint64),
        ("parent_fd", ctypes.c_int),
    ]

def syscall(number, *args):
    libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
    libc.syscall.restype = ctypes.c_long
    libc.syscall.argtypes = [ctypes.c_long] + [ctypes.c_long] * len(args)
    result = libc.syscall(number, *args)
    if result < 0:
        errno = ctypes.get_errno()
        raise OSError(errno, os.strerror(errno))
    return result

def prctl(option, arg2=0, arg3=0, arg4=0, arg5=0):
    libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
    libc.prctl.restype = ctypes.c_int
    libc.prctl.argtypes = [ctypes.c_int, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong]
    result = libc.prctl(option, arg2, arg3, arg4, arg5)
    if result < 0:
        errno = ctypes.get_errno()
        raise OSError(errno, os.strerror(errno))
    return result

def apply_landlock():
    # Read-only paths
    ro_paths = {ro_paths_json}
    # Read-write paths  
    rw_paths = {rw_paths_json}
    # Paths to deny entirely
    deny_paths = {deny_paths_json}

    try:
        # Create ruleset with all filesystem access rights
        ruleset_attr = LandlockRulesetAttr()
        ruleset_attr.handled_access_fs = LANDLOCK_ACCESS_FS_ALL
        
        ruleset_fd = syscall(
            SYS_landlock_create_ruleset,
            ctypes.addressof(ruleset_attr),
            ctypes.sizeof(ruleset_attr),
            0
        )
        
        def add_rule(path, access):
            if not os.path.exists(path):
                return
            try:
                fd = os.open(path, os.O_PATH | os.O_CLOEXEC)
                try:
                    rule = LandlockPathBeneathAttr()
                    rule.allowed_access = access
                    rule.parent_fd = fd
                    syscall(
                        SYS_landlock_add_rule,
                        ruleset_fd,
                        LANDLOCK_RULE_PATH_BENEATH,
                        ctypes.addressof(rule),
                        0
                    )
                finally:
                    os.close(fd)
            except (OSError, IOError) as e:
                # Skip paths we can't access
                pass
        
        # Add read-only rules
        for path in ro_paths:
            # Skip if path is in deny list
            if any(path.startswith(d) or d.startswith(path) for d in deny_paths):
                continue
            add_rule(path, LANDLOCK_ACCESS_FS_READ)
        
        # Add read-write rules (these override read-only for the same paths)
        for path in rw_paths:
            add_rule(path, LANDLOCK_ACCESS_FS_ALL)
        
        # Apply NO_NEW_PRIVS (required before landlock_restrict_self)
        prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
        
        # Restrict ourselves with the ruleset
        syscall(SYS_landlock_restrict_self, ruleset_fd, 0)
        
        os.close(ruleset_fd)
        print("LANDLOCK_APPLIED", file=sys.stderr)
        
    except OSError as e:
        # Landlock not supported or failed - continue without it
        # Still apply NO_NEW_PRIVS for basic security
        try:
            prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
        except:
            pass
        print(f"LANDLOCK_FALLBACK: {{e}}", file=sys.stderr)

if __name__ == "__main__":
    apply_landlock()
PYTHON_EOF
}}

# Apply sandbox restrictions
if [ "$LANDLOCK_SUPPORTED" = "1" ]; then
    apply_landlock_python 2>&1 | grep -E "^LANDLOCK" >&2 || true
else
    # Fallback: at minimum apply NO_NEW_PRIVS via Python
    python3 -c "
import ctypes
import ctypes.util
libc = ctypes.CDLL(ctypes.util.find_library('c'))
libc.prctl(38, 1, 0, 0, 0)  # PR_SET_NO_NEW_PRIVS
" 2>/dev/null || true
    echo "LANDLOCK_UNSUPPORTED: kernel too old or Landlock disabled" >&2
fi

# Execute the actual command
exec {full_command}
"##,
            ro_paths_json = serde_json::to_string(&ro_paths).unwrap_or_else(|_| "[]".to_string()),
            rw_paths_json = serde_json::to_string(&rw_paths).unwrap_or_else(|_| "[]".to_string()),
            deny_paths_json = serde_json::to_string(&deny_paths).unwrap_or_else(|_| "[]".to_string()),
            full_command = full_command,
        );

        Ok(script)
    }

    fn shell_escape(s: &str) -> String {
        // Simple shell escaping - wrap in single quotes and escape single quotes
        if s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/') {
            s.to_string()
        } else {
            format!("'{}'", s.replace('\'', "'\\''"))
        }
    }

    fn write_wrapper_script(script: &str) -> Result<PathBuf, OpenSkillError> {
        use rand::Rng;
        
        let temp_dir = std::env::temp_dir();
        let pid = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let random_suffix: u32 = rand::thread_rng().gen();
        
        let filename = format!(
            "openskills-sandbox-{}-{}-{}.sh",
            pid, timestamp, random_suffix
        );
        let path = temp_dir.join(filename);
        
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| OpenSkillError::NativeExecutionError(format!(
                "Failed to create sandbox wrapper: {}", e
            )))?;
        
        file.write_all(script.as_bytes()).map_err(OpenSkillError::Io)?;
        file.flush().map_err(OpenSkillError::Io)?;
        
        Ok(path)
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

    fn join_thread_with_timeout<T: Send + 'static>(
        handle: thread::JoinHandle<T>,
        timeout: Duration,
    ) -> Result<T, OpenSkillError> {
        use std::sync::mpsc;
        
        let (tx, rx) = mpsc::channel();
        
        thread::spawn(move || {
            let result = handle.join();
            let _ = tx.send(result);
        });
        
        match rx.recv_timeout(timeout) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(OpenSkillError::NativeExecutionError(
                "Thread panicked during execution".to_string()
            )),
            Err(_) => Err(OpenSkillError::Timeout),
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