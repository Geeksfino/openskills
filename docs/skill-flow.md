# Skill Execution Flow Documentation

This document illustrates the complete end-to-end process of how skills are discovered, matched, activated, and executed in the OpenSkills runtime. Two complete workflows are provided:

1. **Code Review Skill (WASM Execution)** - Demonstrates WASM-based sandboxing
2. **PDF Skill (Native Python Execution)** - Demonstrates native seatbelt sandboxing on macOS

## Workflow 1: Code Review Skill (WASM Execution)

### Phase 1: Discovery (Tier 1 - Metadata Only)

```
User Query: "Can you review my code?"
     │
     ▼
Agent calls: runtime.discover_skills()
     │
     ├─► SCAN DIRECTORIES
     │   ├─ ~/.claude/skills/
     │   ├─ .claude/skills/
     │   └─ examples/skills/ (custom directory)
     │
     ├─► For each directory found:
     │   ├─ Walk subdirectories
     │   ├─ Check for SKILL.md file
     │   └─ If found: LOAD TIER 1 ONLY
     │
     ▼
File I/O: Read examples/skills/code-review/SKILL.md
     │
     ├─► Parse YAML frontmatter (lines 1-7):
     │   ├─ name: "code-review"
     │   ├─ description: "Reviews code for quality..."
     │   ├─ allowed-tools: ["Read", "Grep", "Glob", "LS"]
     │   ├─ context: "fork"
     │   └─ agent: "Explore"
     │
     ├─► SKIP Markdown body (Tier 2 - not loaded yet)
     │
     └─► Create SkillDescriptor:
         {
           id: "code-review",
           description: "Reviews code for quality...",
           location: Custom,
           user_invocable: true
         }
     │
     ▼
Return: Vec<SkillDescriptor> (metadata only, ~100 tokens)
     │
     └─► Agent receives skill list for semantic matching
```

### Phase 2: Skill Matching

```
Agent receives skill descriptors:
  - code-review: "Reviews code for quality..."
  - test-generator: "Generates test cases..."
  - pdf: "PDF manipulation toolkit..."

Agent performs semantic matching:
  Query: "Can you review my code?"
  Match: "code-review" (high confidence)
     │
     ▼
Agent decides to use: "code-review"
```

### Phase 3: Activation (Tier 2 - Instructions)

```
Agent calls: runtime.get_skill("code-review")
     │
     ├─► Registry.get("code-review")
     │   └─ Returns: Skill (already loaded during discovery)
     │
     ▼
File I/O: SKILL.md already parsed, but now we use the full content
     │
     ├─► Tier 1 (already loaded):
     │   └─ Manifest metadata
     │
     ├─► Tier 2 (now accessed):
     │   └─ Full SKILL.md markdown body (lines 9-67)
     │       ├─ "# Code Review Skill"
     │       ├─ "## Review Checklist"
     │       ├─ "### 1. Correctness"
     │       ├─ "### 2. Security"
     │       ├─ "### 3. Performance"
     │       ├─ "### 4. Maintainability"
     │       ├─ "### 5. Testing"
     │       ├─ "## Output Format"
     │       └─ "## Guidelines"
     │
     └─► Return: LoadedSkill {
           id: "code-review",
           manifest: SkillManifest {...},
           instructions: "# Code Review Skill\n\nPerform thorough...",
           location: Custom
         }
     │
     ▼
Agent receives full instructions (~2000 tokens)
     │
     └─► Agent now has complete skill context
```

### Phase 4: Execution (Tier 3 - WASM Module)

```
Agent calls: runtime.execute_skill("code-review", ExecutionOptions {
    input: json!({"code": "...", "file": "src/main.rs"}),
    timeout_ms: Some(30000)
})
     │
     ├─► Registry.get("code-review") → Skill
     │
     ├─► Validate skill structure
     │
     ├─► Check permissions for risky tools
     │   └─ allowed-tools: ["Read", "Grep", "Glob", "LS"]
     │       └─ All are low-risk, no permission prompt needed
     │
     ├─► Map allowed-tools to WASI capabilities:
     │   ├─ Read → filesystem read access
     │   ├─ Grep → filesystem read access
     │   ├─ Glob → filesystem read access
     │   └─ LS → filesystem read access
     │
     ├─► Create PermissionEnforcer:
     │   ├─ allowed_tools: ["Read", "Grep", "Glob", "LS"]
     │   ├─ wasm_config: { filesystem: { read: [...] }, ... }
     │   └─ skill_root: "examples/skills/code-review"
     │
     ▼
Detect execution mode: detect_execution_mode()
     │
     ├─► Check for WASM module:
     │   ├─ examples/skills/code-review/wasm/skill.wasm ✓ FOUND
     │   └─ Return: ExecutionMode::Wasm { wasm_module: "wasm/skill.wasm" }
     │
     ▼
File I/O: Load WASM module (Tier 3 - on-demand)
     │
     ├─► Read: examples/skills/code-review/wasm/skill.wasm
     │   └─ Binary file read into memory
     │
     ▼
WASM Execution: execute_wasm()
     │
     ├─► Configure Wasmtime:
     │   ├─ Engine with WASI 0.3 (component model)
     │   ├─ Epoch interruption (for timeout)
     │   └─ Async support enabled
     │
     ├─► Build WASI Context:
     │   ├─ Preopen skill root: /skill (read-only)
     │   ├─ Preopen read paths (if any from allowed-tools)
     │   ├─ Inject environment variables:
     │   │   ├─ SKILL_ID="code-review"
     │   │   ├─ SKILL_NAME="code-review"
     │   │   ├─ SKILL_INPUT='{"code":"...","file":"src/main.rs"}'
     │   │   └─ TIMEOUT_MS="30000"
     │   └─ Capture stdout/stderr (in-memory buffers)
     │
     ├─► Load WASM Component:
     │   ├─ Parse wasm/skill.wasm
     │   ├─ Validate WASI 0.3 compatibility
     │   └─ Create Component instance
     │
     ├─► Execute in WASM Sandbox:
     │   ├─ Runtime: Wasmtime with WASI 0.3
     │   ├─ Memory: Isolated linear memory
     │   ├─ Filesystem: Capability-based (only /skill and preopened dirs)
     │   ├─ Network: Denied (not in allowed-tools)
     │   ├─ Process: Denied (not in allowed-tools)
     │   └─ Timeout: 30 seconds (epoch-based interruption)
     │
     ├─► WASM Module Execution:
     │   ├─ Receives input via SKILL_INPUT env var
     │   ├─ Can read from /skill directory (skill resources)
     │   ├─ Can read from preopened paths (if any)
     │   ├─ Processes code review logic
     │   ├─ Writes output to stdout (captured)
     │   └─ Returns JSON result
     │
     ├─► Capture Output:
     │   ├─ stdout: "Review results..."
     │   ├─ stderr: "" (empty)
     │   └─ exit_status: Success
     │
     └─► Return: ExecutionArtifacts {
           output: json!({"review": "...", "issues": [...]}),
           stdout: "Review results...",
           stderr: "",
           permissions_used: ["Read", "Grep", "Glob", "LS"],
           exit_status: Success
         }
     │
     ▼
Agent receives execution result
     │
     └─► Agent uses review results in response to user
```

---

## Workflow 2: PDF Skill (Native Python Execution)

### Phase 1: Discovery (Tier 1 - Metadata Only)

```
User Query: "Extract text from this PDF file"
     │
     ▼
Agent calls: runtime.discover_skills()
     │
     ├─► SCAN DIRECTORIES
     │   ├─ ~/.claude/skills/
     │   ├─ .claude/skills/
     │   └─ examples/claude-official-skills/skills/ (submodule)
     │
     ├─► For each directory found:
     │   ├─ Walk subdirectories
     │   ├─ Check for SKILL.md file
     │   └─ If found: LOAD TIER 1 ONLY
     │
     ▼
File I/O: Read examples/claude-official-skills/skills/pdf/SKILL.md
     │
     ├─► Parse YAML frontmatter (lines 1-5):
     │   ├─ name: "pdf"
     │   ├─ description: "Comprehensive PDF manipulation toolkit..."
     │   └─ license: "Proprietary. LICENSE.txt has complete terms"
     │
     ├─► SKIP Markdown body (Tier 2 - not loaded yet)
     │
     ├─► SKIP scripts/ directory (Tier 3 - not loaded yet)
     │
     └─► Create SkillDescriptor:
         {
           id: "pdf",
           description: "Comprehensive PDF manipulation toolkit...",
           location: Custom,
           user_invocable: true
         }
     │
     ▼
Return: Vec<SkillDescriptor> (metadata only, ~100 tokens)
     │
     └─► Agent receives skill list for semantic matching
```

### Phase 2: Skill Matching

```
Agent receives skill descriptors:
  - code-review: "Reviews code..."
  - pdf: "Comprehensive PDF manipulation toolkit..."
  - docx: "Word document processing..."

Agent performs semantic matching:
  Query: "Extract text from this PDF file"
  Match: "pdf" (high confidence)
     │
     ▼
Agent decides to use: "pdf"
```

### Phase 3: Activation (Tier 2 - Instructions)

```
Agent calls: runtime.get_skill("pdf")
     │
     ├─► Registry.get("pdf")
     │   └─ Returns: Skill (already loaded during discovery)
     │
     ▼
File I/O: SKILL.md already parsed, but now we use the full content
     │
     ├─► Tier 1 (already loaded):
     │   └─ Manifest metadata
     │
     ├─► Tier 2 (now accessed):
     │   └─ Full SKILL.md markdown body (lines 7-295)
     │       ├─ "# PDF Processing Guide"
     │       ├─ "## Overview"
     │       ├─ "## Quick Start"
     │       ├─ "## Python Libraries"
     │       │   ├─ "### pypdf - Basic Operations"
     │       │   ├─ "### pdfplumber - Text and Table Extraction"
     │       │   └─ "### pdf2image - Convert to Images"
     │       ├─ "## Command-Line Tools"
     │       ├─ References to:
     │       │   ├─ forms.md (for form filling)
     │       │   └─ reference.md (for advanced features)
     │       └─ Script examples and usage patterns
     │
     └─► Return: LoadedSkill {
           id: "pdf",
           manifest: SkillManifest {...},
           instructions: "# PDF Processing Guide\n\n## Overview...",
           location: Custom
         }
     │
     ▼
Agent receives full instructions (~5000+ tokens)
     │
     └─► Agent now has complete skill context
```

### Phase 4: Execution (Tier 3 - Native Python Scripts)

```
Agent calls: runtime.execute_skill("pdf", ExecutionOptions {
    input: json!({
        "action": "extract_text",
        "file": "/path/to/document.pdf"
    }),
    timeout_ms: Some(10000)
})
     │
     ├─► Registry.get("pdf") → Skill
     │
     ├─► Validate skill structure
     │
     ├─► Check permissions for risky tools
     │   └─ allowed-tools: [] (empty = all tools allowed)
     │       └─ No explicit restrictions, proceed
     │
     ├─► Map allowed-tools to capabilities:
     │   └─ Empty list → default permissions (read/write in skill root)
     │
     ├─► Create PermissionEnforcer:
     │   ├─ allowed_tools: [] (all allowed)
     │   ├─ wasm_config: default (for native, used for path mapping)
     │   └─ skill_root: "examples/claude-official-skills/skills/pdf"
     │
     ▼
Detect execution mode: detect_execution_mode()
     │
     ├─► Check for WASM module:
     │   ├─ examples/.../pdf/wasm/skill.wasm ✗ NOT FOUND
     │   └─ Continue to native script detection
     │
     ├─► Check for native scripts:
     │   ├─ examples/.../pdf/scripts/extract_form_field_info.py ✓ FOUND
     │   ├─ examples/.../pdf/scripts/fill_fillable_fields.py
     │   ├─ examples/.../pdf/scripts/convert_pdf_to_images.py
     │   └─ ... (8 Python scripts total)
     │
     ├─► Based on input action="extract_text":
     │   └─ Select: scripts/extract_form_field_info.py
     │       (or agent may choose based on instructions)
     │
     └─► Return: ExecutionMode::Native {
           script_path: "scripts/extract_form_field_info.py",
           script_type: ScriptType::Python
         }
     │
     ▼
File I/O: Load Python script (Tier 3 - on-demand)
     │
     ├─► Read: examples/.../pdf/scripts/extract_form_field_info.py
     │   └─ Source code read (but NOT loaded into agent context)
     │       └─ Only script output will be returned
     │
     ▼
Native Execution: execute_native() [macOS seatbelt]
     │
     ├─► Detect platform: macOS ✓
     │
     ├─► Prepare seatbelt profile:
     │   ├─ Canonicalize skill_root path
     │   ├─ Get read paths from PermissionEnforcer
     │   ├─ Get write paths from PermissionEnforcer
     │   ├─ Determine network access: false (no WebSearch/Fetch in tools)
     │   ├─ Determine process access: false (no Bash/Terminal in tools)
     │   └─ Get Python executable path: /usr/bin/python3
     │
     ├─► Build seatbelt profile string:
     │   ├─ "(version 1)"
     │   ├─ "(deny default)"  ← Start with deny-all
     │   ├─ "(allow sysctl-read)"
     │   ├─ System read paths:
     │   │   ├─ "/System", "/usr/lib", "/usr/bin", etc.
     │   │   └─ "(allow file-read* file-map-executable (subpath \"/usr/bin\"))"
     │   ├─ Temp paths:
     │   │   └─ "(allow file-read* file-write* (subpath \"/tmp\"))"
     │   ├─ Skill root (read-only):
     │   │   └─ "(allow file-read* (subpath \".../pdf\"))"
     │   ├─ Read paths (if any from allowed-tools):
     │   │   └─ "(allow file-read* (subpath \"...\"))"
     │   └─ Write paths (if any from allowed-tools):
     │       └─ "(allow file-write* (subpath \"...\"))"
     │
     ├─► Write seatbelt profile to temp file:
     │   └─ /tmp/openskills-seatbelt-{pid}-{attempt}.sb
     │
     ├─► Prepare command:
     │   ├─ Program: "sandbox-exec"
     │   ├─ Args: ["-f", profile_path, "--", "python3", script_path]
     │   ├─ Working directory: skill_root
     │   ├─ stdin: piped (for input JSON)
     │   ├─ stdout: piped (for capture)
     │   └─ stderr: piped (for capture)
     │
     ├─► Set environment variables:
     │   ├─ SKILL_ID="pdf"
     │   ├─ SKILL_NAME="pdf"
     │   ├─ SKILL_INPUT='{"action":"extract_text","file":"/path/to/document.pdf"}'
     │   ├─ TIMEOUT_MS="10000"
     │   ├─ PYTHONNOUSERSITE="1" (isolate from user site-packages)
     │   └─ PATH, PYTHONPATH (minimal, sandboxed)
     │
     ├─► Spawn process with seatbelt:
     │   └─ sandbox-exec -f /tmp/openskills-seatbelt-12345-0.sb -- \
     │       python3 scripts/extract_form_field_info.py
     │
     ├─► Write input JSON to stdin
     │
     ├─► Monitor execution with timeout:
     │   ├─ Start timeout thread (10 seconds)
     │   ├─ Wait for process completion
     │   └─ If timeout: kill process, return timeout error
     │
     ├─► Read stdout/stderr:
     │   ├─ stdout: Captured output from Python script
     │   └─ stderr: Any error messages
     │
     ├─► Python Script Execution (in seatbelt sandbox):
     │   ├─ Reads SKILL_INPUT from environment
     │   ├─ Parses JSON: {"action": "extract_text", "file": "..."}
     │   ├─ Can read from skill root: scripts/, forms.md, reference.md
     │   ├─ Can read from allowed read paths (if any)
     │   ├─ Can write to allowed write paths (if any)
     │   ├─ CANNOT access network (denied by seatbelt)
     │   ├─ CANNOT spawn processes (denied by seatbelt)
     │   ├─ CANNOT access files outside allowed paths
     │   ├─ Executes PDF extraction logic
     │   ├─ Uses pypdf or pdfplumber (if installed)
     │   └─ Outputs results to stdout (JSON)
     │
     ├─► Cleanup:
     │   └─ Remove temporary seatbelt profile file
     │
     └─► Return: ExecutionArtifacts {
           output: json!({"text": "Extracted text content...", "pages": 10}),
           stdout: "Extracted text content...",
           stderr: "",
           permissions_used: [],
           exit_status: Success
         }
     │
     ▼
Agent receives execution result
     │
     └─► Agent uses extracted text in response to user
```

---

## Sandbox Environment Support

OpenSkills provides two complementary sandbox environments, each optimized for different use cases:

### 1. WASM/WASI 0.3 Sandbox (Primary)

**Purpose**: Cross-platform, capability-based sandboxing for compiled skills

**Technology Stack**:
- **Runtime**: Wasmtime 40+ with WASI 0.3 (component model)
- **Isolation**: Linear memory model, capability-based filesystem access
- **Platform Support**: macOS, Linux, Windows (identical behavior)

**How It Works**:

1. **Skill Compilation**: JavaScript/TypeScript skills are compiled to WASM components using tools like `javy` (QuickJS-based) or `wasm-pack`
2. **Module Loading**: WASM modules are loaded from `wasm/skill.wasm` in the skill directory
3. **WASI Context Setup**:
   - Preopens filesystem paths based on `allowed-tools` mapping
   - Grants read/write permissions only to explicitly allowed directories
   - Injects environment variables (SKILL_ID, SKILL_INPUT, etc.)
   - Captures stdout/stderr in memory buffers
4. **Capability Mapping**: `allowed-tools` values are mapped to WASI capabilities:
   - `Read`, `Grep`, `Glob`, `LS` → filesystem read access
   - `Write` → filesystem write access
   - `WebSearch`, `Fetch` → network access (with host allowlist)
   - `Bash`, `Terminal` → process spawning (if supported)
5. **Execution**: Component runs in isolated WASM instance with:
   - Timeout enforcement via epoch interruption
   - Memory limits (configurable)
   - No access to host filesystem except preopened paths
   - No network access unless explicitly allowed
   - Deterministic execution across platforms

**Advantages**:
- ✅ **Cross-platform consistency**: Same security model everywhere
- ✅ **Memory safety**: Linear memory prevents buffer overflows
- ✅ **Portability**: Skills can ship pre-compiled WASM modules
- ✅ **Fine-grained permissions**: Capability-based access control
- ✅ **No native dependencies**: Pure WASM execution

**Limitations**:
- Requires compilation step (TS/JS → WASM)
- Limited to WASI-compatible APIs
- Cannot use native Python packages directly
- Performance overhead compared to native execution

**Use Cases**:
- TypeScript/JavaScript-based skills
- Skills requiring cross-platform consistency
- Skills that can be compiled to WASM
- Enterprise deployments with strict security requirements

---

### 2. Native Seatbelt Sandbox (macOS Only)

**Purpose**: OS-level sandboxing for native Python and shell scripts

**Technology Stack**:
- **Runtime**: macOS `sandbox-exec` with seatbelt profiles
- **Isolation**: Process-level sandboxing with filesystem and network restrictions
- **Platform Support**: macOS only (Linux seccomp support planned)

**How It Works**:

1. **Script Detection**: Runtime detects native scripts (`.py`, `.sh`) in `scripts/` directory
2. **Seatbelt Profile Generation**:
   - Starts with `(deny default)` - deny all by default
   - Allows system read paths: `/System`, `/usr/bin`, `/usr/lib`, etc.
   - Allows temporary paths: `/tmp`, `/private/tmp` (read/write)
   - Allows skill root directory (read-only by default)
   - Adds read/write paths based on `allowed-tools` mapping
   - Grants `file-map-executable` permission for Python interpreter and script parent directory
   - Conditionally allows network access if `WebSearch` or `Fetch` in allowed-tools
   - Conditionally allows process spawning if `Bash` or `Terminal` in allowed-tools
3. **Profile Writing**: Seatbelt profile written to temporary file (`/tmp/openskills-seatbelt-{pid}-{attempt}.sb`)
4. **Process Execution**:
   - Spawns `sandbox-exec -f {profile} -- python3 {script}`
   - Sets working directory to skill root
   - Pipes input JSON to stdin
   - Captures stdout/stderr
   - Enforces timeout via separate monitoring thread
5. **Environment Isolation**:
   - Sets `PYTHONNOUSERSITE=1` to prevent loading user site-packages
   - Minimal PATH and PYTHONPATH
   - Injects skill metadata as environment variables
6. **Cleanup**: Removes temporary seatbelt profile after execution

**Advantages**:
- ✅ **Native Python support**: Can use any Python package (pypdf, pdfplumber, etc.)
- ✅ **Shell script support**: Can execute bash scripts
- ✅ **Full OS API access**: Within sandbox constraints
- ✅ **No compilation required**: Direct script execution
- ✅ **Strong isolation**: Process-level sandboxing

**Limitations**:
- macOS only (Linux seccomp support planned)
- Platform-specific behavior
- Requires native Python/system dependencies
- Less portable than WASM

**Use Cases**:
- Python-based skills (PDF processing, document manipulation)
- Skills requiring native libraries
- Legacy scripts that cannot be compiled to WASM
- macOS-specific deployments

---

## Execution Mode Detection

The runtime automatically detects which execution mode to use:

```rust
fn detect_execution_mode(skill_root: &PathBuf, wasm_override: Option<String>) 
    -> Result<ExecutionMode, OpenSkillError>
```

**Detection Priority**:
1. **WASM Override**: If `wasm_module` specified in ExecutionOptions, use WASM
2. **WASM Module**: Look for `wasm/skill.wasm`, `skill.wasm`, `module.wasm`, or any `.wasm` file
3. **Native Script**: Look for `.py` or `.sh` files in `scripts/` directory
4. **Error**: If neither found, return error

**Skill Directory Structure**:

```
my-skill/
├── SKILL.md              # Tier 1 & 2: Metadata + Instructions
├── wasm/
│   └── skill.wasm        # Tier 3: WASM module (if using WASM)
└── scripts/
    └── process.py        # Tier 3: Native script (if using native)
```

**Note**: A skill can have both WASM and native scripts, but only one will be executed based on detection priority.

---

## Permission Mapping

Both sandbox environments use the same `allowed-tools` → capabilities mapping:

| allowed-tools | WASM Capability | Native Seatbelt |
|---------------|----------------|-----------------|
| `Read`, `Grep`, `Glob`, `LS` | Filesystem read (preopened dirs) | Filesystem read (profile paths) |
| `Write` | Filesystem write (preopened dirs) | Filesystem write (profile paths) |
| `WebSearch`, `Fetch` | Network access (host allowlist) | Network access (profile allows) |
| `Bash`, `Terminal` | Process spawning (if supported) | Process spawning (profile allows) |

**Empty `allowed-tools`**: Means all tools are allowed (no restrictions)

**Risky Tools**: Tools like `Write`, `Bash`, `Terminal`, `WebSearch` may trigger permission prompts via the `PermissionManager` callback before execution.

---

## Summary of File I/O Operations

### Code Review (WASM):
1. **Discovery**: Read `SKILL.md` (frontmatter only) - Tier 1
2. **Activation**: Use already-parsed `SKILL.md` (full content) - Tier 2
3. **Execution**: Read `wasm/skill.wasm` (binary, on-demand) - Tier 3

### PDF (Native Python):
1. **Discovery**: Read `SKILL.md` (frontmatter only) - Tier 1
2. **Activation**: Use already-parsed `SKILL.md` (full content) - Tier 2
3. **Execution**: Read `scripts/extract_form_field_info.py` (source, on-demand, not in context) - Tier 3

Both workflows follow the **progressive disclosure pattern**: metadata → instructions → resources, with resources loaded only when needed for execution. The source code of scripts is never loaded into the agent's context window - only their outputs are returned.
