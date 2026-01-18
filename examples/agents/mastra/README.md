## Mastra + OpenSkills

This example wraps OpenSkills skills as a Mastra tool. If your Mastra version
uses different imports or tool helpers, update the imports accordingly.

### Setup
```bash
cd examples/agents/mastra
npm install
export OPENAI_API_KEY=...
```

### Run
```bash
npm run start
```

### Notes
- Skills are loaded from `examples/skills`
- Ensure each skill has a built `wasm/skill.wasm`
