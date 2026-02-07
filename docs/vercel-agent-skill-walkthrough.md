# How a Vercel Agent Uses a Skill via OpenSkills (Native Seatbelt Example)

This document traces the **exact end-to-end flow** of how a Vercel AI SDK agent discovers, activates, and executes a skill through the OpenSkills runtime, using the native seatbelt sandbox as the concrete example.

We'll use the **code-review** skill as our running example, since it's a real skill in the repo with a SKILL.md, allowed-tools, and fork context.

---

## The Big Picture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Your Vercel AI SDK Agent                        │
│  (examples/agents/with_vercel-ai-sdk/src/index.ts)                 │
│                                                                     │
│  1. Imports OpenSkillRuntime + createSkillTools + getAgentSystemPrompt│
│  2. Calls streamText() with tools + system prompt                   │
│  3. LLM decides which tools to call via function calling            │
└──────────────────────────────┬──────────────────────────────────────┘
                               │  (tool calls go through Vercel AI SDK)
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                Pre-built Tools Layer (tools.js)                      │
│  list_skills, activate_skill, read_skill_file, run_skill_script,   │
│  run_sandboxed_bash, write_file, read_file, etc.                   │
└──────────────────────────────┬──────────────────────────────────────┘
                               │  (calls into NAPI-RS native bindings)
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│           TypeScript Bindings (bindings/ts/src/lib.rs)              │
│  OpenSkillRuntimeWrapper → wraps Rust OpenSkillRuntime via NAPI-RS │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                  Rust Core Runtime (runtime/src/)                    │
│  lib.rs → registry.rs → executor.rs → native_runner.rs             │
│                                                                     │
│  Discovers skills, parses SKILL.md, enforces permissions,           │
│  builds seatbelt profiles, spawns sandboxed processes               │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Step-by-Step Flow

### Step 0: Agent Initialization (Your Code)

The Vercel agent starts in `examples/agents/with_vercel-ai-sdk/src/index.ts`:

```typescript
// 1. Create the runtime, pointing it at a skills directory
const runtime = OpenSkillRuntime.fromDirectory("./skills");

// 2. Discover all skills (reads SKILL.md frontmatter from every subdirectory)
runtime.discoverSkills();

// 3. Create pre-built tools — these become the LLM's "function calling" tools
const tools = createSkillTools(runtime, { workspaceDir: "./output" });

// 4. Generate a system prompt that teaches the LLM how to use skills
const systemPrompt = getAgentSystemPrompt(runtime);

// 5. Run the agent
const result = streamText({
  model,
  system: systemPrompt,
  prompt: userQuery,
  tools,             // ← these are the 10 pre-built tools
  maxSteps: 20,
});
```

That's ~30 lines of application code. The rest is handled by the runtime. Let's trace what happens inside each call.

---

### Step 1: `OpenSkillRuntime.fromDirectory("./skills")`

**What happens (TypeScript bindings → Rust):**

1. The JS `OpenSkillRuntime` class (actually `OpenSkillRuntimeWrapper` from `bindings/ts/src/lib.rs`) is instantiated.
2. It creates a `RuntimeConfig` with `custom_directories: ["./skills"]` and `use_standard_locations: false`.
3. A Rust `OpenSkillRuntime` is constructed, which internally creates:
   - A `SkillRegistry` (empty `HashMap<String, SkillMetadata>`)
   - A workspace directory (defaults to `~/.cache/openskills/workspace/{session-id}/`)
   - An audit sink (NoopAuditSink by default)

**No skills are loaded yet** — this just sets up the configuration.

---

### Step 2: `runtime.discoverSkills()`

**What happens (Rust `SkillRegistry::scan_directory()`):**

The registry walks every subdirectory under `./skills/` looking for `SKILL.md` files. For each one found (e.g., `./skills/code-review/SKILL.md`):

1. **Read ONLY the YAML frontmatter** — the Markdown body is discarded at this point (progressive disclosure Tier 1):

   ```yaml
   ---
   name: code-review
   description: Reviews code for quality, best practices, and potential issues.
   allowed-tools: Read, Grep, Glob, LS
   context: fork
   agent: Explore
   ---
   ```

2. **Parse into `SkillManifest`** struct:
   - `name`: `"code-review"`
   - `description`: `"Reviews code for quality..."`
   - `allowed_tools`: `AllowedTools::CommaSeparated("Read, Grep, Glob, LS")`
   - `context`: `Some("fork")`
   - `agent`: `Some("Explore")`

3. **Validate** the manifest:
   - Name: 1-64 chars, lowercase/alphanumeric/hyphens only, no leading/trailing/consecutive hyphens
   - Description: 1-1024 chars, no XML tags
   - Directory name `code-review` must match manifest `name: code-review`
   - Context value must be `"fork"` or absent

4. **Store as `SkillMetadata`** (NOT `Skill` — instructions are NOT loaded):
   ```rust
   SkillMetadata {
       id: "code-review",
       root: PathBuf::from("./skills/code-review"),
       manifest: SkillManifest { /* parsed frontmatter */ },
       location: SkillLocation::Custom,
   }
   ```

5. **Insert into registry** `HashMap<String, SkillMetadata>`.

**Key point:** At discovery time, the Markdown body of SKILL.md (the actual instructions) is **never read**. This is the progressive disclosure optimization — if you have 50 skills, you only load ~100 tokens of metadata each, not thousands of tokens of instructions.

---

### Step 3: `createSkillTools(runtime, { workspaceDir: "./output" })`

**What happens (tools.js):**

This function creates **10 Vercel AI SDK `tool()` definitions** that wrap the runtime:

| Tool | Description | What it calls on the runtime |
|------|-------------|------------------------------|
| `list_skills` | List available skills | `runtime.listSkills()` |
| `activate_skill` | Get full SKILL.md instructions | `runtime.activateSkill(id)` |
| `read_skill_file` | Read helper files from skill dir | `runtime.readSkillFile(id, path)` |
| `list_skill_files` | List files in a skill directory | `runtime.listSkillFiles(id, ...)` |
| `run_skill_script` | Run a script/WASM in sandbox | `runtime.runSkillTarget(id, opts)` |
| `run_sandboxed_bash` | Run a bash command in sandbox | `runSandboxedShellCommand(cmd, ...)` |
| `write_file` | Write to workspace | `fs.writeFileSync(...)` |
| `read_file` | Read from workspace | `fs.readFileSync(...)` |
| `list_workspace_files` | List workspace files | `fs.readdirSync(...)` |
| `get_file_info` | Get file metadata | `fs.statSync(...)` |

Each tool is defined using Vercel AI SDK's `tool()` with a Zod schema for parameters. The LLM sees these as available function calls.

---

### Step 4: `getAgentSystemPrompt(runtime)`

**What happens (tools.js → Rust `get_agent_system_prompt()`):**

Generates a system prompt like:

```
You have access to Claude Skills that provide specialized capabilities.

## Available Skills

- **code-review**: Reviews code for quality, best practices, and potential issues.
- **explaining-code**: Explains code in detail...

## How to Use Skills

When a user's request matches a skill's capabilities:

1. **Activate the skill**: Call `activate_skill(skill_id)` to load the full SKILL.md instructions
2. **Read the instructions carefully**: The SKILL.md contains everything you need to know
3. **Follow the instructions exactly**: Execute the steps as described in SKILL.md
4. **Use helper files if referenced**: Call `read_skill_file(skill_id, path)` to read referenced docs
5. **Run scripts or WASM modules as instructed**: Call `run_skill_script()` to execute them
```

**Key insight:** The system prompt is **skill-agnostic**. It teaches the LLM the *protocol* for using skills (discover → activate → follow instructions) but contains **zero domain knowledge** about what any skill does. All domain knowledge lives inside each skill's SKILL.md.

---

### Step 5: `streamText()` — The Agent Runs

Now the Vercel AI SDK sends the system prompt + user query + tool definitions to the LLM. The LLM is in control from here.

**Example user query:** `"Can you review my code in src/main.rs?"`

The LLM reads the system prompt, sees `code-review` in the available skills list, and decides to use it. Here's what the LLM does (via tool calls):

---

### Step 6: LLM calls `list_skills()` (optional)

The LLM might call `list_skills` first to see what's available:

```json
// Tool call
{ "query": "review" }

// Tool result (from runtime.listSkills())
[
  { "id": "code-review", "description": "Reviews code for quality, best practices..." },
  { "id": "explaining-code", "description": "Explains code in detail..." }
]
```

**Rust path:** `OpenSkillRuntime::list_skills()` → iterates the registry's `SkillMetadata` entries and returns `Vec<SkillDescriptor>` (just `id` + `description` + `location` + `user_invocable`). Still no instructions loaded.

---

### Step 7: LLM calls `activate_skill("code-review")` — Tier 2 Loading

This is where **instructions are first loaded**. The LLM calls:

```json
{ "skill_id": "code-review" }
```

**What happens in Rust (`OpenSkillRuntime::activate_skill()` → `SkillRegistry::load_full_skill()`):**

1. Look up `"code-review"` in the registry → get `SkillMetadata`
2. **Now re-read the FULL SKILL.md** file (this time including the Markdown body):
   ```
   skills/code-review/SKILL.md
   ```
3. Parse both frontmatter AND body to create a full `Skill` struct:
   ```rust
   Skill {
       id: "code-review",
       root: PathBuf::from("./skills/code-review"),
       manifest: SkillManifest { /* same as before */ },
       instructions: "# Code Review Skill\n\nPerform thorough code reviews...\n\n## Review Checklist\n...",
       location: SkillLocation::Custom,
   }
   ```
4. Return a `LoadedSkill` to the TypeScript layer:
   ```typescript
   {
     id: "code-review",
     name: "code-review",
     allowedTools: ["Read", "Grep", "Glob", "LS"],
     instructions: "# Code Review Skill\n\nPerform thorough code reviews..."
   }
   ```

**The LLM now receives the full Markdown instructions** (~2000 tokens for code-review). This is the skill's "brain" — it tells the LLM exactly what to do, how to structure the review, what to look for, and how to format output.

**This is progressive disclosure Tier 2**: instructions loaded on-demand, only for the skill the LLM chose to activate.

---

### Step 8: LLM Follows the Instructions

The code-review skill is **instruction-only** — it doesn't have any scripts or WASM to execute. It just tells the LLM how to perform a code review using the tools it already has (Read, Grep, Glob, LS).

So the LLM will now use other tools (like `read_skill_file` or `read_file`) to read the code the user mentioned, then produce a review following the skill's template.

**But what about skills that DO have scripts?** Let's trace that path next.

---

### Step 9: LLM calls `run_skill_script()` — Tier 3 Native Execution (Seatbelt)

For a skill with scripts (e.g., a PDF skill with `scripts/extract_text.py`), the LLM would call:

```json
{
  "skill_id": "pdf",
  "script_path": "scripts/extract_text.py",
  "input": "{\"file\": \"/path/to/document.pdf\"}",
  "timeout_ms": 30000
}
```

**Here's the full execution chain:**

#### 9a. tools.js → NAPI-RS binding

```javascript
// tools.js run_skill_script tool execute function:
const result = runtime.runSkillTarget(skill_id, {
  path: script_path,          // "scripts/extract_text.py"
  args: args ?? [],
  input: input,               // JSON string
  timeoutMs: timeout_ms ?? 30000,
  workspaceDir: workspaceDir, // "./output"
});
```

#### 9b. NAPI-RS binding → Rust `OpenSkillRuntime::run_skill_target()`

The TypeScript `runSkillTarget` method (defined in `bindings/ts/src/lib.rs`) converts JS types to Rust types:

```rust
// bindings/ts/src/lib.rs
fn run_skill_target(&self, skill_id: String, options: TargetExecutionOptionsJs) -> Result<ExecutionResult> {
    // Convert JS options to Rust TargetExecutionOptions
    let target = ExecutionTarget::Path {
        path: options.path.unwrap(),           // "scripts/extract_text.py"
        args: options.args.unwrap_or_default(),
    };
    // ... calls into core runtime
}
```

#### 9c. Rust core: `executor::run_skill_target()`

In `runtime/src/executor.rs`:

1. **Load the full skill** (if not already loaded from activation):
   ```rust
   let skill = registry.load_full_skill("pdf")?;
   ```

2. **Map `allowed-tools` to capabilities:**
   ```rust
   let allowed_tools = skill.manifest.get_allowed_tools();
   // e.g., ["Read", "Write", "Bash"] → determines sandbox permissions
   let wasm_config = map_tools_to_capabilities(&allowed_tools);
   ```

3. **Create a `PermissionEnforcer`:**
   ```rust
   let enforcer = PermissionEnforcer::new(
       allowed_tools.clone(),
       wasm_config.clone(),
       skill.root.clone(), // "./skills/pdf"
   );
   ```

4. **Detect execution mode from file extension:**
   ```rust
   // For "scripts/extract_text.py" → .py extension detected
   ExecutionTarget::Path { path: "scripts/extract_text.py", args: [] }
   // Resolves to: native execution with ScriptType::Python
   ```

5. **Dispatch to `native_runner::execute_native()`.**

#### 9d. Native Runner: Build Seatbelt Profile (macOS) / Landlock Rules (Linux)

In `runtime/src/native_runner.rs`, the **seatbelt sandbox profile is dynamically generated**:

**On macOS:**

```
(version 1)
(deny default)                                    ← Start by denying EVERYTHING

;; Core permissions for interpreter execution
(allow sysctl-read)
(allow process-exec)
(allow mach-lookup)
(allow signal)

;; DENY sensitive paths FIRST (first-match-wins in seatbelt)
(deny file-read* (subpath "/Users/you/.ssh"))
(deny file-read* (subpath "/Users/you/.aws"))
(deny file-read* (subpath "/Users/you/.gnupg"))
;; ... more sensitive paths ...

;; ALLOW broad file reads (after denies, so sensitive paths stay blocked)
(allow file-read*)

;; ALLOW writes ONLY to specific paths
(allow file-write* (literal "/dev/null"))
(allow file-write* (subpath "/tmp"))
(allow file-write* (subpath "/private/tmp"))
(allow file-write* (subpath "./skills/pdf"))       ← skill root
(allow file-write* (subpath "./output"))            ← workspace dir

;; NO network access (unless WebSearch/Fetch in allowed-tools)
;; NO process forking (unless Bash/Terminal in allowed-tools)
```

**On Linux (Landlock LSM):**

Instead of seatbelt profiles, the Linux implementation uses Landlock (kernel 5.13+):
- Creates a Landlock ruleset with `AccessFs` permissions
- Adds read-only rules for system paths and skill root
- Adds read-write rules for temp dirs, skill root, and workspace
- Applies via `restrict_self()` in a `pre_exec` hook (between `fork()` and `exec()`)
- Falls back to `NO_NEW_PRIVS` on kernels without Landlock support

#### 9e. Spawn the Sandboxed Process

**On macOS:**
```bash
sandbox-exec -f /tmp/openskills-seatbelt-12345-1738886400000-42.sb -- \
  /usr/bin/python3 scripts/extract_text.py
```

**On Linux:**
```bash
# (Landlock rules applied via pre_exec, so it's just)
/usr/bin/python3 scripts/extract_text.py
```

**Environment variables injected:**
```
SKILL_ID=pdf
SKILL_NAME=pdf
SKILL_INPUT={"file": "/path/to/document.pdf"}
SKILL_ROOT=./skills/pdf
SKILL_WORKSPACE=./output
TIMEOUT_MS=30000
PYTHONUNBUFFERED=1
PYTHONDONTWRITEBYTECODE=1
PYTHONNOUSERSITE=1
PATH=/usr/bin:/bin:/usr/sbin:/sbin
```

The JSON input is also piped to stdin.

#### 9f. Script Execution (Inside Sandbox)

The Python script runs inside the sandbox:
- It reads `SKILL_INPUT` from environment (or stdin) to get the input JSON
- It can read files from the skill directory (`SKILL_ROOT`)
- It can write output files to `SKILL_WORKSPACE`
- It **cannot** access `~/.ssh`, `~/.aws`, or other sensitive paths
- It **cannot** access the network (unless `WebSearch`/`Fetch` is in allowed-tools)
- It **cannot** spawn subprocesses (unless `Bash`/`Terminal` is in allowed-tools)
- It writes its result to stdout (ideally as JSON)

#### 9g. Output Capture and Return

Back in Rust, the runtime:

1. **Captures stdout and stderr** from the child process via piped streams
2. **Enforces timeout** — polls `child.try_wait()` every 10ms; if elapsed time exceeds `timeout_ms`, kills the process
3. **Parses output:**
   - If stdout is valid JSON → use it as the structured output
   - If stdout is plain text → wrap it in `{ "status": "success", "output": "<text>" }`
   - If process failed → `{ "status": "error", "error": "<stderr or exit code>" }`
4. **Cleans up** the temporary seatbelt profile file
5. **Returns `ExecutionArtifacts`:**
   ```rust
   ExecutionArtifacts {
       output: json!({"text": "Extracted text...", "pages": 10}),
       stdout: "...",
       stderr: "",
       permissions_used: ["Read"],
       exit_status: ExecutionStatus::Success,
   }
   ```

#### 9h. Result Back to LLM

The tools.js layer truncates large outputs (max 32KB per field) and returns to the LLM:

```json
{
  "stdout": "...",
  "stderr": "",
  "output": {"text": "Extracted text...", "pages": 10}
}
```

The LLM then uses this result to formulate its response to the user.

---

## Summary: The Complete Chain

```
User: "Extract text from my PDF"
  │
  ▼
LLM (via Vercel AI SDK streamText)
  │
  ├─ Tool call: list_skills()
  │   └─ JS tools.js → NAPI-RS → Rust registry.list() → returns metadata only
  │
  ├─ Tool call: activate_skill("pdf")
  │   └─ JS tools.js → NAPI-RS → Rust registry.load_full_skill()
  │       └─ Reads FULL SKILL.md (frontmatter + markdown body)
  │       └─ Returns instructions to LLM (~5000 tokens)
  │
  ├─ Tool call: run_skill_script("pdf", "scripts/extract_text.py", ...)
  │   └─ JS tools.js → NAPI-RS → Rust executor::run_skill_target()
  │       ├─ Maps allowed-tools → sandbox permissions
  │       ├─ Creates PermissionEnforcer
  │       ├─ Detects: .py → native execution
  │       ├─ Builds seatbelt profile (macOS) or Landlock rules (Linux)
  │       ├─ Spawns: sandbox-exec -- python3 scripts/extract_text.py
  │       ├─ Injects: SKILL_INPUT, SKILL_ROOT, SKILL_WORKSPACE env vars
  │       ├─ Pipes JSON input to stdin
  │       ├─ Captures stdout/stderr
  │       ├─ Enforces timeout
  │       └─ Returns structured output to LLM
  │
  └─ LLM generates final response to user
```

---

## Key Architecture Decisions

1. **Progressive Disclosure (3-tier loading)**: Discovery loads only YAML frontmatter (~100 tokens per skill). Instructions (~2K-5K tokens) only load when the LLM activates a specific skill. Scripts/WASM only load when executed. This keeps the LLM's context window efficient.

2. **Skill-Agnostic Agent**: The agent code has **zero knowledge** about what any skill does. The system prompt teaches the protocol; SKILL.md teaches the domain. This means adding a new skill requires zero code changes to the agent.

3. **Dual Sandbox with Auto-Detection**: The runtime auto-detects whether to use WASM sandbox (.wasm files) or native sandbox (.py/.sh files) based on file extension. The same `run_skill_script` tool handles both transparently.

4. **Permission Mapping**: The `allowed-tools` field in SKILL.md frontmatter maps to concrete sandbox permissions:
   - `Read`, `Grep`, `Glob`, `LS` → filesystem read access
   - `Write` → filesystem write access
   - `WebSearch`, `Fetch` → network access
   - `Bash`, `Terminal` → subprocess spawning

5. **Defense in Depth**: Even with seatbelt/Landlock, the runtime also:
   - Denies access to sensitive paths (~/.ssh, ~/.aws, etc.) explicitly
   - Clears environment variables that might leak credentials
   - Sets `PYTHONNOUSERSITE=1` to isolate Python
   - Uses per-execution temporary seatbelt profiles (not shared)
   - Enforces timeouts with process kill
