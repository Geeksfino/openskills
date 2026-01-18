# OpenSkills vs Claude Skills Architecture Diagram

## Progressive Disclosure Flow Comparison

### Claude Skills Official Flow

```
Session Start
     │
     ├─► SCAN DIRECTORIES (~/.claude/skills, .claude/skills/)
     │
     ├─► LOAD TIER 1 (Metadata)
     │   ├─ name: "code-review"
     │   └─ description: "Reviews code..."
     │   (~100 tokens per skill)
     │
     ├─► INJECT INTO SYSTEM PROMPT ⭐
     │   "You have access to these skills:
     │    - code-review: Reviews code...
     │    - test-generator: Generates tests..."
     │
     └─► Model knows available skills

User Request: "Review my code"
     │
     ├─► SEMANTIC MATCH on descriptions
     │   → "code-review" matches
     │
     ├─► LOAD TIER 2 (Instructions) ⭐
     │   → Full SKILL.md markdown body
     │   (~2000 tokens)
     │
     └─► Execute skill instructions

Skill needs script/resource
     │
     └─► LOAD TIER 3 (Resources) ⭐
         → scripts/review.py (source not in context)
         → Only output enters context
```

### OpenSkills Updated Flow

```
runtime.discover_skills()
     │
     ├─► SCAN DIRECTORIES (~/.claude/skills, .claude/skills/, custom)
     │
     ├─► LOAD TIER 1 (Metadata) ✅
     │   ├─ id: "code-review"
     │   └─ description: "Reviews code..."
     │
     └─► RETURN Vec<SkillDescriptor>

✅ System prompt helpers available
     │
     └─► let prompt = runtime.get_system_prompt_metadata();

runtime.activate_skill("code-review")
     │
     ├─► LOAD TIER 2 (Instructions) ✅
     │   → Full SKILL.md content
     │
     └─► RETURN LoadedSkill

runtime.execute_skill("code-review", options)
     │
     ├─► LOAD TIER 3 (WASM Module) ✅
     │   → Find wasm/skill.wasm
     │   → Load on-demand
     │
     └─► Execute in WASM sandbox
```

---

## Context Fork Mechanism

### Claude Skills with Context Fork

```
Main Context (User Conversation)
│
├─ User: "Explore the authentication system"
│
├─ Claude activates skill: explorer (context: fork)
│  │
│  └─► FORKED CONTEXT (Isolated) ⭐
│      ├─ Read auth.ts
│      ├─ Read user.ts
│      ├─ Analyze patterns
│      ├─ Debug output: "Found 3 auth flows..."
│      ├─ Trial 1: Check JWT
│      ├─ Trial 2: Check OAuth
│      └─ GENERATE SUMMARY
│          "Authentication uses JWT + OAuth2.
│           3 main flows: login, refresh, logout."
│
├─ ⬅️ SUMMARY INJECTED to main context
│  (Not all intermediate steps)
│
└─ User sees clean summary, not debug logs

Token savings: ~5000 tokens (kept intermediate work isolated)
UX benefit: Clean conversation flow
```

### OpenSkills Updated Behavior

```
Main Context (User Conversation)
│
├─ runtime.execute_skill_with_context("explorer", options, &ctx)
│  │
│  └─► FORKED CONTEXT ✅
│      ├─ Read auth.ts → OUTPUT IN FORK
│      ├─ Read user.ts → OUTPUT IN FORK
│      ├─ Analyze patterns → OUTPUT IN FORK
│      ├─ Debug output → OUTPUT IN FORK
│      ├─ Trial 1 → OUTPUT IN FORK
│      ├─ Trial 2 → OUTPUT IN FORK
│      └─ Summary returned to parent
│
└─ User sees summary only

✅ Context isolated, reduced token usage
```

---

## Permission Model Comparison

### Claude Skills: Ask-Before-Act

```
Skill: "file-organizer"
allowed-tools: Read, Write, Bash

User: "Organize my downloads folder"

Claude: "I'll use the file-organizer skill."

Skill tries to execute: rm -rf old_files/

┌─────────────────────────────────────────────┐
│ ⚠️  Permission Required                     │
│                                             │
│ The skill wants to execute:                 │
│   • Tool: Bash                              │
│   • Command: rm -rf old_files/              │
│   • Risk: Deletes files permanently         │
│                                             │
│ [Allow Once] [Allow Always] [Deny]          │
└─────────────────────────────────────────────┘

User clicks [Allow Once]

✅ Execution proceeds
📝 Audit log: User approved Bash at 2026-01-18 10:30
```

### OpenSkills: Ask-Before-Act Permissions

```
Skill: "file-organizer"
allowed-tools: Read, Write, Bash

runtime.with_permission_callback(CliPermissionCallback)
       .execute_skill("file-organizer", options)

Skill tries to execute: rm -rf old_files/

┌─────────────────────────────────────────────┐
│ ⚠️  Permission Required                     │
│                                             │
│ [Allow Once] [Allow Always] [Deny]          │
└─────────────────────────────────────────────┘

✅ Execution proceeds only after approval
📝 Audit log: User decision recorded
```

---

## Sandbox Architecture

### Claude Skills: OS-Level Sandbox

```
┌─────────────────────────────────────────┐
│   macOS Process                         │
│   ┌───────────────────────────────┐     │
│   │ Sandbox (seatbelt)            │     │
│   │                               │     │
│   │  skill.sh                     │     │
│   │    │                          │     │
│   │    ├─ read: /allowed/path ✅  │     │
│   │    ├─ write: /tmp/out ✅      │     │
│   │    └─ access: /etc/passwd ❌  │     │
│   │       (blocked by seatbelt)   │     │
│   └───────────────────────────────┘     │
└─────────────────────────────────────────┘

Pros: Native performance
Cons: OS-specific (seatbelt/seccomp)
```

### OpenSkills: WASM Sandbox

```
┌─────────────────────────────────────────┐
│   Any OS (macOS/Linux/Windows)          │
│   ┌───────────────────────────────┐     │
│   │ Wasmtime Runtime              │     │
│   │                               │     │
│   │  skill.wasm (WASI)            │     │
│   │    │                          │     │
│   │    ├─ preopens:               │     │
│   │    │  /skill → read-only ✅    │     │
│   │    │  /tmp → read-write ✅     │     │
│   │    │                          │     │
│   │    └─ access: /etc/passwd ❌  │     │
│   │       (not in preopens)       │     │
│   └───────────────────────────────┘     │
└─────────────────────────────────────────┘

Pros: Cross-platform, portable, capability-based
Cons: Requires WASM compilation
```

---

## Token Optimization Comparison

### Claude Skills: Enforced Best Practices

```
SKILL.md Token Budget:
├─ Tier 1 (Metadata)
│  ├─ name: ~2 tokens
│  ├─ description: ~30 tokens (max 1024 chars)
│  └─ Total: ~100 tokens/skill ✅
│
├─ Tier 2 (Instructions)
│  ├─ Markdown body: ~2000 tokens (recommended)
│  ├─ ⚠️ Warning if > 5000 tokens
│  └─ Validation: skills-ref validate
│
└─ Tier 3 (Resources)
   ├─ scripts/ → 0 tokens (not in context)
   ├─ references/ → 0 tokens (loaded on demand)
   └─ Only outputs count ✅

Total upfront cost: ~100 tokens
On-demand cost: 2000-5000 tokens
```

### OpenSkills: Token Analysis Available

```
SKILL.md Token Budget:
├─ Tier 1 (Metadata)
│  ├─ Constrained: name max 64 chars ✅
│  ├─ Constrained: description max 1024 chars ✅
│  └─ Token analysis via `openskills analyze` ✅
│
├─ Tier 2 (Instructions)
│  ├─ Length estimates in analysis output ✅
│  ├─ Validation tooling via `openskills validate` ✅
│  └─ Automated reporting in CLI ✅
│
└─ Tier 3 (Resources)
   ├─ WASM modules → 0 tokens ✅
   └─ Loaded on-demand ✅

✅ Automated guidance via analysis output
```

---

## Validation Workflow

### Claude Skills

```
Developer writes my-skill/SKILL.md
     │
     ├─► skills-ref validate my-skill
     │   ├─ ✅ Format valid
     │   ├─ ⚠️ Description too long (truncated to 1024 chars)
     │   └─ ℹ️ Tier 2 is 3500 tokens (good)
     │
     ├─► skills-ref analyze my-skill
     │   ├─ Tier 1: 95 tokens
     │   ├─ Tier 2: 3500 tokens
     │   ├─ References: 2 files (not counted)
     │   └─ Optimization score: A
     │
     └─► CI/CD pipeline
         ├─ Run validation
         ├─ Block if errors
         └─ Deploy ✅
```

### OpenSkills

```
Developer writes my-skill/SKILL.md
     │
     ├─► openskills validate my-skill
     │   ├─ ✅ Format valid
     │   └─ ⚠️ Warnings (optional)
     │
     ├─► openskills analyze my-skill
     │   ├─ Tier 1 token estimate
     │   ├─ Tier 2 token estimate
     │   └─ Optimization hints
     │
     └─► CI can gate on validation ✅
```

---

## Data Flow Summary

### Progressive Disclosure Timeline

```
Time →

Claude Skills:
  0ms     100ms           5000ms            10000ms
  │       │               │                 │
  ├─ Scan ├─ Load Tier1  ├─ Match+Tier2   ├─ Execute+Tier3
  │       │ (all skills)  │ (1 skill)      │ (on demand)
  │       └─► Inject prompt                │
  └─► Session ready                        └─► Complete
      Model knows skills

OpenSkills:
  0ms     100ms           200ms             5000ms
  │       │               │                 │
  ├─ Scan ├─ Load Tier1  ├─ Inject prompt  ├─ Execute+Tier3
  │       │ (all skills)  │ (helpers)       │ (on demand)
  │       └─► Return Vec  └─► Model aware   └─► Complete
```

---

## Implementation Checklist

### What OpenSkills Does Well ✅

- [x] Tier 1 metadata loading
- [x] Tier 2 instruction loading  
- [x] Tier 3 resource loading
- [x] SKILL.md parsing (YAML + Markdown)
- [x] Standard directory discovery
- [x] Custom directory support
- [x] Sandbox isolation (WASM)
- [x] Permission mapping (allowed-tools → capabilities)
- [x] Audit logging
- [x] Format validation (name/description constraints)
- [x] System prompt metadata helpers
- [x] Ask-before-act permissions (callback)
- [x] Context fork + summary return
- [x] Validation CLI + token analysis
- [x] Cross-platform consistency

### Remaining Considerations (Client-Side)

- [ ] Sub-agent selection and orchestration when `agent` is specified
- [ ] Optional best-practices linting or CI policy enforcement

---

## Conclusion

**OpenSkills now fully implements the runtime scope of the Claude Skills architecture**, with a superior cross-platform sandbox model.

**The progressive disclosure mechanism is fully functional** - all three tiers load correctly and on-demand as specified in the Claude Skills architecture.

**Key differentiator**: OpenSkills trades OS-specific native execution for portable WASM-based execution, which is a strategic advantage for cross-platform deployments.

---

*Diagram Version: 1.0*  
*Date: 2026-01-18*
