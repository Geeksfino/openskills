## LangChainJS + OpenSkills

This example turns OpenSkills skills into a LangChainJS tool and lets the agent
call skills on demand.

### Setup
```bash
cd examples/agents/langchain-js
npm install
export OPENAI_API_KEY=...
```

### Run
```bash
npm run start
```

### Advanced Example
```bash
npm run advanced
```

### Notes
- Skills are loaded from `examples/skills`
- Ensure each skill has a built `wasm/skill.wasm`
