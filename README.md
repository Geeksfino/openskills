# OpenSkills

A **Claude Skills compatible runtime** with WASM-based sandboxing. OpenSkills implements the [Claude Code Agent Skills specification](https://code.claude.com/docs/en/skills), providing a secure, cross-platform runtime for executing skills.

## Features

- ✅ **100% Claude Skills Compatible**: Full SKILL.md format support
- 🔒 **WASM Sandbox**: Secure execution via WASI instead of OS-level sandboxing
- 📊 **Progressive Disclosure**: Efficient tiered loading (metadata → instructions → resources)
- 🔌 **Multi-Language**: Rust core with TypeScript and Python bindings
- 🛡️ **Capability-Based Security**: Fine-grained permissions via WASI

## Quick Start

### Installation

```bash
# Rust (from source)
git clone <repository-url>
cd openskills/runtime
cargo build --release

# TypeScript
npm install @openskills/runtime

# Python
pip install openskills
```

### Usage

```rust
use openskills_runtime::{OpenSkillRuntime, ExecutionOptions};
use serde_json::json;

let mut runtime = OpenSkillRuntime::new("./skills");
runtime.load_skills()?;

let result = runtime.execute_skill(
    "my-skill",
    json!({"input": "data"}),
    ExecutionOptions { timeout_ms: Some(5000) }
)?;
```

See [Developer Guide](docs/developers.md) for detailed usage examples.

## Architecture

OpenSkills uses a Rust core runtime with language bindings:

```
┌────────────────────┐
│  Your Application  │
│  (TS/Python/Rust)  │
└──────────┬──────────┘
           │
    ┌──────▼──────┐
    │  Bindings   │  (napi-rs / PyO3)
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

## Why WASM Sandbox?

Unlike Claude Code's OS-level sandboxing (seatbelt/seccomp), OpenSkills uses WASM/WASI:

| Aspect | Claude Code | OpenSkills |
|--------|-------------|------------|
| Sandbox | seatbelt/seccomp | WASM/WASI |
| Cross-platform | OS-specific | Identical everywhere |
| Security model | OS capabilities | WASI capabilities |
| Script execution | Native with sandbox | WASM modules |

## Project Structure

```
openskills/
├── runtime/              # Rust core runtime
├── bindings/             # Language bindings
│   ├── ts/              # TypeScript (napi-rs)
│   └── python/           # Python (PyO3)
├── docs/                 # Documentation
│   ├── developers.md     # Developer guide
│   ├── contributing.md   # Contributing guide
│   ├── architecture.md   # Architecture details
│   └── spec.md           # Specification
├── examples/             # Example skills
└── scripts/              # Build scripts
```

## Documentation

- **[Developer Guide](docs/developers.md)**: Using OpenSkills in your applications
- **[Contributing Guide](docs/contributing.md)**: How to contribute to OpenSkills
- **[Architecture](docs/architecture.md)**: Internal architecture and design
- **[Specification](docs/spec.md)**: Complete runtime specification

See [docs/README.md](docs/README.md) for documentation index.

## Building

```bash
# Build everything
./scripts/build_all.sh

# Build runtime only
./scripts/build_runtime.sh

# Build bindings
./scripts/build_bindings.sh
```

## Status

- ✅ **Rust Runtime**: Fully functional
- 🚧 **TypeScript Bindings**: Build issues (napi linking)
- ✅ **Python Bindings**: Ready (requires Python ≤3.12 or compatibility flag)
- 🚧 **WASM Execution**: WASI linker integration pending

## License

MIT
