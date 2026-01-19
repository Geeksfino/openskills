# OpenSkills Build Tool

The `openskills build` command compiles TypeScript/JavaScript skills to WASM components for execution in the OpenSkills runtime.

## How It Works: javy-codegen Library

OpenSkills uses **`javy-codegen`** (a Rust crate from crates.io) as a library dependency to compile JavaScript/TypeScript to WASM. This approach:

- **No CLI Required**: Unlike the `javy` CLI tool, we use the library directly, so no separate binary installation is needed
- **Programmatic Compilation**: JavaScript → WASM compilation happens via Rust API calls, not shell commands
- **Plugin-Based**: The library requires a `plugin.wasm` file (the javy plugin) to perform the actual compilation

### Understanding the Plugin

The **javy plugin** (`plugin.wasm`) is a WASM module that contains the QuickJS runtime and compilation logic. It's required because:

1. **javy-codegen** is the Rust library that orchestrates the compilation process
2. **plugin.wasm** contains the actual JavaScript engine (QuickJS) and WASM generation code
3. The plugin must be "wizened" (initialized) before use, which produces `plugin_wizened.wasm`

This separation allows:
- **Library flexibility**: The Rust code can be versioned and distributed via crates.io
- **Plugin updates**: The plugin can be updated independently if needed
- **No CLI dependency**: Everything happens programmatically within the OpenSkills build process

## Quick Start

```bash
# Build a skill from the current directory
openskills build

# Build a specific skill directory
openskills build my-skill

# Build with verbose output
openskills build --verbose

# Force rebuild (ignore up-to-date check)
openskills build --force
```

## Requirements

### Required: javy Plugin

OpenSkills uses **`javy-codegen`** as a library dependency (no CLI installation needed), but requires a **`plugin.wasm`** file to perform the actual JavaScript → WASM compilation.

#### What is the Plugin?

The javy plugin is a WASM module that contains:
- **QuickJS runtime**: The JavaScript engine used to execute and compile JS code
- **Compilation logic**: Code that transforms JavaScript into WebAssembly bytecode
- **WASI bindings**: System interface bindings for WASM execution

The plugin must be **"wizened"** (initialized) before use, which processes the raw plugin and produces `plugin_wizened.wasm`.

#### Getting the Plugin

The plugin can be provided via:

1. **Helper script** (recommended):
   ```bash
   ./scripts/build_javy_plugin.sh
   export JAVY_PLUGIN_PATH=/tmp/javy/target/wasm32-wasip1/release/plugin_wizened.wasm
   ```

2. **Environment variable**: Set `JAVY_PLUGIN_PATH` to point to an existing `plugin_wizened.wasm` file

3. **Current directory**: Place `plugin_wizened.wasm` in the current directory (will be auto-detected)

4. **Manual build** (if you need to customize):
   ```bash
   git clone https://github.com/bytecodealliance/javy.git
   cd javy
   rustup target add wasm32-wasip1
   cargo build --release --target wasm32-wasip1 -p javy-plugin
   cargo run -p javy-cli -- init-plugin \
     target/wasm32-wasip1/release/plugin.wasm \
     --out target/wasm32-wasip1/release/plugin_wizened.wasm
   export JAVY_PLUGIN_PATH=$(pwd)/target/wasm32-wasip1/release/plugin_wizened.wasm
   ```

#### Why Not Bundle the Plugin?

The plugin is not bundled with OpenSkills because:
- **Size**: The plugin is large (~several MB) and would bloat the OpenSkills binary
- **Flexibility**: Users can build the plugin themselves or use pre-built versions
- **Versioning**: The plugin version can be independent of OpenSkills version
- **One-time setup**: Once built, the plugin can be reused for all skill builds

### Optional (for TypeScript)
- **esbuild** (recommended, faster): Automatically installed via `npx` if not present
- **TypeScript compiler (tsc)**: Alternative to esbuild
  ```bash
  npm install -g typescript
  ```

## Supported Source Files

The build tool automatically detects source files in this order:
1. `src/index.ts`
2. `src/index.js`
3. `index.ts`
4. `index.js`
5. `src/main.ts`
6. `src/main.js`

## Build Process

### TypeScript
1. **Transpile**: TypeScript → JavaScript (using esbuild or tsc)
2. **Compile**: JavaScript → WASM component (using `javy-codegen` library + plugin)
3. **Output**: `wasm/skill.wasm`

### JavaScript
1. **Compile**: JavaScript → WASM component (using `javy-codegen` library + plugin)
2. **Output**: `wasm/skill.wasm`

### Under the Hood

When you run `openskills build`, here's what happens:

1. **Source Detection**: Finds your TypeScript/JavaScript source file
2. **TypeScript Transpilation** (if needed): Uses esbuild or tsc to convert TS → JS
3. **Plugin Loading**: Loads the javy plugin from `JAVY_PLUGIN_PATH` or current directory
4. **WASM Generation**: Uses `javy-codegen::Generator` to:
   - Read JavaScript source
   - Execute it in the QuickJS runtime (via plugin)
   - Generate WASM bytecode
   - Embed the bytecode in a WASM component
5. **Output**: Writes the compiled WASM to `wasm/skill.wasm`

All of this happens programmatically—no CLI tools are invoked during the build process.

## Example Skill Structure

```
my-skill/
├── SKILL.md              # Skill manifest
├── src/
│   └── index.ts         # TypeScript source
└── wasm/
    └── skill.wasm       # Compiled WASM (generated)
```

## Incremental Builds

The build tool checks file modification times:
- If `wasm/skill.wasm` is newer than source, build is skipped
- Use `--force` to rebuild regardless

## Output

By default, compiled WASM is written to `wasm/skill.wasm` relative to the skill directory.

You can specify a custom output path:
```bash
openskills build --output custom/path/skill.wasm
```

## Error Handling

The build tool provides clear error messages:
- Missing source files
- Missing javy plugin (with instructions on how to obtain it)
- TypeScript compilation errors
- JavaScript to WASM compilation errors

## Integration with Git

**Recommended**: Commit both source and compiled WASM:
```bash
git add src/index.ts wasm/skill.wasm
git commit -m "Add skill implementation"
```

This allows:
- Source code review
- Immediate use without build toolchain
- CI/CD verification (rebuild and compare hashes)

## CI/CD Integration

Example GitHub Actions workflow:
```yaml
- name: Build skill
  run: |
    # Build javy plugin first
    scripts/build_javy_plugin.sh
    openskills build --verbose
    
- name: Verify WASM matches source
  run: |
    # Rebuild and compare hash
    openskills build --force
    # Fail if hash differs from committed version
```
