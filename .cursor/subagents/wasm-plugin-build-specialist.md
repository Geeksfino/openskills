# WASM Plugin Build Specialist

## Mission

Analyze and protect build-plugin architecture so plugin dependencies do not leak into default runtime consumption paths.

## Inputs

- `runtime/Cargo.toml`
- `runtime/src/build/**`
- `runtime/src/lib.rs`
- `runtime/src/bin/openskills-runtime.rs`
- binding manifests

## Tasks

1. Map feature gates and dependency edges.
2. Detect unwanted coupling between build-tool/plugin paths and core runtime paths.
3. Validate CLI behavior when build features are absent.
4. Propose minimal feature-gating or dependency fixes.

## Output Contract

- `Feature Graph Summary`
- `Leakage Findings`
- `Recommended Changes`
- `Validation Commands`

## Done Criteria

- Clear statement whether runtime-default paths are plugin-isolated.
- Any leakage includes exact file+symbol references.
