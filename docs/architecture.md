# OpenSkills Architecture

This document describes the internal architecture of OpenSkills Runtime.

## Overview

OpenSkills is built with Rust as the core runtime, providing a WASM-based sandbox for executing Claude Skills. The architecture emphasizes:

- **Security**: WASM sandboxing with capability-based permissions
- **Performance**: Efficient skill discovery and execution
- **Compatibility**: 100% Claude Skills format compatibility
- **Extensibility**: Language bindings for multiple ecosystems

## Core Components

### 1. Skill Registry (`registry.rs`)

Responsible for:
- Scanning directories for skills
- Loading skill metadata (Tier 1)
- Caching skill descriptors
- Validating skill structure

**Key Types:**
- `SkillRegistry`: Main registry interface
- `Skill`: Internal skill representation
- `SkillDescriptor`: Public skill metadata

### 2. Manifest Parser (`manifest.rs`, `skill_parser.rs`)

Responsible for:
- Parsing YAML frontmatter from SKILL.md
- Validating required and optional fields
- Extracting Markdown instructions
- Enforcing format constraints

**Key Types:**
- `SkillManifest`: Parsed manifest data
- `ExecutionKind`: Execution type (Wasm/Http/Local)
- `Permissions`: Permission configuration

### 3. WASM Runner (`wasm_runner.rs`)

Responsible for:
- Loading WASM modules via Wasmtime
- Configuring WASI capabilities
- Enforcing permissions
- Capturing stdout/stderr
- Timeout enforcement

**Key Features:**
- WASI preview1 compatibility
- Capability-based filesystem access
- Network allowlist enforcement
- Deterministic execution support

### 4. Permission System (`permissions.rs`)

Responsible for:
- Mapping `allowed-tools` to WASI capabilities
- Enforcing filesystem permissions
- Network access control
- Environment variable filtering

**Capability Mapping:**
- `Read` → Filesystem read access
- `Write` → Filesystem write access
- `Bash` → Full filesystem access
- `WebSearch` → Network access

### 5. Audit System (`audit.rs`)

Responsible for:
- Recording execution traces
- Hashing inputs/outputs
- Tracking resource usage
- Generating audit records

**Audit Record Fields:**
- Skill ID and version
- Input/output hashes
- Execution timing
- Permissions used
- Exit status

## Data Flow

### Skill Discovery

```
1. Registry.scan()
   └─> Read directory entries
   └─> For each directory:
       ├─> Check for SKILL.md
       ├─> Parse YAML frontmatter (Tier 1)
       ├─> Validate format
       └─> Cache SkillDescriptor

2. Runtime.load_skills()
   └─> Call Registry.scan()
   └─> Return Vec<SkillDescriptor>
```

### Skill Execution

```
1. Runtime.execute_skill()
   ├─> Load full skill (Tier 2) if not cached
   ├─> Validate input against schema
   ├─> Create PermissionEnforcer
   └─> Call executor based on ExecutionKind

2. Executor.execute()
   ├─> For Wasm:
   │   ├─> Load WASM module
   │   ├─> Configure WASI context
   │   ├─> Set up permissions
   │   ├─> Execute with timeout
   │   └─> Capture output
   │
   ├─> For Http:
   │   ├─> Check network permissions
   │   ├─> Make HTTP request
   │   └─> Parse response
   │
   └─> For Local:
       └─> (Currently disabled)

3. Post-execution
   ├─> Validate output against schema
   ├─> Create audit record
   └─> Return ExecutionResult
```

## Progressive Disclosure

The runtime implements three-tier loading:

1. **Tier 1 (Metadata)**: Loaded at startup
   - Only `name` and `description` from YAML frontmatter
   - Minimal token cost
   - Fast discovery

2. **Tier 2 (Instructions)**: Loaded on activation
   - Full SKILL.md content (Markdown body)
   - Loaded when skill is selected/activated
   - Moderate token cost

3. **Tier 3 (Resources)**: Loaded on demand
   - Scripts, WASM modules, data files
   - Loaded only when needed
   - Zero token cost until output enters context

## Security Model

### WASM Sandbox

- **Isolation**: Each execution runs in isolated WASM instance
- **Capabilities**: Filesystem access via WASI preopens
- **Network**: Domain allowlist enforcement
- **Timeouts**: Epoch interruption for timeout enforcement
- **Memory**: Configurable memory limits

### Permission Enforcement

Permissions are enforced at multiple levels:

1. **Manifest Level**: `allowed-tools` defines capabilities
2. **Runtime Level**: PermissionEnforcer validates operations
3. **WASI Level**: WASI context configured with minimal capabilities

## Error Handling

Error types are defined in `errors.rs`:

- `SkillNotFound`: Skill ID not in registry
- `InvalidManifest`: SKILL.md parsing/validation failed
- `PermissionDenied`: Operation not allowed
- `Timeout`: Execution exceeded time limit
- `ExecutionFailure`: WASM execution error

All errors implement `std::error::Error` and are serializable.

## Performance Considerations

### Caching

- Skill metadata cached after first load
- WASM modules can be cached (future optimization)
- Registry uses HashMap for O(1) lookups

### Lazy Loading

- Instructions loaded only when needed
- Resources loaded on demand
- Reduces initial memory footprint

### Concurrent Execution

- Runtime is not thread-safe (use Mutex if needed)
- WASM execution is single-threaded
- Future: Consider async execution for I/O

## Extension Points

### Custom Executors

Implement `Executor` trait for custom execution modes:

```rust
pub trait Executor {
    fn execute(
        &self,
        skill: &Skill,
        input: Value,
        ctx: &ExecutionContext
    ) -> Result<ExecutionArtifacts, OpenSkillError>;
}
```

### Custom Audit Sinks

Implement `AuditSink` trait for custom audit logging:

```rust
pub trait AuditSink: Send + Sync {
    fn record(&self, audit: &AuditRecord);
}
```

### Language Bindings

Bindings follow a common pattern:

1. Wrap `OpenSkillRuntime` in language-specific types
2. Convert between language types and Rust types
3. Handle errors appropriately
4. Provide idiomatic API for target language

## WASI Linker Integration (Current Status)

The WASM runner currently has a placeholder for WASI linker integration. To complete WASM execution:

**Current State**: Linker is created but WASI functions are not registered, preventing WASI modules from instantiating.

**Required**: Add WASI preview1 functions to the linker using wasmtime-wasi 20.0.2 API.

**Options**:
1. **Component Model**: Use `wasmtime::component::Linker` with `wasmtime_wasi::add_to_linker_sync`
2. **Preview1 API**: Use `wasmtime_wasi::preview1::add_to_linker` with regular modules
3. **Manual Registration**: Manually register WASI functions (more control, more code)

**Location**: `runtime/src/wasm_runner.rs` lines ~118-160

See wasmtime-wasi 20.0.2 documentation for the correct API pattern.

## Future Improvements

- **Async Execution**: Support async/await for I/O operations
- **WASM Module Caching**: Cache compiled WASM modules
- **Component Model**: Full WASI component model support
- **Distributed Execution**: Support for remote skill execution
- **Skill Dependencies**: Support for skill-to-skill dependencies

## Related Documentation

- [Specification](spec.md) - Runtime specification
- [Developer Guide](developers.md) - Using the runtime
- [Contributing Guide](contributing.md) - Contributing code
