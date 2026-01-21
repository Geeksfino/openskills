use openskills_runtime::{
    CommandPermissions, ExecutionContext, ExecutionOptions, OpenSkillRuntime, OutputType,
    RuntimeConfig, RuntimeExecutionStatus, SkillExecutionSession, SkillLocation,
    run_sandboxed_command,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;

#[pyclass]
struct OpenSkillRuntimeWrapper {
    inner: Mutex<OpenSkillRuntime>,
}

#[pyclass]
struct SkillExecutionSessionWrapper {
    inner: Mutex<SkillExecutionSession>,
}

#[pyclass]
struct ExecutionContextWrapper {
    inner: Mutex<ExecutionContext>,
}

#[pymethods]
impl OpenSkillRuntimeWrapper {
    #[new]
    fn new() -> Self {
        Self {
            inner: Mutex::new(OpenSkillRuntime::new()),
        }
    }

    #[staticmethod]
    fn with_project_root(project_root: String) -> Self {
        Self {
            inner: Mutex::new(OpenSkillRuntime::with_project_root(project_root)),
        }
    }

    #[staticmethod]
    fn from_directory(skills_dir: String) -> Self {
        Self {
            inner: Mutex::new(OpenSkillRuntime::from_directory(skills_dir)),
        }
    }

    /// Create runtime with custom directories and configuration
    #[staticmethod]
    #[pyo3(signature = (custom_directories, *, use_standard_locations = true, project_root = None))]
    fn with_custom_directories(
        custom_directories: Vec<String>,
        use_standard_locations: bool,
        project_root: Option<String>,
    ) -> Self {
        let config = RuntimeConfig {
            custom_directories: custom_directories
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            use_standard_locations,
            project_root: project_root.map(PathBuf::from),
        };
        Self {
            inner: Mutex::new(OpenSkillRuntime::from_config(config)),
        }
    }

    /// Discover skills from standard locations (~/.claude/skills/, .claude/skills/, nested)
    fn discover_skills(&self, py: Python) -> PyResult<Py<PyAny>> {
        let mut runtime = self.inner.lock().unwrap();
        let skills = runtime
            .discover_skills()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let list = PyList::empty(py);
        for s in skills {
            let item = PyDict::new(py);
            item.set_item("id", s.id)?;
            item.set_item("description", s.description)?;
            item.set_item(
                "location",
                match s.location {
                    SkillLocation::Personal => "personal",
                    SkillLocation::Project => "project",
                    SkillLocation::Nested => "nested",
                    SkillLocation::Custom => "custom",
                },
            )?;
            item.set_item("user_invocable", s.user_invocable)?;
            list.append(item.as_any())?;
        }

        Ok(list.into())
    }

    /// Load skills from a specific directory (additive - can be called multiple times)
    fn load_from_directory(&self, py: Python, dir: String) -> PyResult<Py<PyAny>> {
        let mut runtime = self.inner.lock().unwrap();
        let skills = runtime
            .load_from_directory(dir)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let list = PyList::empty(py);
        for s in skills {
            let item = PyDict::new(py);
            item.set_item("id", s.id)?;
            item.set_item("description", s.description)?;
            item.set_item(
                "location",
                match s.location {
                    SkillLocation::Personal => "personal",
                    SkillLocation::Project => "project",
                    SkillLocation::Nested => "nested",
                    SkillLocation::Custom => "custom",
                },
            )?;
            item.set_item("user_invocable", s.user_invocable)?;
            list.append(item.as_any())?;
        }

        Ok(list.into())
    }

    /// List skills (progressive disclosure - descriptors only)
    fn list_skills(&self, py: Python) -> PyResult<Py<PyAny>> {
        let runtime = self.inner.lock().unwrap();
        let skills = runtime.list_skills();

        let list = PyList::empty(py);
        for s in skills {
            let item = PyDict::new(py);
            item.set_item("id", s.id)?;
            item.set_item("description", s.description)?;
            item.set_item(
                "location",
                match s.location {
                    SkillLocation::Personal => "personal",
                    SkillLocation::Project => "project",
                    SkillLocation::Nested => "nested",
                    SkillLocation::Custom => "custom",
                },
            )?;
            item.set_item("user_invocable", s.user_invocable)?;
            list.append(item.as_any())?;
        }

        Ok(list.into())
    }

    /// Activate a skill (load full SKILL.md content)
    fn activate_skill(&self, py: Python, skill_id: String) -> PyResult<Py<PyAny>> {
        let runtime = self.inner.lock().unwrap();
        let loaded = runtime
            .activate_skill(&skill_id)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let skill = PyDict::new(py);
        skill.set_item("id", loaded.id.clone())?;
        skill.set_item("name", loaded.manifest.name.clone())?;
        skill.set_item("description", loaded.manifest.description.clone())?;
        skill.set_item("allowed_tools", loaded.manifest.get_allowed_tools())?;
        skill.set_item("model", loaded.manifest.model.clone())?;
        skill.set_item("context", loaded.manifest.context.clone())?;
        skill.set_item("agent", loaded.manifest.agent.clone())?;
        skill.set_item("user_invocable", loaded.manifest.is_user_invocable())?;
        skill.set_item(
            "location",
            match loaded.location {
                SkillLocation::Personal => "personal",
                SkillLocation::Project => "project",
                SkillLocation::Nested => "nested",
                SkillLocation::Custom => "custom",
            },
        )?;
        skill.set_item("instructions", loaded.instructions)?;

        Ok(skill.into())
    }

    /// Execute a skill's WASM module
    #[pyo3(signature = (skill_id, input=None, timeout_ms=None, memory_mb=None))]
    fn execute_skill(
        &self,
        py: Python<'_>,
        skill_id: String,
        input: Option<Bound<'_, PyAny>>,
        timeout_ms: Option<u64>,
        memory_mb: Option<u64>,
    ) -> PyResult<Py<PyAny>> {
        let mut runtime = self.inner.lock().unwrap();

        // Convert Python object to JSON if provided
        let input_val: Option<Value> = if let Some(input_obj) = input {
            let json_module = py.import("json")?;
            let json_dumps = json_module.getattr("dumps")?;
            let json_str: String = json_dumps.call1((input_obj,))?.extract()?;
            Some(
                serde_json::from_str(&json_str).map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid JSON: {e}"))
                })?,
            )
        } else {
            None
        };

        let options = ExecutionOptions {
            timeout_ms,
            memory_mb,
            input: input_val,
        };

        let result = runtime
            .execute_skill(&skill_id, options)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        // Convert Value to JSON string, then parse to Python object
        let json_str = serde_json::to_string(&result.output)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Serialization error: {e}"
            )))?;
            let json_module = py.import("json")?;
            let json_loads = json_module.getattr("loads")?;
            let output: Py<PyAny> = json_loads.call1((json_str,))?.into();

        let exit_status = match result.audit.exit_status {
            RuntimeExecutionStatus::Success => "success".to_string(),
            RuntimeExecutionStatus::Timeout => "timeout".to_string(),
            RuntimeExecutionStatus::PermissionDenied => "permission_denied".to_string(),
            RuntimeExecutionStatus::Failed(msg) => format!("failed:{}", msg),
        };

        let audit = PyDict::new(py);
        audit.set_item("skill_id", result.audit.skill_id)?;
        audit.set_item("version", result.audit.version)?;
        audit.set_item("input_hash", result.audit.input_hash)?;
        audit.set_item("output_hash", result.audit.output_hash)?;
        audit.set_item("start_time_ms", result.audit.start_time_ms)?;
        audit.set_item("duration_ms", result.audit.duration_ms)?;
        audit.set_item("permissions_used", result.audit.permissions_used)?;
        audit.set_item("exit_status", exit_status)?;
        audit.set_item("stdout", result.audit.stdout)?;
        audit.set_item("stderr", result.audit.stderr)?;

        let response = PyDict::new(py);
        response.set_item("output", output)?;
        response.set_item("stdout", result.stdout)?;
        response.set_item("stderr", result.stderr)?;
        response.set_item("audit", audit)?;

        Ok(response.into())
    }

    /// Check if a tool is allowed for a skill
    fn is_tool_allowed(&self, skill_id: String, tool: String) -> PyResult<bool> {
        let runtime = self.inner.lock().unwrap();
        runtime
            .is_tool_allowed(&skill_id, &tool)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Start an instruction-based skill session (for context: fork behavior).
    #[pyo3(signature = (skill_id, input=None, parent_context=None))]
    fn start_skill_session(
        &self,
        py: Python<'_>,
        skill_id: String,
        input: Option<Bound<'_, PyAny>>,
        parent_context: Option<&ExecutionContextWrapper>,
    ) -> PyResult<SkillExecutionSessionWrapper> {
        let mut runtime = self.inner.lock().unwrap();

        let input_val: Option<Value> = if let Some(input_obj) = input {
            let json_module = py.import("json")?;
            let json_dumps = json_module.getattr("dumps")?;
            let json_str: String = json_dumps.call1((input_obj,))?.extract()?;
            Some(
                serde_json::from_str(&json_str).map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid JSON: {e}"))
                })?,
            )
        } else {
            None
        };

        let parent = parent_context
            .map(|ctx| ctx.inner.lock().unwrap().clone());
        let parent_ref = parent.as_ref();

        let session = runtime
            .start_skill_session(&skill_id, input_val, parent_ref)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(SkillExecutionSessionWrapper {
            inner: Mutex::new(session),
        })
    }

    /// Finish a skill session and return an execution result.
    #[pyo3(signature = (session, output, *, stdout = "", stderr = "", exit_status = None))]
    fn finish_skill_session(
        &self,
        py: Python<'_>,
        session: &SkillExecutionSessionWrapper,
        output: Bound<'_, PyAny>,
        stdout: &str,
        stderr: &str,
        exit_status: Option<String>,
    ) -> PyResult<Py<PyAny>> {
        let mut runtime = self.inner.lock().unwrap();
        let json_module = py.import("json")?;
        let json_dumps = json_module.getattr("dumps")?;
        let json_str: String = json_dumps.call1((output,))?.extract()?;
        let output_val: Value = serde_json::from_str(&json_str).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid JSON: {e}"))
        })?;

        let status = parse_execution_status(exit_status);
        let session = session.inner.lock().unwrap();

        let result = runtime
            .finish_skill_session(
                session.clone(),
                output_val,
                stdout.to_string(),
                stderr.to_string(),
                status,
            )
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        // Convert Value to JSON string, then parse to Python object
        let result_json = serde_json::to_string(&result.output)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "Serialization error: {e}"
            )))?;
        let json_loads = json_module.getattr("loads")?;
        let output_obj: Py<PyAny> = json_loads.call1((result_json,))?.into();

        let exit_status = match result.audit.exit_status {
            RuntimeExecutionStatus::Success => "success".to_string(),
            RuntimeExecutionStatus::Timeout => "timeout".to_string(),
            RuntimeExecutionStatus::PermissionDenied => "permission_denied".to_string(),
            RuntimeExecutionStatus::Failed(msg) => format!("failed:{}", msg),
        };

        let audit = PyDict::new(py);
        audit.set_item("skill_id", result.audit.skill_id)?;
        audit.set_item("version", result.audit.version)?;
        audit.set_item("input_hash", result.audit.input_hash)?;
        audit.set_item("output_hash", result.audit.output_hash)?;
        audit.set_item("start_time_ms", result.audit.start_time_ms)?;
        audit.set_item("duration_ms", result.audit.duration_ms)?;
        audit.set_item("permissions_used", result.audit.permissions_used)?;
        audit.set_item("exit_status", exit_status)?;
        audit.set_item("stdout", result.audit.stdout)?;
        audit.set_item("stderr", result.audit.stderr)?;

        let response = PyDict::new(py);
        response.set_item("output", output_obj)?;
        response.set_item("stdout", result.stdout)?;
        response.set_item("stderr", result.stderr)?;
        response.set_item("audit", audit)?;

        Ok(response.into())
    }

    /// Check if a tool call is permitted for a skill (ask-before-act for risky tools).
    #[pyo3(signature = (skill_id, tool, description=None))]
    fn check_tool_permission(
        &self,
        skill_id: String,
        tool: String,
        description: Option<String>,
    ) -> PyResult<bool> {
        let runtime = self.inner.lock().unwrap();
        runtime
            .check_tool_permission(&skill_id, &tool, description, std::collections::HashMap::new())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }
}

#[pymethods]
impl SkillExecutionSessionWrapper {
    fn is_forked(&self) -> PyResult<bool> {
        Ok(self.inner.lock().unwrap().is_forked())
    }

    fn context_id(&self) -> PyResult<Option<String>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .context_id()
            .map(|id| id.to_string()))
    }

    fn record_tool_call(&self, py: Python<'_>, tool: String, output: Bound<'_, PyAny>) -> PyResult<()> {
        let json_module = py.import("json")?;
        let json_dumps = json_module.getattr("dumps")?;
        let json_str: String = json_dumps.call1((output,))?.extract()?;
        let output_val: Value = serde_json::from_str(&json_str).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid JSON: {e}"))
        })?;
        self.inner.lock().unwrap().record_tool_call(&tool, &output_val);
        Ok(())
    }

    fn record_result(&self, py: Python<'_>, output: Bound<'_, PyAny>) -> PyResult<()> {
        let json_module = py.import("json")?;
        let json_dumps = json_module.getattr("dumps")?;
        let json_str: String = json_dumps.call1((output,))?.extract()?;
        let output_val: Value = serde_json::from_str(&json_str).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid JSON: {e}"))
        })?;
        self.inner.lock().unwrap().record_result(&output_val);
        Ok(())
    }

    fn record_stdout(&self, stdout: String) {
        self.inner.lock().unwrap().record_stdout_if_present(&stdout);
    }

    fn record_stderr(&self, stderr: String) {
        self.inner.lock().unwrap().record_stderr_if_present(&stderr);
    }

    fn summarize(&self) -> PyResult<String> {
        Ok(self.inner.lock().unwrap().summarize_fork())
    }
}

#[pymethods]
impl ExecutionContextWrapper {
    #[new]
    fn new() -> Self {
        Self {
            inner: Mutex::new(ExecutionContext::new()),
        }
    }

    fn fork(&self) -> ExecutionContextWrapper {
        let forked = self.inner.lock().unwrap().fork();
        ExecutionContextWrapper {
            inner: Mutex::new(forked),
        }
    }

    fn id(&self) -> PyResult<String> {
        Ok(self.inner.lock().unwrap().id().to_string())
    }

    fn is_forked(&self) -> PyResult<bool> {
        Ok(self.inner.lock().unwrap().is_forked())
    }

    fn parent_id(&self) -> PyResult<Option<String>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .parent_id()
            .map(|id| id.to_string()))
    }

    fn summary(&self) -> PyResult<Option<String>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .summary()
            .map(|s| s.to_string()))
    }

    fn record_output(&self, output_type: String, content: String) -> PyResult<()> {
        let output_type = parse_output_type(&output_type)?;
        self.inner
            .lock()
            .unwrap()
            .record_output(output_type, content);
        Ok(())
    }

    fn summarize(&self) -> PyResult<String> {
        Ok(self.inner.lock().unwrap().summarize())
    }
}

/// Run a shell command in a sandboxed environment (macOS only).
///
/// This provides Claude Code-like sandboxed bash execution for agents.
/// Uses macOS Seatbelt sandbox-exec.
///
/// Args:
///     command: Shell command to execute
///     working_dir: Working directory for the command
///     allow_network: Allow network access (default: False)
///     allow_process: Allow subprocess spawning (default: False)
///     read_paths: List of paths the command can read from
///     write_paths: List of paths the command can write to
///     env_vars: Dict of environment variables to pass through
///     timeout_ms: Timeout in milliseconds (default: 30000)
///
/// Returns:
///     Dict with exit_code, stdout, stderr, timed_out
#[pyfunction]
#[pyo3(signature = (command, working_dir, *, allow_network = false, allow_process = false, read_paths = None, write_paths = None, env_vars = None, timeout_ms = 30000))]
fn run_sandboxed_shell_command(
    py: Python<'_>,
    command: String,
    working_dir: String,
    allow_network: bool,
    allow_process: bool,
    read_paths: Option<Vec<String>>,
    write_paths: Option<Vec<String>>,
    env_vars: Option<&Bound<'_, PyDict>>,
    timeout_ms: u64,
) -> PyResult<Py<PyAny>> {
    // Convert env_vars from Python dict to Vec<(String, String)>
    let env_vec: Vec<(String, String)> = if let Some(env_dict) = env_vars {
        env_dict
            .iter()
            .filter_map(|(k, v)| {
                let key: Option<String> = k.extract().ok();
                let value: Option<String> = v.extract().ok();
                key.zip(value)
            })
            .collect()
    } else {
        Vec::new()
    };

    let perms = CommandPermissions {
        allow_network,
        allow_process,
        read_paths: read_paths
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect(),
        write_paths: write_paths
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect(),
        env_vars: env_vec,
        timeout_ms,
    };

    let result = run_sandboxed_command(&command, &PathBuf::from(&working_dir), perms)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    let dict = PyDict::new(py);
    dict.set_item("exit_code", result.exit_code)?;
    dict.set_item("stdout", result.stdout)?;
    dict.set_item("stderr", result.stderr)?;
    dict.set_item("timed_out", result.timed_out)?;

    Ok(dict.into())
}

#[pymodule]
fn openskills(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<OpenSkillRuntimeWrapper>()?;
    m.add_class::<SkillExecutionSessionWrapper>()?;
    m.add_class::<ExecutionContextWrapper>()?;
    m.add_function(wrap_pyfunction!(run_sandboxed_shell_command, m)?)?;
    Ok(())
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

fn parse_output_type(value: &str) -> PyResult<OutputType> {
    match value.to_ascii_lowercase().as_str() {
        "stdout" => Ok(OutputType::Stdout),
        "stderr" => Ok(OutputType::Stderr),
        "toolcall" | "tool_call" | "tool" => Ok(OutputType::ToolCall),
        "result" => Ok(OutputType::Result),
        _ => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Invalid output_type: {}",
            value
        ))),
    }
}
