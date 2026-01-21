/**
 * Pre-built AI SDK tools for OpenSkills runtime.
 *
 * These tools provide a ready-to-use integration with Vercel AI SDK.
 * Agents can import these directly instead of implementing their own.
 *
 * Usage:
 * ```typescript
 * import { OpenSkillRuntime } from '@finogeek/openskills';
 * import { createSkillTools } from '@finogeek/openskills/tools';
 *
 * const runtime = OpenSkillRuntime.fromDirectory('./skills');
 * runtime.discoverSkills();
 *
 * const tools = createSkillTools(runtime, { workspaceDir: './output' });
 * // Use with Vercel AI SDK generateText/streamText
 * ```
 *
 * @module tools
 */

const { OpenSkillRuntime, runSandboxedShellCommand } = require('./index.js');
const fs = require('fs');
const path = require('path');

/**
 * Create pre-built AI SDK tools for OpenSkills runtime.
 *
 * @param {OpenSkillRuntime} runtime - The OpenSkills runtime instance
 * @param {Object} options - Tool configuration options
 * @param {string} [options.workspaceDir] - Workspace directory for file I/O
 * @returns {Object} Object containing AI SDK tool definitions
 */
function createSkillTools(runtime, options = {}) {
  // Lazy import to avoid requiring ai/zod if not used
  // Support both CommonJS and ES modules
  let tool, z;
  
  // Helper to try requiring from different contexts
  function tryRequire(moduleName) {
    // Try direct require first (works in CommonJS)
    if (typeof require !== 'undefined') {
      try {
        return require(moduleName);
      } catch (e) {
        // If that fails, try createRequire from current working directory
        // This works when tools.js is imported as ES module but packages are in project's node_modules
        try {
          const { createRequire } = require('module');
          // Try from process.cwd() (the project root)
          const projectRequire = createRequire(path.join(process.cwd(), 'package.json'));
          return projectRequire(moduleName);
        } catch (e2) {
          // Try from tools.js location as fallback
          try {
            const { createRequire } = require('module');
            const toolsFile = __filename || require.resolve('./tools.js');
            const localRequire = createRequire(toolsFile);
            return localRequire(moduleName);
          } catch (e3) {
            throw e; // Throw original error
          }
        }
      }
    }
    throw new Error(`require is not available and cannot load ${moduleName}`);
  }
  
  try {
    const aiModule = tryRequire('ai');
    const zodModule = tryRequire('zod');
    tool = aiModule.tool;
    z = zodModule.default || zodModule.z || zodModule;
  } catch (e) {
    throw new Error(
      'createSkillTools requires "ai" and "zod" packages. Install them with: npm install ai zod. ' +
      'Error: ' + (e.message || e)
    );
  }

  const workspaceDir = options.workspaceDir || process.cwd();

  // Ensure workspace exists
  if (!fs.existsSync(workspaceDir)) {
    fs.mkdirSync(workspaceDir, { recursive: true });
  }

  // Helper function to validate that a path is within the workspace directory
  // This prevents directory traversal attacks by ensuring the resolved path
  // is actually within the workspace, not just a string prefix match
  function isPathWithinWorkspace(relativePath) {
    const resolvedWorkspace = path.resolve(workspaceDir);
    const resolvedPath = path.resolve(workspaceDir, relativePath);
    
    // If paths are equal, it's valid
    if (resolvedPath === resolvedWorkspace) {
      return true;
    }
    
    // Use path.relative() to check if path is within workspace
    // If relative path doesn't start with '..', it's within the workspace
    const relative = path.relative(resolvedWorkspace, resolvedPath);
    // Empty string means paths are equal (already handled above)
    // If relative path starts with '..' or is absolute, it's outside the workspace
    return relative !== '' && !relative.startsWith('..') && !path.isAbsolute(relative);
  }

  // Create/update package.json in workspace to support CommonJS (used by Claude Skills)
  // This ensures CommonJS scripts work even if parent project uses ES modules
  const workspacePackageJson = path.join(workspaceDir, 'package.json');
  const packageJsonContent = JSON.stringify({ type: 'commonjs' }, null, 2) + '\n';
  if (!fs.existsSync(workspacePackageJson)) {
    fs.writeFileSync(workspacePackageJson, packageJsonContent);
  } else {
    // Update if it exists but doesn't have the right type
    try {
      const existing = JSON.parse(fs.readFileSync(workspacePackageJson, 'utf-8'));
      if (existing.type !== 'commonjs') {
        existing.type = 'commonjs';
        fs.writeFileSync(workspacePackageJson, JSON.stringify(existing, null, 2) + '\n');
      }
    } catch (e) {
      // If parsing fails, overwrite with correct content
      fs.writeFileSync(workspacePackageJson, packageJsonContent);
    }
  }

  // Helper function to format bytes
  function formatBytes(bytes) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.min(
      Math.floor(Math.log(bytes) / Math.log(k)),
      sizes.length - 1
    );
    return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + ' ' + sizes[i];
  }

  // Helper function for glob pattern matching (simple implementation)
  // Properly escapes all regex metacharacters while preserving glob wildcards
  function matchesPattern(filename, pattern) {
    // Use placeholders for glob wildcards to avoid escaping issues
    const withPlaceholders = pattern
      .replace(/\*/g, '__STAR__')
      .replace(/\?/g, '__QUESTION__');
    
    // Escape all regex metacharacters: [ ] ( ) { } + | \ ^ $ .
    const escaped = withPlaceholders.replace(/[\[\](){}|\\^$+.]/g, '\\$&');
    
    // Convert placeholders back to regex patterns
    const regexPattern = escaped
      .replace(/__STAR__/g, '.*')
      .replace(/__QUESTION__/g, '.');
    
    const regex = new RegExp('^' + regexPattern + '$');
    return regex.test(filename);
  }

  return {
    /**
     * List all available skills.
     */
    list_skills: tool({
      description: 'List all available Claude Skills with their IDs and descriptions.',
      parameters: z.object({
        query: z.string().optional().describe('Optional search query to filter skills'),
      }),
      execute: async ({ query }) => {
        const skills = runtime.listSkills();
        const filtered = query
          ? skills.filter(
              (s) =>
                s.id.includes(query) ||
                s.description.toLowerCase().includes(query.toLowerCase())
            )
          : skills;
        return JSON.stringify(
          filtered.map((s) => ({ id: s.id, description: s.description })),
          null,
          2
        );
      },
    }),

    /**
     * Activate a skill to get its full instructions.
     */
    activate_skill: tool({
      description:
        'Activate a Claude Skill to get its full SKILL.md instructions. Use this when the user\'s request matches a skill\'s capabilities.',
      parameters: z.object({
        skill_id: z.string().describe('The skill ID to activate'),
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
          return `Error activating skill ${skill_id}: ${error.message || error}`;
        }
      },
    }),

    /**
     * Read a file from a skill directory.
     */
    read_skill_file: tool({
      description:
        'Read a file from a skill directory. Use this to read helper files referenced in SKILL.md instructions.',
      parameters: z.object({
        skill_id: z.string().describe('The skill ID'),
        path: z.string().describe('Relative path within the skill directory'),
      }),
      execute: async ({ skill_id, path: relativePath }) => {
        try {
          return runtime.readSkillFile(skill_id, relativePath);
        } catch (error) {
          return `Error reading ${relativePath} from skill ${skill_id}: ${error.message || error}`;
        }
      },
    }),

    /**
     * List files in a skill directory.
     */
    list_skill_files: tool({
      description: 'List files in a skill directory to discover available resources.',
      parameters: z.object({
        skill_id: z.string().describe('The skill ID'),
        subdir: z.string().optional().describe('Optional subdirectory'),
        recursive: z.boolean().optional().describe('List recursively (default: false)'),
      }),
      execute: async ({ skill_id, subdir, recursive }) => {
        try {
          const files = runtime.listSkillFiles(skill_id, subdir, recursive ?? false);
          return JSON.stringify(files, null, 2);
        } catch (error) {
          return `Error listing files in skill ${skill_id}: ${error.message || error}`;
        }
      },
    }),

    /**
     * Run a script from a skill in the sandbox.
     */
    run_skill_script: tool({
      description:
        'Run a Python or Shell script from a skill directory in a sandbox. Use when SKILL.md instructions say to run a script.',
      parameters: z.object({
        skill_id: z.string().describe('The skill ID'),
        script_path: z.string().describe('Path to the script relative to skill root'),
        args: z.array(z.string()).optional().describe('Arguments to pass to the script'),
        timeout_ms: z.number().optional().describe('Timeout in milliseconds (default: 30000)'),
      }),
      execute: async ({ skill_id, script_path, args, timeout_ms }) => {
        try {
          const result = runtime.runSkillTarget(skill_id, {
            targetType: 'script',
            path: script_path,
            args: args ?? [],
            timeoutMs: timeout_ms ?? 30000,
          });
          return JSON.stringify(
            {
              stdout: result.stdout,
              stderr: result.stderr,
              output: result.outputJson,
            },
            null,
            2
          );
        } catch (error) {
          return `Error running ${script_path} from skill ${skill_id}: ${error.message || error}`;
        }
      },
    }),

    /**
     * Run a sandboxed bash command.
     */
    run_sandboxed_bash: tool({
      description:
        'Run a bash command in a sandboxed environment. Use for executing shell commands securely. Set allow_process=true when executing scripts (e.g., npx tsx, node).',
      parameters: z.object({
        command: z.string().describe('The bash command to execute'),
        working_dir: z.string().optional().describe('Working directory (defaults to workspace)'),
        allow_network: z.boolean().optional().describe('Allow network access (default: false)'),
        allow_process: z.boolean().optional().describe('Allow subprocess spawning (required for npx, node, etc.) (default: false)'),
        timeout_ms: z.number().optional().describe('Timeout in milliseconds (default: 30000)'),
      }),
      execute: async ({ command, working_dir, allow_network, allow_process, timeout_ms }) => {
        try {
          const cwd = working_dir || workspaceDir;
          const result = runSandboxedShellCommand(command, cwd, {
            allowNetwork: allow_network ?? false,
            allowProcess: allow_process ?? false,
            timeoutMs: timeout_ms ?? 30000,
          });
          return JSON.stringify(
            {
              exitCode: result.exitCode,
              stdout: result.stdout,
              stderr: result.stderr,
              timedOut: result.timedOut,
            },
            null,
            2
          );
        } catch (error) {
          return `Error running command: ${error.message || error}`;
        }
      },
    }),

    /**
     * Write a file to the workspace.
     */
    write_file: tool({
      description: 'Write a file to the workspace directory.',
      parameters: z.object({
        path: z.string().describe('Relative path within the workspace'),
        content: z.string().describe('File content to write'),
      }),
      execute: async ({ path: relativePath, content }) => {
        try {
          // Security: ensure path is within workspace (prevents directory traversal)
          if (!isPathWithinWorkspace(relativePath)) {
            return `Error: Path ${relativePath} escapes workspace directory`;
          }
          const fullPath = path.resolve(workspaceDir, relativePath);
          // Ensure parent directory exists
          const dir = path.dirname(fullPath);
          if (!fs.existsSync(dir)) {
            fs.mkdirSync(dir, { recursive: true });
          }
          fs.writeFileSync(fullPath, content, 'utf-8');
          return `Successfully wrote ${content.length} bytes to ${relativePath}`;
        } catch (error) {
          return `Error writing file: ${error.message || error}`;
        }
      },
    }),

    /**
     * Read a file from the workspace.
     */
    read_file: tool({
      description: 'Read a file from the workspace directory.',
      parameters: z.object({
        path: z.string().describe('Relative path within the workspace'),
      }),
      execute: async ({ path: relativePath }) => {
        try {
          // Security: ensure path is within workspace (prevents directory traversal)
          if (!isPathWithinWorkspace(relativePath)) {
            return `Error: Path ${relativePath} escapes workspace directory`;
          }
          const fullPath = path.resolve(workspaceDir, relativePath);
          if (!fs.existsSync(fullPath)) {
            return `Error: File not found: ${relativePath}`;
          }
          return fs.readFileSync(fullPath, 'utf-8');
        } catch (error) {
          return `Error reading file: ${error.message || error}`;
        }
      },
    }),

    /**
     * List files in the workspace directory.
     */
    list_workspace_files: tool({
      description: 'List all files in the workspace directory. Use this to discover files generated by skills.',
      parameters: z.object({
        subdir: z.string().optional().describe('Optional subdirectory to list'),
        recursive: z.boolean().optional().describe('List recursively (default: false)'),
        pattern: z.string().optional().describe('Optional glob pattern to filter files (e.g., "*.docx")'),
      }),
      execute: async ({ subdir, recursive, pattern }) => {
        try {
          const targetDir = subdir ? path.join(workspaceDir, subdir) : workspaceDir;
          if (!fs.existsSync(targetDir)) {
            return JSON.stringify({ files: [], error: 'Directory not found' });
          }

          const files = [];
          const walkDir = (dir, basePath = '') => {
            const entries = fs.readdirSync(dir, { withFileTypes: true });
            for (const entry of entries) {
              const fullPath = path.join(dir, entry.name);
              const relPath = path.join(basePath, entry.name);

              if (entry.isDirectory()) {
                if (recursive) {
                  walkDir(fullPath, relPath);
                }
              } else {
                // Apply pattern filter if provided
                if (pattern && !matchesPattern(entry.name, pattern)) {
                  continue;
                }

                const stats = fs.statSync(fullPath);
                files.push({
                  path: relPath,
                  size: stats.size,
                  modified: stats.mtime.toISOString(),
                });
              }
            }
          };

          walkDir(targetDir, subdir || '');
          return JSON.stringify({ files }, null, 2);
        } catch (error) {
          return `Error listing workspace files: ${error.message || error}`;
        }
      },
    }),

    /**
     * Get file information (size, type, path).
     */
    get_file_info: tool({
      description: 'Get information about a file in the workspace (size, type, path). Use this to reference files in your response.',
      parameters: z.object({
        path: z.string().describe('Relative path within the workspace'),
      }),
      execute: async ({ path: relativePath }) => {
        try {
          // Security: ensure path is within workspace (prevents directory traversal)
          if (!isPathWithinWorkspace(relativePath)) {
            return `Error: Path ${relativePath} escapes workspace directory`;
          }
          const fullPath = path.resolve(workspaceDir, relativePath);
          if (!fs.existsSync(fullPath)) {
            return `Error: File not found: ${relativePath}`;
          }

          const stats = fs.statSync(fullPath);
          const ext = path.extname(relativePath).toLowerCase();
          const mimeTypes = {
            '.docx': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
            '.xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
            '.pptx': 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
            '.pdf': 'application/pdf',
            '.png': 'image/png',
            '.jpg': 'image/jpeg',
            '.jpeg': 'image/jpeg',
            '.gif': 'image/gif',
            '.svg': 'image/svg+xml',
            '.txt': 'text/plain',
            '.md': 'text/markdown',
            '.json': 'application/json',
            '.html': 'text/html',
            '.css': 'text/css',
            '.js': 'application/javascript',
            '.ts': 'application/typescript',
          };

          return JSON.stringify(
            {
              path: relativePath,
              fullPath: fullPath,
              size: stats.size,
              sizeHuman: formatBytes(stats.size),
              type: mimeTypes[ext] || 'application/octet-stream',
              extension: ext,
              modified: stats.mtime.toISOString(),
            },
            null,
            2
          );
        } catch (error) {
          return `Error getting file info: ${error.message || error}`;
        }
      },
    }),
  };
}

/**
 * Get a skill-agnostic system prompt from the runtime.
 *
 * @param {OpenSkillRuntime} runtime - The OpenSkills runtime instance
 * @returns {string} A complete system prompt for skill-based agents
 */
function getAgentSystemPrompt(runtime) {
  // Call the runtime method if available, otherwise build a basic one
  if (typeof runtime.getAgentSystemPrompt === 'function') {
    return runtime.getAgentSystemPrompt();
  }

  // Fallback implementation
  const skills = runtime.listSkills();
  if (skills.length === 0) {
    return 'No skills are currently available.';
  }

  let prompt = `You have access to Claude Skills that provide specialized capabilities.

## Available Skills

`;

  for (const skill of skills) {
    prompt += `- **${skill.id}**: ${skill.description}\n`;
  }

  prompt += `
## How to Use Skills

When a user's request matches a skill's capabilities:

1. **Activate the skill**: Call \`activate_skill(skill_id)\` to load the full SKILL.md instructions
2. **Read the instructions carefully**: The SKILL.md contains everything you need to know
3. **Follow the instructions exactly**: Execute the steps as described in SKILL.md
4. **Use helper files if referenced**: Call \`read_skill_file(skill_id, path)\` to read referenced docs
5. **Run scripts as instructed**: Call \`run_skill_script(skill_id, script_path, args)\` when needed

## Important

- Each skill's SKILL.md contains all the knowledge you need - do NOT assume prior knowledge
- Output files are written to the workspace directory
`;

  return prompt;
}

module.exports = {
  createSkillTools,
  getAgentSystemPrompt,
};
