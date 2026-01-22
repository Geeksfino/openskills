## 示例代理

本文件夹展示了如何将 OpenSkills 运行时集成到流行的代理框架中，使 TypeScript 和 Python 开发者都能够复用相同的技能。

### ⭐ 推荐：简单示例

**`simple`** 示例展示了使用预构建工具的**推荐方案**：

- ✅ **约120行代码**（相比手动方案的约470行）
- ✅ **预构建工具**：使用 `@finogeek/openskills/tools` 中的 `createSkillTools()`
- ✅ **技能无关**：无需硬编码技能知识
- ✅ **工作区管理**：自动沙箱文件 I/O
- ✅ **系统提示生成**：运行时生成技能无关的提示

```typescript
import { createSkillTools, getAgentSystemPrompt } from '@finogeek/openskills/tools';

// 一次调用创建所有工具（减少约200行代码）
const tools = createSkillTools(runtime, { workspaceDir: './output' });

// 获取技能无关的系统提示
const systemPrompt = getAgentSystemPrompt(runtime);
```

详见 [simple/README.md](simple/README.md)。

### 前置条件
- 在 `examples/skills` 下构建或放置技能（参见 `runtime/BUILD.md`）
- 为您的语言安装运行时绑定：
  - TypeScript: `npm install @finogeek/openskills`
  - Python: `pip install finclip-openskills`

### 示例
- **`simple`** ⭐：**推荐** - Vercel AI SDK 和预构建工具（约120行）
- `langchain-js`：LangChainJS 代理（手动工具定义）
- `langchain-python`：LangChain（Python）代理和预构建工具

### 主要改进

**之前（手动设置）：**
- 约470行工具定义
- 手动工作区管理
- 自定义系统提示
- 技能特定知识硬编码

**之后（预构建工具）：**
- 约120行总代码量
- 自动工作区管理
- 运行时生成的系统提示
- 技能无关设计

### 文档
- `QUICKSTART.md`：5分钟跨框架设置
- `GUIDE.md`：集成模式和最佳实践
- `simple/README.md`：使用预构建工具的完整示例
