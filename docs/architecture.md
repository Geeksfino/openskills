# OpenSkills Architecture

This document describes the internal architecture of OpenSkills Runtime.

## Overview

OpenSkills is built with Rust as the core runtime, providing native OS-level sandboxing (macOS seatbelt + Linux Landlock) as the primary execution method, with experimental WASM-based sandboxing available for specific use cases. The architecture emphasizes:

- **Security**: OS-level sandboxing (macOS seatbelt + Linux Landlock) as primary, experimental WASM sandboxing with capability-based permissions
- **Performance**: Efficient skill discovery and execution
- **Compatibility**: 100% Claude Skills format compatibility
- **Extensibility**: Language bindings for multiple ecosystems

**Note**: WASM sandboxing is experimental and not the primary execution method. Most skills use native Python and shell scripts via seatbelt sandboxing.

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

### 3. WASM Runner (`wasm_runner.rs`) - Experimental

**Status**: Experimental feature, not the primary execution method.

Responsible for:
- Loading WASM modules via Wasmtime
- Configuring WASI capabilities
- Enforcing permissions
- Capturing stdout/stderr
- Timeout enforcement

**Key Features:**
- WASI 0.3 (WASIp3) component-model-only execution
- Capability-based filesystem access
- Network allowlist enforcement
- Deterministic execution support

**Use Cases**: Deterministic logic, policy enforcement, orchestration. Not suitable for full Python ecosystem or native libraries.

### 4. Native Runner (`native_runner.rs`) - Primary

**Status**: Primary execution method, production-ready.

Responsible for:
- Executing native Python and shell scripts with OS-level sandboxing (macOS seatbelt + Linux Landlock)
- Building sandbox profiles from permissions
- Capturing stdout/stderr
- Timeout enforcement

**Use Cases**: Full Python ecosystem, native libraries, ML/quant code, legacy skills. This is the recommended approach for most skills.

### 5. Permission System (`permissions.rs`)

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

### 6. Audit System (`audit.rs`)

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
   └─> Auto-detect execution mode

2. Executor.execute()
   ├─> For Native (macOS/Linux, primary):
   │   ├─> Build sandbox profile (seatbelt/Landlock)
   │   ├─> Execute Python/shell script
   │   ├─> Enforce filesystem/network permissions
   │   └─> Capture output
   │
   └─> For WASM (experimental):
       ├─> Load WASM module
       ├─> Configure WASI context
       ├─> Set up permissions
       ├─> Execute with timeout
       └─> Capture output

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

### Native OS Sandboxing (macOS/Linux) - Primary

**Status**: Production-ready, primary execution method.

- **Isolation**: Script execution is restricted by OS-level sandbox (seatbelt on macOS, Landlock on Linux)
- **Filesystem**: Subpath read/write allowlists from `allowed-tools`
- **Network**: Allowed only when `WebSearch`/`Fetch` are enabled
- **Timeouts**: Epoch interruption for timeout enforcement
- **Memory**: Configurable memory limits

### WASM Sandbox - Experimental

**Status**: Experimental feature, not the primary execution method.

- **Isolation**: Each execution runs in isolated WASM instance
- **Capabilities**: Filesystem access via WASI preopens
- **Network**: Domain allowlist enforcement

**Limitations**: Cannot access full Python ecosystem, native libraries, or OS-native behaviors. Best for deterministic logic and policy enforcement.

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

## WASI Execution Model (Experimental)

**Status**: Experimental feature. Native scripts are the primary execution method.

OpenSkills executes **only WASI 0.3 (WASIp3) components** via Wasmtime's component model when WASM modules are present.

- Legacy "core module" WASM artifacts are **rejected**.
- WASM execution is optional - most skills use native Python/shell scripts.

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
