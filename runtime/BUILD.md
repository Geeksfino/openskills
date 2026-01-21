# OpenSkills Build Tool

The `openskills build` command compiles TypeScript/JavaScript skills to WASM components for execution in the OpenSkills runtime.

## How It Works: Plugin-Based Build Backends

OpenSkills uses a **plugin-based build system** to compile JavaScript/TypeScript to WASM. The default plugin is **`javy`** (via `javy-codegen`), but other plugins can be added over time. This approach:

- **No CLI Required**: Plugins can compile via Rust libraries without external binaries
- **Programmatic Compilation**: JavaScript → WASM compilation happens via Rust API calls
- **Pluggable Backends**: Developers choose which compiler backend to use

### Understanding the Default Plugin (javy)

The **javy plugin** (`plugin.wasm`) is a WASM module that contains the QuickJS runtime and compilation logic. It's required because:

1. **javy-codegen** is the Rust library that orchestrates the compilation process
2. **plugin.wasm** contains the actual JavaScript engine (QuickJS) and WASM generation code
3. The plugin must be "wizened" (initialized) before use, which produces `plugin_wizened.wasm`

This separation allows:
- **Library flexibility**: The Rust code can be versioned and distributed via crates.io
- **Plugin updates**: The plugin can be updated independently if needed
- **No CLI dependency**: Everything happens programmatically within the OpenSkills build process

## Setup

**Before building your first skill**, run the setup script to install required tools:

```bash
# From the OpenSkills repository root
./scripts/setup_build_tools.sh
```

This script will:
- ✅ Download the WASI preview1 adapter (required for WASI 0.3 components)
- ✅ Install `javy` CLI (downloads pre-built binary when available, falls back to building from source)
- ✅ Install `wasm-tools` (for component conversion)
- ✅ Check for optional tools like AssemblyScript

The setup script automatically detects your OS and architecture, downloads pre-built binaries when available, and falls back to building from source if needed.

## Quick Start

After running the setup script, you can build skills:

```bash
# Build a skill from the current directory (auto-detect plugin)
openskills build

# Build a specific skill directory
openskills build my-skill

# Build with verbose output
openskills build --verbose

# Force rebuild (ignore up-to-date check)
openskills build --force

# List available plugins
openskills build --list-plugins

# Choose a plugin explicitly
openskills build --plugin quickjs  # Recommended: uses pre-built tools

# QuickJS via javy CLI + wasm-tools component conversion
openskills build --plugin quickjs

# AssemblyScript via asc + wasm-tools component conversion
openskills build --plugin assemblyscript

# Provide plugin options (example: override adapter path)
openskills build --plugin quickjs \
  --plugin-option adapter_path=/path/to/wasi_preview1_adapter.wasm
```

### Config File (Optional)

Place `.openskills.toml` or `openskills.toml` in the skill directory to set defaults:

```toml
[build]
plugin = "javy"

[build.plugin_options]
plugin_path = "/tmp/javy/plugin_wizened.wasm"
```

CLI flags override config file values.

## Requirements

### Required: javy Plugin (default backend)

OpenSkills uses **`javy-codegen`** as a library dependency (no CLI installation needed), but requires a **`plugin.wasm`** file to perform the actual JavaScript → WASM compilation.

### QuickJS Plugin (javy CLI + wasm-tools)

**Quick Setup** (recommended):
```bash
./scripts/setup_build_tools.sh
openskills build --plugin quickjs
```

The setup script downloads the WASI adapter and checks for required tools. The adapter is also **auto-downloaded** on first build if not found.

**Manual Setup**:
1. Install `javy` CLI:
   ```bash
   git clone https://github.com/bytecodealliance/javy.git /tmp/javy
   cd /tmp/javy && cargo install --path crates/cli
   ```
2. Install `wasm-tools`:
   ```bash
   cargo install wasm-tools
   ```
3. The WASI adapter is auto-downloaded to `~/.cache/openskills/`, or provide manually:
   ```bash
   export WASI_ADAPTER_PATH=/path/to/wasi_snapshot_preview1.command.wasm
   ```

### AssemblyScript Plugin (asc + wasm-tools)

**Quick Setup** (recommended):
```bash
./scripts/setup_build_tools.sh
openskills build --plugin assemblyscript
```

**Manual Setup**:
1. Install AssemblyScript:
   ```bash
   npm install -g assemblyscript
   ```
2. Install `wasm-tools`:
   ```bash
   cargo install wasm-tools
   ```
3. The WASI adapter is auto-downloaded, or set `WASI_ADAPTER_PATH` environment variable.

### Adapter Auto-Detection

The QuickJS and AssemblyScript plugins automatically search for the WASI adapter in:
1. Explicit `--plugin-option adapter_path=...`
2. `WASI_ADAPTER_PATH` environment variable
3. `~/.cache/openskills/wasi_preview1_adapter.wasm`
4. `~/.wasmtime/wasi_snapshot_preview1.command.wasm`
5. Current directory (`wasi_preview1_adapter.wasm`)

If not found, the adapter is **automatically downloaded** from the Bytecode Alliance wasmtime releases.

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
2. **Compile**: JavaScript → WASM component (using the selected build plugin)
3. **Output**: `wasm/skill.wasm`

### JavaScript
1. **Compile**: JavaScript → WASM component (using the selected build plugin)
2. **Output**: `wasm/skill.wasm`

### Under the Hood

When you run `openskills build`, here's what happens:

1. **Source Detection**: Finds your TypeScript/JavaScript source file
2. **TypeScript Transpilation** (if needed): Uses esbuild or tsc to convert TS → JS
3. **Plugin Loading**: Resolves the selected build plugin and its dependencies
4. **WASM Generation**: Uses the plugin backend to:
   - Read JavaScript source
   - Execute it in the JavaScript runtime (QuickJS for the default javy plugin)
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
