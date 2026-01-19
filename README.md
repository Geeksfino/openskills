# OpenSkills - Give Your Agents Skills

[English](README.md) | [中文](README.zh.md)

A **Claude Skills compatible runtime** with **dual sandboxing**: WASM-based sandboxing for cross-platform security, plus **macOS seatbelt** for native Python and shell scripts. OpenSkills implements the [Claude Code Agent Skills specification](https://code.claude.com/docs/en/skills), providing a secure, flexible runtime for executing skills in **any agent framework**.

## Philosophy

OpenSkills is **syntactically 100% compatible** with Claude Skills, meaning any skill that follows the Claude Skills format (SKILL.md with YAML frontmatter) will work with OpenSkills. What makes OpenSkills unique is its **dual sandboxing approach**:

- **WASM/WASI sandboxing** for cross-platform security and consistency
- **macOS seatbelt sandboxing** for native Python and shell scripts

This combination provides the best of both worlds: the portability and security of WASM, plus the flexibility of native execution on macOS. OpenSkills can be integrated into **any agent framework** (LangChain, Vercel AI SDK, custom frameworks) to give agents access to Claude-compatible skills.

### Core Design Principles

1. **100% Syntactic Compatibility**: OpenSkills reads and executes skills using the exact same SKILL.md format as Claude Skills. Skills can be shared between Claude Code and OpenSkills without modification.

2. **Dual Sandbox Architecture**: OpenSkills uniquely combines **WASM/WASI 0.3** (component model) with **macOS seatbelt** sandboxing:
   - **WASM/WASI**: Cross-platform security, capability-based permissions, memory safety, deterministic execution
   - **macOS Seatbelt**: Native Python and shell script execution with OS-level sandboxing
   - **Automatic Detection**: Runtime automatically chooses the appropriate sandbox based on skill type
   - **Best of Both Worlds**: WASM for portability and security, seatbelt for native flexibility

3. **JavaScript/TypeScript First**: OpenSkills is optimized for JavaScript/TypeScript-based skills, which can be compiled to WASM components using `javy-codegen` (a Rust library that uses QuickJS). This allows skill writers to use familiar languages and ecosystems, with compilation happening programmatically via the library rather than requiring external CLI tools.

### Target Use Case

OpenSkills is designed for **any agent framework** that needs Claude-compatible skills:

- **Agent Framework Integration**: Works with LangChain, Vercel AI SDK, custom frameworks, or any system that needs tool-like capabilities
- **Enterprise Agents**: Internal skills developed by trusted developers
- **Cross-Platform**: WASM execution works identically on macOS, Linux, Windows
- **Native Flexibility**: macOS seatbelt allows native Python and shell scripts when needed
- **Security & Auditability**: Both sandboxing methods provide strong isolation and audit logging

The dual sandbox approach means you can use WASM for cross-platform skills, or leverage native Python/Shell on macOS when you need access to native libraries or tools.

## Limitations

OpenSkills' WASM-first approach has some limitations compared to native execution:

### Not Supported (Currently)

1. **Native Scripts on Non-macOS**:
   - Native Python and shell scripts are supported only on macOS (seatbelt)
   - Linux seccomp support is planned

2. **Build Workflow Required (for WASM)**:
   - JavaScript/TypeScript skills must be compiled to WASM components before execution
   - Developers need to run `openskills build` to compile source to `wasm/skill.wasm`
   - This adds a build step compared to "drop-in" native scripts

### Why These Limitations Exist

WASM provides strong security and cross-platform consistency, but it requires:
- **Compilation step**: Source code must be compiled to WASM
- **WASI compatibility**: Code must use WASI APIs, not native OS APIs
- **Limited native libraries**: Native Python packages, shell tools, etc. don't work directly

These limitations are acceptable for enterprise use cases where:
- Developers control the skill development process
- Build workflows are standard practice
- Security and cross-platform consistency outweigh convenience

## Roadmap

OpenSkills will evolve to address limitations while maintaining its WASM-first philosophy:

1. **More WASM-Ready Scripts**: We'll provide an expanding library of pre-built WASM components and templates for common tasks, reducing the need for custom compilation.

2. **Native Scripting Support**: Native Python and shell scripts are supported on macOS via seatbelt. Linux seccomp support is planned to complete cross-platform native sandboxing.

3. **Improved Tooling**: Better build tools and templates to make WASM compilation more transparent for developers.

## Features

- ✅ **100% Claude Skills Compatible**: Full SKILL.md format support
- 🔒 **Dual Sandbox Architecture**: WASM/WASI 0.3 + macOS seatbelt (unique in the ecosystem)
- 🧰 **Native Script Support**: Execute Python and shell scripts on macOS via seatbelt
- 🤖 **Any Agent Framework**: Integrate with LangChain, Vercel AI SDK, or custom frameworks
- 📊 **Progressive Disclosure**: Efficient tiered loading (metadata → instructions → resources)
- 🔌 **Multi-Language Bindings**: Rust core with TypeScript and Python bindings
- 🛡️ **Capability-Based Security**: Fine-grained permissions via WASI and seatbelt profiles
- 🏗️ **Build Tool**: `openskills build` for compiling TS/JS to WASM components
- 🌐 **Cross-Platform**: WASM execution is identical on macOS, Linux, Windows

## Quick Start

### Installation

```bash
# Rust (from source)
git clone <repository-url>
cd openskills

# Initialize submodules (required for tests and examples)
git submodule update --init --recursive

cd runtime
cargo build --release

# TypeScript
npm install @finogeek/openskills

# Python
pip install openskills
```

### Building a Skill

OpenSkills uses **`javy-codegen`** (a Rust library) to compile JavaScript/TypeScript to WASM. This approach doesn't require installing the `javy` CLI tool—the compilation happens programmatically using the library.

**Prerequisites**: You need a `plugin.wasm` file (the javy plugin). Build it once using our helper script:

```bash
# Build the javy plugin (one-time setup)
./scripts/build_javy_plugin.sh

# Export the plugin path (or add to your shell profile)
export JAVY_PLUGIN_PATH=/tmp/javy/target/wasm32-wasip1/release/plugin_wizened.wasm
```

**Build a skill**:

```bash
# Build a skill from TypeScript/JavaScript
cd my-skill
openskills build

# This compiles src/index.ts → wasm/skill.wasm using javy-codegen
```

**How it works**:
- OpenSkills uses `javy-codegen` (a Rust crate) as a library dependency
- The library requires a `plugin.wasm` file to perform JavaScript → WASM compilation
- The plugin is built from the javy repository and "wizened" (initialized) for use
- Once you have the plugin, you can build skills without any CLI tools

See [Build Tool Guide](runtime/BUILD.md) for detailed information about the build process and plugin mechanism.

### Using Skills

```rust
use openskills_runtime::{OpenSkillRuntime, ExecutionOptions};
use serde_json::json;

// Discover skills from standard locations
let mut runtime = OpenSkillRuntime::new();
runtime.discover_skills()?;

// Execute a skill
let result = runtime.execute_skill(
    "my-skill",
    ExecutionOptions {
        timeout_ms: Some(5000),
        input: Some(json!({"input": "data"})),
        ..Default::default()
    }
)?;
```

See [Developer Guide](docs/developers.md) for detailed usage examples.

### Integrating with Agent Frameworks

OpenSkills works with **any agent framework** to give agents access to Claude-compatible skills. Here are examples:

**LangChain (TypeScript/Python)**
```typescript
import { OpenSkillRuntime } from "@finogeek/openskills";
import { DynamicStructuredTool } from "@langchain/core/tools";

const runtime = OpenSkillRuntime.fromDirectory("./skills");
runtime.discoverSkills();

const tool = new DynamicStructuredTool({
  name: "run_skill",
  schema: z.object({ skill_id: z.string(), input: z.string() }),
  func: async ({ skill_id, input }) => {
    const result = runtime.executeSkill(skill_id, { input });
    return result.outputJson;
  },
});
```

**Vercel AI SDK**
```typescript
import { OpenSkillRuntime } from "@finogeek/openskills";
import { tool } from "ai";

const runtime = OpenSkillRuntime.fromDirectory("./skills");
const runSkill = tool({
  inputSchema: z.object({ skill_id: z.string(), input: z.string() }),
  execute: async ({ skill_id, input }) => {
    return runtime.executeSkill(skill_id, { input }).outputJson;
  },
});
```

See [examples/agents](examples/agents/) for complete integration examples with LangChain, Vercel AI SDK, and more.

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
    │ Execution  │  (WASM/WASI 0.3 + seatbelt on macOS)
    └─────────────┘
```

### Execution Model

1. **Skill Discovery**: Scans directories for SKILL.md files
2. **Progressive Loading**: Loads metadata → instructions → resources on demand
3. **Execution**: Runs `wasm/skill.wasm` in Wasmtime or native `.py/.sh` via seatbelt on macOS
4. **Permission Enforcement**: Capabilities mapped from `allowed-tools` for WASM or seatbelt
5. **Audit Logging**: All executions logged with input/output hashes

## What Makes OpenSkills Unique

OpenSkills is the **only runtime** that combines:

1. **WASM/WASI Sandboxing**: Cross-platform security with capability-based permissions
2. **macOS Seatbelt Sandboxing**: Native Python and shell script execution with OS-level isolation
3. **Automatic Detection**: Runtime automatically chooses the right sandbox for each skill
4. **Agent Framework Agnostic**: Works with any agent framework (LangChain, Vercel AI SDK, custom)

This dual approach means you get:
- **Portability**: WASM skills run identically on macOS, Linux, Windows
- **Flexibility**: Native Python/Shell scripts on macOS when you need native libraries
- **Security**: Both sandboxing methods provide strong isolation
- **Compatibility**: 100% compatible with Claude Skills specification

## Comparison: OpenSkills vs Claude Code

| Aspect | Claude Code | OpenSkills |
|--------|-------------|------------|
| **SKILL.md Format** | ✅ Full support | ✅ 100% compatible |
| **Sandbox** | seatbelt/seccomp | **WASM/WASI 0.3 + seatbelt (macOS)** ⭐ |
| **Cross-platform** | OS-specific | WASM identical, native macOS only |
| **Script Execution** | Native (Python, shell) | WASM components + native (macOS) |
| **Build Required** | No | No if Python/Shell scripts. Yes if WASM (TS/JS → WASM) |
| **Native Python** | ✅ Supported | ✅ macOS (seatbelt) |
| **Shell Scripts** | ✅ Supported | ✅ macOS (seatbelt) |
| **Agent Framework** | Claude Desktop & Claude Agent SDK | **Any framework** ⭐ |
| **Use Case** | Desktop users, arbitrary skills | Enterprise agents, any agent framework |

## Project Structure

```
openskills/
├── runtime/              # Rust core runtime
│   ├── src/
│   │   ├── build.rs      # Build tool for TS/JS → WASM (uses javy-codegen)
│   │   ├── wasm_runner.rs # WASI 0.3 execution
│   │   ├── native_runner.rs # Seatbelt execution (macOS)
│   │   └── ...
│   └── BUILD.md          # Build tool documentation
├── scripts/
│   └── build_javy_plugin.sh  # Helper script to build javy plugin
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
- **[Build Tool Guide](runtime/BUILD.md)**: Compiling TypeScript/JavaScript skills
- **[Contributing Guide](docs/contributing.md)**: How to contribute to OpenSkills
- **[Architecture](docs/architecture.md)**: Internal architecture and design
- **[Specification](docs/spec.md)**: Complete runtime specification

## Building

```bash
# Clone with submodules (for tests and examples)
git clone <repository-url>
cd openskills
git submodule update --init --recursive

# Build everything
./scripts/build_all.sh

# Build runtime only
cd runtime
cargo build --release

# Build bindings
./scripts/build_bindings.sh
```

### Submodules

The `examples/claude-official-skills` directory is a git submodule pointing to [anthropics/skills](https://github.com/anthropics/skills). This provides access to official Claude Skills for testing and reference.

- **Initial clone**: Use `git clone --recursive <url>` or run `git submodule update --init --recursive` after cloning
- **Updating**: `cd examples/claude-official-skills && git pull && cd ../.. && git add examples/claude-official-skills && git commit`
- **Tests**: The test suite gracefully skips tests if the submodule is not initialized

## Status

- ✅ **Rust Runtime**: Fully functional with WASI 0.3
- ✅ **TypeScript Bindings**: Working
- ✅ **Python Bindings**: Working (requires Python ≤3.13)
- ✅ **WASM Execution**: WASI 0.3 component model fully supported
- ✅ **Build Tool**: `openskills build` for TS/JS compilation
- ✅ **Native Scripting**: Seatbelt sandbox (macOS)
- 🚧 **Native Scripting (Linux)**: Seccomp support planned

## License

MIT

[English](LICENSE) | [中文](LICENSE.zh.md)