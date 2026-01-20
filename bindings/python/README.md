# OpenSkills Python Bindings

Python bindings for OpenSkills Runtime - Claude Skills compatible runtime with WASM sandbox.

## Installation

```bash
pip install finclip-openskills
```

## Usage

```python
from openskills import OpenSkillRuntime

# Create runtime (discovers from standard locations)
runtime = OpenSkillRuntime()

# Or specify project root
runtime = OpenSkillRuntime.with_project_root('/path/to/project')

# Or load from specific directory
runtime = OpenSkillRuntime.from_directory('/path/to/skills')

# Discover skills from standard locations
skills = runtime.discover_skills()
print(f"Found {len(skills)} skills")

# List skills (progressive disclosure)
for skill in runtime.list_skills():
    print(f"{skill['id']}: {skill['description']}")

# Activate a skill (load full content)
loaded = runtime.activate_skill('my-skill')
print(loaded['instructions'])

# Execute WASM module
result = runtime.execute_skill(
    'my-skill',
    input={'query': 'hello'},
    timeout_ms=5000
)

print(result['output'])
print(result['audit'])

# Check tool permissions
can_read = runtime.is_tool_allowed('my-skill', 'Read')
```
