# WASM Build Output

This directory contains the compiled WASM module for the skill-creator skill.

## Building

To build the WASM module from the TypeScript source:

```bash
# From the skill directory
openskills build

# Or from the repository root
openskills build examples/skills/skill-creator
```

The build process:
1. Transpiles TypeScript (`src/index.ts`) to JavaScript
2. Compiles JavaScript to WASM using javy
3. Outputs `wasm/skill.wasm`

## Requirements

- **javy**: JavaScript to WASM compiler
  
  **Note**: Must be built from source (cargo install --git doesn't work):
  ```bash
  git clone https://github.com/bytecodealliance/javy.git
  cd javy
  brew install binaryen && rustup target add wasm32-wasip1  # macOS
  cargo install --path crates/cli
  ```

- **esbuild** (optional, for TypeScript): Automatically installed via `npx` if not present

## Usage

The WASM module provides three functions:
- `init_skill`: Creates a new skill from template
- `validate_skill`: Validates a skill's structure
- `package_skill`: Packages a skill into a .skill file

All functions are accessed via the main entry point which routes based on the `action` field in the input JSON.
