# Security Policies

This document describes the security model, sandboxing mechanisms, and permission policies implemented in OpenSkills Runtime.

## Overview

OpenSkills Runtime implements a **defense-in-depth** security model with multiple layers:

1. **Native Script Sandbox** (Primary) - OS-level sandboxing (Seatbelt on macOS) for Python/Shell scripts - production-ready
2. **WASM Sandbox** (Experimental) - Capability-based isolation for WASM modules (WASI 0.3) - available for specific use cases
3. **Permission Enforcement** - Tool-based access control via `allowed-tools` and risky tool detection
4. **Context Isolation** - Forked contexts prevent skill output pollution

**Note**: Native scripts via seatbelt are the primary and recommended execution method. WASM sandboxing is experimental.

---

## WASM Sandbox (WASI 0.3) - Experimental

**Status**: Experimental feature. Native scripts are the primary execution method.

WASM modules execute in a capability-based sandbox using WASI 0.3 (WASIp3) with the component model. This is available for specific use cases requiring determinism, but is not suitable for full Python ecosystem or native libraries.

See [README.md](../README.md#wasm-support-long-term-vision) for detailed discussion of WASM's role and limitations.

### Filesystem Access

**Read Access:**
- Default: No read access
- Granted based on `allowed-tools`:
  - `Read`, `Grep`, `Glob`, `LS` → Read access to skill root directory
  - `Bash`, `Terminal` → Read access to skill root directory
- Can be extended via skill manifest `wasm.filesystem.read` configuration

**Write Access:**
- Default: No write access
- Granted based on `allowed-tools`:
  - `Write`, `Edit`, `MultiEdit` → Write access to skill root directory
  - `Bash`, `Terminal` → Write access to skill root directory
- Can be extended via skill manifest `wasm.filesystem.write` configuration

**Path Resolution:**
- Relative paths are resolved relative to the skill root directory
- Absolute paths are used as-is (if allowed)

### Network Access

**Default:** No network access

**Granted when:**
- `allowed-tools` includes `WebSearch` or `Fetch`
- Skill manifest `wasm.network.allow` specifies allowed hosts

**Host Matching:**
- Exact host match: `api.example.com`
- Subdomain match: `*.example.com` matches `sub.api.example.com`
- Wildcard `*` allows all hosts (when `WebSearch` or `Fetch` tools are used)

### Environment Variables

**Default:** No environment variables exposed

**Granted via:**
- Skill manifest `wasm.env.allow` list

### Resource Limits

- **Memory:** Default 128MB, configurable via `wasm.memory_mb`
- **Timeout:** Default 30 seconds, configurable via `wasm.timeout_ms`
- **Random Seed:** Optional deterministic seed via `wasm.random_seed`

---

## Native Script Sandbox (macOS Seatbelt) - Primary

**Status**: Production-ready, primary execution method.

Native Python and Shell scripts execute under macOS Seatbelt sandbox profiles following Claude Code's security model. This is the recommended approach for most skills, providing full access to the Python ecosystem and native tools.

### Security Model

The seatbelt profile uses a **"allow broad reads, deny specific sensitive paths"** approach:

1. **Deny default** - All operations denied by default
2. **Allow broad file reads** - Python and interpreters need system library access
3. **Deny specific sensitive paths** - Credentials and config files explicitly blocked
4. **Allow writes only to specific paths** - Temp directories, skill root, configured paths

### Core Permissions

All native scripts receive these base permissions:
- `sysctl-read` - System information queries
- `process-exec` - Execute the interpreter binary
- `process-fork` - Fork child processes
- `mach-lookup` - Mach port lookups (required for process execution)
- `signal` - Signal handling

### File Read Access

**Allowed:**
- **Broad read access** (`allow file-read*`) - Required for Python/interpreters to access:
  - System libraries (`/usr/lib`, `/System/Library`)
  - Python frameworks (`/Library/Frameworks/Python.framework`)
  - Homebrew installations (`/opt/homebrew`)
  - Standard system paths (`/usr/bin`, `/bin`, `/sbin`)
  - User directories (`/Users`)
  - Temporary directories (`/tmp`, `/private/tmp`)

**Explicitly Denied (Sensitive Paths):**
```
~/.ssh              # SSH keys
~/.gnupg            # GPG keys
~/.aws              # AWS credentials
~/.azure            # Azure credentials
~/.config/gcloud    # Google Cloud credentials
~/.kube             # Kubernetes config
~/.docker           # Docker config
~/.npmrc            # npm credentials
~/.pypirc           # PyPI credentials
~/.netrc             # Network credentials
~/.gitconfig         # Git config
~/.git-credentials   # Git credentials
~/.bashrc            # Shell config
~/.zshrc             # Shell config
~/.profile           # Shell config
~/.bash_profile      # Shell config
~/.zprofile          # Shell config
```

**Note:** `~` is expanded to the user's home directory at runtime.

### File Write Access

**Allowed:**
- `/dev/null` - Output redirection
- Temporary directories:
  - `/tmp`
  - `/private/tmp`
  - `/private/var/tmp`
  - `/private/var/folders`
- Skill root directory (where the skill is located)
- Explicitly configured write paths (from skill manifest)

**Denied:**
- All other paths (including system directories, user home, etc.)

### Process Execution

**Default:** No process execution

**Granted when:**
- Script type is `Shell` or `Python` (interpreter execution required)
- `allowed-tools` includes `Bash` or `Terminal`
- Full `process*` permissions granted (allows subprocess spawning)

### Network Access

**Default:** No network access

**Granted when:**
- `allowed-tools` includes `WebSearch` or `Fetch`
- `allow network*` added to seatbelt profile

---

## Permission Enforcement

### Allowed Tools

Skills can restrict which tools they're allowed to use via the `allowed-tools` field in their manifest:

```yaml
allowed-tools:
  - Read
  - Grep
  - Glob
```

**Behavior:**
- **Empty list** = All tools allowed (no restriction)
- **Non-empty list** = Only listed tools allowed
- Tool calls for unlisted tools are **denied** with `PermissionDenied` error

### Risky Tools

Some tools are classified as "risky" and require explicit permission via a callback:

**Low Risk:**
- `Read`, `Grep`, `Glob`, `LS` - Read-only operations

**Medium Risk:**
- `Write`, `Edit`, `MultiEdit` - File modification
- `WebSearch`, `Fetch` - Network access

**High Risk:**
- `Bash`, `Terminal` - Arbitrary command execution
- `Delete` - File deletion

**Permission Flow:**
1. Agent calls `check_tool_permission(skill_id, tool, description)`
2. Runtime checks if tool is in `allowed-tools` (if list is non-empty)
3. If tool is risky, runtime calls permission callback:
   - `DenyAllCallback` - Always denies (strict mode)
   - `CliPermissionCallback` - Prompts user for approval
   - Custom callback - User-defined logic
4. Returns `true` if allowed, `false` or error if denied

### Tool-to-Capability Mapping

Tools are mapped to WASI capabilities as follows:

| Tool | Filesystem Read | Filesystem Write | Network |
|------|----------------|------------------|---------|
| `Read`, `Grep`, `Glob`, `LS` | ✅ Skill root | ❌ | ❌ |
| `Write`, `Edit`, `MultiEdit` | ✅ Skill root | ✅ Skill root | ❌ |
| `Bash`, `Terminal` | ✅ Skill root | ✅ Skill root | ❌ |
| `WebSearch`, `Fetch` | ❌ | ❌ | ✅ All hosts |

---

## Context Isolation

### Forked Contexts

Skills with `context: fork` execute in isolated contexts:

**Isolation:**
- Tool calls recorded in forked context
- Intermediate outputs (stdout, stderr) captured separately
- Only **summary** returned to parent context
- Prevents context pollution from verbose tool outputs

**Summary Generation:**
- Extracts only `Result` type outputs
- Excludes `ToolCall`, `Stdout`, `Stderr` from summary
- Falls back to stdout if no explicit results recorded

**Use Case:**
- Instruction-only skills (no WASM module or native script)
- Agent executes tools and records outputs
- Final summary returned to parent agent

---

## Security Boundaries

### What's Protected

✅ **Credentials** - SSH keys, AWS/Azure/GCP credentials, API keys  
✅ **Config Files** - Git config, shell configs, Docker/Kubernetes configs  
✅ **System Directories** - Write access to system paths denied  
✅ **Network** - No network access unless explicitly allowed  
✅ **Process Execution** - Limited to required interpreters unless `Bash`/`Terminal` allowed  

### What's Allowed

✅ **System Libraries** - Read access for interpreter execution  
✅ **Skill Directory** - Read/write access to skill root  
✅ **Temporary Files** - Write access to `/tmp` and variants  
✅ **Standard Input/Output** - `/dev/null` for redirection  

---

## Audit Logging

All skill executions are logged with:

- **Skill ID** and version
- **Input/Output hashes** (SHA-256)
- **Permissions used** (tools, filesystem paths, network hosts)
- **Execution status** (success, timeout, permission denied, failed)
- **Timing** (start time, duration)
- **Outputs** (stdout, stderr)

Audit records are sent to the configured audit sink (default: no-op sink).

---

## Best Practices

### For Skill Authors

1. **Minimize `allowed-tools`** - Only request tools you actually need
2. **Avoid risky tools** - Prefer `Read` over `Bash` when possible
3. **Use native scripts** - Recommended for most skills; full Python ecosystem access
4. **Specify filesystem paths** - For WASM (experimental): use `wasm.filesystem.read/write` for minimal access
5. **Restrict network** - For WASM (experimental): use `wasm.network.allow` with specific hosts, not `*`
6. **Use `context: fork`** - For instruction-only skills to prevent context pollution

### For Runtime Users

1. **Review `allowed-tools`** - Understand what each skill can do
2. **Configure permission callbacks** - Use `CliPermissionCallback` for interactive approval
3. **Monitor audit logs** - Review execution records for suspicious activity
4. **Trust skill sources** - Only load skills from trusted repositories

---

## Implementation Details

### Seatbelt Profile Generation

The seatbelt profile is generated dynamically based on:
- Skill root directory
- Configured read/write paths
- `allowed-tools` (for network/process permissions)
- Script type (Python/Shell)

Profile follows this structure:
```
(version 1)
(deny default)
(allow sysctl-read)
(allow process-exec)
(allow process-fork)
(allow mach-lookup)
(allow signal)
(allow file-read*)
(deny file-read* (subpath "~/.ssh"))
... (more sensitive path denials)
(allow file-write* (subpath "/tmp"))
... (more write path allowances)
(allow process*)  # if Bash/Terminal allowed
(allow network*) # if WebSearch/Fetch allowed
```

### WASI Capability Preopening

WASM modules receive preopened directories via WASI 0.3:
- Read-only directories: Mapped to skill root or configured read paths
- Write directories: Mapped to skill root or configured write paths
- No access to parent directories or system paths

---

## Related Documentation

- [Architecture](./architecture.md) - Overall system design
- [Skill Flow](./skill-flow.md) - Execution workflows
- [Developers Guide](./developers.md) - API usage examples

---

Last Updated: 2026-01-18
