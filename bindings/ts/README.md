# OpenSkills TypeScript Bindings

TypeScript/Node.js bindings for OpenSkills Runtime - Claude Skills compatible runtime with WASM sandbox.

## Installation

```bash
npm install @finogeek/openskills
```

## Usage

### Basic Runtime API

```typescript
import { OpenSkillRuntime } from '@finogeek/openskills';

// Create runtime (discovers from standard locations)
const runtime = new OpenSkillRuntime();

// Or specify project root
const runtime = OpenSkillRuntime.withProjectRoot('/path/to/project');

// Or load from specific directory
const runtime = OpenSkillRuntime.fromDirectory('/path/to/skills');

// Discover skills from standard locations
const skills = runtime.discoverSkills();
console.log(`Found ${skills.length} skills`);

// List skills (progressive disclosure)
for (const skill of runtime.listSkills()) {
  console.log(`${skill.id}: ${skill.description}`);
}

// Activate a skill (load full content)
const loaded = runtime.activateSkill('my-skill');
console.log(loaded.instructions);

// Execute WASM module
const result = runtime.executeSkill('my-skill', {
  timeoutMs: 5000,
  input: JSON.stringify({ query: 'hello' })
});

console.log(result.output_json);
console.log(result.audit);

// Check tool permissions
const canRead = runtime.isToolAllowed('my-skill', 'Read');
```

### ⭐ Pre-built Tools (Recommended)

For agent integration, use pre-built tools that eliminate boilerplate:

```typescript
import { OpenSkillRuntime } from '@finogeek/openskills';
import { createSkillTools, getAgentSystemPrompt } from '@finogeek/openskills/tools';
import { generateText } from 'ai';

// Initialize runtime
const runtime = OpenSkillRuntime.fromDirectory('./skills');
runtime.discoverSkills();

// Create pre-built tools (~200 lines less code)
const tools = createSkillTools(runtime, {
  workspaceDir: './output'  // Optional: sandboxed workspace
});

// Get skill-agnostic system prompt
const systemPrompt = getAgentSystemPrompt(runtime);

// Use with Vercel AI SDK
const result = await generateText({
  model: yourModel,
  system: systemPrompt,
  prompt: userQuery,
  tools,
});
```

**Available Tools:**
- `list_skills` - List available skills
- `activate_skill` - Load full SKILL.md instructions
- `read_skill_file` - Read helper files from skills
- `list_skill_files` - List files in skill directories
- `run_skill_script` - Execute sandboxed scripts or WASM modules
- `run_sandboxed_bash` - Run sandboxed bash commands
- `write_file` - Write to workspace (with security validation)
- `read_file` - Read from workspace (with security validation)
- `list_workspace_files` - List files in workspace
- `get_file_info` - Get file metadata

**Benefits:**
- ✅ **~200 lines less code**: No manual tool definitions
- ✅ **Security built-in**: Path validation, workspace isolation
- ✅ **Workspace management**: Automatic sandboxed file I/O
- ✅ **Skill-agnostic**: Works with any skill without code changes

See [examples/agents/simple](examples/agents/simple/) for a complete example.

## API

See `index.d.ts` for full TypeScript definitions. See `tools.d.ts` for pre-built tools API.
