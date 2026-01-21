/**
 * Pre-built AI SDK tools for OpenSkills runtime.
 * @module tools
 */

import type { OpenSkillRuntimeWrapper } from './index';

/**
 * CoreTool type from Vercel AI SDK.
 * Defined locally to avoid requiring 'ai' package at type-check time.
 * The actual implementation uses the real CoreTool from 'ai' at runtime.
 */
type CoreTool = {
  description?: string;
  parameters: unknown; // Required by AI SDK Tool type
  execute?: (...args: unknown[]) => Promise<unknown>;
  [key: string]: unknown;
};

/**
 * Options for creating skill tools.
 */
export interface SkillToolsOptions {
  /**
   * Workspace directory for file I/O operations.
   * Defaults to process.cwd().
   */
  workspaceDir?: string;
}

/**
 * Pre-built AI SDK tools for skill-based agents.
 * Includes index signature to be compatible with AI SDK's ToolSet type.
 */
export interface SkillTools {
  [key: string]: CoreTool;
  /** List all available skills. */
  list_skills: CoreTool;
  /** Activate a skill to get its full instructions. */
  activate_skill: CoreTool;
  /** Read a file from a skill directory. */
  read_skill_file: CoreTool;
  /** List files in a skill directory. */
  list_skill_files: CoreTool;
  /** Run a script or WASM module from a skill in the sandbox. Auto-detects sandbox type from file extension. */
  run_skill_script: CoreTool;
  /** Run a sandboxed bash command. */
  run_sandboxed_bash: CoreTool;
  /** Write a file to the workspace. */
  write_file: CoreTool;
  /** Read a file from the workspace. */
  read_file: CoreTool;
  /** List files in the workspace directory. */
  list_workspace_files: CoreTool;
  /** Get file information (size, type, path). */
  get_file_info: CoreTool;
}

/**
 * Create pre-built AI SDK tools for OpenSkills runtime.
 *
 * @param runtime - The OpenSkills runtime instance
 * @param options - Tool configuration options
 * @returns Object containing AI SDK tool definitions
 *
 * @example
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
 */
export function createSkillTools(
  runtime: OpenSkillRuntimeWrapper,
  options?: SkillToolsOptions
): SkillTools;

/**
 * Get a skill-agnostic system prompt from the runtime.
 *
 * This returns a complete system prompt that teaches the agent how to use
 * Claude Skills without any skill-specific knowledge.
 *
 * @param runtime - The OpenSkills runtime instance
 * @returns A complete system prompt for skill-based agents
 *
 * @example
 * ```typescript
 * import { OpenSkillRuntime } from '@finogeek/openskills';
 * import { getAgentSystemPrompt } from '@finogeek/openskills/tools';
 *
 * const runtime = OpenSkillRuntime.fromDirectory('./skills');
 * runtime.discoverSkills();
 *
 * const systemPrompt = getAgentSystemPrompt(runtime);
 * // Use as system prompt in generateText/streamText
 * ```
 */
export function getAgentSystemPrompt(runtime: OpenSkillRuntimeWrapper): string;
