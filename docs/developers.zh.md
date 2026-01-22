# 开发者指南

本指南面向希望在应用程序中使用 OpenSkills 运行时的开发者。

## 概述

OpenSkills 运行时是一个与 Claude Skills 兼容的运行时，在基于 WASM 的沙箱中执行技能。它提供了一个 Rust 核心以及 TypeScript 和 Python 绑定。

## 架构

```
┌─────────────────────┐
│  您的应用程序       │
└──────────┬──────────┘
           │
    ┌──────▼──────┐
    │   绑定层    │  (TypeScript/Python/Rust)
    └──────┬──────┘
           │
    ┌──────▼──────┐
    │  Rust 核心  │  (openskills-runtime)
    └──────┬──────┘
           │
    ┌──────▼──────┐
    │  Wasmtime   │  (WASM 执行)
    └─────────────┘
```

## 快速开始

### Rust

```rust
use openskills_runtime::{OpenSkillRuntime, ExecutionOptions};
use serde_json::json;

// 创建运行时并发现技能
let mut runtime = OpenSkillRuntime::new();
runtime.discover_skills()?;

// 执行一个技能
let result = runtime.execute_skill(
    "my-skill",
    ExecutionOptions {
        input: Some(json!({"input": "data"})),
        timeout_ms: Some(5000),
        ..Default::default()
    }
)?;

println!("Output: {}", result.output);
```

### TypeScript

**基本用法：**
```typescript
import { OpenSkillRuntime } from '@finogeek/openskills';

const runtime = OpenSkillRuntime.fromDirectory('./skills');
runtime.discoverSkills();

const result = runtime.executeSkill('my-skill', {
  timeoutMs: 5000,
  input: JSON.stringify({ input: 'data' })
});

console.log(result.outputJson);
```

**使用预构建工具（推荐）：**
```typescript
import { OpenSkillRuntime } from '@finogeek/openskills';
import { createSkillTools, getAgentSystemPrompt } from '@finogeek/openskills/tools';
import { generateText } from 'ai';

const runtime = OpenSkillRuntime.fromDirectory('./skills');
runtime.discoverSkills();

// 创建预构建工具（替代约 200 行的手动定义）
const tools = createSkillTools(runtime, {
  workspaceDir: './output'  // 沙箱工作区
});

// 获取技能无关的系统提示
const systemPrompt = getAgentSystemPrompt(runtime);

// 与 Vercel AI SDK 一起使用
const result = await generateText({
  model: yourModel,
  system: systemPrompt,
  prompt: userQuery,
  tools,
});
```

### Python

**基本用法：**
```python
from openskills import OpenSkillRuntime

runtime = OpenSkillRuntime.from_directory('./skills')
runtime.discover_skills()

result = runtime.execute_skill(
    'my-skill',
    input={'input': 'data'},
    timeout_ms=5000
)

print(result['output'])
```

**使用预构建工具（推荐）：**
```python
from openskills import OpenSkillRuntime
from openskills_tools import create_langchain_tools, get_agent_system_prompt

runtime = OpenSkillRuntime.from_directory('./skills')
runtime.discover_skills()

# 创建预构建的 LangChain 工具
tools = create_langchain_tools(runtime, workspace_dir='./output')

# 获取系统提示
system_prompt = get_agent_system_prompt(runtime)

# 与 LangChain 一起使用
from langchain.agents import create_agent
agent = create_agent(model, tools, system_prompt=system_prompt)
```

## 预构建工具（简化智能体设置）

OpenSkills 提供**预构建工具定义**，消除样板代码并简化智能体集成。你无需为每个技能操作手动定义工具，而是可以使用适用于任何智能体框架的现成工具。

### TypeScript：`createSkillTools()`

`@finogeek/openskills/tools` 模块为 Vercel AI SDK 提供预构建工具：

```typescript
import { OpenSkillRuntime } from '@finogeek/openskills';
import { createSkillTools, getAgentSystemPrompt } from '@finogeek/openskills/tools';
import { generateText } from 'ai';

const runtime = OpenSkillRuntime.fromDirectory('./skills');
runtime.discoverSkills();

// 一次调用创建所有必要的工具
const tools = createSkillTools(runtime, {
  workspaceDir: './output'  // 可选：沙箱工作区目录
});

// 可用的工具：
// - list_skills: 列出可用的技能
// - activate_skill: 加载完整的 SKILL.md 指令
// - read_skill_file: 从技能读取辅助文件
// - list_skill_files: 列出技能目录中的文件
// - run_skill_script: 执行沙箱化的 Python/shell 脚本
// - run_sandboxed_bash: 运行沙箱化的 bash 命令
// - write_file: 写入工作区（带路径验证）
// - read_file: 从工作区读取（带路径验证）
// - list_workspace_files: 列出工作区中的文件
// - get_file_info: 获取文件元数据

// 获取技能无关的系统提示
const systemPrompt = getAgentSystemPrompt(runtime);

// 与任何 LLM 一起使用
const result = await generateText({
  model: yourModel,
  system: systemPrompt,
  prompt: userQuery,
  tools,
});
```

**优势：**
- ✅ **减少约 200 行代码**：无需手动工具定义
- ✅ **内置安全性**：路径验证、工作区隔离
- ✅ **工作区管理**：自动沙箱化文件 I/O
- ✅ **技能无关**：适用于任何技能，无需代码更改

### Python：`create_langchain_tools()` 和 `create_simple_tools()`

对于 Python，你有两个选项：

**LangChain 集成：**
```python
from openskills import OpenSkillRuntime
from openskills_tools import create_langchain_tools, get_agent_system_prompt

runtime = OpenSkillRuntime.from_directory('./skills')
runtime.discover_skills()

# 创建与 LangChain 兼容的工具
tools = create_langchain_tools(runtime, workspace_dir='./output')

# 获取系统提示
system_prompt = get_agent_system_prompt(runtime)

# 与 LangChain 一起使用
from langchain.agents import create_agent
agent = create_agent(model, tools, system_prompt=system_prompt)
```

**框架无关（简单函数）：**
```python
from openskills import OpenSkillRuntime
from openskills_tools import create_simple_tools

runtime = OpenSkillRuntime.from_directory('./skills')
runtime.discover_skills()

# 创建简单的可调用函数（适用于任何框架）
tools = create_simple_tools(runtime, workspace_dir='./output')

# 直接使用工具
skills = tools['list_skills']()
loaded = tools['activate_skill']('my-skill')
tools['write_file']('output.txt', 'Hello, World!')
```

### 工作区管理

预构建工具包括用于文件 I/O 操作的**自动工作区管理**：

- **沙箱目录**：所有文件操作都隔离在工作区中
- **路径验证**：防止目录遍历攻击
- **自动创建**：如果工作区目录不存在，则会自动创建
- **环境变量**：技能可以通过 `SKILL_WORKSPACE` 环境变量访问工作区

```typescript
// TypeScript
const tools = createSkillTools(runtime, {
  workspaceDir: './output'  // 所有文件 I/O 都在这里
});

// 通过 write_file 工具写入的文件被沙箱化到 ./output
```

```python
# Python
tools = create_langchain_tools(runtime, workspace_dir='./output')

# 通过 write_file 工具写入的文件被沙箱化到 ./output
```

参见 [examples/agents/simple](examples/agents/simple/) 获取完整的工作示例。

## CLI

`openskills` 二进制文件提供发现、激活、执行和验证工具：

```bash
# 从标准位置发现技能
openskills discover

# 列出目录中的技能
openskills list --dir ./skills

# 验证技能目录
openskills validate ./skills/my-skill --warnings

# 分析令牌使用情况
openskills analyze ./skills/my-skill
```

## 核心概念

### 技能发现

技能从包含 `SKILL.md` 文件的目录中发现。运行时扫描技能并首先加载元数据（名称、描述）。

#### 系统提示注入

为了帮助模型发现技能，将技能元数据注入系统提示：

```rust
let mut runtime = OpenSkillRuntime::new();
runtime.discover_skills()?;

// 获取格式化的元数据用于系统提示
let system_prompt = format!(
    "{}\n\n{}",
    base_system_prompt,
    runtime.get_system_prompt_metadata()
);

// 或获取 JSON 格式用于编程使用
let metadata_json = runtime.get_system_prompt_metadata_json()?;

// 或获取令牌受限上下文的紧凑摘要
let summary = runtime.get_system_prompt_summary();
// 返回："Skills: code-review, test-generator (2 total)"
```

**可用方法：**
- `get_system_prompt_metadata()` - 人类可读的格式化文本
- `get_system_prompt_metadata_json()` - 用于编程使用的 JSON 格式
- `get_system_prompt_summary()` - 紧凑的单行摘要

### 验证 API

你可以直接从 Rust 验证技能目录或估计令牌使用情况：

```rust
use openskills_runtime::OpenSkillRuntime;

// 验证技能格式和结构
let validation = OpenSkillRuntime::validate_skill_directory("./skills/my-skill");
if !validation.errors.is_empty() {
    eprintln!("Validation errors: {:?}", validation.errors);
} else {
    println!("✅ Validation passed");
    println!("  Errors: {}", validation.stats.error_count);
    println!("  Warnings: {}", validation.stats.warning_count);
}

// 分析令牌使用情况
let analysis = OpenSkillRuntime::analyze_skill_directory("./skills/my-skill");
println!("Token Analysis:");
println!("  Tier 1 (Metadata): ~{} tokens", analysis.tier1_tokens);
println!("  Tier 2 (Instructions): ~{} tokens", analysis.tier2_tokens);
println!("  Total: ~{} tokens", analysis.total_tokens);
```

**CLI 用法：**

```bash
# 验证技能
openskills validate ./skills/my-skill

# 验证并显示警告
openskills validate ./skills/my-skill --warnings

# 分析令牌使用情况
openskills analyze ./skills/my-skill

# 使用 JSON 输出分析
openskills analyze ./skills/my-skill --format json
```

### 渐进式披露

1. **第 1 层（元数据）**：启动时加载名称和描述
2. **第 2 层（指令）**：激活技能时加载完整的 SKILL.md 内容
3. **第 3 层（资源）**：按需加载支持文件和资源

### 执行模型

技能在安全的沙箱环境中执行。运行时自动处理所有安全和隔离。技能作者只需专注于编写清晰的指令。

#### 上下文分叉

清单中带有 `context: fork` 的技能在隔离的上下文中执行，中间输出被单独捕获。只有摘要返回到父上下文，防止上下文污染。

**重要**：分叉上下文在技能激活**之后**开始，而不是之前。

**分叉生命周期**：

1. **激活阶段**（主上下文）：
   ```rust
   // activate_skill() 在主上下文中加载指令
   let skill = runtime.activate_skill("explorer-skill")?;
   // 指令返回到主对话
   // LLM 在这里读取/理解指令
   ```

2. **执行阶段**（创建分叉）：
   ```rust
   use openskills_runtime::{OpenSkillRuntime, ExecutionContext, ExecutionOptions};

   let mut runtime = OpenSkillRuntime::new();
   let main_context = ExecutionContext::new();

   // 分叉在此处创建，当执行开始时
   // 如果技能有 context: fork，它会自动隔离执行
   let result = runtime.execute_skill_with_context(
       "explorer-skill",
       ExecutionOptions::default(),
       &main_context
   )?;

   // 对于分叉的技能，result.output 只包含摘要
   // 中间输出（工具调用、错误、调试日志）被捕获在分叉中
   // 但不返回到主上下文
   println!("Summary: {}", result.output["summary"]);
   ```

**什么去哪里**：
- **主上下文**：技能激活、指令理解、最终摘要
- **分叉上下文**：工具调用、中间输出、错误、调试日志、试错

**手动上下文管理：**

```rust
// 手动创建和分叉上下文
let main = ExecutionContext::new();
let fork = main.fork();

// 在分叉上下文中记录输出
fork.record_output(OutputType::Stdout, "intermediate output".to_string());

// 从分叉上下文生成摘要
let summary = fork.summarize();
```

#### 带有 `context: fork` 的纯指令技能

当技能主要是指令性的（没有 WASM/原生脚本）时，智能体必须在分叉上下文中执行工具调用并记录它们的输出。使用技能会话来捕获工具调用并返回仅摘要结果：

```rust
use openskills_runtime::{OpenSkillRuntime, ExecutionContext};

let mut runtime = OpenSkillRuntime::new();
let parent = ExecutionContext::new();

// 启动技能会话（如果技能指定 context: fork 则分叉）
let mut session = runtime.start_skill_session(
    "code-review",
    Some(serde_json::json!({ "query": "Review this file" })),
    Some(&parent),
)?;

// 智能体执行工具并在会话中记录输出
runtime.check_tool_permission(
    "code-review",
    "Read",
    None,
    std::collections::HashMap::new(),
)?;
session.record_tool_call("Read", &serde_json::json!({ "path": "src/lib.rs" }));

// 智能体生成最终结果
let final_output = serde_json::json!({ "review": "Looks good." });

// 完成会话（如果分叉则返回摘要）
let result = runtime.finish_skill_session(
    session,
    final_output,
    String::new(),
    String::new(),
    openskills_runtime::ExecutionStatus::Success,
)?;

println!("Summary: {}", result.output["summary"]);
```

### 权限

权限基于技能的 `allowed-tools` 配置强制执行：
- 文件系统访问（读/写路径）
- 网络访问（域白名单）
- 环境变量
- 副作用（写入、执行）

#### 询问后行动权限系统

对于有风险的操作（Write、Bash、WebSearch 等），你可以在执行前要求用户批准：

```rust
use openskills_runtime::{OpenSkillRuntime, CliPermissionCallback};
use std::sync::Arc;

// 启用交互式权限提示
let mut runtime = OpenSkillRuntime::new()
    .with_permission_callback(Arc::new(CliPermissionCallback));

// 或启用严格模式（默认拒绝所有）
let mut runtime = OpenSkillRuntime::new()
    .with_strict_permissions();

// 执行技能 - 将提示有风险的操作
let result = runtime.execute_skill("my-skill", options)?;

// 检查权限审计日志
let audit = runtime.get_permission_audit();
for entry in audit {
    println!("{}: {} {} - {:?}", 
        entry.timestamp, 
        entry.skill_id, 
        entry.tool, 
        entry.response
    );
}

// 重置所有"始终允许"授权
runtime.reset_permission_grants();
```

**自定义权限回调：**

实现 `PermissionCallback` trait 用于自定义 UI（GUI、自动化策略等）：

```rust
use openskills_runtime::{PermissionCallback, PermissionRequest, PermissionResponse, OpenSkillError};

struct MyPermissionCallback;

impl PermissionCallback for MyPermissionCallback {
    fn request_permission(
        &self,
        request: &PermissionRequest,
    ) -> Result<PermissionResponse, OpenSkillError> {
        // 你的自定义逻辑在这里
        // 返回：AllowOnce、AllowAlways 或 Deny
        Ok(PermissionResponse::AllowOnce)
    }
}
```

**内置回调：**
- `CliPermissionCallback` - 交互式终端提示
- `DenyAllCallback` - 严格模式（全部拒绝）

## API 参考

### Rust API

#### `OpenSkillRuntime`

主运行时接口。

```rust
impl OpenSkillRuntime {
    // 构造
    pub fn new() -> Self;
    pub fn from_config(config: RuntimeConfig) -> Self;
    pub fn with_project_root<P: AsRef<Path>>(root: P) -> Self;
    pub fn with_custom_directories<P: AsRef<Path>>(self, dirs: Vec<P>) -> Self;
    pub fn with_permission_callback(self, callback: Arc<dyn PermissionCallback>) -> Self;
    pub fn with_strict_permissions(self) -> Self;
    
    // 发现
    pub fn discover_skills(&mut self) -> Result<Vec<SkillDescriptor>, OpenSkillError>;
    pub fn load_from_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<Vec<SkillDescriptor>, OpenSkillError>;
    pub fn list_skills(&self) -> Vec<SkillDescriptor>;
    
    // 系统提示助手
    pub fn get_system_prompt_metadata(&self) -> String;
    pub fn get_system_prompt_metadata_json(&self) -> Result<String, OpenSkillError>;
    pub fn get_system_prompt_summary(&self) -> String;
    
    // 激活
    pub fn activate_skill(&self, skill_id: &str) -> Result<LoadedSkill, OpenSkillError>;
    
    // 执行
    pub fn execute_skill(&mut self, skill_id: &str, options: ExecutionOptions) -> Result<ExecutionResult, OpenSkillError>;
    pub fn execute_skill_with_context(&mut self, skill_id: &str, options: ExecutionOptions, parent_context: &ExecutionContext) -> Result<ExecutionResult, OpenSkillError>;
    
    // 权限
    pub fn get_permission_audit(&self) -> Vec<PermissionAuditEntry>;
    pub fn reset_permission_grants(&self);
    
    // 验证（静态方法）
    pub fn validate_skill_directory<P: AsRef<Path>>(path: P) -> ValidationResult;
    pub fn analyze_skill_directory<P: AsRef<Path>>(path: P) -> TokenAnalysis;
}
```

#### `ExecutionOptions`

```rust
pub struct ExecutionOptions {
    pub timeout_ms: Option<u64>,
    pub memory_mb: Option<u64>,
    pub input: Option<Value>,
    pub wasm_module: Option<String>,
}
```

#### `ExecutionResult`

```rust
pub struct ExecutionResult {
    pub output: Value,
    pub stdout: String,
    pub stderr: String,
    pub audit: AuditRecord,
}
```

### 错误处理

所有操作返回 `Result<T, OpenSkillError>`。错误类型：

- `SkillNotFound`：未找到技能 ID
- `InvalidManifest`：SKILL.md 解析失败
- `PermissionDenied`：操作不被允许（用户拒绝权限或严格模式）
- `Timeout`：执行超过时间限制
- `ExecutionFailure`：技能执行失败
- `WasmError`：WASM 模块加载或执行错误
- `ValidationError`：技能格式验证失败

## 构建技能

### 技能结构

```
my-skill/
├── SKILL.md           # 必需：YAML 前置元数据 + Markdown
├── examples/          # 可选：示例文件
├── references/        # 可选：参考文档
└── README.md          # 可选：附加文档
```

大多数技能是指令性的，只需要 `SKILL.md`。支持文件可以在指令中引用，但由运行时按需加载。

### SKILL.md 格式

```yaml
---
name: my-skill
description: 它做什么以及何时使用
allowed-tools: Read, Write
---

# 指令

Markdown 内容在这里...
```

完整格式规范参见 [spec.zh.md](spec.zh.md)。

### 指令性技能

大多数技能是指令性的 - 它们为 AI 提供关于如何执行特定任务的清晰指导。技能的指令在 Markdown 主体中告诉 AI 在激活技能时要做什么。

运行时自动处理所有安全和沙箱。技能作者无需了解底层执行环境。

## 最佳实践

1. **错误处理**：始终适当处理 `OpenSkillError`
2. **超时**：为技能执行设置合理的超时
3. **权限**：只授予必要的权限
4. **审计日志**：使用审计记录进行调试和合规性
5. **资源管理**：执行后清理资源

## 示例

参见 `examples/skills/` 获取示例技能实现。

## 故障排除

### 常见问题

**技能未找到**：确保技能目录名称与 SKILL.md 中的 `name` 匹配

**权限被拒绝**：检查技能清单中的 `allowed-tools`

**超时错误**：执行超过时间限制（检查技能复杂度）

## 进一步阅读

- [规范](spec.zh.md) - 完整的运行时规范
- [贡献指南](contributing.zh.md) - 如何贡献
- [架构](architecture.zh.md) - 详细的架构文档
