# OpenSkills Python Bindings

Python bindings for OpenSkills Runtime - Claude Skills compatible runtime with WASM sandbox.

## Installation

```bash
pip install finclip-openskills
```

**Note:** Pre-built wheels are available for **macOS and Linux only**. Windows users need to build from source:

```bash
git clone https://github.com/Geeksfino/openskills.git
cd openskills/bindings/python
pip install maturin
maturin develop
```

## Usage

### Basic Runtime API

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

### ⭐ Pre-built Tools (Recommended)

For agent integration, use pre-built tools that eliminate boilerplate:

**LangChain Integration:**
```python
from openskills import OpenSkillRuntime
from openskills_tools import create_langchain_tools, get_agent_system_prompt

# Initialize runtime
runtime = OpenSkillRuntime.from_directory('./skills')
runtime.discover_skills()

# Create LangChain-compatible tools
tools = create_langchain_tools(runtime, workspace_dir='./output')

# Get skill-agnostic system prompt
system_prompt = get_agent_system_prompt(runtime)

# Use with LangChain
from langchain.agents import create_agent
agent = create_agent(model, tools, system_prompt=system_prompt)
```

**Framework-Agnostic (Simple Functions):**
```python
from openskills import OpenSkillRuntime
from openskills_tools import create_simple_tools

runtime = OpenSkillRuntime.from_directory('./skills')
runtime.discover_skills()

# Create simple callable functions (works with any framework)
tools = create_simple_tools(runtime, workspace_dir='./output')

# Use tools directly
skills = tools['list_skills']()
loaded = tools['activate_skill']('my-skill')
tools['write_file']('output.txt', 'Hello, World!')
```

**Available Tools:**
- `list_skills` - List available skills
- `activate_skill` - Load full SKILL.md instructions
- `read_skill_file` - Read helper files from skills
- `list_skill_files` - List files in skill directories
- `run_skill_script` - Execute sandboxed Python/shell scripts
- `write_file` - Write to workspace (with security validation)
- `read_file` - Read from workspace (with security validation)
- `list_workspace_files` - List files in workspace
- `get_file_info` - Get file metadata

**Benefits:**
- ✅ **~200 lines less code**: No manual tool definitions
- ✅ **Security built-in**: Path validation, workspace isolation
- ✅ **Workspace management**: Automatic sandboxed file I/O
- ✅ **Skill-agnostic**: Works with any skill without code changes
