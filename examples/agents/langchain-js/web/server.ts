/**
 * Web Server for OpenSkills Agent Playground
 * 
 * Provides a web UI for interacting with OpenSkills agents.
 * Reuses existing tool functions from openskills-tool.ts
 */

import "dotenv/config";
import express from "express";
import cors from "cors";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { OpenSkillRuntime } from "@finogeek/openskills";
import { ChatAlibabaTongyi } from "@langchain/community/chat_models/alibaba_tongyi";
import { DynamicStructuredTool } from "@langchain/core/tools";
import { initializeAgentExecutorWithOptions } from "langchain/agents";
import { z } from "zod";
import * as fs from "fs";
import { getSkillMetadata } from "../src/openskills-tool";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const skillsDir = path.resolve(__dirname, "..", "..", "..", "skills");
const publicDir = path.join(__dirname, "public");

// 初始化 runtime
const runtime = OpenSkillRuntime.fromDirectory(skillsDir);
runtime.discoverSkills();

/**
 * 检测技能类型：指令型还是可执行型
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
    skillPath,
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

// 在注册时检测并缓存所有技能的类型
const skills = runtime.listSkills();
const skillTypeCache = new Map<string, "instruction-only" | "executable">();

for (const skill of skills) {
  const skillType = detectSkillType(skill.id, skillsDir);
  skillTypeCache.set(skill.id, skillType);
}

const app = express();
app.use(cors());
app.use(express.json());
app.use(express.static(publicDir));

// API: 获取技能列表
app.get("/api/skills", async (req, res) => {
  try {
    const skills = runtime.listSkills();
    const skillsWithType = skills.map(skill => ({
      id: skill.id,
      description: skill.description,
      type: skillTypeCache.get(skill.id) || "instruction-only",
    }));
    console.log(`[API] GET /api/skills - 返回 ${skillsWithType.length} 个技能`);
    res.json(skillsWithType);
  } catch (error: any) {
    console.error(`[API] GET /api/skills - 错误: ${error.message}`);
    res.status(500).json({ error: error.message });
  }
});

// API: 获取技能详情
app.get("/api/skills/:id", async (req, res) => {
  try {
    const runtime = OpenSkillRuntime.fromDirectory(skillsDir);
    runtime.discoverSkills();
    const skill = runtime.activateSkill(req.params.id);
    console.log(`[API] GET /api/skills/${req.params.id} - 成功`);
    res.json({
      id: req.params.id,
      description: skill.description,
      instructions: skill.instructions,
    });
  } catch (error: any) {
    console.error(`[API] GET /api/skills/${req.params.id} - 错误: ${error.message}`);
    res.status(404).json({ error: error.message });
  }
});

// API: 聊天接口（流式响应）
app.post("/api/chat", async (req, res) => {
  const { message } = req.body;
  const requestId = `req-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;

  console.log(`\n${"=".repeat(60)}`);
  console.log(`[${new Date().toISOString()}] 📨 收到新请求 [${requestId}]`);
  console.log(`用户消息: ${message}`);
  console.log(`${"=".repeat(60)}\n`);

  if (!message || typeof message !== "string") {
    console.error(`[${requestId}] ❌ 错误: Message is required`);
    return res.status(400).json({ error: "Message is required" });
  }

  // 检查 API Key
  if (!process.env.DASHSCOPE_API_KEY) {
    console.error(`[${requestId}] ❌ 错误: DASHSCOPE_API_KEY is not set`);
    return res.status(500).json({ 
      error: "DASHSCOPE_API_KEY is not set in environment variables" 
    });
  }

  // 设置 SSE
  res.setHeader("Content-Type", "text/event-stream");
  res.setHeader("Cache-Control", "no-cache");
  res.setHeader("Connection", "keep-alive");
  res.setHeader("X-Accel-Buffering", "no"); // 禁用 nginx 缓冲

  // 辅助函数：发送日志到前端
  const sendLog = (level: string, message: string) => {
    const logData = {
      type: "log",
      level, // "info", "success", "error", "warning"
      message,
      timestamp: new Date().toISOString(),
    };
    res.write(`data: ${JSON.stringify(logData)}\n\n`);
    // 同时输出到终端
    const emoji = level === "success" ? "✅" : level === "error" ? "❌" : level === "warning" ? "⚠️" : "ℹ️";
    console.log(`[${requestId}] ${emoji} ${message}`);
  };

  try {
    // 发送开始事件
    res.write(`data: ${JSON.stringify({ type: "start" })}\n\n`);
    sendLog("info", `收到新请求: ${message}`);

    // 创建单一的 run_skill 工具（避免 schema 不匹配问题）
    const runSkillTool = new DynamicStructuredTool({
      name: "run_skill",
      description: "Execute an OpenSkills skill by id with a text input. For instruction-only skills, returns the skill instructions for the AI to follow.",
      schema: z.object({
        skill_id: z.string(),
        input: z.string(),
      }),
      func: async ({ skill_id, input }) => {
        const startTime = Date.now();
        sendLog("info", `调用工具: run_skill`);
        sendLog("info", `  技能 ID: ${skill_id}`);
        sendLog("info", `  输入: ${input.substring(0, 100)}${input.length > 100 ? '...' : ''}`);
        
        const skillType = skillTypeCache.get(skill_id) || detectSkillType(skill_id, skillsDir);
        sendLog("info", `  技能类型: ${skillType}`);
        
        if (skillType === "instruction-only") {
          try {
            const loadedSkill = runtime.activateSkill(skill_id);
            const duration = Date.now() - startTime;
            sendLog("success", `指令型技能激活成功 (${duration}ms)`);
            return `[Instruction-Only Skill: ${skill_id}]\n\n` +
                   `Description: ${loadedSkill.description}\n\n` +
                   `Instructions:\n${loadedSkill.instructions}\n\n` +
                   `User Request: ${input}\n\n` +
                   `Please follow the above instructions to help the user.`;
          } catch (activateError: any) {
            const duration = Date.now() - startTime;
            sendLog("error", `激活技能失败 (${duration}ms): ${activateError.message}`);
            return `Error: Failed to activate skill "${skill_id}": ${activateError.message}`;
          }
        } else {
          try {
            const result = runtime.executeSkill(skill_id, {
              timeout_ms: 5000,
              input: JSON.stringify({ query: input }),
            });
            const duration = Date.now() - startTime;
            const outputPreview = (result.outputJson ?? "").substring(0, 100);
            sendLog("success", `可执行型技能执行成功 (${duration}ms)`);
            sendLog("info", `  输出预览: ${outputPreview}${(result.outputJson ?? "").length > 100 ? '...' : ''}`);
            return result.outputJson ?? "";
          } catch (error: any) {
            const duration = Date.now() - startTime;
            let errorMessage = error.message || String(error);
            
            // 检测 WASM 格式错误并提供修复建议
            if (errorMessage.includes("Invalid WASM artifact") || 
                errorMessage.includes("WASI 0.3 component") ||
                errorMessage.includes("legacy core-module")) {
              sendLog("error", `执行技能失败 (${duration}ms): WASM 文件格式不正确`);
              sendLog("warning", `  问题: WASM 文件不是 WASI 0.3 component 格式`);
              sendLog("info", `  解决方案: 需要重新构建技能`);
              sendLog("info", `  执行命令: openskills build examples/skills/${skill_id}`);
              sendLog("info", `  或从技能目录: cd examples/skills/${skill_id} && openskills build`);
              
              return `Error executing skill "${skill_id}": WASM 文件格式不正确。\n\n` +
                     `问题: OpenSkills runtime 需要 WASI 0.3 component 格式的 WASM 文件，但当前文件是旧格式。\n\n` +
                     `解决方案:\n` +
                     `1. 重新构建技能: openskills build examples/skills/${skill_id}\n` +
                     `2. 或从技能目录执行: cd examples/skills/${skill_id} && openskills build\n\n` +
                     `详细说明请参考: runtime/BUILD.md`;
            } else {
              sendLog("error", `执行技能失败 (${duration}ms): ${errorMessage}`);
              return `Error executing skill "${skill_id}": ${errorMessage}`;
            }
          }
        }
      },
    });

    // 获取技能元数据
    const skillMetadata = getSkillMetadata(skillsDir);

    // 创建 LLM
    const llm = new ChatAlibabaTongyi({
      modelName: "qwen-turbo",
      temperature: 0,
      alibabaApiKey: process.env.DASHSCOPE_API_KEY,
    });

    // 构建输入提示，包含技能列表和用户消息
    const inputPrompt = [
      "You can call run_skill to execute OpenSkills skills.",
      "Available skills:",
      skillMetadata,
      "",
      "User request:",
      message,
    ].join("\n");

    // 使用 structured-chat-zero-shot-react-description agent 类型
    // 这个类型支持所有 ChatModel，包括 ChatAlibabaTongyi
    sendLog("info", "创建 Agent Executor...");
    const executor = await initializeAgentExecutorWithOptions(
      [runSkillTool],
      llm,
      {
        agentType: "structured-chat-zero-shot-react-description",
        verbose: true, // 启用详细日志（输出到终端）
      }
    );

    // 执行并返回结果
    sendLog("info", "开始执行 Agent...");
    const executionStartTime = Date.now();
    const result = await executor.invoke({ input: inputPrompt });
    const executionDuration = Date.now() - executionStartTime;

    sendLog("success", `Agent 执行完成 (总耗时: ${executionDuration}ms)`);
    sendLog("info", `响应内容: ${result.output.substring(0, 200)}${result.output.length > 200 ? '...' : ''}`);

    // 发送完成事件
    res.write(`data: ${JSON.stringify({ 
      type: "response", 
      content: result.output 
    })}\n\n`);

    res.write(`data: ${JSON.stringify({ type: "done" })}\n\n`);

  } catch (error: any) {
    // sendLog 已在 try 块之前定义，可以直接使用
    try {
      sendLog("error", `执行出错: ${error.message}`);
      if (error.stack) {
        sendLog("error", `错误堆栈: ${error.stack.substring(0, 500)}${error.stack.length > 500 ? '...' : ''}`);
      }
    } catch (logError) {
      // 如果 sendLog 失败，至少输出到终端
      console.error(`[${requestId}] ❌ 执行出错: ${error.message}`);
    }
    
    res.write(`data: ${JSON.stringify({ 
      type: "error", 
      content: error.message 
    })}\n\n`);
  } finally {
    res.end();
  }
});

// 健康检查
app.get("/api/health", (req, res) => {
  res.json({ status: "ok", timestamp: new Date().toISOString() });
});

const PORT = process.env.PORT || 3000;

app.listen(PORT, () => {
  console.log(`🚀 OpenSkills Agent Playground`);
  console.log(`📱 Server running on http://localhost:${PORT}`);
  console.log(`🔧 Skills directory: ${skillsDir}`);
  console.log(`📂 Public directory: ${publicDir}`);
  console.log(`\n💡 Open http://localhost:${PORT} in your browser to start!`);
});
