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
import { streamText } from "ai";
import { createOpenAI } from "@ai-sdk/openai";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// =============================================================================
// CLI Argument Parsing
// =============================================================================

function parseArgs() {
  const args = process.argv.slice(2);
  let skillsDir: string | undefined;
  let query: string | undefined;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--skills-dir' && args[i + 1]) {
      skillsDir = args[++i];
    } else if (!args[i].startsWith('--')) {
      query = args[i];
    }
  }

  return { skillsDir, query };
}

const cliArgs = parseArgs();

// =============================================================================
// Configuration
// =============================================================================

// Resolve skills directory: CLI flag or default
function resolveSkillsDir(): string {
  if (cliArgs.skillsDir) {
    // CLI argument - resolve relative to cwd
    return path.resolve(process.cwd(), cliArgs.skillsDir);
  }
  // Default - relative to this file's location
  return path.resolve(__dirname, "..", "..", "..", "skills");
}

const config = {
  provider: process.env.LLM_PROVIDER || "deepseek",
  model: process.env.LLM_MODEL || "deepseek-chat",
  apiKey: process.env.DEEPSEEK_API_KEY,
  skillsDir: resolveSkillsDir(),
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

// Host policy configuration (Layer 2)
// Change these values to test different permission scenarios.
// Resolution order: deny > allow > skill trust > fallback
runtime.setHostPolicy(
  false,     // trust_skill_allowed_tools: do not honor skill's allowed-tools declarations
  "prompt",   // fallback: "allow" | "deny" | "prompt"
  [],       // deny: tools to block regardless of other settings
  [],       // allow: tools to grant regardless of fallback
);

// Permission mode controls what happens when fallback is "prompt":
// - "cli": interactive terminal prompt (y/n/always)
// - "deny-all": silently deny all prompted permissions
// - "allow-all": silently approve all (default if not set)
runtime.setPermissionMode("cli");

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
  const userQuery = cliArgs.query ||
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

      // Track progress
      let stepCount = 0;
      const toolCalls: Map<string, { toolName: string; startTime: number }> = new Map();
      let accumulatedText = "";

      const result = streamText({
        model,
        system: systemPrompt,
        prompt: userQuery,
        tools,
        maxSteps: config.maxSteps,
        onStepFinish: (step) => {
          stepCount++;
          const timestamp = new Date().toLocaleTimeString();
          console.log(`\n📊 Step ${stepCount}/${config.maxSteps} completed ${timestamp}`);
        },
      });

      // Stream the text output and track tool calls in real-time
      console.log("\n" + "=".repeat(70));
      console.log("💬 Agent Response (streaming):");
      console.log("=".repeat(70));
      
      // Process the full stream to track tool calls
      const streamPromise = (async () => {
        for await (const delta of result.fullStream) {
          if (delta.type === 'tool-call-streaming-start') {
            const startTime = Date.now();
            const toolCallId = delta.toolCallId;
            toolCalls.set(toolCallId, {
              toolName: delta.toolName,
              startTime,
            });
            console.log(`\n🔧 Calling tool: ${delta.toolName}`);
          } else if (delta.type === 'step-finish') {
            // Tool results are available after step finishes
            // We'll show them in the summary at the end
          }
        }
      })();
      
      // Stream text output
      for await (const textPart of result.textStream) {
        process.stdout.write(textPart);
        accumulatedText += textPart;
      }
      console.log("\n");

      // Wait for stream processing to complete
      await streamPromise;

      // Get final result for tool calls/results summary
      // Use 'steps' to get ALL tool calls from ALL steps (not just the last step)
      const finalResult = await result;
      const allSteps = await finalResult.steps;

      // Collect all tool calls and results from all steps
      const allToolCalls: Array<{ toolName: string; args: unknown }> = [];
      const allToolResults: Array<{ toolName: string; toolCallId?: string; result: unknown }> = [];

      if (allSteps && Array.isArray(allSteps)) {
        for (const step of allSteps) {
          if (step.toolCalls && Array.isArray(step.toolCalls)) {
            allToolCalls.push(...step.toolCalls);
          }
          if (step.toolResults && Array.isArray(step.toolResults)) {
            allToolResults.push(...step.toolResults);
          }
        }
      }

      if (allToolCalls.length > 0) {
        console.log("\n" + "=".repeat(70));
        console.log(`🔧 Tool Calls Summary (${allToolCalls.length} across ${allSteps?.length || 0} steps):`);
        console.log("=".repeat(70));
        for (const call of allToolCalls) {
          const argsPreview = JSON.stringify(call.args).slice(0, 150);
          console.log(`\n  ${call.toolName}:`);
          console.log(`    Args: ${argsPreview}${argsPreview.length >= 150 ? '...' : ''}`);
        }
      }

      if (allToolResults.length > 0) {
        console.log("\n" + "=".repeat(70));
        console.log(`📋 Tool Results Summary (${allToolResults.length} across ${allSteps?.length || 0} steps):`);
        console.log("=".repeat(70));
        for (const res of allToolResults as Array<{ toolName: string; toolCallId?: string; result: unknown }>) {
          // Calculate duration if we tracked this tool call
          let duration = 0;
          if (res.toolCallId) {
            const toolCallInfo = toolCalls.get(res.toolCallId);
            if (toolCallInfo) {
              // Approximate duration (we don't have exact end time)
              duration = Date.now() - toolCallInfo.startTime;
            }
          }

          console.log(`\n✅ Tool result: ${res.toolName}${duration > 0 ? ` (${duration}ms)` : ''}`);
          const fullResult = typeof res.result === 'string'
            ? res.result
            : JSON.stringify(res.result, null, 2);
          const preview = fullResult.length > 500
            ? fullResult.slice(0, 500) + '\n    ... (truncated)'
            : fullResult;
          console.log(`   Result: ${preview}`);

          // Special handling for run_skill_script to verify WASM usage
          if (res.toolName === 'run_skill_script') {
            try {
              const parsed = typeof res.result === 'string' ? JSON.parse(res.result) : res.result;
              if (parsed && typeof parsed === 'object' && parsed !== null && 'output' in parsed) {
                const output = typeof parsed.output === 'string' ? JSON.parse(parsed.output) : parsed.output;
                if (output && typeof output === 'object' && output !== null && 'files' in output) {
                  const files = output.files;
                  if (files && typeof files === 'object' && files !== null && !Array.isArray(files)) {
                    console.log(`\n    ✅ WASM module was used! Returned ${Object.keys(files).length} files.`);
                  } else {
                    console.log(`\n    ⚠️  WASM output contains 'files' but it's not a valid object - may not have used WASM module correctly.`);
                  }
                } else {
                  console.log(`\n    ⚠️  WASM output doesn't contain 'files' object - may not have used WASM module correctly.`);
                }
              }
            } catch (e) {
              // Not JSON, ignore
            }
          }
        }
      }

      console.log("\n" + "=".repeat(70));
      console.log("✅ Agent execution completed successfully");
      console.log(`📊 Total steps: ${stepCount}`);
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
