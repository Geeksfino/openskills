# Skill Creator (WASM)

This is a **WASM-based rewrite** of the original [Claude official skills' skill-creator](https://github.com/anthropics/claude-official-skills/tree/main/skills/skill-creator), demonstrating how to create skills that leverage OpenSkills' WASM sandboxing capabilities.

## Overview

This skill serves a dual purpose:

1. **Functional Tool**: Provides the same skill creation functionality as the original Python-based skill-creator, but implemented as a WASM module for cross-platform execution
2. **Example Implementation**: Demonstrates best practices for building WASM-based skills in OpenSkills

## What Makes This Different

### Original (Python-based)
- Uses Python scripts (`init_skill.py`, `package_skill.py`, `quick_validate.py`)
- Executes in native sandbox (macOS seatbelt, Linux seccomp)
- Platform-specific execution

### This Version (WASM-based)
- Implemented in TypeScript, compiled to WASM
- Executes in WASM/WASI sandbox (cross-platform)
- Same functionality, different execution model
- Demonstrates OpenSkills' dual sandbox architecture

## Features

The skill provides three main functions (all accessible via WASM execution):

- **`init_skill`**: Creates a new skill from template with proper structure
- **`validate_skill`**: Validates a skill's structure and metadata
- **`package_skill`**: Packages a skill into a distributable `.skill` file

## Usage

Execute the skill through the OpenSkills runtime:

```json
{
  "action": "init_skill",
  "skill_name": "my-new-skill",
  "path": "skills/public"
}
```

```json
{
  "action": "validate_skill",
  "skill_path": "skills/public/my-skill",
  "skill_md_content": "...",
  "frontmatter": {...}
}
```

```json
{
  "action": "package_skill",
  "skill_path": "skills/public/my-skill",
  "output_dir": "./dist"
}
```

## Building

To build the WASM module:

```bash
# From repository root
openskills build examples/skills/skill-creator

# Or from the skill directory
cd examples/skills/skill-creator
openskills build
```

This compiles `src/index.ts` → `wasm/skill.wasm` using javy.

## Requirements

- **javy plugin**: OpenSkills uses `javy-codegen` as a library (no CLI needed), but requires a `plugin.wasm` file.
  
  **Quick setup**:
  ```bash
  # From repository root
  ./scripts/build_javy_plugin.sh
  export JAVY_PLUGIN_PATH=/tmp/javy/target/wasm32-wasip1/release/plugin_wizened.wasm
  ```
  
  See [Build Tool Guide](../../../../runtime/BUILD.md) for detailed plugin setup instructions.

- **esbuild** (optional, for TypeScript): Automatically installed via `npx` if not present

## Structure

```
skill-creator/
├── README.md          # This file
├── SKILL.md           # Skill manifest (same content as original)
├── src/               # TypeScript source
│   ├── index.ts      # Main entry point
│   ├── init_skill.ts # Skill initialization
│   ├── validate_skill.ts # Skill validation
│   └── package_skill.ts  # Skill packaging
└── wasm/
    ├── README.md      # Build instructions
    └── skill.wasm     # Compiled WASM (generated)
```

## Why WASM?

This implementation demonstrates:

1. **Cross-platform consistency**: Same WASM binary runs identically on macOS, Linux, Windows
2. **Security**: WASM sandbox provides capability-based isolation
3. **Portability**: Skills can ship pre-compiled WASM modules
4. **Performance**: Compiled WASM is fast and efficient

## Comparison with Original

| Aspect | Original (Python) | This Version (WASM) |
|--------|-------------------|---------------------|
| **Language** | Python 3 | TypeScript → WASM |
| **Execution** | Native (seatbelt/seccomp) | WASM/WASI sandbox |
| **Platform** | OS-specific | Cross-platform |
| **Distribution** | Source code | Pre-compiled binary |
| **Sandbox** | OS-level | Capability-based |

Both versions provide the same functionality, but this WASM version showcases OpenSkills' unique dual sandbox architecture and serves as a practical example for building WASM-based skills.

## See Also

- [OpenSkills README](../../../../README.md) - Overview of OpenSkills runtime
- [BUILD.md](../../../../runtime/BUILD.md) - Build tool documentation
- [Original skill-creator](https://github.com/anthropics/claude-official-skills/tree/main/skills/skill-creator) - Python-based reference implementation
