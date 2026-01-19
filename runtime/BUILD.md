# OpenSkills Build Tool

The `openskills build` command compiles TypeScript/JavaScript skills to WASM components for execution in the OpenSkills runtime.

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

### Required
- **javy plugin**: OpenSkills uses `javy-codegen` as a library dependency (no CLI installation needed), but requires a `plugin.wasm` file.

  The plugin can be provided via:
  1. **Environment variable**: Set `JAVY_PLUGIN_PATH` to point to `plugin_wizened.wasm`
  2. **Current directory**: Place `plugin_wizened.wasm` in the current directory
  3. **Build from source** (helper script):
     ```bash
     scripts/build_javy_plugin.sh
     ```
  4. **Build from source** (manual):
     ```bash
     git clone https://github.com/bytecodealliance/javy.git
     cd javy
     cargo build --release --target wasm32-wasip1 -p javy-plugin
     cargo run -p javy-cli -- init-plugin \
       target/wasm32-wasip1/release/plugin.wasm \
       --out target/wasm32-wasip1/release/plugin_wizened.wasm
     export JAVY_PLUGIN_PATH=$(pwd)/target/wasm32-wasip1/release/plugin_wizened.wasm
     ```

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
2. **Compile**: JavaScript → WASM component (using javy)
3. **Output**: `wasm/skill.wasm`

### JavaScript
1. **Compile**: JavaScript → WASM component (using javy)
2. **Output**: `wasm/skill.wasm`

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
