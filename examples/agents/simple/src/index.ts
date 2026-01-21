/**
 * Simple OpenSkills Agent - Claude Skills Compatible
 *
 * This agent demonstrates the correct pattern for using Claude Skills:
 * - Skill-agnostic: no hardcoded knowledge about any specific skill
 * - Runtime-provided tools: uses pre-built tools from @finogeek/openskills/tools
 * - Generic system prompt: teaches the agent HOW to use skills, not WHAT they do
 *
 * The agent:
 * 1. Discovers available skills at startup
 * 2. Matches user requests to skills via semantic understanding
 * 3. Activates matching skills to get full instructions from SKILL.md
 * 4. Follows the instructions exactly - all domain knowledge comes from the skill
 */

import "dotenv/config";
import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { OpenSkillRuntime } from "@finogeek/openskills";
import { createSkillTools, getAgentSystemPrompt } from "@finogeek/openskills/tools";
import { generateText } from "ai";
import { createOpenAI } from "@ai-sdk/openai";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// =============================================================================
// Configuration
// =============================================================================

const config = {
  provider: process.env.LLM_PROVIDER || "deepseek",
  model: process.env.LLM_MODEL || "deepseek-chat",
  apiKey: process.env.DEEPSEEK_API_KEY,
  skillsDir: path.resolve(__dirname, "..", "..", "..", "skills"),
  workspaceDir: path.resolve(__dirname, "..", "output"),
  maxSteps: parseInt(process.env.MAX_STEPS || "20", 10),
  maxRetries: parseInt(process.env.MAX_RETRIES || "3", 10),
  timeout: parseInt(process.env.API_TIMEOUT || "300000", 10), // 5 minutes default
};

if (!config.apiKey) {
  console.error("Error: DEEPSEEK_API_KEY is not set in .env file");
  console.error("Please create a .env file with your DeepSeek API key:");
  console.error("  DEEPSEEK_API_KEY=your-api-key-here");
  process.exit(1);
}

// =============================================================================
// Initialize Runtime and Tools
// =============================================================================

console.log("🔧 Initializing OpenSkills runtime...");
const runtime = OpenSkillRuntime.fromDirectory(config.skillsDir);
runtime.discoverSkills();

const skills = runtime.listSkills();
if (skills.length === 0) {
  console.error("Error: No skills found in", config.skillsDir);
  process.exit(1);
}

// Ensure workspace directory exists
fs.mkdirSync(config.workspaceDir, { recursive: true });

// Create pre-built tools from the runtime
// This replaces ~200 lines of manual tool definitions
console.log("🔧 Creating skill tools...");
const tools = createSkillTools(runtime, {
  workspaceDir: config.workspaceDir,
});

// Get skill-agnostic system prompt from the runtime
// This teaches the agent HOW to use skills without any skill-specific knowledge
console.log("🔧 Generating system prompt...");
const systemPrompt = getAgentSystemPrompt(runtime);

// =============================================================================
// LLM Configuration
// =============================================================================

const openai = createOpenAI({
  apiKey: config.apiKey,
  baseURL: "https://api.deepseek.com/v1",
});

const model = openai(config.model);

// =============================================================================
// Main Agent Loop
// =============================================================================

async function main() {
  const userQuery = process.argv[2] || 
    "What skills are available? Then help me create a Word document with a title 'Hello World' and a paragraph of text.";

  console.log("\n" + "=".repeat(70));
  console.log("🤖 OpenSkills Agent (Claude Skills Compatible)");
  console.log("=".repeat(70));
  console.log("Model:", config.model);
  console.log("Provider:", config.provider);
  console.log("Skills directory:", config.skillsDir);
  console.log("Skills found:", skills.length);
  console.log("Workspace:", config.workspaceDir);
  console.log("Max steps:", config.maxSteps);
  console.log("\n📝 User request:", userQuery);
  console.log("\n" + "=".repeat(70) + "\n");

  // Retry logic for network errors
  let lastError: Error | null = null;
  for (let attempt = 1; attempt <= config.maxRetries; attempt++) {
    try {
      if (attempt > 1) {
        console.log(`\n🔄 Retry attempt ${attempt}/${config.maxRetries}...`);
        // Wait before retrying (exponential backoff)
        await new Promise(resolve => setTimeout(resolve, Math.min(1000 * Math.pow(2, attempt - 2), 10000)));
      }

    const result = await generateText({
        model,
      system: systemPrompt,
      prompt: userQuery,
        tools,
        maxSteps: config.maxSteps,
      });

      console.log("\n" + "=".repeat(70));
      console.log("✅ Agent Response:");
      console.log("=".repeat(70));
    console.log(result.text);
    
    if (result.toolCalls && result.toolCalls.length > 0) {
        console.log("\n" + "=".repeat(70));
        console.log(`🔧 Tool Calls (${result.toolCalls.length}):`);
        console.log("=".repeat(70));
      for (const call of result.toolCalls) {
          const argsPreview = JSON.stringify(call.args).slice(0, 150);
          console.log(`\n  ${call.toolName}:`);
          console.log(`    Args: ${argsPreview}${argsPreview.length >= 150 ? '...' : ''}`);
        }
      }

    if (result.toolResults && result.toolResults.length > 0) {
        console.log("\n" + "=".repeat(70));
        console.log(`📋 Tool Results (${result.toolResults.length}):`);
        console.log("=".repeat(70));
      for (const res of result.toolResults) {
        const preview = typeof res.result === 'string' 
            ? res.result.slice(0, 200) + (res.result.length > 200 ? '...' : '')
            : JSON.stringify(res.result).slice(0, 200);
          console.log(`\n  [${res.toolName}]:`);
          console.log(`    ${preview}`);
        }
      }

      console.log("\n" + "=".repeat(70));
      console.log("✅ Agent execution completed successfully");
      console.log("=".repeat(70) + "\n");
      
      // Success - break out of retry loop
      return;
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));
      
      // Check if it's a retryable error (network/timeout issues)
      const errorMessage = error instanceof Error ? error.message : String(error);
      const errorName = error instanceof Error ? error.name : '';
      const errorString = String(error);
      const errorCause = (error as any)?.cause;
      const causeMessage = errorCause instanceof Error ? errorCause.message : String(errorCause || '');
      const causeCode = (errorCause as any)?.code || '';
      
      const isRetryable = 
        errorMessage.includes('socket') ||
        errorMessage.includes('timeout') ||
        errorMessage.includes('ECONNRESET') ||
        errorMessage.includes('UND_ERR_SOCKET') ||
        errorMessage.includes('terminated') ||
        errorMessage.includes('Failed to process successful response') ||
        errorName === 'AbortError' ||
        causeMessage.includes('socket') ||
        causeMessage.includes('other side closed') ||
        causeCode === 'UND_ERR_SOCKET';
      
      if (!isRetryable || attempt === config.maxRetries) {
        // Not retryable or out of retries
        console.error("\n" + "=".repeat(70));
        console.error("❌ Error:");
        console.error("=".repeat(70));
        console.error(error);
        if (error instanceof Error) {
          console.error("\nError name:", error.name);
          console.error("Error message:", error.message);
          if (error.stack) {
            console.error("\nStack trace:");
            console.error(error.stack);
          }
        }
        
        // Enhanced error diagnostics
        console.error("\n" + "=".repeat(70));
        console.error("🔍 Error Diagnostics:");
        console.error("=".repeat(70));
        console.error("Full error object:", JSON.stringify(error, Object.getOwnPropertyNames(error), 2));
        if ((error as any)?.cause) {
          console.error("Error cause:", JSON.stringify((error as any).cause, Object.getOwnPropertyNames((error as any).cause), 2));
        }
        if ((error as any)?.response) {
          console.error("HTTP Response status:", (error as any).response?.status);
          console.error("HTTP Response headers:", JSON.stringify((error as any).response?.headers, null, 2));
        }
        if ((error as any)?.request) {
          console.error("HTTP Request details:", JSON.stringify((error as any).request, null, 2));
        }
        
        // If we have partial results, show them
        if (attempt < config.maxRetries) {
          console.error(`\n⚠️  Failed after ${attempt} attempts. This may be a network issue.`);
        }
        
        process.exit(1);
      } else {
        // Retryable error - log and continue to next attempt
        console.warn(`\n⚠️  Attempt ${attempt} failed (${error instanceof Error ? error.message : String(error)}). Retrying...`);
      }
    }
  }
  
  // Should never reach here, but just in case
  if (lastError) {
    throw lastError;
  }
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
