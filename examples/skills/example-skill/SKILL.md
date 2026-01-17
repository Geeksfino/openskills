---
name: example-skill
description: Example skill demonstrating the Claude Skills format with WASM sandbox support.
allowed-tools: Read
---

# Example Skill

This skill demonstrates the OpenSkills runtime format.

## Overview

OpenSkills implements the Claude Code Agent Skills specification with WASM-based sandboxing.

## Key Features

1. **Claude Skills Compatible**: Uses SKILL.md format with YAML frontmatter
2. **WASM Sandbox**: Scripts run in WASI sandbox instead of native OS
3. **Capability-Based Security**: `allowed-tools` maps to WASI capabilities

## WASM Execution

If a WASM module exists at `wasm/skill.wasm`, it can be executed with:

```bash
openskills-runtime execute example-skill --input '{"message": "hello"}'
```

The WASM module receives input via the `SKILL_INPUT` environment variable.

## Directory Structure

```
example-skill/
├── SKILL.md           # This file
├── wasm/
│   ├── skill.wasm     # WASI-compatible module
│   └── README.md      # WASM module documentation
└── README.md          # Optional documentation
```
