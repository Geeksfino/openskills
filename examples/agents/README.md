## Example Agents

This folder shows how to wire the OpenSkills runtime into popular agent
frameworks so both TypeScript and Python developers can reuse the same skills.

### Prerequisites
- Build or place skills under `examples/skills` (see `runtime/BUILD.md`)
- Install the runtime bindings for your language:
  - TypeScript: `npm install @openskills/runtime`
  - Python: `pip install openskills`

### Examples
- `langchain-js`: LangChainJS agent that calls OpenSkills skills as tools
- `langchain-python`: LangChain (Python) agent that calls OpenSkills skills
- `mastra`: Mastra agent example with OpenSkills tool wrapper

Each example reads skills from `examples/skills`, lists them, and exposes a
`run_skill` tool that executes skills via the OpenSkills runtime.

### Docs
- `QUICKSTART.md`: 5-minute setup across frameworks
- `GUIDE.md`: integration patterns and best practices
