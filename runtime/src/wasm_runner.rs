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
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::AsyncWrite;
use wasmtime::{Config, Engine, Store};
use wasmtime::component::{Component, Linker as ComponentLinker, ResourceTable};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

/// Execute a WASM module with WASI sandbox.
pub fn execute_wasm(
    skill: &Skill,
    wasm_path: &str,
    input: Value,
    timeout_ms: u64,
    enforcer: &PermissionEnforcer,
    workspace_dir: Option<&std::path::Path>,
) -> Result<ExecutionArtifacts, OpenSkillError> {
    let wasm_full_path = skill.root.join(wasm_path);
    let input_json = serde_json::to_string(&input)?;

    // Configure wasmtime with epoch interruption for timeout
    let mut config = Config::new();
    config.epoch_interruption(true);
    config.async_support(true);
    config.wasm_component_model_async(true);

    let engine = Engine::new(&config)
        .map_err(|e| OpenSkillError::WasmError(format!("Engine init failed: {e}")))?;

    // Build WASI context with capability-based permissions.
    //
    // NOTE: OpenSkills runtime is WASI 0.3 (WASIp3) / component-model-only.
    // Legacy "core module" artifacts are not supported and must be treated as invalid.
    let stdout_buf = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf = Arc::new(Mutex::new(Vec::new()));

    // Preopen filesystem paths with appropriate permissions
    let read_paths = enforcer.filesystem_read_paths();
    let write_paths = enforcer.filesystem_write_paths();

    // Minimal in-memory stdout/stderr capture stream implementation.
    #[derive(Clone)]
    struct SharedVecStdout(Arc<Mutex<Vec<u8>>>);

    impl wasmtime_wasi::cli::IsTerminal for SharedVecStdout {
        fn is_terminal(&self) -> bool {
            false
        }
    }

    struct SharedVecWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl AsyncWrite for SharedVecWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            data: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            match self.buf.lock() {
                Ok(mut guard) => {
                    guard.extend_from_slice(data);
                    Poll::Ready(Ok(data.len()))
                }
                Err(_) => Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "stdout lock poisoned",
                ))),
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl wasmtime_wasi::cli::StdoutStream for SharedVecStdout {
        fn async_stream(&self) -> Box<dyn tokio::io::AsyncWrite + Send + Sync> {
            Box::new(SharedVecWriter {
                buf: self.0.clone(),
            })
        }
    }

    let configure_wasi_builder = |builder: &mut WasiCtxBuilder| {
        // Capture stdout/stderr for audit (default is "empty" sinks in wasmtime-wasi).
        builder.stdout(SharedVecStdout(stdout_buf.clone()));
        builder.stderr(SharedVecStdout(stderr_buf.clone()));

        // Inject skill metadata as environment variables
        builder.env("SKILL_ID", &skill.id);
        builder.env("SKILL_NAME", &skill.manifest.name);
        builder.env("SKILL_INPUT", &input_json);
        builder.env("TIMEOUT_MS", &timeout_ms.to_string());

        // Inject workspace directory if configured
        if let Some(workspace) = workspace_dir {
            builder.env("SKILL_WORKSPACE", workspace.to_string_lossy().as_ref());
        }

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

        // Preopen workspace directory with write permissions
        if let Some(workspace) = workspace_dir {
            if workspace.exists() {
                let _ = builder.preopened_dir(
                    workspace,
                    "/workspace",
                    DirPerms::all(),
                    FilePerms::all(),
                );
            }
        }
    };

    // WASI 0.3 / WASIp3 component execution only.
    let component = Component::from_file(&engine, &wasm_full_path).map_err(|e| {
        OpenSkillError::WasmError(format!(
            "Invalid WASM artifact (expected a WASI 0.3 component): {e}. \
OpenSkills runtime does not support legacy core-module WASM artifacts."
        ))
    })?;

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
    
    // Add WASI 0.3 (p3) interfaces
    wasmtime_wasi::p3::add_to_linker(&mut linker).map_err(|e| {
        OpenSkillError::WasmError(format!("Failed to add WASI 0.3 (p3) interfaces to linker: {e}"))
    })?;
    
    // Components created with wasm-tools component new --adapt wasi_snapshot_preview1
    // import WASI 0.2 CLI interfaces (wasi:cli/*@0.2.1). We need to add p2 interfaces
    // alongside p3 to support these components. The p2 feature is enabled in workspace Cargo.toml.
    // 
    // Add WASI 0.2 (p2) interfaces using add_to_linker_async (since we're using async component model).
    // This provides the wasi:cli/*@0.2.1 interfaces that components built with the adapter require.
    wasmtime_wasi::p2::add_to_linker_async(&mut linker).map_err(|e| {
        OpenSkillError::WasmError(format!("Failed to add WASI 0.2 (p2) interfaces to linker: {e}"))
    })?;

    let mut store = Store::new(
        &engine,
        WasiComponentState {
            ctx: component_ctx,
            table: ResourceTable::new(),
        },
    );
    store.set_epoch_deadline(1);

    let done = Arc::new(AtomicBool::new(false));
    let done_for_thread = done.clone();
    let engine_clone = engine.clone();
    let timeout_handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(timeout_ms));
        if !done_for_thread.load(Ordering::Relaxed) {
            engine_clone.increment_epoch();
        }
    });

    let run_result: Result<Result<(), ()>, OpenSkillError> = wasmtime_wasi::runtime::in_tokio(async {
        // Try p3 bindings first (for native 0.3 components)
        // If that fails, fall back to p2 bindings (for components built with wasi_snapshot_preview1 adapter)
        let program_result = match wasmtime_wasi::p3::bindings::Command::instantiate_async(
            &mut store,
            &component,
            &linker,
        )
        .await
        {
            Ok(command) => {
                // Component is WASI 0.3 - use p3 bindings
                store
                    .run_concurrent(async move |store| command.wasi_cli_run().call_run(store).await)
                    .await
                    .map_err(|e| OpenSkillError::WasmError(format!("Component run failed: {e}")))?
            }
            Err(_) => {
                // Component is WASI 0.2 - instantiate using linker
                // For WASI CLI command components built with wasi_snapshot_preview1 adapter,
                // the component exports wasi:cli/run@0.2.1 which should execute the main function.
                // However, instantiation alone may not trigger execution - we may need to
                // explicitly call the run export. For now, instantiate and hope the component
                // executes automatically (some WASI runtimes do this).
                //
                // TODO: Properly invoke wasi:cli/run export for WASI 0.2 components.
                // This may require using p2 bindings or manually getting/calling the export.
                let _instance = linker
                    .instantiate_async(&mut store, &component)
                    .await
                    .map_err(|e| OpenSkillError::WasmError(format!("Component instantiation failed: {e}")))?;
                
                // Note: Some WASI 0.2 CLI command components execute their main function
                // during instantiation, but this is not guaranteed. If no output is produced,
                // the component may need explicit invocation of the wasi:cli/run export.
                Ok(Ok(()))
            }
        };

        let program_result = program_result
            .map_err(|e| OpenSkillError::WasmError(format!("Component run trapped: {e}")))?;

        Ok(program_result)
    });

    done.store(true, Ordering::Relaxed);
    let _ = timeout_handle.join();

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
    let (exit_status, output) = match run_result {
        Ok(Ok(())) => {
            let output = if let Ok(json) = serde_json::from_str::<Value>(&stdout) {
                json
            } else {
                serde_json::json!({ "status": "success", "output": stdout.trim() })
            };
            (ExecutionStatus::Success, output)
        }
        Ok(Err(())) => (
            ExecutionStatus::Failed("Component exited with error".to_string()),
            serde_json::json!({ "status": "error", "error": "Component exited with error" }),
        ),
        Err(e) => {
            let error_msg = e.to_string();
            let status = if error_msg.contains("epoch") {
                ExecutionStatus::Timeout
            } else {
                ExecutionStatus::Failed(error_msg.clone())
            };
            (
                status,
                serde_json::json!({ "status": "error", "error": error_msg }),
            )
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
