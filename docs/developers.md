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

// Create runtime and discover skills
let mut runtime = OpenSkillRuntime::new();
runtime.discover_skills()?;

// Execute a skill
let result = runtime.execute_skill(
    "my-skill",
    ExecutionOptions {
        input: Some(json!({"input": "data"})),
        timeout_ms: Some(5000),
        ..Default::default()
    }
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

## CLI

The `openskills` binary provides discovery, activation, execution, and validation tooling:

```bash
# Discover skills from standard locations
openskills discover

# List skills in a directory
openskills list --dir ./skills

# Validate a skill directory
openskills validate ./skills/my-skill --warnings

# Analyze token usage
openskills analyze ./skills/my-skill
```

## Core Concepts

### Skill Discovery

Skills are discovered from directories containing `SKILL.md` files. The runtime scans for skills and loads metadata (name, description) first.

#### System Prompt Injection

To help the model discover skills, inject skill metadata into the system prompt:

```rust
let mut runtime = OpenSkillRuntime::new();
runtime.discover_skills()?;

// Get formatted metadata for system prompt
let system_prompt = format!(
    "{}\n\n{}",
    base_system_prompt,
    runtime.get_system_prompt_metadata()
);

// Or get JSON format for programmatic use
let metadata_json = runtime.get_system_prompt_metadata_json()?;

// Or get compact summary for token-constrained contexts
let summary = runtime.get_system_prompt_summary();
// Returns: "Skills: code-review, test-generator (2 total)"
```

**Available methods:**
- `get_system_prompt_metadata()` - Human-readable formatted text
- `get_system_prompt_metadata_json()` - JSON format for programmatic use
- `get_system_prompt_summary()` - Compact one-line summary

### Validation API

You can validate a skill directory or estimate token usage directly from Rust:

```rust
use openskills_runtime::OpenSkillRuntime;

// Validate skill format and structure
let validation = OpenSkillRuntime::validate_skill_directory("./skills/my-skill");
if !validation.errors.is_empty() {
    eprintln!("Validation errors: {:?}", validation.errors);
} else {
    println!("✅ Validation passed");
    println!("  Errors: {}", validation.stats.error_count);
    println!("  Warnings: {}", validation.stats.warning_count);
}

// Analyze token usage
let analysis = OpenSkillRuntime::analyze_skill_directory("./skills/my-skill");
println!("Token Analysis:");
println!("  Tier 1 (Metadata): ~{} tokens", analysis.tier1_tokens);
println!("  Tier 2 (Instructions): ~{} tokens", analysis.tier2_tokens);
println!("  Total: ~{} tokens", analysis.total_tokens);
```

**CLI Usage:**

```bash
# Validate a skill
openskills validate ./skills/my-skill

# Validate with warnings
openskills validate ./skills/my-skill --warnings

# Analyze token usage
openskills analyze ./skills/my-skill

# Analyze with JSON output
openskills analyze ./skills/my-skill --format json
```

### Progressive Disclosure

1. **Tier 1 (Metadata)**: Name and description loaded at startup
2. **Tier 2 (Instructions)**: Full SKILL.md content loaded when skill is activated
3. **Tier 3 (Resources)**: Supporting files and resources loaded on demand

### Execution Model

Skills are executed in a secure sandbox environment. The runtime handles all
security and isolation automatically. Skill authors only need to focus on
writing clear instructions.

#### Context Forking

Skills with `context: fork` in their manifest execute in isolated contexts where intermediate outputs are captured separately. Only summaries are returned to the parent context, preventing context pollution:

```rust
use openskills_runtime::{OpenSkillRuntime, ExecutionContext, ExecutionOptions};

let mut runtime = OpenSkillRuntime::new();
let main_context = ExecutionContext::new();

// Execute skill with context management
// If skill has context: fork, it automatically isolates execution
let result = runtime.execute_skill_with_context(
    "explorer-skill",
    ExecutionOptions::default(),
    &main_context
)?;

// For forked skills, result.output contains only the summary
// Intermediate outputs are captured but not returned
println!("Summary: {}", result.output["summary"]);
```

**Manual context management:**

```rust
// Create and fork contexts manually
let main = ExecutionContext::new();
let fork = main.fork();

// Record outputs in forked context
fork.record_output(OutputType::Stdout, "intermediate output".to_string());

// Generate summary from forked context
let summary = fork.summarize();
```

### Permissions

Permissions are enforced based on the skill's `allowed-tools` configuration:
- Filesystem access (read/write paths)
- Network access (domain allowlist)
- Environment variables
- Side effects (writes, executes)

#### Ask-Before-Act Permission System

For risky operations (Write, Bash, WebSearch, etc.), you can require user approval before execution:

```rust
use openskills_runtime::{OpenSkillRuntime, CliPermissionCallback};
use std::sync::Arc;

// Enable interactive permission prompts
let mut runtime = OpenSkillRuntime::new()
    .with_permission_callback(Arc::new(CliPermissionCallback));

// Or enable strict mode (deny all by default)
let mut runtime = OpenSkillRuntime::new()
    .with_strict_permissions();

// Execute skill - will prompt for risky operations
let result = runtime.execute_skill("my-skill", options)?;

// Check permission audit log
let audit = runtime.get_permission_audit();
for entry in audit {
    println!("{}: {} {} - {:?}", 
        entry.timestamp, 
        entry.skill_id, 
        entry.tool, 
        entry.response
    );
}

// Reset all "allow always" grants
runtime.reset_permission_grants();
```

**Custom Permission Callbacks:**

Implement `PermissionCallback` trait for custom UI (GUI, automated policies, etc.):

```rust
use openskills_runtime::{PermissionCallback, PermissionRequest, PermissionResponse, OpenSkillError};

struct MyPermissionCallback;

impl PermissionCallback for MyPermissionCallback {
    fn request_permission(
        &self,
        request: &PermissionRequest,
    ) -> Result<PermissionResponse, OpenSkillError> {
        // Your custom logic here
        // Return: AllowOnce, AllowAlways, or Deny
        Ok(PermissionResponse::AllowOnce)
    }
}
```

**Built-in callbacks:**
- `CliPermissionCallback` - Interactive terminal prompts
- `DenyAllCallback` - Strict mode (all denied)

## API Reference

### Rust API

#### `OpenSkillRuntime`

Main runtime interface.

```rust
impl OpenSkillRuntime {
    // Construction
    pub fn new() -> Self;
    pub fn from_config(config: RuntimeConfig) -> Self;
    pub fn with_project_root<P: AsRef<Path>>(root: P) -> Self;
    pub fn with_custom_directories<P: AsRef<Path>>(self, dirs: Vec<P>) -> Self;
    pub fn with_permission_callback(self, callback: Arc<dyn PermissionCallback>) -> Self;
    pub fn with_strict_permissions(self) -> Self;
    
    // Discovery
    pub fn discover_skills(&mut self) -> Result<Vec<SkillDescriptor>, OpenSkillError>;
    pub fn load_from_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<Vec<SkillDescriptor>, OpenSkillError>;
    pub fn list_skills(&self) -> Vec<SkillDescriptor>;
    
    // System prompt helpers
    pub fn get_system_prompt_metadata(&self) -> String;
    pub fn get_system_prompt_metadata_json(&self) -> Result<String, OpenSkillError>;
    pub fn get_system_prompt_summary(&self) -> String;
    
    // Activation
    pub fn activate_skill(&self, skill_id: &str) -> Result<LoadedSkill, OpenSkillError>;
    
    // Execution
    pub fn execute_skill(&mut self, skill_id: &str, options: ExecutionOptions) -> Result<ExecutionResult, OpenSkillError>;
    pub fn execute_skill_with_context(&mut self, skill_id: &str, options: ExecutionOptions, parent_context: &ExecutionContext) -> Result<ExecutionResult, OpenSkillError>;
    
    // Permissions
    pub fn get_permission_audit(&self) -> Vec<PermissionAuditEntry>;
    pub fn reset_permission_grants(&self);
    
    // Validation (static methods)
    pub fn validate_skill_directory<P: AsRef<Path>>(path: P) -> ValidationResult;
    pub fn analyze_skill_directory<P: AsRef<Path>>(path: P) -> TokenAnalysis;
}
```

#### `ExecutionOptions`

```rust
pub struct ExecutionOptions {
    pub timeout_ms: Option<u64>,
    pub memory_mb: Option<u64>,
    pub input: Option<Value>,
    pub wasm_module: Option<String>,
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
- `PermissionDenied`: Operation not allowed (user denied permission or strict mode)
- `Timeout`: Execution exceeded time limit
- `ExecutionFailure`: Skill execution failed
- `WasmError`: WASM module loading or execution error
- `ValidationError`: Skill format validation failed

## Building Skills

### Skill Structure

```
my-skill/
├── SKILL.md           # Required: YAML frontmatter + Markdown
├── examples/          # Optional: Example files
├── references/        # Optional: Reference documentation
└── README.md          # Optional: Additional documentation
```

Most skills are instructional and only need `SKILL.md`. Supporting files
can be referenced in the instructions but are loaded on-demand by the runtime.

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

### Instructional Skills

Most skills are instructional - they provide clear guidance to the AI on how to
perform specific tasks. The skill's instructions in the Markdown body tell the
AI what to do when the skill is activated.

The runtime handles all security and sandboxing automatically. Skill authors
don't need to know about the underlying execution environment.

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

**Timeout errors**: Execution exceeded time limit (check skill complexity)

## Further Reading

- [Specification](spec.md) - Complete runtime specification
- [Contributing Guide](contributing.md) - How to contribute
- [Architecture](architecture.md) - Detailed architecture documentation
