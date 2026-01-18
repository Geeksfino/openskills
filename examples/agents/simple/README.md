## OpenSkills + AI SDK + DeepSeek

This example demonstrates using OpenSkills with Vercel's AI SDK, configured to use DeepSeek Chat as the LLM provider and the official docx skill for Word document creation.

### Features

- ✅ Uses DeepSeek Chat model via OpenAI-compatible API
- ✅ Integrates with Claude official docx skill for Word document operations
- ✅ Environment-based configuration via `.env` file
- ✅ Supports creating, editing, and analyzing Word documents

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
- The docx skill is primarily instructional (no WASM module required)
- DeepSeek uses OpenAI-compatible API, so we configure it with `baseURL: "https://api.deepseek.com/v1"`
- The agent will guide you through the document creation process step by step
