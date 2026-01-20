import "dotenv/config";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as fs from "fs";
import { OpenSkillRuntime } from "@finogeek/openskills";
// 使用阿里云通义千问模型
import { ChatAlibabaTongyi } from "@langchain/community/chat_models/alibaba_tongyi";
import { DynamicStructuredTool } from "@langchain/core/tools";
import { initializeAgentExecutorWithOptions } from "langchain/agents";
import { z } from "zod";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const skillsDir = path.resolve(__dirname, "..", "..", "..", "skills");

/**
 * 检测技能类型：指令型还是可执行型
 * @param skillId 技能 ID
 * @param skillsDir 技能目录路径
 * @returns "instruction-only" | "executable"
 */
function detectSkillType(skillId: string, skillsDir: string): "instruction-only" | "executable" {
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

const runtime = OpenSkillRuntime.fromDirectory(skillsDir);
runtime.discoverSkills();

// 在注册时检测并缓存所有技能的类型
const skills = runtime.listSkills();
const skillTypeCache = new Map<string, "instruction-only" | "executable">();

for (const skill of skills) {
  const skillType = detectSkillType(skill.id, skillsDir);
  skillTypeCache.set(skill.id, skillType);
}

const catalog = skills
  .map((skill) => {
    const type = skillTypeCache.get(skill.id);
    const typeLabel = type === "instruction-only" ? "[Instruction]" : "[Executable]";
    return `- ${skill.id} ${typeLabel}: ${skill.description}`;
  })
  .join("\n");

const runSkillTool = new DynamicStructuredTool({
  name: "run_skill",
  description: "Execute an OpenSkills skill by id with a text input. For instruction-only skills, returns the skill instructions for the AI to follow.",
  schema: z.object({
    skill_id: z.string(),
    input: z.string(),
  }),
  func: async ({ skill_id, input }) => {
    // 从缓存中获取技能类型（已在注册时检测过）
    const skillType = skillTypeCache.get(skill_id) || detectSkillType(skill_id, skillsDir);
    
    if (skillType === "instruction-only") {
      // 指令型技能：激活并返回指令让 AI 遵循
      try {
        const loadedSkill = runtime.activateSkill(skill_id);
        // 注意：activateSkill 返回的是扁平化的对象，不是嵌套的 manifest
        return `[Instruction-Only Skill: ${skill_id}]\n\n` +
               `Description: ${loadedSkill.description}\n\n` +
               `Instructions:\n${loadedSkill.instructions}\n\n` +
               `User Request: ${input}\n\n` +
               `Please follow the above instructions to help the user.`;
      } catch (activateError: any) {
        return `Error: Failed to activate skill "${skill_id}": ${activateError.message}`;
      }
    } else {
      // 可执行型技能：正常执行
      try {
        const result = runtime.executeSkill(skill_id, {
          timeout_ms: 5000,
          input: JSON.stringify({ query: input }),
        });
        return result.outputJson ?? "";
      } catch (error: any) {
        return `Error executing skill "${skill_id}": ${error.message}`;
      }
    }
  },
});

// 使用阿里云通义千问模型（替代 OpenAI）
const llm = new ChatAlibabaTongyi({
  modelName: "qwen-turbo", // 或 "qwen-plus", "qwen-max"
  temperature: 0,
  alibabaApiKey: process.env.DASHSCOPE_API_KEY, // 阿里云 DashScope API Key
});

// 注意：openai-functions 只支持 OpenAI 模型，需要使用其他 agent 类型
// 使用 structured-chat-zero-shot-react-description，它支持所有 ChatModel
const executor = await initializeAgentExecutorWithOptions(
  [runSkillTool],
  llm,
  {
    agentType: "structured-chat-zero-shot-react-description", // 兼容非 OpenAI 模型
    verbose: true,
  }
);

const response = await executor.invoke({
  input: [
    "You can call run_skill to execute OpenSkills skills.",
    "Available skills:",
    catalog,
    "",
    "User request: Summarize the following text using an appropriate skill:",
    "OpenSkills provides a WASM runtime for Claude-compatible skills.",
  ].join("\n"),
});

console.log(response.output);
