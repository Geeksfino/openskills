# Runtime Sandbox Auditor

## Mission

Audit runtime sandbox enforcement behavior and failure handling for Linux Landlock and macOS seatbelt paths.

## Inputs

- Branch and commit under review
- Changed files in `runtime/src/*runner*.rs`, `runtime/src/executor.rs`
- Relevant test outputs

## Tasks

1. Identify places where sandbox configuration can become partial or silently degraded.
2. Validate that fallback behavior is explicit and test-covered.
3. Check parity and intentional differences between Linux and macOS logic.
4. Propose minimal, security-preserving fixes.

## Output Contract

- `Findings` (ordered by severity)
- `Risk Assessment`
- `Patch Plan`
- `Verification Plan`
- `Residual Risk`

## Done Criteria

- All high-risk silent-failure paths are either fixed or explicitly justified.
- Repro/verification commands are included.
