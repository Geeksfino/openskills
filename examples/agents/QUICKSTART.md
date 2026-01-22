## OpenSkills Agent Integration - Quick Start

Get started with OpenSkills in your agent framework in under 5 minutes.

### Choose Your Path

#### Python Developer (LangChain)
```bash
cd with_langchain-python
pip install -r requirements.txt
python main.py
```

#### TypeScript Developer (Vercel AI SDK Example)
```bash
cd with_vercel-ai-sdk
npm install
npm run start
```

#### TypeScript Developer (Existing Project - LangChainJS)
```bash
cd with_langchain-js
npm install
npm run start
```

#### Advanced Example (LangChainJS)
```bash
cd with_langchain-js
npm run advanced
```

### What You Get
- A minimal agent per framework that exposes a `run_skill` tool
- A single advanced example (LangChainJS) using one tool per skill
- Skills loaded from `examples/skills`

### Basic Integration (TypeScript)
```typescript
import { OpenSkillRuntime } from "@finogeek/openskills";

const runtime = OpenSkillRuntime.fromDirectory("./examples/skills");
runtime.discoverSkills();

const skills = runtime.listSkills();
console.log(skills.map((s) => s.id));

const result = runtime.executeSkill("example-skill", {
  input: JSON.stringify({ query: "hello" }),
  timeout_ms: 5000,
});
console.log(result.outputJson);
```

### Basic Integration (Python)
```python
from openskills import OpenSkillRuntime

runtime = OpenSkillRuntime.from_directory("./examples/skills")
runtime.discover_skills()

skills = runtime.list_skills()
print([s["id"] for s in skills])

result = runtime.execute_skill(
    "example-skill",
    input={"query": "hello"},
    timeout_ms=5000,
)
print(result.get("output", ""))
```

### Next Steps
- Read the integration guide: `GUIDE.md`
- Browse the examples: `with_langchain-js/`, `with_langchain-python/`, `with_vercel-ai-sdk/`
- Build your own skills under `examples/skills`
