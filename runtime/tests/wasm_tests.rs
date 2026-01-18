use openskills_runtime::{OpenSkillRuntime, ExecutionOptions, RuntimeError};
use serde_json::json;
use std::path::PathBuf;

fn get_examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join("skills")
}

#[test]
fn test_wasm_execution_basic() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    
    // Ensure we can find the skill
    let skills = runtime.discover_skills().unwrap();
    if !skills.iter().any(|s| s.id == "example-skill") {
        panic!("example-skill not found in examples/skills");
    }

    // Execute the skill
    let options = ExecutionOptions {
        timeout_ms: Some(5000),
        input: Some(json!({
            "query": "hello world"
        })),
        ..Default::default()
    };

    let result = runtime.execute_skill("example-skill", options);
    
    match result {
        Ok(_) => {
            panic!("Execution should fail because skill.wasm is a placeholder text file");
        },
        Err(e) => {
            // If WASM is missing or invalid, it might fail. 
            // We want to distinguish between "runtime broken" and "artifact missing"
            println!("Got expected execution error: {:?}", e);
            
            // The placeholder is ASCII text, so wasmtime will report invalid magic number or similar
            match e {
                RuntimeError::WasmError(msg) => {
                    println!("WASM Error message: {}", msg);
                    // Verify it's an error related to invalid WASM format
                    assert!(msg.contains("Invalid WASM") || msg.contains("magic number") || msg.contains("binary") || msg.contains("component"));
                },
                _ => panic!("Expected WasmError, got {:?}", e),
            }
        }
    }
}
