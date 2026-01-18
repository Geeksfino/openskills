use openskills_runtime::{
    ExecutionOptions, OpenSkillRuntime, RuntimeConfig, RuntimeExecutionStatus,
    SkillLocation,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::Bound;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;

#[pyclass]
struct OpenSkillRuntimeWrapper {
    inner: Mutex<OpenSkillRuntime>,
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
                .map(|s| PathBuf::from(s))
                .collect(),
            use_standard_locations,
            project_root: project_root.map(|s| PathBuf::from(s)),
        };
        Self {
            inner: Mutex::new(OpenSkillRuntime::from_config(config)),
        }
    }

    /// Discover skills from standard locations (~/.claude/skills/, .claude/skills/, nested)
    fn discover_skills(&self, py: Python) -> PyResult<PyObject> {
        let mut runtime = self.inner.lock().unwrap();
        let skills = runtime
            .discover_skills()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let list = PyList::empty_bound(py);
        for s in skills {
            let item = PyDict::new_bound(py);
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
    fn load_from_directory(&self, py: Python, dir: String) -> PyResult<PyObject> {
        let mut runtime = self.inner.lock().unwrap();
        let skills = runtime
            .load_from_directory(dir)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let list = PyList::empty_bound(py);
        for s in skills {
            let item = PyDict::new_bound(py);
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
    fn list_skills(&self, py: Python) -> PyResult<PyObject> {
        let runtime = self.inner.lock().unwrap();
        let skills = runtime.list_skills();

        let list = PyList::empty_bound(py);
        for s in skills {
            let item = PyDict::new_bound(py);
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
    fn activate_skill(&self, py: Python, skill_id: String) -> PyResult<PyObject> {
        let runtime = self.inner.lock().unwrap();
        let loaded = runtime
            .activate_skill(&skill_id)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let skill = PyDict::new_bound(py);
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
    ) -> PyResult<PyObject> {
        let mut runtime = self.inner.lock().unwrap();

        // Convert Python object to JSON if provided
        let input_val: Option<Value> = if let Some(input_obj) = input {
            let json_module = py.import_bound("json")?;
            let json_dumps = json_module.getattr("dumps")?;
            let json_str: String = json_dumps.call1((input_obj.as_ref(),))?.extract()?;
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
            let json_module = py.import_bound("json")?;
            let json_loads = json_module.getattr("loads")?;
            let output: PyObject = json_loads.call1((json_str,))?.into();

        let exit_status = match result.audit.exit_status {
            RuntimeExecutionStatus::Success => "success".to_string(),
            RuntimeExecutionStatus::Timeout => "timeout".to_string(),
            RuntimeExecutionStatus::PermissionDenied => "permission_denied".to_string(),
            RuntimeExecutionStatus::Failed(msg) => format!("failed:{}", msg),
        };

        let audit = PyDict::new_bound(py);
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

        let response = PyDict::new_bound(py);
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
}

#[pymodule]
fn openskills(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<OpenSkillRuntimeWrapper>()?;
    Ok(())
}
