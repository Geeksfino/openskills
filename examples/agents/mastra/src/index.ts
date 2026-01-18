import path from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import { OpenSkillRuntime } from "@openskills/runtime";
import { Agent } from "@mastra/core";
import { createTool } from "@mastra/core/tools";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const skillsDir = path.resolve(__dirname, "..", "..", "..", "skills");

const runtime = OpenSkillRuntime.fromDirectory(skillsDir);
runtime.discoverSkills();

const catalog = runtime
  .listSkills()
  .map((skill) => `- ${skill.id}: ${skill.description}`)
  .join("\n");

const runSkill = createTool({
  name: "run_skill",
  description: "Execute an OpenSkills skill by id with a text input.",
  schema: z.object({
    skill_id: z.string(),
    input: z.string(),
  }),
  execute: async ({ skill_id, input }) => {
    const result = runtime.executeSkill(skill_id, {
      timeout_ms: 5000,
      input: JSON.stringify({ query: input }),
    });
    return result.output_json ?? result.output ?? "";
  },
});

const agent = new Agent({
  name: "openskills-mastra",
  instructions: [
    "You can call run_skill to execute OpenSkills skills.",
    "Available skills:",
    catalog,
  ].join("\n"),
  model: {
    provider: "openai",
    name: "gpt-4o-mini",
  },
  tools: [runSkill],
});

const response = await agent.run(
  "Summarize the following text using an appropriate skill: OpenSkills provides a WASM runtime for Claude-compatible skills."
);

console.log(response);
