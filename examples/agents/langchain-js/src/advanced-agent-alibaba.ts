/**
 * Advanced OpenSkills + LangChain.js agent with Alibaba Tongyi (Qwen) model.
 *
 * Demonstrates:
 * - Individual tools per skill (better reasoning)
 * - Multiple skills working together
 * - Skill metadata in the system prompt
 * - Using Alibaba Tongyi instead of OpenAI
 */

import "dotenv/config";
import path from "node:path";
import { fileURLToPath } from "node:url";
// 使用阿里云通义千问模型
import { ChatAlibabaTongyi } from "@langchain/community/chat_models/alibaba_tongyi";
import { AgentExecutor, createToolCallingAgent } from "langchain/agents";
import { ChatPromptTemplate } from "@langchain/core/prompts";
import { createOpenSkillsTools, getSkillMetadata } from "./openskills-tool";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const skillsDir = path.resolve(__dirname, "..", "..", "..", "skills");

async function createAdvancedAgent() {
  console.log("Creating individual tools for each skill...");
  const tools = createOpenSkillsTools({ skillsDir });
  console.log(`Created ${tools.length} tools:`);
  tools.forEach((tool) => console.log(`  - ${tool.name}`));
  console.log();

  const skillMetadata = getSkillMetadata(skillsDir);

  // 使用阿里云通义千问模型
  const llm = new ChatAlibabaTongyi({
    modelName: "qwen-turbo", // 可选: "qwen-plus", "qwen-max"
    temperature: 0,
    alibabaApiKey: process.env.DASHSCOPE_API_KEY, // 从环境变量读取
  });

  const systemPrompt = `You are an expert assistant with specialized skills.

${skillMetadata}

When a user asks for help:
1. Analyze the request
2. Choose the best skill(s)
3. Use the skill tool(s) to complete the task
4. Provide a concise response

Guidelines:
- Use skills when they add value
- Combine multiple skills when helpful
- Explain briefly why you used each skill`;

  const prompt = ChatPromptTemplate.fromMessages([
    ["system", systemPrompt],
    ["human", "{input}"],
    ["placeholder", "{agent_scratchpad}"],
  ]);

  const agent = await createToolCallingAgent({
    llm,
    tools,
    prompt,
  });

  return new AgentExecutor({
    agent,
    tools,
    verbose: true,
    maxIterations: 10,
  });
}

async function main() {
  // 检查 API Key
  if (!process.env.DASHSCOPE_API_KEY) {
    console.error("Error: DASHSCOPE_API_KEY is not set in .env file");
    console.error("Please set your Alibaba DashScope API Key:");
    console.error('  DASHSCOPE_API_KEY=sk-your-alibaba-key-here');
    process.exit(1);
  }

  const agent = await createAdvancedAgent();

  console.log("=".repeat(60));
  console.log("Example 1: Code Review Task");
  console.log("=".repeat(60));

  const result1 = await agent.invoke({
    input: `Review this TypeScript function for security issues:
\`\`\`typescript
function processInput(userData: any) {
  return eval(userData);
}
\`\`\``,
  });

  console.log("\nAgent Response:");
  console.log(result1.output);
  console.log();

  console.log("=".repeat(60));
  console.log("Example 2: Multiple Skills Working Together");
  console.log("=".repeat(60));

  const result2 = await agent.invoke({
    input: `Explain what a TypeScript async function is and why it's useful.
Then review this code for best practices:
\`\`\`typescript
async function fetchData(url: string) {
  const response = await fetch(url);
  return response.json();
}
\`\`\``,
  });

  console.log("\nAgent Response:");
  console.log(result2.output);
  console.log();
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}
