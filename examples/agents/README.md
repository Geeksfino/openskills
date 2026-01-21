## Example Agents

This folder shows how to wire the OpenSkills runtime into popular agent
frameworks so both TypeScript and Python developers can reuse the same skills.

### ⭐ Recommended: Simple Example

The **`simple`** example demonstrates the **recommended approach** using pre-built tools:

- ✅ **~120 lines total** (vs ~470 lines manually)
- ✅ **Pre-built tools**: Uses `createSkillTools()` from `@finogeek/openskills/tools`
- ✅ **Skill-agnostic**: No hardcoded skill knowledge
- ✅ **Workspace management**: Automatic sandboxed file I/O
- ✅ **System prompt generation**: Runtime generates skill-agnostic prompts

```typescript
import { createSkillTools, getAgentSystemPrompt } from '@finogeek/openskills/tools';

// Create all tools in one call (~200 lines less code)
const tools = createSkillTools(runtime, { workspaceDir: './output' });

// Get skill-agnostic system prompt
const systemPrompt = getAgentSystemPrompt(runtime);
```

See [simple/README.md](simple/README.md) for details.

### Prerequisites
- Build or place skills under `examples/skills` (see `runtime/BUILD.md`)
- Install the runtime bindings for your language:
  - TypeScript: `npm install @finogeek/openskills`
  - Python: `pip install finclip-openskills`

### Examples
- **`simple`** ⭐: **Recommended** - Vercel AI SDK with pre-built tools (~120 lines)
- `langchain-js`: LangChainJS agent (manual tool definitions)
- `langchain-python`: LangChain (Python) agent with pre-built tools

### Key Improvements

**Before (Manual Setup):**
- ~470 lines of tool definitions
- Manual workspace management
- Custom system prompts
- Skill-specific knowledge hardcoded

**After (Pre-built Tools):**
- ~120 lines total
- Automatic workspace management
- Runtime-generated system prompts
- Skill-agnostic design

### Docs
- `QUICKSTART.md`: 5-minute setup across frameworks
- `GUIDE.md`: integration patterns and best practices
- `simple/README.md`: Complete example using pre-built tools