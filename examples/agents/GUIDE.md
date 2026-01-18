## OpenSkills Agent Integration Guide

This guide shows how to integrate the OpenSkills runtime with popular agent
frameworks, and how to expose skills as tools for your agents.

### Overview
OpenSkills:
- Discovers skills from directories containing `SKILL.md`
- Supports progressive disclosure (metadata → instructions → resources)
- Executes skills in a sandbox (WASM, and native on macOS when available)
- Works with TypeScript and Python bindings

### Integration Patterns

#### Pattern A: Single Tool (Simple)
Expose one tool that can execute any skill by ID. This is the pattern used by
the minimal examples in `langchain-python` and `simple`.

**Pros:** Less code, faster to wire up  
**Cons:** Agent must supply `skill_id` in tool calls

#### Pattern B: One Tool Per Skill (Recommended)
Expose one tool per skill with a clear description. This improves agent
reasoning and tool selection. The LangChainJS advanced example uses this.

**Pros:** Better tool selection and prompting  
**Cons:** Slightly more setup code

#### Pattern C: Prompt Injection (Recommended)
Inject skill metadata into the system prompt so the agent can decide when to
use a skill.

### Common Integration Steps

#### 1) Initialize Runtime
**TypeScript**
```typescript
import { OpenSkillRuntime } from "@finogeek/openskills";

const runtime = OpenSkillRuntime.fromDirectory("./examples/skills");
runtime.discoverSkills();
```

**Python**
```python
from openskills import OpenSkillRuntime

runtime = OpenSkillRuntime.from_directory("./examples/skills")
runtime.discover_skills()
```

#### 2) List Available Skills
**TypeScript**
```typescript
const skills = runtime.listSkills();
skills.forEach((skill) => {
  console.log(`${skill.id}: ${skill.description}`);
});
```

**Python**
```python
skills = runtime.list_skills()
for skill in skills:
    print(f"{skill['id']}: {skill['description']}")
```

#### 3) Execute a Skill
**TypeScript**
```typescript
const result = runtime.executeSkill("example-skill", {
  input: JSON.stringify({ query: "hello" }),
  timeout_ms: 5000,
});
console.log(result.outputJson);
```

**Python**
```python
result = runtime.execute_skill(
    "example-skill",
    input={"query": "hello"},
    timeout_ms=5000,
)
print(result.get("output", ""))
```

### LangChainJS Advanced Pattern (One Tool Per Skill)
The advanced example builds a tool for each skill and injects skill metadata
into the system prompt:

- Tool creation helper: `langchain-js/src/openskills-tool.ts`
- Agent example: `langchain-js/src/advanced-agent.ts`

### Best Practices
- Discover skills once at startup
- Use skill metadata in prompts to improve selection
- Prefer per-skill tools for production agents
- Keep skills in `examples/skills` with built artifacts in `wasm/`

### Troubleshooting
- **"Skill not found"**: check `examples/skills/<skill>/SKILL.md`
- **"WASM module not found"**: run `openskills build` in the skill folder
- **Missing API key**: set `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`

### Resources
- `README.md` in each framework folder for setup details
- `docs/developers.md` for building skills
