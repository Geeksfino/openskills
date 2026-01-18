# OpenSkills TypeScript Bindings

TypeScript/Node.js bindings for OpenSkills Runtime - Claude Skills compatible runtime with WASM sandbox.

## Installation

```bash
npm install @finogeek/openskills
```

## Usage

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

## API

See `index.d.ts` for full TypeScript definitions.
