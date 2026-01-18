use napi::bindgen_prelude::*;
use napi_derive::napi;
use openskills_runtime::{
    ExecutionContext, ExecutionOptions, OpenSkillRuntime, OutputType, RuntimeConfig,
    RuntimeExecutionStatus, SkillExecutionSession, SkillLocation,
};
use std::sync::Mutex;

#[napi(object)]
pub struct SkillDescriptorJs {
    pub id: String,
    pub description: String,
    pub location: String,
    pub user_invocable: bool,
}

#[napi(object)]
pub struct LoadedSkillJs {
    pub id: String,
    pub name: String,
    pub description: String,
    pub allowed_tools: Vec<String>,
    pub model: Option<String>,
    pub context: Option<String>,
    pub agent: Option<String>,
    pub user_invocable: bool,
    pub location: String,
    pub instructions: String,
}

#[napi(object)]
pub struct ExecutionOptionsJs {
    #[napi(ts_type = "number")]
    pub timeout_ms: Option<i64>,
    #[napi(ts_type = "number")]
    pub memory_mb: Option<i64>,
    pub input: Option<String>, // JSON string
}

#[napi(object)]
pub struct AuditRecord {
    pub skill_id: String,
    pub version: String,
    pub input_hash: String,
    pub output_hash: String,
    #[napi(ts_type = "number")]
    pub start_time_ms: i64,
    #[napi(ts_type = "number")]
    pub duration_ms: i64,
    pub permissions_used: Vec<String>,
    pub exit_status: String,
    pub stdout: String,
    pub stderr: String,
}

#[napi(object)]
pub struct ExecutionResult {
    pub output_json: String,
    pub stdout: String,
    pub stderr: String,
    pub audit: AuditRecord,
}

#[napi]
pub struct SkillExecutionSessionWrapper {
    inner: Mutex<SkillExecutionSession>,
}

#[napi]
pub struct ExecutionContextWrapper {
    inner: Mutex<ExecutionContext>,
}

#[napi]
impl ExecutionContextWrapper {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ExecutionContext::new()),
        }
    }

    #[napi]
    pub fn fork(&self) -> ExecutionContextWrapper {
        let forked = self.inner.lock().unwrap().fork();
        ExecutionContextWrapper {
            inner: Mutex::new(forked),
        }
    }

    #[napi]
    pub fn id(&self) -> String {
        self.inner.lock().unwrap().id().to_string()
    }

    #[napi]
    pub fn is_forked(&self) -> bool {
        self.inner.lock().unwrap().is_forked()
    }

    #[napi]
    pub fn parent_id(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .parent_id()
            .map(|id| id.to_string())
    }

    #[napi]
    pub fn summary(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .summary()
            .map(|s| s.to_string())
    }

    #[napi]
    pub fn record_output(&self, output_type: String, content: String) -> Result<()> {
        let output_type = parse_output_type(&output_type)?;
        self.inner
            .lock()
            .unwrap()
            .record_output(output_type, content);
        Ok(())
    }

    #[napi]
    pub fn summarize(&self) -> String {
        self.inner.lock().unwrap().summarize()
    }
}

#[napi]
pub struct OpenSkillRuntimeWrapper {
    inner: Mutex<OpenSkillRuntime>,
}

#[napi]
impl OpenSkillRuntimeWrapper {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(OpenSkillRuntime::new()),
        }
    }

    #[napi(factory)]
    pub fn with_project_root(project_root: String) -> Self {
        Self {
            inner: Mutex::new(OpenSkillRuntime::with_project_root(project_root)),
        }
    }

    #[napi(factory)]
    pub fn from_directory(skills_dir: String) -> Self {
        Self {
            inner: Mutex::new(OpenSkillRuntime::from_directory(skills_dir)),
        }
    }

    /// Create runtime with custom directories and configuration
    #[napi(factory)]
    pub fn with_custom_directories(
        custom_directories: Vec<String>,
        use_standard_locations: Option<bool>,
        project_root: Option<String>,
    ) -> Self {
        let config = RuntimeConfig {
            custom_directories: custom_directories
                .into_iter()
                .map(|s| s.into())
                .collect(),
            use_standard_locations: use_standard_locations.unwrap_or(true),
            project_root: project_root.map(|s| s.into()),
        };
        Self {
            inner: Mutex::new(OpenSkillRuntime::from_config(config)),
        }
    }


    /// Discover skills from standard locations (~/.claude/skills/, .claude/skills/, nested)
    #[napi]
    pub fn discover_skills(&self) -> Result<Vec<SkillDescriptorJs>> {
        let mut runtime = self.inner.lock().unwrap();
        let skills = runtime
            .discover_skills()
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(skills
            .into_iter()
            .map(|s| SkillDescriptorJs {
                id: s.id,
                description: s.description,
                location: match s.location {
                    SkillLocation::Personal => "personal".to_string(),
                    SkillLocation::Project => "project".to_string(),
                    SkillLocation::Nested => "nested".to_string(),
                    SkillLocation::Custom => "custom".to_string(),
                },
                user_invocable: s.user_invocable,
            })
            .collect())
    }

    /// Load skills from a specific directory (additive - can be called multiple times)
    #[napi]
    pub fn load_from_directory(&self, dir: String) -> Result<Vec<SkillDescriptorJs>> {
        let mut runtime = self.inner.lock().unwrap();
        let skills = runtime
            .load_from_directory(dir)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(skills
            .into_iter()
            .map(|s| SkillDescriptorJs {
                id: s.id,
                description: s.description,
                location: match s.location {
                    SkillLocation::Personal => "personal".to_string(),
                    SkillLocation::Project => "project".to_string(),
                    SkillLocation::Nested => "nested".to_string(),
                    SkillLocation::Custom => "custom".to_string(),
                },
                user_invocable: s.user_invocable,
            })
            .collect())
    }

    /// List skills (progressive disclosure - descriptors only)
    #[napi]
    pub fn list_skills(&self) -> Result<Vec<SkillDescriptorJs>> {
        let runtime = self.inner.lock().unwrap();
        let skills = runtime.list_skills();

        Ok(skills
            .into_iter()
            .map(|s| SkillDescriptorJs {
                id: s.id,
                description: s.description,
                location: match s.location {
                    SkillLocation::Personal => "personal".to_string(),
                    SkillLocation::Project => "project".to_string(),
                    SkillLocation::Nested => "nested".to_string(),
                    SkillLocation::Custom => "custom".to_string(),
                },
                user_invocable: s.user_invocable,
            })
            .collect())
    }

    /// Activate a skill (load full SKILL.md content)
    #[napi]
    pub fn activate_skill(&self, skill_id: String) -> Result<LoadedSkillJs> {
        let runtime = self.inner.lock().unwrap();
        let loaded = runtime
            .activate_skill(&skill_id)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(LoadedSkillJs {
            id: loaded.id.clone(),
            name: loaded.manifest.name.clone(),
            description: loaded.manifest.description.clone(),
            allowed_tools: loaded.manifest.get_allowed_tools(),
            model: loaded.manifest.model.clone(),
            context: loaded.manifest.context.clone(),
            agent: loaded.manifest.agent.clone(),
            user_invocable: loaded.manifest.is_user_invocable(),
            location: match loaded.location {
                SkillLocation::Personal => "personal".to_string(),
                SkillLocation::Project => "project".to_string(),
                SkillLocation::Nested => "nested".to_string(),
                SkillLocation::Custom => "custom".to_string(),
            },
            instructions: loaded.instructions.clone(),
        })
    }

    /// Execute a skill's WASM module
    #[napi]
    pub fn execute_skill(
        &self,
        skill_id: String,
        options: Option<ExecutionOptionsJs>,
    ) -> Result<ExecutionResult> {
        let mut runtime = self.inner.lock().unwrap();

        let exec_options = if let Some(opts) = options {
            ExecutionOptions {
                timeout_ms: opts.timeout_ms.map(|t| if t < 0 { 0 } else { t as u64 }),
                memory_mb: opts.memory_mb.map(|m| if m < 0 { 0 } else { m as u64 }),
                input: opts.input.and_then(|s| {
                    serde_json::from_str(&s).ok()
                }),
            }
        } else {
            ExecutionOptions::default()
        };

        let result = runtime
            .execute_skill(&skill_id, exec_options)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        let output_json = serde_json::to_string(&result.output)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        let exit_status = match result.audit.exit_status {
            RuntimeExecutionStatus::Success => "success".to_string(),
            RuntimeExecutionStatus::Timeout => "timeout".to_string(),
            RuntimeExecutionStatus::PermissionDenied => "permission_denied".to_string(),
            RuntimeExecutionStatus::Failed(msg) => format!("failed:{}", msg),
        };

        Ok(ExecutionResult {
            output_json,
            stdout: result.stdout,
            stderr: result.stderr,
            audit: AuditRecord {
                skill_id: result.audit.skill_id,
                version: result.audit.version,
                input_hash: result.audit.input_hash,
                output_hash: result.audit.output_hash,
                start_time_ms: result.audit.start_time_ms.min(i64::MAX as u64) as i64,
                duration_ms: result.audit.duration_ms.min(i64::MAX as u64) as i64,
                permissions_used: result.audit.permissions_used,
                exit_status,
                stdout: result.audit.stdout,
                stderr: result.audit.stderr,
            },
        })
    }

    /// Start an instruction-based skill session (for context: fork behavior).
    #[napi]
    pub fn start_skill_session(
        &self,
        skill_id: String,
        input_json: Option<String>,
        parent_context: Option<&ExecutionContextWrapper>,
    ) -> Result<SkillExecutionSessionWrapper> {
        let mut runtime = self.inner.lock().unwrap();
        let input = input_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());
        let parent = parent_context
            .map(|ctx| ctx.inner.lock().unwrap().clone());
        let parent_ref = parent.as_ref();

        let session = runtime
            .start_skill_session(&skill_id, input, parent_ref)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(SkillExecutionSessionWrapper {
            inner: Mutex::new(session),
        })
    }

    /// Finish a skill session and return an ExecutionResult.
    #[napi]
    pub fn finish_skill_session(
        &self,
        session: &SkillExecutionSessionWrapper,
        output_json: String,
        stdout: String,
        stderr: String,
        exit_status: Option<String>,
    ) -> Result<ExecutionResult> {
        let mut runtime = self.inner.lock().unwrap();
        let output: serde_json::Value = serde_json::from_str(&output_json)
            .unwrap_or_else(|_| serde_json::json!({ "output": output_json }));
        let status = parse_execution_status(exit_status);

        let session = session.inner.lock().unwrap();
        let result = runtime
            .finish_skill_session(
                session.clone(),
                output,
                stdout,
                stderr,
                status,
            )
            .map_err(|e| Error::from_reason(e.to_string()))?;

        let output_json = serde_json::to_string(&result.output)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        let exit_status = match result.audit.exit_status {
            RuntimeExecutionStatus::Success => "success".to_string(),
            RuntimeExecutionStatus::Timeout => "timeout".to_string(),
            RuntimeExecutionStatus::PermissionDenied => "permission_denied".to_string(),
            RuntimeExecutionStatus::Failed(msg) => format!("failed:{}", msg),
        };

        Ok(ExecutionResult {
            output_json,
            stdout: result.stdout,
            stderr: result.stderr,
            audit: AuditRecord {
                skill_id: result.audit.skill_id,
                version: result.audit.version,
                input_hash: result.audit.input_hash,
                output_hash: result.audit.output_hash,
                start_time_ms: result.audit.start_time_ms.min(i64::MAX as u64) as i64,
                duration_ms: result.audit.duration_ms.min(i64::MAX as u64) as i64,
                permissions_used: result.audit.permissions_used,
                exit_status,
                stdout: result.audit.stdout,
                stderr: result.audit.stderr,
            },
        })
    }

    /// Check if a tool is allowed for a skill
    #[napi]
    pub fn is_tool_allowed(&self, skill_id: String, tool: String) -> Result<bool> {
        let runtime = self.inner.lock().unwrap();
        runtime
            .is_tool_allowed(&skill_id, &tool)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Check if a tool call is permitted for a skill (ask-before-act for risky tools).
    #[napi]
    pub fn check_tool_permission(
        &self,
        skill_id: String,
        tool: String,
        description: Option<String>,
    ) -> Result<bool> {
        let runtime = self.inner.lock().unwrap();
        runtime
            .check_tool_permission(&skill_id, &tool, description, std::collections::HashMap::new())
            .map_err(|e| Error::from_reason(e.to_string()))
    }
}

#[napi]
impl SkillExecutionSessionWrapper {
    #[napi]
    pub fn is_forked(&self) -> bool {
        self.inner.lock().unwrap().is_forked()
    }

    #[napi]
    pub fn context_id(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .context_id()
            .map(|id| id.to_string())
    }

    #[napi]
    pub fn record_tool_call(&self, tool: String, output_json: String) -> Result<()> {
        let output: serde_json::Value = serde_json::from_str(&output_json)
            .unwrap_or_else(|_| serde_json::json!({ "output": output_json }));
        self.inner.lock().unwrap().record_tool_call(&tool, &output);
        Ok(())
    }

    #[napi]
    pub fn record_result(&self, output_json: String) -> Result<()> {
        let output: serde_json::Value = serde_json::from_str(&output_json)
            .unwrap_or_else(|_| serde_json::json!({ "output": output_json }));
        self.inner.lock().unwrap().record_result(&output);
        Ok(())
    }

    #[napi]
    pub fn record_stdout(&self, stdout: String) {
        self.inner.lock().unwrap().record_stdout_if_present(&stdout);
    }

    #[napi]
    pub fn record_stderr(&self, stderr: String) {
        self.inner.lock().unwrap().record_stderr_if_present(&stderr);
    }

    #[napi]
    pub fn summarize(&self) -> String {
        self.inner.lock().unwrap().summarize_fork()
    }
}

fn parse_output_type(value: &str) -> Result<OutputType> {
    match value.to_ascii_lowercase().as_str() {
        "stdout" => Ok(OutputType::Stdout),
        "stderr" => Ok(OutputType::Stderr),
        "toolcall" | "tool_call" | "tool" => Ok(OutputType::ToolCall),
        "result" => Ok(OutputType::Result),
        _ => Err(Error::from_reason(format!(
            "Invalid output_type: {}",
            value
        ))),
    }
}

fn parse_execution_status(status: Option<String>) -> openskills_runtime::RuntimeExecutionStatus {
    match status.as_deref() {
        Some("timeout") => openskills_runtime::RuntimeExecutionStatus::Timeout,
        Some("permission_denied") => openskills_runtime::RuntimeExecutionStatus::PermissionDenied,
        Some(s) if s.starts_with("failed:") => {
            openskills_runtime::RuntimeExecutionStatus::Failed(
                s.trim_start_matches("failed:").to_string(),
            )
        }
        _ => openskills_runtime::RuntimeExecutionStatus::Success,
    }
}
