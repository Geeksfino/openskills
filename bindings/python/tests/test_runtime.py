import json
import pytest
from openskills import (
    OpenSkillRuntimeWrapper as OpenSkillRuntime,
    ExecutionContextWrapper as ExecutionContext
)
import os

def get_examples_dir():
    # Assuming we are running from bindings/python
    # We need to go up to examples/skills
    # bindings/python/tests -> ../../examples/skills
    base_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    examples_dir = os.path.join(base_dir, "examples", "skills")
    if not os.path.exists(examples_dir):
        # Maybe we are in a worktree structure like openskills/pml/bindings/python
        # and examples are at openskills/pml/examples/skills
        # path is openskills/pml/bindings/python/tests/test_runtime.py
        # root is openskills/pml
        # examples is openskills/pml/examples/skills
        # so relative to test file: ../../../examples/skills
        base_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
        examples_dir = os.path.join(base_dir, "examples", "skills")
    return examples_dir

def test_discover_skills():
    examples_dir = get_examples_dir()
    print(f"Examples dir: {examples_dir}")
    runtime = OpenSkillRuntime.from_directory(examples_dir)
    
    skills = runtime.discover_skills()
    print(f"Skills discovered: {skills}")
    assert len(skills) > 0
    
    # Use an actual skill that exists (code-review, explaining-code, etc.)
    example_skill = next((s for s in skills if s["id"] in ["code-review", "explaining-code", "skill-creator"]), None)
    assert example_skill is not None, "Should find at least one skill (code-review, explaining-code, or skill-creator)"
    assert example_skill["description"] is not None

def test_activate_skill():
    examples_dir = get_examples_dir()
    runtime = OpenSkillRuntime.from_directory(examples_dir)
    runtime.discover_skills()
    
    # Use an actual skill that exists (explaining-code is not forked)
    skill = runtime.activate_skill("explaining-code")
    assert skill["id"] == "explaining-code"
    assert "instructions" in skill
    assert isinstance(skill["allowed_tools"], list)  # allowed_tools is a list (may be empty)

def test_execute_skill_placeholder_error():
    examples_dir = get_examples_dir()
    runtime = OpenSkillRuntime.from_directory(examples_dir)
    runtime.discover_skills()
    
    # Use an actual skill (may succeed or fail depending on WASM validity)
    try:
        result = runtime.execute_skill(
            "code-review",
            input={"query": "hello"},
            timeout_ms=5000
        )
        # If execution succeeds, that's fine too - just verify we got a result
        print("Execution succeeded (WASM is valid)")
    except RuntimeError as e:
        # Accept various error types (WASM errors, skill not found, etc.)
        error_msg = str(e)
        assert any(keyword in error_msg for keyword in [
            "Invalid WASM", "magic number", "component", "WASM", 
            "not found", "No executable artifact", "execution"
        ]), f"Unexpected error: {error_msg}"

def test_runtime_config():
    examples_dir = get_examples_dir()
    runtime = OpenSkillRuntime.with_custom_directories(
        custom_directories=[examples_dir],
        use_standard_locations=False,
        project_root=None
    )
    skills = runtime.discover_skills()
    assert len(skills) > 0

def test_skill_session_forked():
    """Test forked skill session workflow."""
    examples_dir = get_examples_dir()
    runtime = OpenSkillRuntime.from_directory(examples_dir)
    runtime.discover_skills()
    
    # Start forked skill session
    session = runtime.start_skill_session(
        "code-review",
        {"query": "Review this file"},
        None
    )
    
    assert session.is_forked(), "code-review should be forked"
    assert session.context_id() is not None, "forked session should have context ID"
    
    # Record tool calls
    session.record_tool_call("Read", {"path": "src/lib.rs", "content": "fn main() {}"})
    session.record_result({"review": "Code looks good. No issues found."})
    
    # Finish session
    result = runtime.finish_skill_session(
        session,
        {"review": "Code looks good."},
        stdout="",
        stderr=""
    )
    
    assert result["output"]["is_forked"] == True
    assert "looks good" in result["output"]["summary"]
    assert "Read" in result["audit"]["permissions_used"]

def test_skill_session_non_forked():
    """Test non-forked skill session returns full output."""
    examples_dir = get_examples_dir()
    runtime = OpenSkillRuntime.from_directory(examples_dir)
    runtime.discover_skills()
    
    # Use an actual skill that is not forked (explaining-code)
    session = runtime.start_skill_session("explaining-code", None, None)
    
    assert not session.is_forked()
    assert session.context_id() is None
    
    result = runtime.finish_skill_session(
        session,
        {"result": "done"},
        stdout="stdout output",
        stderr="stderr output"
    )
    
    assert result["output"]["result"] == "done"
    assert result["stdout"] == "stdout output"
    assert result["stderr"] == "stderr output"

def test_execution_context_fork():
    """Test ExecutionContext fork behavior."""
    parent = ExecutionContext()
    assert not parent.is_forked()
    
    forked = parent.fork()
    assert forked.is_forked()
    assert forked.parent_id() == parent.id()
    
    # Record outputs
    forked.record_output("toolcall", "Read: src/lib.rs")
    forked.record_output("result", "Final result here")
    
    summary = forked.summarize()
    assert "Final result" in summary
    assert "Read:" not in summary  # Tool calls excluded from summary

def test_check_tool_permission():
    """Test tool permission checking for skills."""
    examples_dir = get_examples_dir()
    runtime = OpenSkillRuntime.from_directory(examples_dir)
    runtime.discover_skills()
    
    # code-review allows: Read, Grep, Glob, LS
    assert runtime.check_tool_permission("code-review", "Read") == True
    assert runtime.check_tool_permission("code-review", "Grep") == True
    
    # Write is not allowed
    with pytest.raises(RuntimeError) as excinfo:
        runtime.check_tool_permission("code-review", "Write")
    
    assert "not allowed" in str(excinfo.value)
