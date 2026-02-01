# OpenSkills - Give Your Agents Skills

[English](README.md) | [中文](README.zh.md)

A **Claude Skills compatible runtime** with **dual sandboxing**: **macOS seatbelt** for native Python and shell scripts (primary), plus **experimental WASM-based sandboxing** for cross-platform security. OpenSkills implements the [Claude Code Agent Skills specification](https://code.claude.com/docs/en/skills), providing a secure, flexible runtime for executing skills in **any agent framework**.

## Philosophy

OpenSkills is **syntactically 100% compatible** with Claude Skills, meaning any skill that follows the Claude Skills format (SKILL.md with YAML frontmatter) will work with OpenSkills. What makes OpenSkills unique is its **dual sandboxing approach**:

- **macOS seatbelt sandboxing** (primary) for native Python and shell scripts - production-ready, fully supported
- **WASM/WASI sandboxing** (experimental) for cross-platform security and consistency - available for early adopters

**Primary execution model**: Native Python and shell scripts via macOS seatbelt (with Linux seccomp support planned). This is the recommended, production-ready approach that works with the full Python ecosystem and native tools.

**Experimental WASM support**: WASM sandboxing is available for developers who want to explore cross-platform deterministic execution, but it is not required for using OpenSkills. Most skills work perfectly fine with native scripts.

OpenSkills can be integrated into **any agent framework** (LangChain, Vercel AI SDK, custom frameworks) to give agents access to Claude-compatible skills.

### Core Design Principles

1. **100% Syntactic Compatibility**: OpenSkills reads and executes skills using the exact same SKILL.md format as Claude Skills. Skills can be shared between Claude Code and OpenSkills without modification.

2. **Dual Sandbox Architecture**: OpenSkills combines **macOS seatbelt** (primary) with **experimental WASM/WASI 0.3** sandboxing:
   - **macOS Seatbelt** (primary): Native Python and shell script execution with OS-level sandboxing - production-ready, full ecosystem support
   - **WASM/WASI** (experimental): Cross-platform security, capability-based permissions, memory safety, deterministic execution - available for early adopters
   - **Automatic Detection**: Runtime automatically chooses the appropriate sandbox based on skill type
   - **Native-first**: Most skills use native scripts; WASM is optional for specific use cases

3. **Native Scripts First**: OpenSkills prioritizes native Python and shell script execution, which provides full access to the Python ecosystem and native tools. WASM compilation is available as an experimental option for specific use cases requiring cross-platform determinism.

### Target Use Case

OpenSkills is designed for **any agent framework** that needs Claude-compatible skills:

- **Agent Framework Integration**: Works with LangChain, Vercel AI SDK, custom frameworks, or any system that needs tool-like capabilities
- **Enterprise Agents**: Internal skills developed by trusted developers
- **Native Scripts**: Primary execution model using Python and shell scripts with OS-level sandboxing
- **Cross-Platform Native**: macOS seatbelt (production), Linux seccomp (planned)
- **Experimental WASM**: Optional WASM execution for specific use cases requiring determinism
- **Security & Auditability**: Both sandboxing methods provide strong isolation and audit logging

**Recommended approach**: Use native Python and shell scripts for most skills. WASM is available for experimental use cases but is not required.

## Limitations

### Current Limitations

1. **Native Scripts on Non-macOS**:
   - Native Python and shell scripts are supported only on macOS (seatbelt)
   - Linux seccomp support is planned

2. **WASM Support (Experimental)**:
   - WASM sandboxing is experimental and not the primary execution method
   - Build workflow required: JavaScript/TypeScript skills must be compiled to WASM components before execution
   - Limited native library support: Native Python packages, shell tools, etc. don't work in WASM
   - WASI compatibility required: Code must use WASI APIs, not native OS APIs

**Recommendation**: Use native Python and shell scripts for production skills. WASM is available for experimental use cases but is not required.

## Roadmap

OpenSkills will evolve to address limitations while maintaining its native-first approach:

1. **Linux Native Scripting**: Linux seccomp support is planned to complete cross-platform native sandboxing (macOS seatbelt is already production-ready).

2. **WASM Improvements** (experimental): Continued development of WASM support for specific use cases requiring determinism and cross-platform consistency.

3. **Enhanced Tooling**: Better development tools and templates for both native scripts and WASM compilation.

## Features

- ✅ **100% Claude Skills Compatible**: Full SKILL.md format support
- 🔒 **Dual Sandbox Architecture**: macOS seatbelt (primary) + experimental WASM/WASI 0.3
- 🧰 **Native Script Support**: Execute Python and shell scripts on macOS via seatbelt (production-ready)
- 🤖 **Any Agent Framework**: Integrate with LangChain, Vercel AI SDK, or custom frameworks
- 🚀 **Pre-built Tools**: Ready-to-use tool definitions for TS/Python (~200 lines less code)
- 📊 **Progressive Disclosure**: Efficient tiered loading (metadata → instructions → resources)
- 🔌 **Multi-Language Bindings**: Rust core with TypeScript and Python bindings
- 🛡️ **Capability-Based Security**: Fine-grained permissions via seatbelt profiles (and WASI for experimental WASM)
- 🏗️ **Build Tool**: `openskills build` for compiling TS/JS to WASM components (experimental)
- 🌐 **Cross-Platform Native**: macOS seatbelt (production), Linux seccomp (planned)
- 📁 **Workspace Management**: Built-in sandboxed workspace for file I/O operations

## Quick Start

### Installation

```bash
# Rust (from source)
git clone https://github.com/Geeksfino/openskills.git
cd openskills

# Initialize submodules (required for tests and examples)
git submodule update --init --recursive

cd runtime
cargo build --release

# TypeScript
npm install @finogeek/openskills

# Python
pip install finclip-openskills
# Note: Pre-built wheels are available for macOS and Linux only.
# Windows users need to build from source: git clone https://github.com/Geeksfino/openskills.git && cd openskills/bindings/python && pip install maturin && maturin develop
```

### Building a Skill

OpenSkills uses a **plugin-based build system** for compiling JavaScript/TypeScript → WASM. The system supports multiple build backends (plugins), allowing you to choose the compiler that best fits your needs.

**Plugin System Architecture:**
- **Plugins**: Modular build backends that handle compilation (e.g., `javy`, `quickjs`, `assemblyscript`)
- **Auto-detection**: When no plugin is specified, the system tries available plugins in order until one works
- **Plugin selection**: Choose explicitly via `--plugin` flag or `.openskills.toml` config file

**Recommended for new users**: The `quickjs` plugin (easiest setup - just run the setup script below)

**First-time setup** (required before building skills):

Run the setup script to install build tools and download dependencies:

```bash
# This will:
# - Download the WASI adapter
# - Install javy CLI (downloads pre-built binary when available)
# - Install wasm-tools
# - Check for optional tools (AssemblyScript)
./scripts/setup_build_tools.sh
```

**Build a skill**:

```bash
# Build a skill from TypeScript/JavaScript
cd my-skill
openskills build

# Auto-detection: tries plugins in order (javy → quickjs → assemblyscript)
# until it finds one that's available and has all dependencies
```

**Choose a plugin explicitly**:
```bash
openskills build --plugin quickjs       # Recommended: easiest setup
openskills build --plugin javy          # Requires javy plugin.wasm file
openskills build --plugin assemblyscript # Requires asc compiler
openskills build --list-plugins         # Show all available plugins and their status
```

**Plugin comparison:**
- **`quickjs`** (recommended): Easiest setup - just run setup script. Uses javy CLI + wasm-tools. Supports WASI 0.3.
- **`javy`**: Requires building javy plugin.wasm file. Uses javy-codegen library. Legacy support.
- **`assemblyscript`**: High-performance TypeScript-like language. Requires asc compiler.

**Alternative: javy plugin setup** (if you prefer the default javy plugin):

If you want to use the `javy` plugin instead of `quickjs`, you need to build the javy plugin:

```bash
# Build the javy plugin (one-time setup)
./scripts/build_javy_plugin.sh

# Export the plugin path (or add to your shell profile)
export JAVY_PLUGIN_PATH=/tmp/javy/target/wasm32-wasip1/release/plugin_wizened.wasm
```

**Config file (optional)**: place `.openskills.toml` or `openskills.toml` in the skill directory.

```toml
[build]
plugin = "quickjs"  # or "assemblyscript"

# Plugin options are usually auto-detected
# [build.plugin_options]
# adapter_path = "~/.cache/openskills/wasi_preview1_adapter.wasm"
```

**How the plugin system works**:
1. **Plugin selection**: You can specify a plugin via `--plugin` flag, config file, or let the system auto-detect
2. **Auto-detection**: When no plugin is specified, the system tries registered plugins in order until it finds one that:
   - Is available (has all required dependencies)
   - Supports the source file extension (.ts, .js, etc.)
3. **Plugin execution**: Each plugin handles the full compilation pipeline:
   - TypeScript transpilation (if needed)
   - JavaScript/TypeScript → WASM core module
   - WASM core → WASI 0.3 component (for quickjs/assemblyscript)
4. **Automatic setup**: QuickJS/AssemblyScript plugins auto-download the WASI adapter if needed
5. **Configuration**: Plugins can be configured via `.openskills.toml` or `--plugin-option` flags

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

OpenSkills works with **any agent framework** to give agents access to Claude-compatible skills. The runtime provides **pre-built tools** that eliminate boilerplate code and simplify agent setup.

#### ⭐ Recommended: Pre-built Tools (Simplified Setup)

**Vercel AI SDK (TypeScript)** - ~120 lines total:
```typescript
import { OpenSkillRuntime } from "@finogeek/openskills";
import { createSkillTools, getAgentSystemPrompt } from "@finogeek/openskills/tools";
import { generateText } from "ai";

// Initialize runtime
const runtime = OpenSkillRuntime.fromDirectory("./skills");
runtime.discoverSkills();

// Create pre-built tools (replaces ~200 lines of manual tool definitions)
const tools = createSkillTools(runtime, {
  workspaceDir: "./output"  // Sandboxed workspace for file I/O
});

// Get skill-agnostic system prompt (teaches agent HOW to use skills)
const systemPrompt = getAgentSystemPrompt(runtime);

// Use with any LLM
const result = await generateText({
  model: yourModel,
  system: systemPrompt,
  prompt: userQuery,
  tools,
});
```

**LangChain (Python)** - Pre-built tools available:
```python
from openskills import OpenSkillRuntime
from openskills_tools import create_langchain_tools, get_agent_system_prompt

# Initialize runtime
runtime = OpenSkillRuntime.from_directory("./skills")
runtime.discover_skills()

# Create pre-built LangChain tools
tools = create_langchain_tools(runtime, workspace_dir="./output")

# Get system prompt
system_prompt = get_agent_system_prompt(runtime)

# Use with LangChain agent
agent = create_agent(model, tools, system_prompt=system_prompt)
```

**Benefits of Pre-built Tools:**
- ✅ **~200 lines less code**: No need to manually define tools
- ✅ **Workspace management**: Automatic sandboxed file I/O
- ✅ **Skill-agnostic prompts**: Runtime generates system prompts
- ✅ **Security built-in**: Path validation, permission checks
- ✅ **Works with any skill**: No code changes needed

#### Manual Integration (Advanced)

If you need custom tool definitions, you can still integrate manually:

**Vercel AI SDK (Manual)**
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

See [examples/agents/simple](examples/agents/simple/) for a complete example using pre-built tools, or [examples/agents](examples/agents/) for other integration patterns.

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
- **Native Flexibility**: Full Python ecosystem and native tools via seatbelt (primary)
- **Experimental WASM**: Cross-platform determinism for specific use cases (optional)
- **Security**: Both sandboxing methods provide strong isolation
- **Compatibility**: 100% compatible with Claude Skills specification

## WASM Support: Long-Term Vision

> **Status**: WASM sandboxing is **experimental** and not the primary execution method. Most skills work perfectly with native Python and shell scripts.

> **Developer Note on WASI Versions**: The documentation refers to "WASI 0.3" as our target, but the current build toolchain (using the `wasi_snapshot_preview1` adapter) produces **WASI 0.2 components**. The runtime supports both WASI 0.2 and 0.3 - it attempts 0.3 instantiation first, then falls back to 0.2. Native WASI 0.3 toolchains (e.g., Rust's `wasm32-wasip3` target) are expected to mature in 2026, at which point components can be built natively for WASI 0.3 without the adapter.

### Why We Support WASM (Long-Term)

While native scripts are our primary execution model, we're investing in WASM support for specific use cases where it provides unique value. Here's our perspective on WASM's role:

#### What WASM is Good At (Today)

✅ **Determinism**: Same input → same output, critical for audit, replay, and compliance  
✅ **Fast Startup**: Millisecond-level startup times, great for frequently-invoked agent skills  
✅ **Strong Isolation by Design**: No syscalls unless explicitly exposed, capability-based access via WASI  
✅ **Portability**: Identical execution on macOS, Linux, Windows  
✅ **Narrow Attack Surface**: No shell, no fork bombs, no ptrace exploits  

**Best for**: Policy logic, orchestration, validation, scoring, reasoning glue, and deterministic workflows.

#### What WASM is Not Good At (And Won't Be Soon)

❌ **Full Python Ecosystem**: NumPy, SciPy, pandas, PyTorch rely on native extensions, BLAS, CUDA  
❌ **GPU & Hardware Acceleration**: Experimental, fragile, not regulator-friendly  
❌ **OS-Native Behaviors**: File watchers, shared memory tricks, complex IPC  
❌ **Legacy Skills**: Many assume Python + OS capabilities  

**You cannot wish these away.** This is why we prioritize native scripts for production use.

### The Right Mental Model

**Docker is an OS boundary. WASM is a language boundary.**

They are **complementary, not competing**:

```
┌─────────────────────────────┐
│ Agent Runtime               │
│                             │
│  ┌───────────────────────┐ │
│  │ WASM Skill Sandbox     │ │ ← Experimental: logic, policy, orchestration
│  │  - deterministic       │ │
│  │  - auditable           │ │
│  │  - fast startup        │ │
│  └───────────────────────┘ │
│             │               │
│     delegate call            │
│             ▼               │
│  ┌───────────────────────┐ │
│  │ Native Skill Sandbox   │ │ ← Primary: Python, ML, quant, native tools
│  │  - Python              │ │
│  │  - ML / Quant          │ │
│  │  - Seatbelt/seccomp    │ │
│  └───────────────────────┘ │
└─────────────────────────────┘
```

**WASM is:**
- Always available (experimental)
- Default for specific use cases requiring determinism
- Trusted for logic and policy enforcement

**Native is:**
- Primary execution model
- Required for full ecosystem access
- Heavily controlled via OS sandboxes

### Is WASM a Docker Replacement?

**No. And it shouldn't be.**

- **Docker** = process isolation, filesystem virtualization, networking namespaces, cgroups
- **WASM** = instruction sandbox, capability runtime

They solve different problems. Trying to replace Docker with WASM leads to complexity, disappointment, and hacks.

### Security: Is WASM "Strong Enough" Alone?

**WASM is a strong sandbox, but not a complete one.**

**What WASM isolates well:**
- ✅ Memory safety (no arbitrary memory access)
- ✅ CPU instructions (no privileged ops)
- ✅ No syscalls unless exposed
- ✅ Deterministic execution

**What WASM cannot fully control alone:**
- ❌ Resource exhaustion (CPU time, memory growth, infinite loops) - needs host-enforced limits
- ❌ Host bugs - if the WASM runtime has a vulnerability, no second line of defense
- ❌ Native escapes via host functions - filesystem, networking, crypto functions run natively

**Industry reality**: Even serious systems layer sandboxes:
- Cloudflare Workers: WASM + OS isolation
- Fastly Compute@Edge: WASM + VM
- Wasmtime in production: WASM + seccomp
- Deno: V8 + OS sandbox

**Nobody serious runs WASM naked at high trust boundaries.**

### Finance-Specific Perspective

For finance agents, you care about:

| Requirement | Native Scripts | WASM (Experimental) |
|-------------|----------------|---------------------|
| Auditability | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Determinism | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Policy enforcement | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Legacy quant code | ⭐⭐⭐⭐⭐ | ❌ |
| ML ecosystem | ⭐⭐⭐⭐⭐ | ❌ |

**The answer is not "WASM or not."**

**The answer is: Native scripts first, WASM when necessary.**

### Long-Term Viability (5–10 Year View)

**WASM will:**
- Get better WASI support
- Get better language support
- Become a standard control layer

**WASM will not:**
- Replace Python ML stacks
- Replace OS-level sandboxes
- Become "run anything"

**Betting on it as a universal runtime is risky.**  
**Betting on it as a core logic sandbox is smart.**

### Our Recommendation

✅ **Yes, support WASM/WASI long-term** - for specific use cases  
❌ **No, do not rely on WASM alone** - native scripts are primary  
✅ **Treat WASM as the control plane** - logic, policy, orchestration  
✅ **Layer OS sandbox for native code** - full ecosystem access  
❌ **Do not promise "Docker replacement"** - they solve different problems  

**One sentence to anchor our architecture:**

> "WASM gives us deterministic control; OS sandboxes give us practical power."

This gives us:
- **Credibility**: Honest about limitations
- **Safety**: Defense in depth
- **Flexibility**: Right tool for the job
- **Future optionality**: Can evolve as WASM matures

## Comparison: OpenSkills vs Claude Code

| Aspect | Claude Code | OpenSkills |
|--------|-------------|------------|
| **SKILL.md Format** | ✅ Full support | ✅ 100% compatible |
| **Sandbox** | seatbelt/seccomp | **seatbelt (macOS, primary) + WASM/WASI 0.3 (experimental)** ⭐ |
| **Cross-platform** | OS-specific | Native macOS (production), Linux planned; WASM identical (experimental) |
| **Script Execution** | Native (Python, shell) | Native (macOS, primary) + WASM components (experimental) |
| **Build Required** | No | No for native scripts. Yes for WASM (experimental, TS/JS → WASM) |
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
git clone https://github.com/Geeksfino/openskills.git
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

- ✅ **Rust Runtime**: Fully functional
- ✅ **TypeScript Bindings**: Working
- ✅ **Python Bindings**: Working (requires Python ≤3.13)
- ✅ **Native Scripting**: Seatbelt sandbox (macOS, production-ready)
- 🧪 **WASM Execution**: WASI 0.3 component model (experimental)
- 🧪 **Build Tool**: `openskills build` for TS/JS → WASM compilation (experimental)
- 🚧 **Native Scripting (Linux)**: Seccomp support planned

## Related Projects

- **[FinClip ChatKit](https://github.com/Geeksfino/finclip-chatkit)**: A mobile-friendly SDK for building AI-powered chat experiences. Provides production-ready chat UI components for iOS and Android, with support for AG-UI, MCP-UI and OpenAI Apps SDK integration. Perfect for developers building mobile agent applications that need both the runtime capabilities of OpenSkills and polished chat interfaces.

## License

MIT

[English](LICENSE) | [中文](LICENSE.zh.md)