# OpenSkills Agent Web UI

基于 Express 的 Web 界面，提供友好的交互体验来使用 OpenSkills Agent。

> **目录结构**：Web 界面相关代码位于 `web/` 子目录下，包括：
> - `web/server.ts` - Express 后端服务器
> - `web/public/` - 前端静态文件（HTML、CSS、JavaScript）

## 功能特性

- 🎨 **现代化 UI**：使用 Tailwind CSS 构建的响应式界面
- 💬 **实时对话**：与 AI Agent 进行流畅的对话交互
- 🛠️ **技能管理**：可视化查看和管理所有可用技能
- 📊 **状态指示**：实时显示服务状态和连接状态
- ⚡ **流式响应**：使用 Server-Sent Events (SSE) 实现实时响应

## 快速开始

### 1. 确保环境变量已配置

确保 `.env` 文件中包含：

```bash
DASHSCOPE_API_KEY=sk-your-alibaba-key-here
```

### 2. 启动 Web 服务器

```bash
npm run web
```

服务器将在 `http://localhost:3000` 启动。

### 3. 打开浏览器

在浏览器中访问 `http://localhost:3000`，即可开始使用。

## 使用说明

### 基本对话

1. 在输入框中输入你的问题
2. 点击"发送"按钮或按 Enter 键
3. AI Agent 会分析你的问题，选择合适的技能，并返回结果

### 使用技能

- **查看技能列表**：左侧面板显示所有可用技能
- **快速插入技能**：点击技能卡片，技能名称会自动插入到输入框
- **查看技能详情**：点击技能卡片后，可以在输入框中补充具体需求

### 状态指示

- 🟢 **绿色**：就绪，可以正常使用
- 🟡 **黄色**：处理中，Agent 正在思考或执行
- 🔴 **红色**：错误，连接失败或出现错误

## API 接口

### GET `/api/skills`

获取所有可用技能列表。

**响应示例：**
```json
[
  {
    "id": "code-review",
    "description": "Reviews code for quality, best practices, and potential issues."
  }
]
```

### GET `/api/skills/:id`

获取指定技能的详细信息。

**响应示例：**
```json
{
  "id": "code-review",
  "description": "Reviews code for quality...",
  "instructions": "When reviewing code..."
}
```

### POST `/api/chat`

发送消息给 Agent，使用 Server-Sent Events (SSE) 流式返回响应。

**请求体：**
```json
{
  "message": "你的问题"
}
```

**响应格式（SSE）：**
```
data: {"type":"start"}
data: {"type":"response","content":"AI 的回复内容"}
data: {"type":"done"}
```

### GET `/api/health`

健康检查接口。

**响应示例：**
```json
{
  "status": "ok",
  "timestamp": "2024-01-20T10:00:00.000Z"
}
```

## 技术栈

- **后端**：Express.js + TypeScript
- **前端**：原生 HTML/CSS/JavaScript + Tailwind CSS
- **实时通信**：Server-Sent Events (SSE)
- **Agent 框架**：LangChain.js
- **LLM**：Alibaba Tongyi (Qwen)

## 文件结构

```
langchain-js/
├── src/
│   ├── openskills-tool.ts # 工具函数（被 web 服务器复用）
│   └── ...                # 其他命令行示例
├── web/                   # Web 界面独立子目录
│   ├── server.ts          # Express 后端服务器
│   └── public/            # 前端静态文件
│       ├── index.html     # 前端 HTML
│       └── app.js         # 前端 JavaScript
└── package.json
```

## 自定义配置

### 修改端口

设置环境变量：

```bash
PORT=8080 npm run web
```

### 修改技能目录

在 `web/server.ts` 中修改 `skillsDir` 路径。

## 故障排除

### 端口被占用

如果 3000 端口被占用，可以设置其他端口：

```bash
PORT=3001 npm run web
```

### API Key 未设置

确保 `.env` 文件中包含 `DASHSCOPE_API_KEY`。

### 技能加载失败

检查技能目录路径是否正确，确保技能文件存在。

## 与命令行版本的区别

- **Web UI**：提供图形界面，适合交互式使用
- **命令行版本**：适合脚本化和自动化场景

两者使用相同的底层 Agent 逻辑，功能完全一致。
