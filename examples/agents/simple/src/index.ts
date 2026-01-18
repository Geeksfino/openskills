import "dotenv/config";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import { OpenSkillRuntime } from "@finogeek/openskills";
// Use ai SDK directly for tool calling with DeepSeek
import { generateText, tool } from "ai";
import { createOpenAI } from "@ai-sdk/openai";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Load configuration from environment variables
const LLM_PROVIDER = process.env.LLM_PROVIDER || "deepseek";
const LLM_MODEL = process.env.LLM_MODEL || "deepseek-chat";
const DEEPSEEK_API_KEY = process.env.DEEPSEEK_API_KEY;

if (!DEEPSEEK_API_KEY) {
  console.error("Error: DEEPSEEK_API_KEY is not set in .env file");
  process.exit(1);
}

// DeepSeek uses OpenAI-compatible API, so we use the OpenAI provider with custom baseURL

// Load skills from claude-official-skills directory
const claudeSkillsDir = path.resolve(
  __dirname,
  "..",
  "..",
  "..",
  "claude-official-skills",
  "skills"
);

const runtime = OpenSkillRuntime.fromDirectory(claudeSkillsDir);
runtime.discoverSkills();

// Get the docx skill specifically
const skills = runtime.listSkills();
const docxSkill = skills.find((s) => s.id === "docx");

if (!docxSkill) {
  console.error("Error: docx skill not found in claude-official-skills/skills");
  process.exit(1);
}

const catalog = skills
  .map((skill) => `- ${skill.id}: ${skill.description}`)
  .join("\n");

// Create OpenAI client configured for DeepSeek
const openai = createOpenAI({
  apiKey: DEEPSEEK_API_KEY,
  baseURL: "https://api.deepseek.com/v1",
});

// Enhanced skill execution tool that supports docx operations
const runSkill = tool({
  description: `Execute an OpenSkills skill by id. The 'docx' skill is available for creating, editing, and analyzing Word documents (.docx files).`,
  inputSchema: z.object({
    skill_id: z.string().describe("The skill ID to execute (e.g., 'docx' for Word document operations)"),
    input: z.string().describe("Input text or query for the skill. For docx skill, describe what document you want to create or edit."),
  }),
  execute: async ({ skill_id, input }) => {
    try {
      const result = runtime.executeSkill(skill_id, {
        timeout_ms: 30000, // Increased timeout for docx operations
        input: JSON.stringify({ query: input }),
      });
      return result.outputJson ?? "";
    } catch (error) {
      return `Error executing skill ${skill_id}: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});

const systemPrompt = [
  "You are a helpful assistant specialized in creating and editing Word documents (.docx files).",
  "",
  "You have access to the 'docx' skill which can help you:",
  "- Create new Word documents from scratch",
  "- Edit existing Word documents",
  "- Analyze document contents",
  "- Work with tracked changes and comments",
  "",
  "When a user asks you to create or edit a Word document:",
  "1. Use the 'docx' skill by calling run_skill with skill_id='docx'",
  "2. Provide clear instructions in the input about what document to create or what changes to make",
  "3. The skill will guide you through the process using docx-js for new documents or OOXML editing for existing ones",
  "",
  "Available skills:",
  catalog,
  "",
  "Always be helpful and provide clear explanations of what you're doing.",
].join("\n");

// Example: Create a Word document
async function main() {
  const userQuery = process.argv[2] || 
    "Create a Word document with a title page, table of contents, and a section about OpenSkills runtime capabilities. Include headings and formatted text.";

  console.log("🤖 OpenSkills DocX Assistant");
  console.log("Using model:", LLM_MODEL);
  console.log("Using provider:", LLM_PROVIDER);
  console.log("\n📝 User request:", userQuery);
  console.log("\n" + "=".repeat(60) + "\n");

  try {
    const result = await generateText({
      model: openai(LLM_MODEL) as any,
      system: systemPrompt,
      prompt: userQuery,
      tools: {
        run_skill: runSkill,
      },
    });
    console.log("\n" + "=".repeat(60));
    console.log("\n✅ Response:");
    console.log(result.text);
    if (result.toolCalls && result.toolCalls.length > 0) {
      console.log("\n🔧 Tool calls made:", result.toolCalls.length);
    }
  } catch (error) {
    console.error("\n❌ Error:", error);
    process.exit(1);
  }
}

main();
