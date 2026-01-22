## LangChain (Python) + OpenSkills

This example exposes OpenSkills skills as a LangChain tool and lets the agent
invoke them on demand.

### Setup
```bash
cd examples/agents/with_langchain-python
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
export OPENAI_API_KEY=...
```

### Run
```bash
python main.py
```

### Notes
- Skills are loaded from `examples/skills`
- Ensure each skill has a built `wasm/skill.wasm`
