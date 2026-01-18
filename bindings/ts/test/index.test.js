const { OpenSkillRuntimeWrapper: OpenSkillRuntime } = require("../index");
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
      timeout_ms: 5000
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

async function runTests() {
  try {
    await testDiscoverSkills();
    await testActivateSkill();
    await testExecuteSkillPlaceholderError();
    console.log("All tests passed!");
  } catch (err) {
    console.error("Test failed:", err);
    process.exit(1);
  }
}

runTests();
