export type SkillDescriptor = {
  id: string;
  description: string;
  location: "personal" | "project" | "nested" | "custom";
  user_invocable: boolean;
};

export type LoadedSkill = {
  id: string;
  name: string;
  description: string;
  allowed_tools: string[];
  model: string | null;
  context: string | null;
  agent: string | null;
  user_invocable: boolean;
  location: "personal" | "project" | "nested" | "custom";
  instructions: string;
};

export type ExecutionOptions = {
  timeout_ms?: number; // milliseconds
  memory_mb?: number; // megabytes
  input?: string; // JSON string
};

export type AuditRecord = {
  skill_id: string;
  version: string;
  input_hash: string;
  output_hash: string;
  start_time_ms: number;
  duration_ms: number;
  permissions_used: string[];
  exit_status: string;
  stdout: string;
  stderr: string;
};

export type ExecutionResult = {
  output_json: string;
  stdout: string;
  stderr: string;
  audit: AuditRecord;
};

export class OpenSkillRuntime {
  constructor();
  static withProjectRoot(projectRoot: string): OpenSkillRuntime;
  static fromDirectory(skillsDir: string): OpenSkillRuntime;
  
  /**
   * Discover skills from standard locations (~/.claude/skills/, .claude/skills/, nested)
   */
  discoverSkills(): SkillDescriptor[];
  
  /**
   * List skills (progressive disclosure - descriptors only)
   */
  listSkills(): SkillDescriptor[];
  
  /**
   * Activate a skill (load full SKILL.md content)
   */
  activateSkill(skillId: string): LoadedSkill;
  
  /**
   * Execute a skill's WASM module
   */
  executeSkill(skillId: string, options?: ExecutionOptions): ExecutionResult;
  
  /**
   * Check if a tool is allowed for a skill
   */
  isToolAllowed(skillId: string, tool: string): boolean;
}
