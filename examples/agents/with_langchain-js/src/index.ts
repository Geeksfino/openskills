import path from "node:path";
import { fileURLToPath } from "node:url";
import { OpenSkillRuntime } from "@finogeek/openskills";
import { ChatOpenAI } from "@langchain/openai";
import { DynamicStructuredTool } from "@langchain/core/tools";
import { initializeAgentExecutorWithOptions } from "langchain/agents";
import { z } from "zod";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const skillsDir = path.resolve(__dirname, "..", "..", "..", "skills");

const runtime = OpenSkillRuntime.fromDirectory(skillsDir);
runtime.discoverSkills();

const catalog = runtime
  .listSkills()
  .map((skill) => `- ${skill.id}: ${skill.description}`)
  .join("\n");

const runSkillTool = new DynamicStructuredTool({
  name: "run_skill",
  description: "Execute an OpenSkills skill by id with a text input.",
  schema: z.object({
    skill_id: z.string(),
    input: z.string(),
  }),
  func: async ({ skill_id, input }) => {
    const result = runtime.executeSkill(skill_id, {
      timeout_ms: 5000,
      input: JSON.stringify({ query: input }),
    });
    return result.outputJson ?? "";
  },
});

const llm = new ChatOpenAI({
  model: "gpt-4o-mini",
  temperature: 0,
});

const executor = await initializeAgentExecutorWithOptions(
  [runSkillTool],
  llm,
  {
    agentType: "openai-functions",
    verbose: true,
  }
);

const response = await executor.invoke({
  input: [
    "You can call run_skill to execute OpenSkills skills.",
    "Available skills:",
    catalog,
    "",
    "User request: Summarize the following text using an appropriate skill:",
    "OpenSkills provides a WASM runtime for Claude-compatible skills.",
  ].join("\n"),
});

console.log(response.output);
