## OpenSkills + AI SDK + DeepSeek (Claude Skills Compatible)

This example demonstrates using OpenSkills with Vercel's AI SDK, configured to use DeepSeek Chat as the LLM provider. It properly implements the Claude Skills pattern: instruction-based skills where the LLM reads and follows the SKILL.md instructions.

### How Claude Skills Work

1. **Discovery**: Agent discovers skills from the skills directory
2. **Activation**: When a request matches a skill, the agent calls `activateSkill()` to get the full SKILL.md instructions
3. **Follow Instructions**: The LLM reads and follows the instructions step-by-step
4. **Use Scripts**: Skills may include Python scripts that the LLM runs as instructed

### Features

- ✅ Uses DeepSeek Chat model via OpenAI-compatible API
- ✅ Proper Claude Skills implementation (instruction-based, not execution-based)
- ✅ `activate_skill` tool to load SKILL.md instructions
- ✅ `list_skills` tool to discover available skills
- ✅ Environment-based configuration via `.env` file

### Setup

1. **Install dependencies:**
```bash
cd examples/agents/simple
npm install
```

2. **Configure environment variables:**
```bash
cp .env.example .env
```

Then edit `.env` and add your DeepSeek API key:
```env
LLM_PROVIDER=deepseek
LLM_MODEL=deepseek-chat
DEEPSEEK_API_KEY=your_deepseek_api_key_here
```

3. **Get a DeepSeek API key:**
   - Visit https://platform.deepseek.com/
   - Sign up or log in
   - Navigate to API keys section
   - Create a new API key
   - Copy it to your `.env` file

### Run

**Basic usage:**
```bash
npm run start
```

**With custom query:**
```bash
npm run start "Create a Word document with a professional report about AI agents"
```

### Example Queries

- "Create a Word document with a title page and table of contents about OpenSkills"
- "Generate a Word document with formatted headings and a summary section"
- "Create a professional report document with multiple sections and proper formatting"

### How It Works

1. The agent loads the `docx` skill from `examples/claude-official-skills/skills/docx`
2. When you request a Word document, the agent uses the docx skill
3. The skill provides instructions for creating documents using docx-js or editing using OOXML
4. The agent follows the skill's guidance to help you create the document

### Notes

- Skills are loaded from `examples/claude-official-skills/skills`
- Claude Skills are instruction-based: SKILL.md contains instructions for the LLM to follow
- The agent calls `activateSkill()` to get full instructions, not `executeSkill()`
- Skills may reference helper files (docx-js.md, ooxml.md) and Python scripts
- DeepSeek uses OpenAI-compatible API with `baseURL: "https://api.deepseek.com/v1"`
- Uses AI SDK v4 for DeepSeek compatibility (v6 has breaking changes)
