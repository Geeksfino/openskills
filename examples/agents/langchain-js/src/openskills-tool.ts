// @ts-ignore - example dependencies are installed by the user
import { OpenSkillRuntime } from "@openskills/runtime";
// @ts-ignore - example dependencies are installed by the user
import { DynamicStructuredTool } from "@langchain/core/tools";
import { z } from "zod";

type SkillDescriptor = {
  id: string;
  description: string;
};

type ToolOptions = {
  skillsDir: string;
  includeSkills?: string[];
  excludeSkills?: string[];
};

function createRuntime(skillsDir: string) {
  const runtime = OpenSkillRuntime.fromDirectory(skillsDir);
  runtime.discoverSkills();
  return runtime;
}

export function createOpenSkillsTools(options: ToolOptions) {
  const runtime = createRuntime(options.skillsDir);
  const skills = runtime.listSkills() as SkillDescriptor[];

  return skills
    .filter((skill) => {
      if (options.includeSkills && !options.includeSkills.includes(skill.id)) {
        return false;
      }
      if (options.excludeSkills && options.excludeSkills.includes(skill.id)) {
        return false;
      }
      return true;
    })
    .map(
      (skill) =>
        new DynamicStructuredTool({
          name: skill.id,
          description: skill.description,
          schema: z.object({
            input: z.string(),
          }),
          func: async ({ input }) => {
            const result = runtime.executeSkill(skill.id, {
              timeout_ms: 5000,
              input: JSON.stringify({ query: input }),
            });
            return result.output_json ?? result.output ?? "";
          },
        })
    );
}

export function getSkillMetadata(skillsDir: string) {
  const runtime = createRuntime(skillsDir);
  const skills = runtime.listSkills() as SkillDescriptor[];

  if (!skills.length) {
    return "No skills available.";
  }

  const lines = ["Available Skills:"];
  for (const skill of skills) {
    lines.push(`- ${skill.id}: ${skill.description}`);
  }
  return lines.join("\n");
}
