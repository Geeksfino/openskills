# OpenSkills Runtime Specification (v0.2)

OpenSkills is a Claude Skills compatible runtime that uses **OS-level sandboxing (macOS seatbelt + Linux Landlock) as the primary execution method**, with **experimental WASM-based sandboxing** available for specific use cases.

## Claude Skills Conformance

This runtime implements the [Claude Code Agent Skills specification](https://code.claude.com/docs/en/skills).

### Skill Format

Skills are directories containing a `SKILL.md` file with YAML frontmatter and Markdown instructions:

```
my-skill/
├── SKILL.md           # Required: YAML frontmatter + Markdown instructions
├── scripts/           # Optional: Supporting scripts
├── examples/          # Optional: Example files
├── wasm/              # Optional: WASM modules for sandboxed execution
│   └── skill.wasm
└── README.md          # Optional: Documentation
```

### SKILL.md Format

```yaml
---
name: my-skill
description: What the skill does and when to use it. Claude uses this to decide when to apply the Skill.
allowed-tools: Read, Write, Bash
model: claude-sonnet-4-20250514
context: fork
agent: Explore
user-invocable: true
---

# Instructions

Markdown content that Claude follows when the Skill is active.
Reference supporting files as needed.
```

### Required Fields

| Field | Constraints |
|-------|-------------|
| `name` | Lowercase letters, numbers, hyphens only. Max 64 characters. Must match directory name. |
| `description` | Non-empty. Max 1024 characters. No XML tags. |

### Optional Fields

| Field | Description |
|-------|-------------|
| `allowed-tools` | Tools Claude can use without permission when Skill is active. |
| `model` | Model to use (e.g., `claude-sonnet-4-20250514`). |
| `context` | Set to `fork` for isolated sub-agent context. |
| `agent` | Agent type when `context: fork` (e.g., `Explore`, `Plan`). |
| `hooks` | Lifecycle hooks (`PreToolUse`, `PostToolUse`, `Stop`). |
| `user-invocable` | Whether Skill appears in slash command menu (default: true). |

## Discovery Locations

Skills are discovered from (in order, later overrides earlier):

1. **Personal**: `~/.claude/skills/`
2. **Project**: `.claude/skills/` (relative to project root)
3. **Nested**: Any `.claude/skills/` in subdirectories (monorepo support)
4. **Custom**: Agent-configured directories (via `with_custom_directory` or `RuntimeConfig`)

### Agent-Configured Directories

Agents can configure custom skill directories in addition to or instead of standard locations:

- **Multiple directories**: Agents can specify multiple custom directories
- **Override standard locations**: Agents can disable standard location discovery
- **Flexible configuration**: Support for builder pattern or configuration struct

This allows agents to:
- Load skills from agent-specific directories
- Share skills across multiple agents
- Override standard Claude Skills locations when needed

## Progressive Disclosure

1. **Discovery**: At startup, only `name` and `description` are loaded.
2. **Activation**: When a Skill is triggered, full `SKILL.md` content is loaded.
3. **Execution**: Supporting files and WASM modules are loaded on demand.

## WASM Sandbox (OpenSkills Extension) - Experimental

**Status**: Experimental feature. Native scripts via OS-level sandboxing (macOS seatbelt + Linux Landlock) are the primary execution method.

OpenSkills provides **experimental** WASM/WASI sandboxing as an optional execution method for specific use cases.

### Why WASM? (Long-Term Vision)

WASM support is provided for use cases requiring:
- **Determinism**: Same input → same output, critical for audit and compliance
- **Fast startup**: Millisecond-level startup times
- **Cross-platform consistency**: Same sandbox behavior on macOS, Linux, Windows
- **Capability-based security**: Fine-grained control via WASI capabilities
- **Isolation**: Strong memory and execution isolation

**Best for**: Policy logic, orchestration, validation, scoring, reasoning glue.

**Not suitable for**: Full Python ecosystem, ML libraries (NumPy, PyTorch), native libraries, OS-native behaviors.

See [README.md](../README.md#wasm-support-long-term-vision) for detailed discussion of WASM's role and limitations.

### WASM Execution Model (Experimental)

**Note**: WASM execution is experimental. Most skills use native Python/shell scripts.

Skills may optionally include WASM modules for sandboxed script execution:

```
my-skill/
├── SKILL.md
└── wasm/
    └── skill.wasm     # WASI-compatible module (optional)
```

If a WASM module is present, the runtime:
1. Loads the WASM module using Wasmtime
2. Configures WASI capabilities based on `allowed-tools`
3. Preopens filesystem paths with appropriate permissions
4. Executes with timeout and memory limits
5. Captures stdout/stderr for audit

**If no WASM module is present**, the runtime uses native Python/shell scripts via OS-level sandboxing (seatbelt on macOS).

### Native Script Dependency Model

Native scripts execute with the resolved host interpreter (for Python: `python3` from `PATH` first, then platform fallback locations).

- The runtime does **not** install third-party packages (no automatic `pip`/venv provisioning).
- Skills that import third-party modules (for example `yaml`) require that the selected interpreter already has those dependencies.
- For portability, prefer stdlib-only scripts where practical, or document required dependencies for deployers.

### Capability Mapping

`allowed-tools` values are mapped to WASI capabilities:

| Tool | WASI Capability |
|------|-----------------|
| `Read`, `Grep`, `Glob`, `LS` | Filesystem read |
| `Write`, `Edit`, `MultiEdit` | Filesystem write |
| `Bash`, `Terminal` | Full filesystem |
| `WebSearch`, `Fetch` | Network access |

### WASM Module Interface

WASM modules should be WASI-compatible. The runtime provides:

**Environment Variables:**
- `SKILL_ID`: Skill identifier
- `SKILL_NAME`: Skill name from manifest
- `SKILL_INPUT`: JSON input data
- `TIMEOUT_MS`: Execution timeout
- `RANDOM_SEED`: Deterministic seed (if configured)

**Preopened Directories:**
- `/skill`: Skill root directory (read-only)
- Additional paths based on `allowed-tools`

**Output:**
- Write JSON to stdout for structured output
- stderr is captured for logging/debugging

### Constraints

```yaml
# Defaults
timeout_ms: 30000    # 30 seconds
memory_mb: 128       # 128 MB
```

## Audit Model

Every execution produces an audit record:

```
skill_id: string
version: string
input_hash: sha256
output_hash: sha256
start_time_ms: timestamp
duration_ms: number
permissions_used: [string]
exit_status: success | failed | timeout
stdout: string
stderr: string
```

## API

### Rust

```rust
use openskills_runtime::{OpenSkillRuntime, ExecutionOptions};

// Discover from standard locations
let mut runtime = OpenSkillRuntime::new();
let skills = runtime.discover_skills()?;

// List available skills (progressive disclosure)
for skill in runtime.list_skills() {
    println!("{}: {}", skill.id, skill.description);
}

// Activate a skill (load full instructions)
let loaded = runtime.activate_skill("my-skill")?;
println!("{}", loaded.instructions);

// Execute WASM module (if present)
let result = runtime.execute_skill("my-skill", ExecutionOptions::default())?;
println!("{}", result.output);
```

### CLI

```bash
# Discover skills from standard locations
openskills discover

# List skills from a specific directory
openskills list --dir ./skills

# Activate (load full content)
openskills activate my-skill --json

# Execute WASM module
openskills execute my-skill --input '{"query": "hello"}'
```

## Compatibility Notes

### What Works

- Full SKILL.md format support (YAML frontmatter + Markdown)
- All metadata fields (`allowed-tools`, `model`, `context`, `agent`, `hooks`, `user-invocable`)
- Standard discovery paths
- Progressive disclosure
- Validation of name/description constraints

### What's Different

- **Sandboxing**: WASM/WASI instead of seatbelt/seccomp
- **Script execution**: Scripts must be compiled to WASM or run via WASM interpreters
- **Environment**: WASI environment instead of native OS

### Migration from Native Scripts

Skills with native scripts (`.sh`, `.py`) need WASM-compatible alternatives:

1. **Compile to WASM**: Use Rust, Go, or other languages with WASM targets
2. **Use WASM interpreters**: Ship a WASM-compiled interpreter
3. **Keep instructional**: Most skills are instructional and don't need modification

## Error Taxonomy

- `SkillNotFound`: Skill ID not in registry
- `InvalidManifest`: SKILL.md parsing or validation failed
- `PermissionDenied`: Operation not allowed by skill configuration
- `Timeout`: Execution exceeded time limit
- `ToolNotAllowed`: Tool not in `allowed-tools` list
- `WasmError`: WASM module loading or execution failed
