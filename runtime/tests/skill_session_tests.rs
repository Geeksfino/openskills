use openskills_runtime::{ExecutionContext, OpenSkillRuntime, RuntimeExecutionStatus};
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
fn test_skill_session_forked_context_summary() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    runtime.discover_skills().unwrap();

    let parent = ExecutionContext::new();
    let mut session = runtime
        .start_skill_session(
            "code-review",
            Some(json!({ "query": "Review this file" })),
            Some(&parent),
        )
        .expect("start skill session");

    assert!(session.is_forked(), "code-review should be forked");
    assert!(session.context_id().is_some(), "forked session should have context");

    session.record_tool_call("Read", &json!({ "path": "src/lib.rs" }));

    let final_output = json!({ "review": "Looks good." });
    let result = runtime
        .finish_skill_session(
            session,
            final_output,
            String::new(),
            String::new(),
            RuntimeExecutionStatus::Success,
        )
        .expect("finish skill session");

    assert_eq!(result.output["is_forked"], json!(true));
    assert!(
        result.output["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("Looks good"),
        "summary should include result content"
    );
    assert!(
        result.audit.permissions_used.iter().any(|p| p == "Read"),
        "tool call should be recorded in permissions_used"
    );
}

#[test]
fn test_skill_session_non_forked_returns_full_output() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    runtime.discover_skills().unwrap();

    let session = runtime
        .start_skill_session("explaining-code", None, None)
        .expect("start skill session");

    assert!(!session.is_forked(), "explaining-code should not be forked");
    assert!(session.context_id().is_none(), "non-forked session has no context");

    let final_output = json!({ "review": "OK" });
    let result = runtime
        .finish_skill_session(
            session,
            final_output.clone(),
            "stdout".to_string(),
            "stderr".to_string(),
            RuntimeExecutionStatus::Success,
        )
        .expect("finish skill session");

    assert_eq!(result.output, final_output);
    assert_eq!(result.stdout, "stdout");
    assert_eq!(result.stderr, "stderr");
}

#[test]
fn test_skill_session_multiple_tool_calls() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    runtime.discover_skills().unwrap();

    let parent = ExecutionContext::new();
    let mut session = runtime
        .start_skill_session(
            "code-review",
            Some(json!({ "file": "src/main.rs" })),
            Some(&parent),
        )
        .expect("start skill session");

    // Simulate multiple tool calls
    session.record_tool_call("Read", &json!({ "path": "src/main.rs" }));
    session.record_tool_call("Grep", &json!({ "pattern": "TODO" }));
    session.record_tool_call("Read", &json!({ "path": "src/lib.rs" }));

    let result = runtime
        .finish_skill_session(
            session,
            json!({ "issues": ["Missing error handling", "TODO items found"] }),
            String::new(),
            String::new(),
            RuntimeExecutionStatus::Success,
        )
        .expect("finish skill session");

    // Should record all tools as used (Read appears twice but should be deduplicated)
    assert!(result.audit.permissions_used.len() >= 2);
    assert!(result.audit.permissions_used.contains(&"Read".to_string()));
    assert!(result.audit.permissions_used.contains(&"Grep".to_string()));
}

#[test]
fn test_skill_session_summary_excludes_intermediate_outputs() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    runtime.discover_skills().unwrap();

    let mut session = runtime
        .start_skill_session("code-review", None, None)
        .expect("start skill session");

    // Record verbose intermediate outputs
    session.record_stdout_if_present("Loading file...");
    session.record_stdout_if_present("Analyzing patterns...");
    session.record_stderr_if_present("Warning: deprecated API detected");
    session.record_tool_call("Read", &json!({ "content": "very long file content..." }));

    // Record final result
    session.record_result(&json!({
        "verdict": "Approved",
        "summary": "Code quality is good"
    }));

    let result = runtime
        .finish_skill_session(
            session,
            json!({ "approved": true }),
            String::new(),
            String::new(),
            RuntimeExecutionStatus::Success,
        )
        .expect("finish skill session");

    let summary = result.output["summary"].as_str().unwrap();

    // Summary should focus on results, not intermediate steps
    assert!(summary.contains("good") || summary.contains("Approved"));
    assert!(!summary.contains("Loading file"));
    assert!(!summary.contains("very long file content"));
}

#[test]
fn test_check_tool_permission_for_forked_skill() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    runtime.discover_skills().unwrap();

    // code-review allows: Read, Grep, Glob, LS
    assert!(runtime
        .check_tool_permission(
            "code-review",
            "Read",
            None,
            std::collections::HashMap::new()
        )
        .unwrap());
    assert!(runtime
        .check_tool_permission(
            "code-review",
            "Grep",
            None,
            std::collections::HashMap::new()
        )
        .unwrap());

    // Write is not allowed
    let result = runtime.check_tool_permission(
        "code-review",
        "Write",
        None,
        std::collections::HashMap::new(),
    );
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("not allowed"));
}

#[test]
fn test_skill_session_with_parent_context() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    runtime.discover_skills().unwrap();

    // Create a parent context with some history
    let mut parent = ExecutionContext::new();
    parent.record_output(
        openskills_runtime::OutputType::Stdout,
        "Parent context output".to_string(),
    );

    let parent_id = parent.id().to_string();

    // Start forked session from parent
    let session = runtime
        .start_skill_session("code-review", None, Some(&parent))
        .expect("start skill session");

    assert!(session.is_forked());

    // The forked context should have the parent ID
    let context_parent_id = session
        .context()
        .and_then(|ctx| ctx.parent_id())
        .map(|id| id.to_string());

    assert_eq!(context_parent_id, Some(parent_id));
}

#[test]
fn test_fork_test_skill() {
    let examples_dir = get_examples_dir();
    let mut runtime = OpenSkillRuntime::from_directory(&examples_dir);
    runtime.discover_skills().unwrap();

    // Verify fork-test skill was discovered
    let skills = runtime.list_skills();
    let fork_test_skill = skills.iter().find(|s| s.id == "fork-test");
    assert!(
        fork_test_skill.is_some(),
        "fork-test skill should be discovered"
    );

    // Start fork-test session
    let mut session = runtime
        .start_skill_session(
            "fork-test",
            Some(json!({ "test": "data" })),
            None,
        )
        .expect("start fork-test skill session");

    assert!(session.is_forked(), "fork-test should be forked");

    // Record some tool calls
    session.record_tool_call("Read", &json!({ "path": "test.txt" }));

    // Finish with result
    let result = runtime
        .finish_skill_session(
            session,
            json!({ "test": "data" }),
            String::new(),
            String::new(),
            RuntimeExecutionStatus::Success,
        )
        .expect("finish fork-test session");

    assert_eq!(result.output["is_forked"], json!(true));
    assert!(result.output["summary"].as_str().is_some());
    assert!(result.audit.permissions_used.contains(&"Read".to_string()));
}
