# Test Prompts for OpenSkills Project Skills

## 1. Runtime Debug Path
"I'm seeing test failures in the seatbelt sandbox tests after my recent changes to native_runner.rs. Can you debug this?"

Expected route: openskills-runtime-debug → runtime-sandbox-auditor (if needed)

## 2. Plugin Separation Path
"I want to add a new WASM plugin for AssemblyScript. How do I ensure it doesn't break the core runtime compilation?"

Expected route: openskills-plugin-separation → wasm-plugin-build-specialist

## 3. Bindings Path
"I changed the runtime API in lib.rs. Will this break the TypeScript bindings?"

Expected route: openskills-bindings-maintainer → bindings-compatibility-agent

## 4. Skill Authoring Path
"Review this new skill I created for financial analysis and tell me if it follows OpenSkills conventions"

Expected route: openskills-skill-authoring → skill-spec-conformance-agent

## 5. E2E Test Path
"Before I merge this PR, run all the tests and example agents to make sure nothing is broken"

Expected route: openskills-e2e-test-runbook → examples-e2e-agent

## 6. Release Path
"Prepare a release readiness report for openskills v0.3.0"

Expected route: openskills-release-ops → release-gatekeeper-agent

## 7. Orchestrator Direct
"I need help with openskills development. What should I do first?"

Expected route: openskills-dev-orchestrator (routing assessment)
