const { 
  OpenSkillRuntimeWrapper: OpenSkillRuntime,
  ExecutionContextWrapper: ExecutionContext 
} = require("../index");
const path = require("path");
const assert = require("assert");

function getExamplesDir() {
  return path.resolve(__dirname, "../../../examples/skills");
}

async function testDiscoverSkills() {
  console.log("Running testDiscoverSkills...");
  const examplesDir = getExamplesDir();
  const runtime = OpenSkillRuntime.fromDirectory(examplesDir);
  
  const skills = runtime.discoverSkills();
  assert(skills.length > 0, "Should find at least one skill");
  
  const exampleSkill = skills.find(s => s.id === "example-skill");
  assert(exampleSkill, "Should find example-skill");
  assert.strictEqual(exampleSkill.id, "example-skill");
  assert(exampleSkill.description, "Description should exist");
  console.log("testDiscoverSkills passed");
}

async function testActivateSkill() {
  console.log("Running testActivateSkill...");
  const examplesDir = getExamplesDir();
  const runtime = OpenSkillRuntime.fromDirectory(examplesDir);
  runtime.discoverSkills();
  
  const skill = runtime.activateSkill("example-skill");
  assert.strictEqual(skill.id, "example-skill");
  assert(skill.instructions, "Instructions should exist");
  assert(Array.isArray(skill.allowedTools), "allowedTools should be an array");
  console.log("testActivateSkill passed");
}

async function testExecuteSkillPlaceholderError() {
  console.log("Running testExecuteSkillPlaceholderError...");
  const examplesDir = getExamplesDir();
  const runtime = OpenSkillRuntime.fromDirectory(examplesDir);
  runtime.discoverSkills();
  
  try {
    runtime.executeSkill("example-skill", {
      input: JSON.stringify({ query: "hello" }),
      timeoutMs: 5000
    });
    assert.fail("Should have thrown an error");
  } catch (err) {
    const msg = err.message;
    if (msg.includes("Invalid WASM") || msg.includes("magic number") || msg.includes("component")) {
      console.log("Caught expected error:", msg);
    } else {
      throw err;
    }
  }
  console.log("testExecuteSkillPlaceholderError passed");
}

async function testSkillSessionForked() {
  console.log("Running testSkillSessionForked...");
  const examplesDir = getExamplesDir();
  const runtime = OpenSkillRuntime.fromDirectory(examplesDir);
  runtime.discoverSkills();
  
  // Create parent context
  const parentContext = new ExecutionContext();
  
  // Start forked skill session
  const session = runtime.startSkillSession(
    "code-review",
    JSON.stringify({ query: "Review this file" }),
    parentContext
  );
  
  assert(session.isForked(), "code-review should be forked");
  assert(session.contextId() !== null, "forked session should have context ID");
  
  // Record tool calls
  session.recordToolCall("Read", JSON.stringify({ path: "src/lib.rs", content: "fn main() {}" }));
  session.recordResult(JSON.stringify({ review: "Code looks good. No issues found." }));
  
  // Finish session
  const result = runtime.finishSkillSession(
    session,
    JSON.stringify({ review: "Code looks good." }),
    "",
    ""
  );
  
  const output = JSON.parse(result.outputJson);
  assert.strictEqual(output.is_forked, true, "Result should indicate forked execution");
  assert(output.summary.includes("looks good"), "Summary should contain result content");
  assert(result.audit.permissionsUsed.includes("Read"), "Read tool should be recorded");
  console.log("testSkillSessionForked passed");
}

async function testSkillSessionNonForked() {
  console.log("Running testSkillSessionNonForked...");
  const examplesDir = getExamplesDir();
  const runtime = OpenSkillRuntime.fromDirectory(examplesDir);
  runtime.discoverSkills();
  
  // Start non-forked skill session
  const session = runtime.startSkillSession("example-skill", null, null);
  
  assert(!session.isForked(), "example-skill should not be forked");
  assert(session.contextId() === null, "non-forked session should not have context ID");
  
  // Finish session
  const result = runtime.finishSkillSession(
    session,
    JSON.stringify({ result: "done" }),
    "stdout output",
    "stderr output"
  );
  
  const output = JSON.parse(result.outputJson);
  assert.strictEqual(output.result, "done", "Should return full output for non-forked");
  assert.strictEqual(result.stdout, "stdout output");
  assert.strictEqual(result.stderr, "stderr output");
  console.log("testSkillSessionNonForked passed");
}

async function testExecutionContextFork() {
  console.log("Running testExecutionContextFork...");
  
  const parent = new ExecutionContext();
  assert(!parent.isForked(), "Parent context should not be forked");
  
  const forked = parent.fork();
  assert(forked.isForked(), "Forked context should be forked");
  assert(forked.parentId() === parent.id(), "Forked parent_id should match parent id");
  
  // Record outputs in forked context
  forked.recordOutput("toolcall", "Read: src/lib.rs");
  forked.recordOutput("result", "Final result here");
  
  const summary = forked.summarize();
  assert(summary.includes("Final result"), "Summary should include result content");
  assert(!summary.includes("Read:"), "Summary should exclude tool calls");
  
  console.log("testExecutionContextFork passed");
}

async function testCheckToolPermission() {
  console.log("Running testCheckToolPermission...");
  const examplesDir = getExamplesDir();
  const runtime = OpenSkillRuntime.fromDirectory(examplesDir);
  runtime.discoverSkills();
  
  // code-review allows: Read, Grep, Glob, LS
  assert(runtime.checkToolPermission("code-review", "Read"), "Read should be allowed");
  assert(runtime.checkToolPermission("code-review", "Grep"), "Grep should be allowed");
  
  // Write is not allowed
  try {
    runtime.checkToolPermission("code-review", "Write");
    assert.fail("Write should not be allowed for code-review");
  } catch (err) {
    assert(err.message.includes("not allowed"), "Should throw permission error");
  }
  
  console.log("testCheckToolPermission passed");
}

async function runTests() {
  try {
    await testDiscoverSkills();
    await testActivateSkill();
    await testExecuteSkillPlaceholderError();
    await testSkillSessionForked();
    await testSkillSessionNonForked();
    await testExecutionContextFork();
    await testCheckToolPermission();
    console.log("All tests passed!");
  } catch (err) {
    console.error("Test failed:", err);
    process.exit(1);
  }
}

runTests();
