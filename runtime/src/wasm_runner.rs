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
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder};

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
    // Note: Component model enabled for future component support
    // For now, we use regular modules with preview1 WASI
    config.wasm_component_model(true);

    let engine = Engine::new(&config)
        .map_err(|e| OpenSkillError::WasmError(format!("Engine init failed: {e}")))?;

    // Build WASI context with capability-based permissions
    let stdout_buf = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf = Arc::new(Mutex::new(Vec::new()));

    let mut wasi_builder = WasiCtxBuilder::new();

    // Inject skill metadata as environment variables
    wasi_builder.env("SKILL_ID", &skill.id);
    wasi_builder.env("SKILL_NAME", &skill.manifest.name);
    wasi_builder.env("SKILL_INPUT", &input_json);
    wasi_builder.env("TIMEOUT_MS", &timeout_ms.to_string());

    // Inject random seed if configured
    if let Some(seed) = enforcer.random_seed() {
        wasi_builder.env("RANDOM_SEED", &seed.to_string());
    }

    // Inject allowed environment variables from host
    for key in enforcer.env_allowlist() {
        if let Ok(val) = std::env::var(key) {
            wasi_builder.env(key, &val);
        }
    }

    // Preopen filesystem paths with appropriate permissions
    let read_paths = enforcer.filesystem_read_paths();
    let write_paths = enforcer.filesystem_write_paths();

    for dir in &read_paths {
        if dir.exists() && dir.is_dir() {
            let guest_path = format!(
                "/{}",
                dir.file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("data")
            );
            let _ = wasi_builder.preopened_dir(
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
                let _ = wasi_builder.preopened_dir(
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
        let _ = wasi_builder.preopened_dir(
            &skill.root,
            "/skill",
            DirPerms::READ,
            FilePerms::READ,
        );
    }

    let wasi_ctx = wasi_builder.build();

    // Load the WASM module
    let module = Module::from_file(&engine, &wasm_full_path)
        .map_err(|e| OpenSkillError::WasmError(format!("Module load failed: {e}")))?;

    // Create linker for WASM module
    // 
    // WASI LINKER INTEGRATION - What needs to be done:
    //
    // For wasmtime-wasi 20.0.2, the API structure has changed:
    //
    // Option 1: Use Component Model (for WASI Preview 2 / components)
    //   - Use `wasmtime::component::Linker`
    //   - Use `wasmtime_wasi::add_to_linker_sync` with component linker
    //   - Requires component-model enabled (already done)
    //
    // Option 2: Use Preview1 API (for regular WASI Preview 1 modules)
    //   - For regular modules, wasmtime-wasi 20.0.2 may require:
    //     a) Using `WasiView` trait implementation
    //     b) Or using a different API structure
    //   - The `add_to_linker_sync` function signature may have changed
    //
    // Option 3: Manual WASI function registration
    //   - Manually register WASI functions (fd_write, fd_read, etc.)
    //   - More control but more code
    //
    // CURRENT STATUS: Placeholder - linker created but WASI functions not added
    // This means WASI modules will fail to instantiate (missing imports)
    //
    // TO COMPLETE:
    // 1. Research wasmtime-wasi 20.0.2 API documentation
    // 2. Determine correct API for preview1 modules
    // 3. Implement either:
    //    - Component model linker with add_to_linker_sync
    //    - Preview1 API with correct WasiView implementation
    //    - Manual function registration
    // 4. Test with a simple WASI module to verify
    //
    let mut linker: Linker<WasiCtx> = Linker::new(&engine);
    
    // TODO: Add WASI preview1 functions to linker
    // This is the critical missing piece for 100% completion
    // 
    // Expected behavior after completion:
    // - WASI modules can be instantiated
    // - WASI imports (fd_write, fd_read, etc.) are resolved
    // - Modules can access filesystem, environment, etc. based on permissions
    //
    // Research needed:
    // - Check wasmtime-wasi 20.0.2 docs for preview1 API
    // - Verify if WasiCtx implements WasiView or needs wrapper
    // - Test with simple WASI module

    // Create store with WASI context
    let mut store = Store::new(&engine, wasi_ctx);

    // Set epoch deadline for timeout
    store.set_epoch_deadline(1);

    // Spawn a thread to increment epoch after timeout
    let engine_clone = engine.clone();
    let timeout_handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(timeout_ms));
        engine_clone.increment_epoch();
    });

    // Instantiate and run
    // Note: Without proper WASI linker setup, this will fail for WASI modules
    // with error: "unknown import" or "missing import"
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| {
            OpenSkillError::WasmError(format!(
                "Instantiation failed: {}. \
                Note: WASI linker integration is incomplete. \
                WASI modules require proper linker setup to resolve WASI imports (fd_write, fd_read, etc.). \
                See wasm_runner.rs for implementation details.",
                e
            ))
        })?;

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
    // Note: Currently using placeholder buffers
    // Proper implementation would capture from WASI stdout/stderr
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
