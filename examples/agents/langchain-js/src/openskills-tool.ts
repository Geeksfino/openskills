// @ts-ignore - example dependencies are installed by the user
import { OpenSkillRuntime } from "@finogeek/openskills";
// @ts-ignore - example dependencies are installed by the user
import { DynamicStructuredTool } from "@langchain/core/tools";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

type SkillDescriptor = {
  id: string;
  description: string;
};

type ToolOptions = {
  skillsDir: string;
  includeSkills?: string[];
  excludeSkills?: string[];
};

type SkillType = "instruction-only" | "executable";

function createRuntime(skillsDir: string) {
  const runtime = OpenSkillRuntime.fromDirectory(skillsDir);
  runtime.discoverSkills();
  return runtime;
}

/**
 * 检测技能类型：指令型还是可执行型
 * @param skillId 技能 ID
 * @param skillsDir 技能目录路径
 * @returns "instruction-only" | "executable"
 */
function detectSkillType(skillId: string, skillsDir: string): SkillType {
  const skillPath = path.join(skillsDir, skillId);
  
  // 检查是否有可执行文件
  // 1. 检查 WASM 文件
  const wasmCandidates = [
    path.join(skillPath, "skill.wasm"),
    path.join(skillPath, "wasm", "skill.wasm"),
    path.join(skillPath, "module.wasm"),
    path.join(skillPath, "main.wasm"),
  ];
  
  for (const wasmPath of wasmCandidates) {
    if (fs.existsSync(wasmPath) && fs.statSync(wasmPath).isFile()) {
      return "executable";
    }
  }
  
  // 检查目录中是否有任何 .wasm 文件
  try {
    const entries = fs.readdirSync(skillPath, { recursive: true });
    for (const entry of entries) {
      const fullPath = path.join(skillPath, entry);
      if (fs.statSync(fullPath).isFile() && entry.endsWith(".wasm")) {
        return "executable";
      }
    }
  } catch (e) {
    // 忽略读取错误
  }
  
  // 2. 检查原生脚本文件（.py, .sh, .bash）
  const scriptCandidates = [
    path.join(skillPath, "script.py"),
    path.join(skillPath, "main.py"),
    path.join(skillPath, "src", "main.py"),
    path.join(skillPath, "index.py"),
    path.join(skillPath, "src", "index.py"),
    path.join(skillPath, "script.sh"),
    path.join(skillPath, "main.sh"),
    path.join(skillPath, "src", "main.sh"),
    path.join(skillPath, "index.sh"),
    path.join(skillPath, "src", "index.sh"),
    path.join(skillPath, "script.bash"),
    path.join(skillPath, "main.bash"),
    path.join(skillPath, "src", "main.bash"),
    path.join(skillPath, "index.bash"),
    path.join(skillPath, "src", "index.bash"),
  ];
  
  for (const scriptPath of scriptCandidates) {
    if (fs.existsSync(scriptPath) && fs.statSync(scriptPath).isFile()) {
      return "executable";
    }
  }
  
  // 检查 scripts 目录或 src 目录中的脚本文件
  const scriptDirs = [
    path.join(skillPath, "scripts"),
    path.join(skillPath, "src"),
    skillPath, // 直接在技能根目录
  ];
  
  for (const dir of scriptDirs) {
    try {
      if (fs.existsSync(dir) && fs.statSync(dir).isDirectory()) {
        const entries = fs.readdirSync(dir, { recursive: true });
        for (const entry of entries) {
          const fullPath = path.join(dir, entry);
          if (fs.statSync(fullPath).isFile()) {
            const ext = path.extname(entry).toLowerCase();
            if (ext === ".py" || ext === ".sh" || ext === ".bash") {
              return "executable";
            }
          }
        }
      }
    } catch (e) {
      // 忽略读取错误
    }
  }
  
  // 如果没有找到可执行文件，则是指令型技能
  return "instruction-only";
}

// 技能类型缓存（在模块级别缓存，避免重复检测）
const skillTypeCache = new Map<string, Map<string, "instruction-only" | "executable">>();

function getOrCreateSkillTypeCache(skillsDir: string): Map<string, "instruction-only" | "executable"> {
  if (!skillTypeCache.has(skillsDir)) {
    skillTypeCache.set(skillsDir, new Map());
  }
  return skillTypeCache.get(skillsDir)!;
}

export function createOpenSkillsTools(options: ToolOptions) {
  const runtime = createRuntime(options.skillsDir);
  const skills = runtime.listSkills() as SkillDescriptor[];
  
  // 在注册时检测并缓存所有技能的类型
  const typeCache = getOrCreateSkillTypeCache(options.skillsDir);
  for (const skill of skills) {
    if (!typeCache.has(skill.id)) {
      const skillType = detectSkillType(skill.id, options.skillsDir);
      typeCache.set(skill.id, skillType);
    }
  }

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
            // 从缓存中获取技能类型（已在注册时检测过）
            const typeCache = getOrCreateSkillTypeCache(options.skillsDir);
            const skillType = typeCache.get(skill.id) || detectSkillType(skill.id, options.skillsDir);
            
            if (skillType === "instruction-only") {
              // 指令型技能：激活并返回指令让 AI 遵循
              try {
                const loadedSkill = runtime.activateSkill(skill.id);
                // 注意：activateSkill 返回的是扁平化的对象，不是嵌套的 manifest
                return `[Instruction-Only Skill: ${skill.id}]\n\n` +
                       `Description: ${loadedSkill.description}\n\n` +
                       `Instructions:\n${loadedSkill.instructions}\n\n` +
                       `User Request: ${input}\n\n` +
                       `Please follow the above instructions to help the user.`;
              } catch (activateError: any) {
                return `Error: Failed to activate skill "${skill.id}": ${activateError.message}`;
              }
            } else {
              // 可执行型技能：正常执行
              try {
                const result = runtime.executeSkill(skill.id, {
                  timeout_ms: 5000,
                  input: JSON.stringify({ query: input }),
                });
                return result.outputJson ?? "";
              } catch (error: any) {
                return `Error executing skill "${skill.id}": ${error.message}`;
              }
            }
          },
        })
    );
}

export function getSkillMetadata(skillsDir: string) {
  const runtime = createRuntime(skillsDir);
  const skills = runtime.listSkills() as SkillDescriptor[];
  
  // 在获取元数据时也检测并缓存技能类型，以便在描述中显示
  const typeCache = getOrCreateSkillTypeCache(skillsDir);
  for (const skill of skills) {
    if (!typeCache.has(skill.id)) {
      const skillType = detectSkillType(skill.id, skillsDir);
      typeCache.set(skill.id, skillType);
    }
  }

  if (!skills.length) {
    return "No skills available.";
  }

  const lines = ["Available Skills:"];
  for (const skill of skills) {
    const type = typeCache.get(skill.id);
    const typeLabel = type === "instruction-only" ? "[Instruction]" : "[Executable]";
    lines.push(`- ${skill.id} ${typeLabel}: ${skill.description}`);
  }
  return lines.join("\n");
}
