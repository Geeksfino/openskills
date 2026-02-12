//! Skill execution with WASM sandbox and native sandbox support.
//!
//! Executes skill scripts in a WASM sandbox or, on supported platforms,
//! in a native OS-level sandbox (seatbelt/seccomp).
//!
//! ## Execution Modes
//!
//! - **Auto-detect**: `execute_skill()` finds entry points automatically
//! - **Target execution**: `run_skill_target()` runs a specific script/WASM within a skill
//!
//! Both modes use the same permission model and sandbox.

use crate::audit::ExecutionStatus;
use crate::errors::OpenSkillError;
use crate::native_runner::{detect_script_type, execute_native, ScriptType};
use crate::permissions::{map_tools_to_capabilities, PermissionEnforcer};
use crate::registry::Skill;
use crate::wasm_runner::execute_wasm;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

/// Artifacts from skill execution.
pub struct ExecutionArtifacts {
    /// Output from the execution (for WASM: JSON result).
    pub output: Value,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Permissions that were used.
    pub permissions_used: Vec<String>,
    /// Exit status.
    pub exit_status: ExecutionStatus,
}

/// Options for skill execution.
#[derive(Debug, Clone, Default)]
pub struct ExecutionOptions {
    /// Override timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Override memory limit in MB.
    pub memory_mb: Option<u64>,
    /// Input data for WASM execution.
    pub input: Option<Value>,
    /// WASM module path override (relative to skill root).
    pub wasm_module: Option<String>,
    /// Workspace directory for skill I/O operations.
    /// If set, SKILL_WORKSPACE env var is injected and the directory is
    /// accessible with write permissions in the sandbox.
    pub workspace_dir: Option<PathBuf>,
    /// Host-policy-resolved tools used for sandbox capability mapping.
    /// Set by OpenSkillRuntime after resolving the skill manifest's allowed-tools
    /// against the host policy. Defaults to empty (no tools approved).
    pub effective_tools: Vec<String>,
}

/// Target for skill execution (what to run within a skill).
#[derive(Debug, Clone)]
pub enum ExecutionTarget {
    /// Auto-detect: find entry point (script.py, main.py, skill.wasm, etc.)
    Auto,
    /// Run a specific file - auto-detect type from extension (.wasm = WASM, .py/.sh = native).
    /// This is the recommended variant for transparent sandbox selection.
    Path {
        path: String,
        args: Vec<String>, // Only used for native scripts, ignored for WASM
    },
    /// Run a specific script (relative path within skill directory).
    /// Deprecated: Use `Path` instead for transparent sandbox selection.
    Script {
        path: String,
        args: Vec<String>,
    },
    /// Run a specific WASM module (relative path within skill directory).
    /// Deprecated: Use `Path` instead for transparent sandbox selection.
    Wasm {
        path: String,
    },
}

impl Default for ExecutionTarget {
    fn default() -> Self {
        Self::Auto
    }
}

/// Options for targeted skill execution.
#[derive(Debug, Clone, Default)]
pub struct TargetExecutionOptions {
    /// What to execute within the skill.
    pub target: ExecutionTarget,
    /// Override timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Override memory limit in MB.
    pub memory_mb: Option<u64>,
    /// Input data (JSON).
    pub input: Option<Value>,
    /// Working directory override (relative to skill root).
    pub working_dir: Option<String>,
    /// Workspace directory for skill I/O operations.
    /// If set, SKILL_WORKSPACE env var is injected and the directory is
    /// accessible with write permissions in the sandbox.
    pub workspace_dir: Option<PathBuf>,
    /// Host-policy-resolved tools used for sandbox capability mapping.
    /// Set by OpenSkillRuntime after resolving the skill manifest's allowed-tools
    /// against the host policy. Defaults to empty (no tools approved).
    pub effective_tools: Vec<String>,
}

#[derive(Debug)]
enum ExecutionMode {
    Wasm { wasm_module: String },
    Native { script_path: PathBuf, script_type: ScriptType },
}

/// Execute a skill's WASM module or native script in a sandbox.
///
/// For Claude Skills compatibility:
/// - Skills are primarily instructional (Claude follows the instructions)
/// - Script execution is sandboxed (WASM or native seatbelt/seccomp)
pub fn execute_skill(
    skill: &Skill,
    options: ExecutionOptions,
) -> Result<ExecutionArtifacts, OpenSkillError> {
    let allowed_tools = options.effective_tools.clone();
    let mut wasm_config = map_tools_to_capabilities(&allowed_tools);

    // Apply option overrides
    if let Some(timeout) = options.timeout_ms {
        wasm_config.timeout_ms = timeout;
    }
    if let Some(memory) = options.memory_mb {
        wasm_config.memory_mb = memory;
    }

    let enforcer = PermissionEnforcer::new(
        allowed_tools.clone(),
        wasm_config.clone(),
        skill.root.clone(),
    );

    // Prepare input
    let input = options.input.unwrap_or_else(|| {
        serde_json::json!({
            "skill_id": skill.id,
            "instructions": skill.instructions
        })
    });

    match detect_execution_mode(&skill.root, options.wasm_module)? {
        ExecutionMode::Wasm { wasm_module } => {
            let wasm_path = skill.root.join(&wasm_module);
            if !wasm_path.exists() {
                return Err(OpenSkillError::WasmError(format!(
                    "WASM module not found: {}",
                    wasm_path.display()
                )));
            }
            execute_wasm(
                skill,
                &wasm_module,
                input,
                wasm_config.timeout_ms,
                &enforcer,
                options.workspace_dir.as_deref(),
            )
        }
        ExecutionMode::Native {
            script_path,
            script_type,
        } => execute_native(
            skill,
            &script_path,
            script_type,
            input,
            wasm_config.timeout_ms,
            &enforcer,
            &allowed_tools,
            options.workspace_dir.as_deref(),
            &[], // No user-provided args for auto-detected scripts
        ),
    }
}

/// Execute a specific target within a skill.
///
/// Unlike `execute_skill()` which auto-detects the entry point, this function
/// runs a specific script or WASM module specified by the caller. This is
/// designed for Claude Skills where SKILL.md instructions tell the agent
/// which script to run.
///
/// # Arguments
///
/// * `skill` - The skill containing the target
/// * `options` - Target execution options including what to run
///
/// # Example
///
/// ```rust,ignore
/// // Run a specific Python script within a skill
/// let options = TargetExecutionOptions {
///     target: ExecutionTarget::Script {
///         path: "scripts/unpack.py".to_string(),
///         args: vec!["input.docx".to_string(), "output/".to_string()],
///     },
///     timeout_ms: Some(30000),
///     ..Default::default()
/// };
/// let result = run_skill_target(&skill, options)?;
/// ```
pub fn run_skill_target(
    skill: &Skill,
    options: TargetExecutionOptions,
) -> Result<ExecutionArtifacts, OpenSkillError> {
    // Map allowed-tools to capabilities
    let allowed_tools = options.effective_tools.clone();
    let mut wasm_config = map_tools_to_capabilities(&allowed_tools);

    // Apply option overrides
    if let Some(timeout) = options.timeout_ms {
        wasm_config.timeout_ms = timeout;
    }
    if let Some(memory) = options.memory_mb {
        wasm_config.memory_mb = memory;
    }

    let enforcer = PermissionEnforcer::new(
        allowed_tools.clone(),
        wasm_config.clone(),
        skill.root.clone(),
    );

    // Prepare input
    let input = options.input.unwrap_or_else(|| {
        serde_json::json!({
            "skill_id": skill.id,
            "instructions": skill.instructions
        })
    });

    match options.target {
        ExecutionTarget::Auto => {
            // Fallback to auto-detection
            match detect_execution_mode(&skill.root, None)? {
                ExecutionMode::Wasm { wasm_module } => {
                    let wasm_path = skill.root.join(&wasm_module);
                    if !wasm_path.exists() {
                        return Err(OpenSkillError::WasmError(format!(
                            "WASM module not found: {}",
                            wasm_path.display()
                        )));
                    }
                    execute_wasm(
                        skill,
                        &wasm_module,
                        input,
                        wasm_config.timeout_ms,
                        &enforcer,
                        options.workspace_dir.as_deref(),
                    )
                }
                ExecutionMode::Native {
                    script_path,
                    script_type,
                } => execute_native(
                    skill,
                    &script_path,
                    script_type,
                    input,
                    wasm_config.timeout_ms,
                    &enforcer,
                    &allowed_tools,
                    options.workspace_dir.as_deref(),
                    &[], // No user-provided args for auto-detected scripts
                ),
            }
        }
        ExecutionTarget::Path { path, args } => {
            // Auto-detect execution type from file extension
            let full_path = skill.root.join(&path);
            if !full_path.exists() {
                return Err(OpenSkillError::NativeExecutionError(format!(
                    "File not found: {}",
                    full_path.display()
                )));
            }

            // Validate path is within skill directory (security)
            // Use race-free path validation with canonicalization
            let canonical_skill = skill.root.canonicalize().map_err(|e| {
                OpenSkillError::NativeExecutionError(format!(
                    "Failed to canonicalize skill root: {}",
                    e
                ))
            })?;
            
            // Check if path exists before canonicalization to avoid race conditions
            if !full_path.exists() {
                return Err(OpenSkillError::NativeExecutionError(format!(
                    "File not found: {}",
                    full_path.display()
                )));
            }
            
            let canonical_file = full_path.canonicalize().map_err(|e| {
                OpenSkillError::NativeExecutionError(format!(
                    "Failed to canonicalize file path: {}",
                    e
                ))
            })?;
            
            // Use proper path comparison that handles edge cases
            if !canonical_file.starts_with(&canonical_skill) {
                return Err(OpenSkillError::NativeExecutionError(format!(
                    "Path escapes skill directory: {}",
                    path
                )));
            }

            // Auto-detect execution type from extension
            let path_lower = path.to_lowercase();
            if path_lower.ends_with(".wasm") {
                // Execute as WASM
                execute_wasm(
                    skill,
                    &path,
                    input,
                    wasm_config.timeout_ms,
                    &enforcer,
                    options.workspace_dir.as_deref(),
                )
            } else {
                // Execute as native script (seatbelt on macOS, seccomp on Linux when available)
                let script_type = detect_script_type(&full_path)?;
                // Keep args in input JSON for scripts that read from SKILL_INPUT
                let input_with_args = if args.is_empty() {
                    input
                } else {
                    let mut obj = match input {
                        Value::Object(map) => map,
                        _ => serde_json::Map::new(),
                    };
                    obj.insert("args".to_string(), Value::Array(
                        args.iter().map(|a| Value::String(a.clone())).collect()
                    ));
                    Value::Object(obj)
                };
                execute_native(
                    skill,
                    &full_path,
                    script_type,
                    input_with_args,
                    wasm_config.timeout_ms,
                    &enforcer,
                    &allowed_tools,
                    options.workspace_dir.as_deref(),
                    &args, // Pass args as command-line arguments too
                )
            }
        }
        ExecutionTarget::Script { path, args } => {
            let script_path = skill.root.join(&path);
            if !script_path.exists() {
                return Err(OpenSkillError::NativeExecutionError(format!(
                    "Script not found: {}",
                    script_path.display()
                )));
            }

            // Validate script is within skill directory (security)
            let canonical_skill = skill.root.canonicalize().map_err(|e| {
                OpenSkillError::NativeExecutionError(format!(
                    "Failed to canonicalize skill root: {}",
                    e
                ))
            })?;
            
            // Verify script exists before canonicalization
            if !script_path.exists() {
                return Err(OpenSkillError::NativeExecutionError(format!(
                    "Script not found: {}",
                    script_path.display()
                )));
            }
            
            let canonical_script = script_path.canonicalize().map_err(|e| {
                OpenSkillError::NativeExecutionError(format!(
                    "Failed to canonicalize script path: {}",
                    e
                ))
            })?;
            
            // Use proper path comparison that handles edge cases
            if !canonical_script.starts_with(&canonical_skill) {
                return Err(OpenSkillError::NativeExecutionError(format!(
                    "Script path escapes skill directory: {}",
                    path
                )));
            }

            let script_type = detect_script_type(&script_path)?;

            // Add args to input JSON for scripts that read from SKILL_INPUT
            let input_with_args = if args.is_empty() {
                input
            } else {
                let mut obj = match input {
                    Value::Object(map) => map,
                    _ => serde_json::Map::new(),
                };
                obj.insert("args".to_string(), Value::Array(
                    args.iter().map(|a| Value::String(a.clone())).collect()
                ));
                Value::Object(obj)
            };

            execute_native(
                skill,
                &script_path,
                script_type,
                input_with_args,
                wasm_config.timeout_ms,
                &enforcer,
                &allowed_tools,
                options.workspace_dir.as_deref(),
                &args, // Pass args as command-line arguments too
            )
        }
        ExecutionTarget::Wasm { path } => {
            let wasm_path = skill.root.join(&path);
            if !wasm_path.exists() {
                return Err(OpenSkillError::WasmError(format!(
                    "WASM module not found: {}",
                    wasm_path.display()
                )));
            }

            // Validate WASM is within skill directory (security)
            let canonical_skill = skill.root.canonicalize().map_err(|e| {
                OpenSkillError::WasmError(format!(
                    "Failed to canonicalize skill root: {}",
                    e
                ))
            })?;
            let canonical_wasm = wasm_path.canonicalize().map_err(|e| {
                OpenSkillError::WasmError(format!(
                    "Failed to canonicalize WASM path: {}",
                    e
                ))
            })?;
            if !canonical_wasm.starts_with(&canonical_skill) {
                return Err(OpenSkillError::WasmError(format!(
                    "WASM path escapes skill directory: {}",
                    path
                )));
            }

            execute_wasm(
                skill,
                &path,
                input,
                wasm_config.timeout_ms,
                &enforcer,
                options.workspace_dir.as_deref(),
            )
        }
    }
}

/// Read a file from a skill directory.
///
/// This allows agents to read helper files (like `docx-js.md`) that skills
/// reference in their SKILL.md instructions.
///
/// # Security
///
/// The path must be within the skill directory. Attempts to escape via
/// `..` or symlinks are rejected.
pub fn read_skill_file(skill_root: &Path, relative_path: &str) -> Result<String, OpenSkillError> {
    let file_path = skill_root.join(relative_path);

    // Validate path is within skill directory
    let canonical_skill = skill_root.canonicalize().map_err(|e| {
        OpenSkillError::NativeExecutionError(format!(
            "Failed to canonicalize skill root: {}",
            e
        ))
    })?;
    let canonical_file = file_path.canonicalize().map_err(|e| {
        OpenSkillError::NativeExecutionError(format!(
            "File not found or not accessible: {} ({})",
            relative_path, e
        ))
    })?;
    if !canonical_file.starts_with(&canonical_skill) {
        return Err(OpenSkillError::NativeExecutionError(format!(
            "Path escapes skill directory: {}",
            relative_path
        )));
    }

    std::fs::read_to_string(&canonical_file).map_err(|e| {
        OpenSkillError::NativeExecutionError(format!(
            "Failed to read file {}: {}",
            relative_path, e
        ))
    })
}

/// List files in a skill directory (or subdirectory).
///
/// Returns relative paths from the skill root.
///
/// # Arguments
///
/// * `skill_root` - The skill root directory
/// * `subdir` - Optional subdirectory (e.g., "scripts", "ooxml/scripts")
/// * `recursive` - Whether to list recursively
pub fn list_skill_files(
    skill_root: &Path,
    subdir: Option<&str>,
    recursive: bool,
) -> Result<Vec<String>, OpenSkillError> {
    let base_path = match subdir {
        Some(sub) => skill_root.join(sub),
        None => skill_root.to_path_buf(),
    };

    // Validate path is within skill directory
    if let Some(sub) = subdir {
        let canonical_skill = skill_root.canonicalize().map_err(|e| {
            OpenSkillError::NativeExecutionError(format!(
                "Failed to canonicalize skill root: {}",
                e
            ))
        })?;
        let canonical_sub = base_path.canonicalize().map_err(|e| {
            OpenSkillError::NativeExecutionError(format!(
                "Subdirectory not found: {} ({})",
                sub, e
            ))
        })?;
        if !canonical_sub.starts_with(&canonical_skill) {
            return Err(OpenSkillError::NativeExecutionError(format!(
                "Subdirectory escapes skill directory: {}",
                sub
            )));
        }
    }

    let mut files = Vec::new();
    collect_files(&base_path, skill_root, recursive, &mut files)?;
    Ok(files)
}

fn collect_files(
    dir: &Path,
    skill_root: &Path,
    recursive: bool,
    files: &mut Vec<String>,
) -> Result<(), OpenSkillError> {
    if !dir.is_dir() {
        return Ok(());
    }

    let entries = std::fs::read_dir(dir).map_err(|e| {
        OpenSkillError::NativeExecutionError(format!(
            "Failed to read directory {}: {}",
            dir.display(),
            e
        ))
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Ok(relative) = path.strip_prefix(skill_root) {
                if let Some(relative_str) = relative.to_str() {
                    files.push(relative_str.to_string());
                }
            }
        } else if path.is_dir() && recursive {
            collect_files(&path, skill_root, true, files)?;
        }
    }

    Ok(())
}

fn detect_execution_mode(
    skill_root: &PathBuf,
    wasm_override: Option<String>,
) -> Result<ExecutionMode, OpenSkillError> {
    if let Some(wasm_module) = wasm_override {
        return Ok(ExecutionMode::Wasm { wasm_module });
    }

    if let Some(wasm_module) = find_wasm_module(skill_root) {
        return Ok(ExecutionMode::Wasm { wasm_module });
    }

    if let Some(script_path) = find_native_script(skill_root) {
        let script_type = detect_script_type(&script_path)?;
        return Ok(ExecutionMode::Native {
            script_path,
            script_type,
        });
    }

    Err(OpenSkillError::NativeExecutionError(
        "No executable artifact found (expected .wasm, .py, or .sh)".to_string(),
    ))
}

/// Find a WASM module in the skill directory.
fn find_wasm_module(skill_root: &PathBuf) -> Option<String> {
    // Look for common patterns
    let candidates = [
        "skill.wasm",
        "wasm/skill.wasm",
        "module.wasm",
        "main.wasm",
    ];

    for candidate in candidates {
        if skill_root.join(candidate).exists() {
            return Some(candidate.to_string());
        }
    }

    // Look for any .wasm file
    if let Ok(entries) = std::fs::read_dir(skill_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "wasm").unwrap_or(false) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    return Some(name.to_string());
                }
            }
        }
    }

    None
}

/// Find a native script in the skill directory.
fn find_native_script(skill_root: &PathBuf) -> Option<PathBuf> {
    let candidates = [
        "script.py",
        "main.py",
        "src/main.py",
        "index.py",
        "src/index.py",
        "script.sh",
        "main.sh",
        "src/main.sh",
        "index.sh",
        "src/index.sh",
        "script.bash",
        "main.bash",
        "src/main.bash",
        "index.bash",
        "src/index.bash",
    ];

    for candidate in candidates {
        let path = skill_root.join(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    for dir in [skill_root.to_path_buf(), skill_root.join("src"), skill_root.join("scripts")] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        let ext = ext.to_ascii_lowercase();
                        if ext == "py" || ext == "sh" || ext == "bash" {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    None
}

// ============================================================================
// Sandboxed Command Execution (skill-independent)
// ============================================================================

/// Permissions for sandboxed command execution.
///
/// This is designed for agents that need to run shell commands with
/// Seatbelt sandboxing (on macOS), similar to Claude Code's bash tool.
#[derive(Debug, Clone, Default)]
pub struct CommandPermissions {
    /// Allow network access.
    pub allow_network: bool,
    /// Allow subprocess spawning.
    pub allow_process: bool,
    /// Directories the command can read from.
    pub read_paths: Vec<PathBuf>,
    /// Directories the command can write to.
    pub write_paths: Vec<PathBuf>,
    /// Environment variables to pass through.
    pub env_vars: Vec<(String, String)>,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
}

/// Result from sandboxed command execution.
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Exit code (0 = success).
    pub exit_code: i32,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Whether the command timed out.
    pub timed_out: bool,
}

/// Run a shell command in a sandboxed environment.
///
/// This provides Claude Code-like sandboxed bash execution for agents.
/// - On macOS, uses Seatbelt sandbox-exec
/// - On Linux, uses Landlock LSM (kernel 5.13+) with NO_NEW_PRIVS fallback
/// - On other platforms, returns an error
///
/// # Arguments
///
/// * `command` - Shell command to execute (passed to /bin/bash -c)
/// * `working_dir` - Working directory for the command
/// * `permissions` - Sandbox permissions
///
/// # Example
///
/// ```rust,ignore
/// let result = run_sandboxed_command(
///     "ls -la",
///     &PathBuf::from("/tmp/workspace"),
///     CommandPermissions {
///         read_paths: vec![PathBuf::from("/tmp/workspace")],
///         write_paths: vec![PathBuf::from("/tmp/workspace/output")],
///         timeout_ms: 30000,
///         ..Default::default()
///     },
/// )?;
/// println!("Output: {}", result.stdout);
/// ```
#[cfg(target_os = "macos")]
pub fn run_sandboxed_command(
    command: &str,
    working_dir: &Path,
    permissions: CommandPermissions,
) -> Result<CommandResult, OpenSkillError> {
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    // Validate working directory exists
    if !working_dir.exists() {
        return Err(OpenSkillError::NativeExecutionError(format!(
            "Working directory does not exist: {}",
            working_dir.display()
        )));
    }

    let canonical_working_dir = working_dir.canonicalize().map_err(|e| {
        OpenSkillError::NativeExecutionError(format!(
            "Failed to canonicalize working directory: {}",
            e
        ))
    })?;

    // Build seatbelt profile
    let profile = build_command_seatbelt_profile(
        &canonical_working_dir,
        &permissions,
    );

    // Write profile to temp file
    let profile_path = write_temp_profile(&profile)?;

    // Build command: sandbox-exec -f <profile> -- /bin/bash -c "<command>"
    let mut cmd = Command::new("sandbox-exec");
    cmd.arg("-f")
        .arg(&profile_path)
        .arg("--")
        .arg("/bin/bash")
        .arg("-c")
        .arg(command);
    cmd.current_dir(&canonical_working_dir);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set up minimal environment
    cmd.env_clear();
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    } else {
        cmd.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:/opt/homebrew/bin");
    }
    if let Ok(lang) = std::env::var("LANG") {
        cmd.env("LANG", lang);
    }
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        cmd.env("TMPDIR", tmpdir);
    }
    // Pass through user-specified environment variables
    for (key, value) in &permissions.env_vars {
        cmd.env(key, value);
    }

    // Spawn the process
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            let _ = std::fs::remove_file(&profile_path);
            return Err(OpenSkillError::SeatbeltError(format!(
                "Failed to execute command with seatbelt: {}",
                e
            )));
        }
    };

    // Read stdout/stderr in separate threads with panic handling
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_handle = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_stream_to_string(stdout)))
            .unwrap_or_else(|_| String::new())
    });
    let stderr_handle = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_stream_to_string(stderr)))
            .unwrap_or_else(|_| String::new())
    });

    // Wait with timeout
    let timeout_ms = if permissions.timeout_ms > 0 {
        permissions.timeout_ms
    } else {
        30000 // 30 second default
    };
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

    // Clean up profile
    let _ = std::fs::remove_file(&profile_path);

    // Collect output with timeout to prevent indefinite blocking
    let stdout_content = join_thread_with_timeout(stdout_handle, Duration::from_secs(5))
        .unwrap_or_else(|_| String::new());
    let stderr_content = join_thread_with_timeout(stderr_handle, Duration::from_secs(5))
        .unwrap_or_else(|_| String::new());

    let exit_code = status
        .and_then(|s| s.code())
        .unwrap_or(if timed_out { -1 } else { 1 });

    Ok(CommandResult {
        exit_code,
        stdout: stdout_content,
        stderr: stderr_content,
        timed_out,
    })
}

#[cfg(target_os = "macos")]
fn build_command_seatbelt_profile(
    working_dir: &Path,
    permissions: &CommandPermissions,
) -> String {
    let mut profile = String::from("(version 1)\n(deny default)\n");

    // Basic system access for running shell commands
    profile.push_str("(allow process-exec)\n");
    profile.push_str("(allow sysctl-read)\n");
    profile.push_str("(allow mach-lookup)\n");
    profile.push_str("(allow ipc-posix-shm-read-data)\n");
    profile.push_str("(allow ipc-posix-shm-write-data)\n");
    profile.push_str("(allow ipc-posix-shm)\n");
    profile.push_str("(allow ipc-posix-sem)\n");
    profile.push_str("(allow iokit-get-properties)\n");
    profile.push_str("(allow file-read-metadata)\n");

    // System directories for basic shell operations
    let system_read_paths = [
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
        "/dev",
    ];
    for path in system_read_paths {
        profile.push_str(&format!(
            "(allow file-read* (subpath \"{}\"))\n",
            escape_seatbelt_path(path)
        ));
    }

    // Temp directories - read and write
    let temp_paths = ["/tmp", "/private/tmp", "/private/var/tmp", "/private/var/folders"];
    for path in temp_paths {
        profile.push_str(&format!(
            "(allow file-read* (subpath \"{}\"))\n",
            escape_seatbelt_path(path)
        ));
        profile.push_str(&format!(
            "(allow file-write* (subpath \"{}\"))\n",
            escape_seatbelt_path(path)
        ));
    }

    // Working directory - always readable
    profile.push_str(&format!(
        "(allow file-read* (subpath \"{}\"))\n",
        escape_seatbelt_path(working_dir.to_string_lossy().as_ref())
    ));

    // User-specified read paths
    for path in &permissions.read_paths {
        if let Ok(canonical) = path.canonicalize() {
            profile.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                escape_seatbelt_path(canonical.to_string_lossy().as_ref())
            ));
        }
    }

    // User-specified write paths
    for path in &permissions.write_paths {
        if let Ok(canonical) = path.canonicalize() {
            profile.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                escape_seatbelt_path(canonical.to_string_lossy().as_ref())
            ));
        } else if path.exists() || path.parent().map(|p| p.exists()).unwrap_or(false) {
            // Path might not exist yet but parent does
            profile.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                escape_seatbelt_path(path.to_string_lossy().as_ref())
            ));
        }
    }

    // Process permissions
    if permissions.allow_process {
        profile.push_str("(allow process-fork)\n");
        profile.push_str("(allow process*)\n");
    }

    // Network permissions
    if permissions.allow_network {
        profile.push_str("(allow network*)\n");
    }

    profile
}

#[cfg(target_os = "macos")]
fn escape_seatbelt_path(path: &str) -> String {
    path.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn write_temp_profile(profile: &str) -> Result<PathBuf, OpenSkillError> {
    use std::io::Write;
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let pid = std::process::id();
    let random_suffix = rand::random::<u32>();
    let path = std::env::temp_dir().join(format!("openskills_cmd_{}_{}_{}.sb", pid, timestamp, random_suffix));
    
    let mut file = std::fs::File::create(&path).map_err(|e| {
        OpenSkillError::SeatbeltError(format!("Failed to create profile file: {}", e))
    })?;
    file.write_all(profile.as_bytes()).map_err(|e| {
        OpenSkillError::SeatbeltError(format!("Failed to write profile: {}", e))
    })?;
    file.flush().map_err(|e| {
        OpenSkillError::SeatbeltError(format!("Failed to flush profile: {}", e))
    })?;
    
    Ok(path)
}

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

fn read_stream_to_string<R: std::io::Read>(stream: Option<R>) -> String {
    let Some(mut stream) = stream else {
        return String::new();
    };
    let mut buf = String::new();
    let _ = stream.read_to_string(&mut buf);
    buf
}

/// Linux implementation of sandboxed command execution using Landlock.
///
/// Uses `CommandExt::pre_exec` to apply Landlock filesystem restrictions
/// in the child process before exec, ensuring the sandbox is actually enforced.
#[cfg(target_os = "linux")]
pub fn run_sandboxed_command(
    command: &str,
    working_dir: &Path,
    permissions: CommandPermissions,
) -> Result<CommandResult, OpenSkillError> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    use landlock::{
        Access, AccessFs, PathBeneath, PathFd,
        Ruleset, RulesetAttr, RulesetCreatedAttr, ABI,
    };

    // Validate working directory exists
    if !working_dir.exists() {
        return Err(OpenSkillError::NativeExecutionError(format!(
            "Working directory does not exist: {}",
            working_dir.display()
        )));
    }

    let canonical_working_dir = working_dir.canonicalize().map_err(|e| {
        OpenSkillError::NativeExecutionError(format!(
            "Failed to canonicalize working directory: {}",
            e
        ))
    })?;

    // --- Collect Landlock path sets ---
    // System paths needed for basic command execution
    let system_ro_paths: &[&str] = &[
        "/usr/lib", "/usr/lib64", "/usr/libexec",
        "/usr/bin", "/usr/sbin", "/usr/share", "/usr/local",
        "/bin", "/sbin", "/lib", "/lib64",
        "/etc", "/proc/self",
        "/dev/null", "/dev/urandom", "/dev/zero",
    ];

    let mut ro_paths: Vec<PathBuf> = system_ro_paths
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();
    ro_paths.push(canonical_working_dir.clone());
    for p in &permissions.read_paths {
        if p.exists() && !ro_paths.contains(p) {
            ro_paths.push(p.clone());
        }
    }

    let mut rw_paths: Vec<PathBuf> = vec![
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
    ];
    for p in &permissions.write_paths {
        if !rw_paths.contains(p) {
            rw_paths.push(p.clone());
        }
    }

    // --- Build command with pre_exec Landlock sandbox ---
    let mut cmd = Command::new("/bin/bash");
    cmd.arg("-c").arg(command);
    cmd.current_dir(&canonical_working_dir);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set up minimal environment
    cmd.env_clear();
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    } else {
        cmd.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin");
    }
    if let Ok(lang) = std::env::var("LANG") {
        cmd.env("LANG", lang);
    }
    cmd.env("TMPDIR", "/tmp");

    // Pass through user-specified environment variables
    for (key, value) in &permissions.env_vars {
        cmd.env(key, value);
    }

    // Apply Landlock sandbox restrictions in the child process before exec
    let ro_clone = ro_paths;
    let rw_clone = rw_paths;
    unsafe {
        cmd.pre_exec(move || {
            let abi = ABI::V1;
            let result = (|| -> Result<(), landlock::RulesetError> {
                let mut ruleset = Ruleset::default()
                    .handle_access(AccessFs::from_all(abi))?
                    .create()?;

                for path in &ro_clone {
                    if let Ok(fd) = PathFd::new(path) {
                        if let Ok(updated_ruleset) =
                            ruleset.add_rule(PathBeneath::new(fd, AccessFs::from_read(abi)))
                        {
                            ruleset = updated_ruleset;
                        }
                    }
                }

                for path in &rw_clone {
                    if let Ok(fd) = PathFd::new(path) {
                        if let Ok(updated_ruleset) =
                            ruleset.add_rule(PathBeneath::new(fd, AccessFs::from_all(abi)))
                        {
                            ruleset = updated_ruleset;
                        }
                    }
                }

                ruleset.restrict_self()?;
                Ok(())
            })();

            if result.is_err() {
                // Fallback: apply NO_NEW_PRIVS at minimum
                unsafe {
                    libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
                }
            }
            Ok(())
        });
    }

    // Spawn the process
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            return Err(OpenSkillError::LinuxSandboxError(format!(
                "Failed to execute command with Landlock sandbox: {}",
                e
            )));
        }
    };

    // Read stdout/stderr in separate threads with panic handling
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_handle = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_stream_to_string(stdout)))
            .unwrap_or_else(|_| String::new())
    });
    let stderr_handle = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read_stream_to_string(stderr)))
            .unwrap_or_else(|_| String::new())
    });

    // Wait with timeout
    let timeout_ms = if permissions.timeout_ms > 0 {
        permissions.timeout_ms
    } else {
        30000 // 30 second default
    };
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

    // Collect output with timeout to prevent indefinite blocking
    let stdout_content = join_thread_with_timeout(stdout_handle, Duration::from_secs(5))
        .unwrap_or_else(|_| String::new());
    let stderr_content = join_thread_with_timeout(stderr_handle, Duration::from_secs(5))
        .unwrap_or_else(|_| String::new());

    let exit_code = status
        .and_then(|s| s.code())
        .unwrap_or(if timed_out { -1 } else { 1 });

    Ok(CommandResult {
        exit_code,
        stdout: stdout_content,
        stderr: stderr_content,
        timed_out,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn run_sandboxed_command(
    _command: &str,
    _working_dir: &Path,
    _permissions: CommandPermissions,
) -> Result<CommandResult, OpenSkillError> {
    Err(OpenSkillError::UnsupportedPlatform(
        "Sandboxed command execution requires macOS (seatbelt) or Linux (Landlock)".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_find_wasm_module_none() {
        let path = PathBuf::from("/nonexistent");
        assert!(find_wasm_module(&path).is_none());
    }

    #[test]
    fn test_find_native_script_none() {
        let path = PathBuf::from("/nonexistent");
        assert!(find_native_script(&path).is_none());
    }

    #[test]
    fn test_find_native_script_detects_python() {
        let temp = TempDir::new().unwrap();
        let skill_root = temp.path();
        let script_path = skill_root.join("script.py");
        std::fs::write(&script_path, "print('ok')").unwrap();

        let found = find_native_script(&skill_root.to_path_buf());
        assert_eq!(found.unwrap(), script_path);
    }
}
