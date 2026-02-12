# OpenSkills Subagent Specs

These files define project-specific subagent roles for deeper or parallel analysis workflows.

## Available Subagents

- `runtime-sandbox-auditor.md`
- `wasm-plugin-build-specialist.md`
- `bindings-compatibility-agent.md`
- `skill-spec-conformance-agent.md`
- `examples-e2e-agent.md`
- `release-gatekeeper-agent.md`

## How to Use

Use each file as a prompt contract when launching a subagent:

1. Copy the role's "Mission", "Inputs", and "Tasks".
2. Provide current branch context and target files.
3. Require the exact "Output Contract" section in the response.
