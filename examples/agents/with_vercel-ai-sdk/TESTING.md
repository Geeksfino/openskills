# OpenSkills Agent Testing Guide

This document describes the key integration tests for the OpenSkills agent and explains what each test validates.

## Quick Reference

| Test | Command | Tests |
|------|---------|-------|
| Official Skills (3P Update) | `npm run start:official -- "Show me what templates the internal-comms skill has, create a 3P update document, then tell me the file size of what you created and read it back to verify"` | Complete skill flow: explore, create, verify |
| Custom Skills (Skill Creation) | `npm start "Create a new skill called 'note-taker'. Details will be filled out later."` | WASM execution with custom skill-creator |

## Test 1: Official Skills - 3P Update

### Command
```bash
npm run start:official -- "Show me what templates the internal-comms skill has, create a 3P update document, then tell me the file size of what you created and read it back to verify"
```

This uses the `start:official` script which points to `claude-official-skills/skills`.

### Tools Exercised

| Tool | Purpose in Test |
|------|-----------------|
| `activate_skill` | Loads full SKILL.md instructions for `internal-comms` |
| `list_skill_files` | Lists available templates in the skill |
| `read_skill_file` | Reads `examples/3p-updates.md` template |
| `write_file` | Creates the 3P update document |
| `get_file_info` | Returns file size and metadata |
| `read_file` | Reads back the created file to verify |
| `list_workspace_files` | Lists all files in workspace |

### What Success Proves

- Skill discovery from custom directories works
- Agent semantically matches user intent to skills
- Skill file exploration (`list_skill_files`) works
- Complete create -> verify workflow with file I/O tools
- All 7 tools work together in a single flow

---

## Test 2: Custom Skills - Skill Creation (WASM)

### Command
```bash
npm start "Create a new skill called 'note-taker'. Details will be filled out later."
```

This uses the default skills directory (`examples/skills`) which contains the `skill-creator` WASM skill.

### Tools Exercised

| Tool | Purpose in Test |
|------|-----------------|
| `activate_skill` | Loads full SKILL.md instructions for skill-creator |
| `run_skill_script` | Executes WASM module with `init_skill` action |
| `write_file` | Writes generated template files (SKILL.md, scripts, references, assets) |
| `run_sandboxed_bash` | Sets executable permission on Python script (not working, missing permissions intentionally) |
| `list_workspace_files` | Verifies created file structure |
| `list_skills` | Checks if new skill is discoverable (optional) |

### Expected Output

When successful, you should see:
1. Tool Calls Summary showing `activate_skill`, `run_skill_script`, `write_file`, etc.
2. WASM validation message: `✅ WASM module was used! Returned N files.`
3. Created directory structure (in workspace `output/` directory):
   ```
   output/skills/public/note-taker/
   ├── SKILL.md              # Template with TODOs
   ├── scripts/
   │   └── example.py        # Example script (executable)
   ├── references/
   │   └── api_reference.md  # Reference template
   └── assets/
       └── example_asset.txt # Asset placeholder
   ```

### What Success Proves

If this test works, it proves:
- **WASM/WASI execution works**: The experimental cross-platform sandbox path is functional
- **Input handling works**: JSON input passed via stdin to WASM module
- **Validation logic runs in WASM**: Skill name validated (hyphen-case, length limits, format)
- **Template generation works**: WASM can generate structured file contents
- **Output handling works**: Runtime receives JSON with files and writes them to disk
- **Cross-platform compatibility**: Same WASM binary works on macOS, Linux, Windows

---

## Interpreting Console Output

### Tool Calls Summary
At the end of execution, you'll see a summary of all tool calls across all steps:
```
======================================================================
🔧 Tool Calls Summary (9 across 10 steps):
======================================================================

  activate_skill:
    Args: {"skill_id":"skill-creator"}

  run_skill_script:
    Args: {"skill_id":"skill-creator","script_path":"wasm/skill.wasm","input":"{\"action\": \"init_skill\"...
```

### Successful WASM Execution
Look for this message after the `run_skill_script` result:
```
✅ Tool result: run_skill_script
   Result: {"stdout": "{\"success\":true,\"message\":\"Skill 'note-taker' initialization instructions generated\"...

    ✅ WASM module was used! Returned 4 files.
```

### Successful Completion
```
✅ Agent execution completed successfully
📊 Total steps: N
```
