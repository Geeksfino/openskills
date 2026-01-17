# Developer Guide

This guide is for developers who want to use OpenSkills Runtime in their applications.

## Overview

OpenSkills Runtime is a Claude Skills-compatible runtime that executes skills in a WASM-based sandbox. It provides a Rust core with TypeScript and Python bindings.

## Architecture

```
┌─────────────────────┐
│  Your Application  │
└──────────┬──────────┘
           │
    ┌──────▼──────┐
    │  Bindings   │  (TypeScript/Python/Rust)
    └──────┬──────┘
           │
    ┌──────▼──────┐
    │ Rust Core   │  (openskills-runtime)
    └──────┬──────┘
           │
    ┌──────▼──────┐
    │  Wasmtime   │  (WASM execution)
    └─────────────┘
```

## Quick Start

### Rust

```rust
use openskills_runtime::{OpenSkillRuntime, ExecutionOptions};
use serde_json::json;

// Create runtime
let mut runtime = OpenSkillRuntime::new("./skills");

// Load skills
runtime.load_skills()?;

// Execute a skill
let result = runtime.execute_skill(
    "my-skill",
    json!({"input": "data"}),
    ExecutionOptions { timeout_ms: Some(5000) }
)?;

println!("Output: {}", result.output);
```

### TypeScript

```typescript
import { OpenSkillRuntime } from '@openskills/runtime';

const runtime = new OpenSkillRuntime('./skills');
const skills = runtime.loadSkills();

const result = await runtime.executeSkill(
  'my-skill',
  { input: 'data' },
  { timeoutMs: 5000 }
);

console.log(result.output);
```

### Python

```python
from openskills import OpenSkillRuntime

runtime = OpenSkillRuntime('./skills')
skills = runtime.load_skills()

result = runtime.execute_skill(
    'my-skill',
    {'input': 'data'},
    timeout_ms=5000
)

print(result['output'])
```

## Core Concepts

### Skill Discovery

Skills are discovered from directories containing `SKILL.md` files. The runtime scans for skills and loads metadata (name, description) first.

### Progressive Disclosure

1. **Tier 1 (Metadata)**: Name and description loaded at startup
2. **Tier 2 (Instructions)**: Full SKILL.md content loaded when skill is activated
3. **Tier 3 (Resources)**: Scripts, WASM modules, and other resources loaded on demand

### Execution Model

- **WASM**: Primary execution mode (sandboxed, secure)
- **HTTP**: Optional external API calls (with network permissions)
- **Local**: Optional native execution (requires explicit permission)

### Permissions

Permissions are enforced based on the skill's `allowed-tools` configuration:
- Filesystem access (read/write paths)
- Network access (domain allowlist)
- Environment variables
- Side effects (writes, executes)

## API Reference

### Rust API

#### `OpenSkillRuntime`

Main runtime interface.

```rust
impl OpenSkillRuntime {
    pub fn new<P: AsRef<Path>>(skills_dir: P) -> Self;
    pub fn load_skills(&mut self) -> Result<Vec<SkillDescriptor>, OpenSkillError>;
    pub fn execute_skill(
        &mut self,
        skill_id: &str,
        input: Value,
        options: ExecutionOptions
    ) -> Result<ExecutionResult, OpenSkillError>;
}
```

#### `ExecutionOptions`

```rust
pub struct ExecutionOptions {
    pub timeout_ms: Option<u64>,
}
```

#### `ExecutionResult`

```rust
pub struct ExecutionResult {
    pub output: Value,
    pub stdout: String,
    pub stderr: String,
    pub audit: AuditRecord,
}
```

### Error Handling

All operations return `Result<T, OpenSkillError>`. Error types:

- `SkillNotFound`: Skill ID not found
- `InvalidManifest`: SKILL.md parsing failed
- `PermissionDenied`: Operation not allowed
- `Timeout`: Execution exceeded time limit
- `ExecutionFailure`: WASM execution failed

## Building Skills

### Skill Structure

```
my-skill/
├── SKILL.md           # Required: YAML frontmatter + Markdown
├── wasm/              # Optional: WASM modules
│   └── skill.wasm
├── scripts/           # Optional: Supporting scripts
└── resources/         # Optional: Data files
```

### SKILL.md Format

```yaml
---
name: my-skill
description: What it does and when to use it
allowed-tools: Read, Write
---

# Instructions

Markdown content here...
```

See [spec.md](spec.md) for complete format specification.

### Creating WASM Modules

WASM modules must be WASI-compatible. Example using Rust:

```rust
// src/lib.rs
#[no_mangle]
pub extern "C" fn execute(input_ptr: *const u8, input_len: usize) -> *const u8 {
    // Read input from memory
    // Process
    // Return output pointer
}
```

Compile with:
```bash
cargo build --target wasm32-wasi --release
```

## Best Practices

1. **Error Handling**: Always handle `OpenSkillError` appropriately
2. **Timeouts**: Set reasonable timeouts for skill execution
3. **Permissions**: Only grant necessary permissions
4. **Audit Logging**: Use audit records for debugging and compliance
5. **Resource Management**: Clean up resources after execution

## Examples

See `examples/skills/` for example skill implementations.

## Troubleshooting

### Common Issues

**Skill not found**: Ensure skill directory name matches `name` in SKILL.md

**Permission denied**: Check `allowed-tools` in skill manifest

**WASM execution fails**: Verify WASM module is WASI-compatible

**Timeout errors**: Increase timeout or optimize skill execution

## Further Reading

- [Specification](spec.md) - Complete runtime specification
- [Contributing Guide](contributing.md) - How to contribute
- [Architecture](architecture.md) - Detailed architecture documentation
