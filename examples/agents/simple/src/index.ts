import "dotenv/config";
import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { z } from "zod";
import { OpenSkillRuntime, runSandboxedShellCommand } from "@finogeek/openskills";
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

// Get available skills
const skills = runtime.listSkills();

if (skills.length === 0) {
  console.error("Error: No skills found in", claudeSkillsDir);
  process.exit(1);
}

// Build skill catalog for the LLM
const catalog = skills
  .map((skill) => `- ${skill.id}: ${skill.description}`)
  .join("\n");

// Create OpenAI client configured for DeepSeek
// DeepSeek uses OpenAI-compatible API
const openai = createOpenAI({
  apiKey: DEEPSEEK_API_KEY,
  baseURL: "https://api.deepseek.com/v1",
});

// Create model instance using AI SDK v1 syntax
const model = openai(LLM_MODEL);

// Restrict file operations to this agent directory, plus the skills directory.
const agentRoot = path.resolve(__dirname);
const outputDir = path.resolve(agentRoot, "output");
fs.mkdirSync(outputDir, { recursive: true });

function assertAllowedPath(p: string): string {
  const abs = path.resolve(p);
  const agentOk = abs === agentRoot || abs.startsWith(agentRoot + path.sep);
  if (agentOk) return abs;

  const skillsAbs = path.resolve(claudeSkillsDir);
  const skillsOk = abs === skillsAbs || abs.startsWith(skillsAbs + path.sep);
  if (skillsOk) return abs;

  throw new Error(
    `Path not allowed: ${p}. Allowed roots: ${agentRoot} and ${claudeSkillsDir}`
  );
}

/**
 * Tool: Activate a skill to get its full instructions.
 * 
 * Claude Skills are instruction-based: the SKILL.md contains instructions
 * that the LLM should read and follow. This tool loads those instructions.
 */
const activateSkillTool = tool({
  description: `Activate a Claude Skill to get its full instructions. Use this when the user's request matches a skill's capabilities. The instructions will tell you how to complete the task.`,
  parameters: z.object({
    skill_id: z.string().describe("The skill ID to activate"),
  }),
  execute: async ({ skill_id }) => {
    try {
      const loaded = runtime.activateSkill(skill_id);
      return JSON.stringify({
        id: loaded.id,
        name: loaded.name,
        allowedTools: loaded.allowedTools,
        instructions: loaded.instructions,
      });
    } catch (error) {
      return `Error activating skill ${skill_id}: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});

/**
 * Tool: List available skills.
 */
const listSkillsTool = tool({
  description: `List all available Claude Skills with their IDs and descriptions.`,
  parameters: z.object({
    query: z.string().describe("Search query to filter skills, or 'all' to list all"),
  }),
  execute: async ({ query }) => {
    const skillList = runtime.listSkills();
    const filtered = query === "all" 
      ? skillList 
      : skillList.filter(s => 
          s.id.includes(query) || s.description.toLowerCase().includes(query.toLowerCase())
        );
    return JSON.stringify(filtered.map(s => ({
      id: s.id,
      description: s.description,
    })));
  },
});

/**
 * Tool: Read a file from a skill directory (via runtime API).
 * 
 * This is more secure than raw file read - it's sandboxed to the skill directory.
 */
const readSkillFileTool = tool({
  description: `Read a file from a skill's directory. Use this to read helper files (like docx-js.md, ooxml.md) referenced in SKILL.md instructions. The path must be relative to the skill root.`,
  parameters: z.object({
    skill_id: z.string().describe("The skill ID (e.g., 'docx')"),
    path: z.string().describe("Relative path within the skill directory (e.g., 'docx-js.md', 'scripts/document.py')"),
  }),
  execute: async ({ skill_id, path: relativePath }) => {
    try {
      const content = runtime.readSkillFile(skill_id, relativePath);
      return content;
    } catch (error) {
      return `Error reading ${relativePath} from skill ${skill_id}: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});

/**
 * Tool: List files in a skill directory.
 */
const listSkillFilesTool = tool({
  description: `List files in a skill's directory. Use this to discover what files are available in a skill (scripts, docs, etc.).`,
  parameters: z.object({
    skill_id: z.string().describe("The skill ID (e.g., 'docx')"),
    subdir: z.string().optional().describe("Optional subdirectory (e.g., 'scripts', 'ooxml/scripts')"),
    recursive: z.boolean().optional().describe("Whether to list recursively (default: false)"),
  }),
  execute: async ({ skill_id, subdir, recursive }) => {
    try {
      const files = runtime.listSkillFiles(skill_id, subdir, recursive ?? false);
      return JSON.stringify(files, null, 2);
    } catch (error) {
      return `Error listing files in skill ${skill_id}: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});

/**
 * Tool: Run a script from a skill in the sandbox.
 * 
 * This executes a Python or Shell script from a skill directory
 * in the runtime's native sandbox (seatbelt on macOS).
 */
const runSkillScriptTool = tool({
  description: `Run a Python or Shell script from a skill directory in the sandbox. Use this when SKILL.md instructions say to run a script (e.g., "run python ooxml/scripts/unpack.py"). The script runs in a secure sandbox with the skill's allowed permissions.`,
  parameters: z.object({
    skill_id: z.string().describe("The skill ID (e.g., 'docx')"),
    script_path: z.string().describe("Path to the script relative to skill root (e.g., 'ooxml/scripts/unpack.py')"),
    args: z.array(z.string()).optional().describe("Arguments to pass to the script"),
    timeout_ms: z.number().optional().describe("Timeout in milliseconds (default: 30000)"),
  }),
  execute: async ({ skill_id, script_path, args, timeout_ms }) => {
    try {
      const result = runtime.runSkillTarget(skill_id, {
        targetType: "script",
        path: script_path,
        args: args ?? [],
        timeoutMs: timeout_ms ?? 30000,
      });
      return JSON.stringify({
        stdout: result.stdout,
        stderr: result.stderr,
        output: result.outputJson,
      }, null, 2);
    } catch (error) {
      return `Error running ${script_path} from skill ${skill_id}: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});

/**
 * Tool: Read a UTF-8 text file (for SKILL.md / referenced docs).
 */
const readFileTool = tool({
  description:
    "Read a UTF-8 text file from disk. Use this to read SKILL.md and referenced docs before acting.",
  parameters: z.object({
    path: z
      .string()
      .describe(
        "Absolute path under the allowed roots, or a path relative to the agent directory"
      ),
  }),
  execute: async ({ path: p }) => {
    const tryPaths: string[] = [];
    if (path.isAbsolute(p)) {
      tryPaths.push(p);
    } else {
      tryPaths.push(path.join(agentRoot, p));
      tryPaths.push(path.join(claudeSkillsDir, p));
    }

    for (const candidate of tryPaths) {
      try {
        const abs = assertAllowedPath(candidate);
        if (fs.existsSync(abs) && fs.statSync(abs).isFile()) {
          return fs.readFileSync(abs, "utf-8");
        }
      } catch {
        // ignore and continue
      }
    }

    // Fallback: search by basename within skills directory (shallow bounded walk).
    const target = path.basename(p);
    const queue: Array<{ dir: string; depth: number }> = [
      { dir: claudeSkillsDir, depth: 0 },
    ];
    const maxDepth = 6;
    const maxVisited = 2000;
    let visited = 0;
    while (queue.length > 0 && visited < maxVisited) {
      const { dir, depth } = queue.shift()!;
      visited += 1;
      let entries: fs.Dirent[];
      try {
        entries = fs.readdirSync(dir, { withFileTypes: true });
      } catch {
        continue;
      }
      for (const ent of entries) {
        const full = path.join(dir, ent.name);
        if (ent.isFile() && ent.name === target) {
          const abs = assertAllowedPath(full);
          return fs.readFileSync(abs, "utf-8");
        }
        if (ent.isDirectory() && depth < maxDepth) {
          queue.push({ dir: full, depth: depth + 1 });
        }
      }
    }

    throw new Error(`File not found under allowed roots: ${p}`);
  },
});

/**
 * Tool: Write a UTF-8 text file (restricted to agent directory).
 */
const writeFileTool = tool({
  description:
    "Write a UTF-8 text file to disk (restricted to the agent directory). Use this to create scripts that generate outputs.",
  parameters: z.object({
    path: z
      .string()
      .describe("Path relative to the agent directory (e.g., output/create-docx.ts)"),
    content: z.string().describe("File contents"),
  }),
  execute: async ({ path: p, content }) => {
    const abs = assertAllowedPath(path.join(agentRoot, p));
    fs.mkdirSync(path.dirname(abs), { recursive: true });
    fs.writeFileSync(abs, content, "utf-8");
    return `Wrote ${abs}`;
  },
});

/**
 * Tool: Run a command (cwd = agent directory).
 */
/**
 * Tool: Run a shell command in a sandboxed environment.
 *
 * Uses OpenSkills Seatbelt sandbox on macOS for security,
 * matching Claude Code's sandboxed bash tool.
 */
const bashTool = tool({
  description:
    "Run a shell command in a sandboxed environment (cwd is the agent directory). Use this to run generated scripts and verify outputs.",
  parameters: z.object({
    command: z
      .string()
      .describe(
        "Shell command to run. Prefer `npx tsx <file>` or `node <file>` and keep it simple."
      ),
  }),
  execute: async ({ command }) => {
    if (!command.trim()) return "";

    try {
      // Use the OpenSkills sandboxed command execution
      // This runs in a Seatbelt sandbox on macOS
      const result = runSandboxedShellCommand(command, agentRoot, {
        allowNetwork: false,
        allowProcess: true, // Allow subprocesses like npx, node
        readPaths: [agentRoot, claudeSkillsDir],
        writePaths: [outputDir],
        envVars: [
          ["PATH", process.env.PATH || "/usr/local/bin:/usr/bin:/bin"],
          ["HOME", process.env.HOME || ""],
          ["NODE_PATH", process.env.NODE_PATH || ""],
        ],
        timeoutMs: 60000, // 60 seconds
      });

      if (result.timedOut) {
        return `Command timed out after 60 seconds.\nPartial STDOUT:\n${result.stdout}\nPartial STDERR:\n${result.stderr}`;
      }

      if (result.exitCode !== 0) {
        return `Command failed (exit ${result.exitCode}).\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`;
      }

      return result.stdout || "(no output)";
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      return `Sandbox error: ${message}`;
    }
  },
});

const systemPrompt = `You are a helpful assistant that uses Claude Skills to complete tasks.

## How Claude Skills Work

Claude Skills are instruction-based. When a user asks you to do something:

1. **Match the request to a skill** - Use the skill catalog below to find a matching skill
2. **Activate the skill** - Call activate_skill to get the full instructions from SKILL.md
3. **Read helper files** - Use read_skill_file to read referenced docs (e.g., docx-js.md)
4. **Follow the instructions** - Read and follow the skill's instructions step by step
5. **Run skill scripts** - Use run_skill_script to execute Python/Shell scripts from the skill

## Available Skills

${catalog}

## Skill-Specific Tools

- **activate_skill**: Get full SKILL.md instructions for a skill
- **read_skill_file**: Read helper files from a skill (docx-js.md, ooxml.md, etc.)
- **list_skill_files**: Discover what files are in a skill directory
- **run_skill_script**: Run Python/Shell scripts from a skill in a secure sandbox

## General Tools

- **read_file**: Read any file from allowed directories
- **write_file**: Write files to the agent directory
- **bash**: Run commands locally

## Important Notes

- Skills contain detailed instructions in SKILL.md - always read them first
- Skills may reference helper files - use read_skill_file to read them
- Skills may include Python scripts in scripts/ folders - use run_skill_script to run them
- The skill's allowed-tools tell you what operations are permitted

When the user asks for document creation, file manipulation, or specialized tasks, first activate the appropriate skill to get detailed instructions.

## IMPORTANT: Actually create files when asked

If the user asks you to create a Word document, you MUST complete ALL these steps:

1. **Activate the docx skill**: Call activate_skill("docx") to get instructions
2. **Read helper docs**: Use read_skill_file("docx", "docx-js.md") to understand the API
3. **Create TypeScript script**: Write a script to \`output/create-docx.ts\` (NOT src/output/ or src/ - use the output/ directory at the project root) with this exact pattern:
   \`\`\`typescript
   import { Document, Packer, Paragraph, TextRun } from 'docx';
   import fs from 'fs';
   
   const doc = new Document({
     sections: [{
       children: [
         new Paragraph({
           children: [new TextRun({ text: "Title", bold: true, size: 48 })],
           alignment: "CENTER"
         }),
         new Paragraph({ children: [new TextRun({ text: "" })] }),
         new Paragraph({
           children: [new TextRun({ text: "Paragraph text", size: 24 })]
         })
       ]
     }]
   });
   
   Packer.toBuffer(doc).then((buffer) => {
     fs.writeFileSync("output/generated.docx", buffer);
     console.log("✓ Document created");
   });
   \`\`\`
4. **EXECUTE THE SCRIPT**: Use bash tool: \`bash: "npx tsx output/create-docx.ts"\`
5. **Verify**: Use bash tool: \`bash: "ls -lh output/generated.docx"\`
6. **Report**: Tell the user the document was created at output/generated.docx

**CRITICAL**: Do NOT stop after step 3. You MUST execute the script (step 4) to actually create the file!`;

async function main() {
  const userQuery = process.argv[2] || 
    "What skills are available? Then help me create a Word document with a title 'Hello World' and a paragraph of text.";

  console.log("🤖 OpenSkills Agent (Claude Skills Compatible)");
  console.log("Using model:", LLM_MODEL);
  console.log("Using provider:", LLM_PROVIDER);
  console.log("Skills directory:", claudeSkillsDir);
  console.log("Skills found:", skills.length);
  console.log("\n📝 User request:", userQuery);
  console.log("\n" + "=".repeat(60) + "\n");

  try {
    const result = await generateText({
      model: model,
      system: systemPrompt,
      prompt: userQuery,
      tools: {
        activate_skill: activateSkillTool,
        list_skills: listSkillsTool,
        read_skill_file: readSkillFileTool,
        list_skill_files: listSkillFilesTool,
        run_skill_script: runSkillScriptTool,
        read_file: readFileTool,
        write_file: writeFileTool,
        bash: bashTool,
      },
      maxSteps: 20,
    });
    
    console.log("\n" + "=".repeat(60));
    console.log("\n✅ Response:");
    console.log(result.text);
    
    if (result.toolCalls && result.toolCalls.length > 0) {
      console.log("\n🔧 Tool calls made:", result.toolCalls.length);
      for (const call of result.toolCalls) {
        console.log(`   - ${call.toolName}(${JSON.stringify(call.args)})`);
      }
    }
    
    // Show tool results if available
    if (result.toolResults && result.toolResults.length > 0) {
      console.log("\n📋 Tool results:");
      for (const res of result.toolResults) {
        const preview = typeof res.result === 'string' 
          ? res.result.slice(0, 500) + (res.result.length > 500 ? '...' : '')
          : JSON.stringify(res.result).slice(0, 500);
        console.log(`   [${res.toolName}]: ${preview}`);
      }
    }
  } catch (error) {
    console.error("\n❌ Error:", error);
    process.exit(1);
  }
}

main();
