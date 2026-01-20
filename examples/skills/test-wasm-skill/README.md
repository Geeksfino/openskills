# Test WASM Skill

This is a simple example skill that demonstrates building a TypeScript skill to WASM using OpenSkills.

## Structure

```
test-wasm-skill/
├── src/
│   └── index.ts      # TypeScript source file
├── wasm/
│   └── skill.wasm    # Compiled WASM component (generated)
└── README.md         # This file
```

## Building

**Prerequisites**: Make sure you've run the setup script first:

```bash
# From the OpenSkills repository root
./scripts/setup_build_tools.sh
```

**Build the skill**:

```bash
# From the OpenSkills repository root
openskills build examples/skills/test-wasm-skill --plugin quickjs

# Or from this directory
cd examples/skills/test-wasm-skill
openskills build --plugin quickjs
```

The build process will:
1. Transpile TypeScript → JavaScript
2. Compile JavaScript → WASM core module (via javy CLI)
3. Convert WASM core → WASI 0.3 component (via wasm-tools + adapter)
4. Output: `wasm/skill.wasm`

## Testing

After building, you can test the skill using the OpenSkills runtime:

```bash
openskills run examples/skills/test-wasm-skill
```

## Source Code

The `src/index.ts` file contains a simple example that demonstrates:
- Basic TypeScript syntax
- Console output
- Simple function definitions

You can modify this file to test different features or use it as a template for your own skills.
