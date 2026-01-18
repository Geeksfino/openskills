//! WASM sandbox execution using wasmtime.
//!
//! Provides capability-based sandboxing as an alternative to OS-level
//! sandboxing (seatbelt on macOS, seccomp on Linux).

use crate::audit::ExecutionStatus;
use crate::errors::OpenSkillError;
use crate::executor::ExecutionArtifacts;
use crate::permissions::PermissionEnforcer;
use crate::registry::Skill;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime::component::{Component, Linker as ComponentLinker, ResourceTable};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

/// Execute a WASM module with WASI sandbox.
pub fn execute_wasm(
    skill: &Skill,
    wasm_path: &str,
    input: Value,
    timeout_ms: u64,
    enforcer: &PermissionEnforcer,
) -> Result<ExecutionArtifacts, OpenSkillError> {
    let wasm_full_path = skill.root.join(wasm_path);
    let input_json = serde_json::to_string(&input)?;

    // Configure wasmtime with epoch interruption for timeout
    let mut config = Config::new();
    config.epoch_interruption(true);
    config.wasm_component_model(true);

    let engine = Engine::new(&config)
        .map_err(|e| OpenSkillError::WasmError(format!("Engine init failed: {e}")))?;

    // Build WASI context with capability-based permissions
    let stdout_buf = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf = Arc::new(Mutex::new(Vec::new()));

    // Preopen filesystem paths with appropriate permissions
    let read_paths = enforcer.filesystem_read_paths();
    let write_paths = enforcer.filesystem_write_paths();

    let configure_wasi_builder = |builder: &mut WasiCtxBuilder| {
        // Inject skill metadata as environment variables
        builder.env("SKILL_ID", &skill.id);
        builder.env("SKILL_NAME", &skill.manifest.name);
        builder.env("SKILL_INPUT", &input_json);
        builder.env("TIMEOUT_MS", &timeout_ms.to_string());

        // Inject random seed if configured
        if let Some(seed) = enforcer.random_seed() {
            builder.env("RANDOM_SEED", &seed.to_string());
        }

        // Inject allowed environment variables from host
        for key in enforcer.env_allowlist() {
            if let Ok(val) = std::env::var(key) {
                builder.env(key, &val);
            }
        }

        for dir in &read_paths {
            if dir.exists() && dir.is_dir() {
                let guest_path = format!(
                    "/{}",
                    dir.file_name()
                        .and_then(|v| v.to_str())
                        .unwrap_or("data")
                );
                let _ = builder.preopened_dir(
                    dir,
                    &guest_path,
                    DirPerms::READ,
                    FilePerms::READ,
                );
            }
        }

        for dir in &write_paths {
            if dir.exists() && dir.is_dir() {
                let guest_path = format!(
                    "/{}",
                    dir.file_name()
                        .and_then(|v| v.to_str())
                        .unwrap_or("data")
                );
                // Check if already preopened (from read paths)
                if !read_paths.contains(dir) {
                    let _ = builder.preopened_dir(
                        dir,
                        &guest_path,
                        DirPerms::all(),
                        FilePerms::all(),
                    );
                }
            }
        }

        // Always preopen the skill's root directory (read-only by default)
        if skill.root.exists() {
            let _ = builder.preopened_dir(
                &skill.root,
                "/skill",
                DirPerms::READ,
                FilePerms::READ,
            );
        }
    };

    // Try component model (WASI Preview 2/0.3.0 preview) first
    if let Ok(component) = Component::from_file(&engine, &wasm_full_path) {
        struct WasiComponentState {
            ctx: WasiCtx,
            table: ResourceTable,
        }

        impl WasiView for WasiComponentState {
            fn ctx(&mut self) -> WasiCtxView<'_> {
                WasiCtxView {
                    ctx: &mut self.ctx,
                    table: &mut self.table,
                }
            }
        }

        let mut component_builder = WasiCtxBuilder::new();
        configure_wasi_builder(&mut component_builder);
        let component_ctx = component_builder.build();

        let mut linker: ComponentLinker<WasiComponentState> = ComponentLinker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .map_err(|e| OpenSkillError::WasmError(format!("Failed to add WASI p2 functions: {e}")))?;

        let mut store = Store::new(
            &engine,
            WasiComponentState {
                ctx: component_ctx,
                table: ResourceTable::new(),
            },
        );
        store.set_epoch_deadline(1);

        let engine_clone = engine.clone();
        let timeout_handle = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(timeout_ms));
            engine_clone.increment_epoch();
        });

        let command = wasmtime_wasi::p2::bindings::sync::Command::instantiate(
            &mut store,
            &component,
            &linker,
        )
        .map_err(|e| OpenSkillError::WasmError(format!("Component instantiation failed: {e}")))?;

        let run_result = command
            .wasi_cli_run()
            .call_run(&mut store)
            .map_err(|e| OpenSkillError::WasmError(format!("Component run failed: {e}")))?;

        drop(timeout_handle);

        let stdout = String::from_utf8_lossy(
            &stdout_buf.lock().unwrap_or_else(|e| e.into_inner()),
        )
        .to_string();
        let stderr = String::from_utf8_lossy(
            &stderr_buf.lock().unwrap_or_else(|e| e.into_inner()),
        )
        .to_string();

        let (exit_status, output) = match run_result {
            Ok(()) => (
                ExecutionStatus::Success,
                serde_json::json!({ "status": "success", "output": stdout.trim() }),
            ),
            Err(()) => (
                ExecutionStatus::Failed("Component exited with error".to_string()),
                serde_json::json!({ "status": "error", "error": "Component exited with error" }),
            ),
        };

        return Ok(ExecutionArtifacts {
            output,
            stdout,
            stderr,
            permissions_used: enforcer.permissions_used(),
            exit_status,
        });
    }

    // Build WASI Preview 1 context
    // For wasmtime 40.0.2, we use build_p1() to get WasiP1Ctx which implements WasiSnapshotPreview1
    use wasmtime_wasi::p1::WasiP1Ctx;
    let mut wasi_builder = WasiCtxBuilder::new();
    configure_wasi_builder(&mut wasi_builder);
    let wasi_p1_ctx = wasi_builder.build_p1();

    // Load the WASM module
    let module = Module::from_file(&engine, &wasm_full_path)
        .map_err(|e| OpenSkillError::WasmError(format!("Module load failed: {e}")))?;

    // Create linker and register all WASI Preview 1 functions
    // For wasmtime-wasi 40.0.2, we use p1::wasi_snapshot_preview1::add_to_linker
    // which registers all ~50+ WASI Preview 1 functions
    use wasmtime_wasi::p1::wasi_snapshot_preview1;
    
    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
    
    // Register all WASI Preview 1 functions to the linker
    // This single call adds all functions from wasi_snapshot_preview1:
    // - File operations: fd_read, fd_write, fd_close, fd_seek, etc.
    // - Filesystem: path_open, path_readdir, path_unlink_file, etc.
    // - Process: proc_exit, proc_raise
    // - Environment: args_get, environ_get, etc.
    // - Time: clock_time_get, clock_res_get
    // - Random: random_get
    // - And more...
    // 
    // For wasmtime 40.0.2, WasiP1Ctx implements WasiSnapshotPreview1 trait
    wasi_snapshot_preview1::add_to_linker(&mut linker, |ctx: &mut WasiP1Ctx| ctx)
        .map_err(|e| OpenSkillError::WasmError(format!("Failed to add WASI Preview 1 functions to linker: {e}")))?;

    // Create store with WASI Preview 1 context
    let mut store = Store::new(&engine, wasi_p1_ctx);

    // Set epoch deadline for timeout
    store.set_epoch_deadline(1);

    // Spawn a thread to increment epoch after timeout
    let engine_clone = engine.clone();
    let timeout_handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(timeout_ms));
        engine_clone.increment_epoch();
    });

    // Instantiate and run
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| OpenSkillError::WasmError(format!("Instantiation failed: {e}")))?;

    // Try to call the main function or _start
    let result = if let Ok(func) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
        func.call(&mut store, ())
    } else if let Ok(func) = instance.get_typed_func::<(), ()>(&mut store, "main") {
        func.call(&mut store, ())
    } else {
        Err(wasmtime::Error::msg("No _start or main function found"))
    };

    // Clean up timeout thread
    drop(timeout_handle);

    // Collect stdout/stderr
    let stdout = String::from_utf8_lossy(
        &stdout_buf.lock().unwrap_or_else(|e| e.into_inner()),
    )
    .to_string();
    let stderr = String::from_utf8_lossy(
        &stderr_buf.lock().unwrap_or_else(|e| e.into_inner()),
    )
    .to_string();

    // Determine exit status and output
    let (exit_status, output) = match result {
        Ok(()) => {
            // Try to parse stdout as JSON, otherwise wrap it
            let output = if let Ok(json) = serde_json::from_str::<Value>(&stdout) {
                json
            } else {
                serde_json::json!({
                    "status": "success",
                    "output": stdout.trim()
                })
            };
            (ExecutionStatus::Success, output)
        }
        Err(e) => {
            let error_msg = e.to_string();
            let status = if error_msg.contains("epoch") {
                ExecutionStatus::Timeout
            } else {
                ExecutionStatus::Failed(error_msg.clone())
            };
            let output = serde_json::json!({
                "status": "error",
                "error": error_msg
            });
            (status, output)
        }
    };

    Ok(ExecutionArtifacts {
        output,
        stdout,
        stderr,
        permissions_used: enforcer.permissions_used(),
        exit_status,
    })
}

#[cfg(test)]
mod tests {
    // WASM execution tests would require actual WASM modules
    // These are integration tests better suited for a separate test harness
}
