# OpenSkills - 让你的Agent获得Skills

[English](README.md) | [中文](README.zh.md)

一个支持**双重沙箱**的 **Claude Skills 兼容运行时**：基于 WASM 的跨平台安全沙箱，加上 **macOS seatbelt** 用于原生 Python 和 Shell 脚本执行。OpenSkills 实现了 [Claude Code Agent Skills 规范](https://code.claude.com/docs/en/skills)，为**任何智能体框架**提供安全、灵活的运行时来执行技能。

## 设计理念

OpenSkills 与 Claude Skills **100% 语法兼容**，这意味着任何遵循 Claude Skills 格式（带有 YAML 前置元数据的 SKILL.md）的技能都可以在 OpenSkills 上运行。OpenSkills 的独特之处在于其**双重沙箱架构**：

- **WASM/WASI 沙箱**：提供跨平台安全性和一致性
- **macOS seatbelt 沙箱**：用于原生 Python 和 Shell 脚本执行

这种组合提供了两全其美的方案：WASM 的可移植性和安全性，加上 macOS 上原生执行的灵活性。OpenSkills 可以集成到**任何智能体框架**（LangChain、Vercel AI SDK、自定义框架）中，为智能体提供 Claude 兼容的技能访问能力。

### 核心设计原则

1. **100% 语法兼容性**：OpenSkills 使用与 Claude Skills 完全相同的 SKILL.md 格式来读取和执行技能。技能可以在 Claude Code 和 OpenSkills 之间共享，无需修改。

2. **双重沙箱架构**：OpenSkills 独特地结合了 **WASM/WASI 0.3**（组件模型）与 **macOS seatbelt** 沙箱：
   - **WASM/WASI**：跨平台安全性、基于能力的权限、内存安全、确定性执行
   - **macOS Seatbelt**：原生 Python 和 Shell 脚本执行，具有操作系统级别的沙箱隔离
   - **自动检测**：运行时根据技能类型自动选择合适的沙箱
   - **两全其美**：WASM 提供可移植性和安全性，seatbelt 提供原生灵活性

3. **JavaScript/TypeScript 优先**：OpenSkills 针对基于 JavaScript/TypeScript 的技能进行了优化，可以使用 `javy`（基于 QuickJS）等工具编译为 WASM 组件。这使得技能编写者可以使用熟悉的语言和生态系统。

### 目标用例

OpenSkills 专为需要 Claude 兼容技能的**任何智能体框架**而设计：

- **智能体框架集成**：可与 LangChain、Vercel AI SDK、自定义框架或任何需要工具式功能的系统配合使用
- **企业智能体**：由受信任的开发人员开发的内部技能
- **跨平台**：WASM 执行在 macOS、Linux、Windows 上完全相同
- **原生灵活性**：macOS seatbelt 允许在需要时使用原生 Python 和 Shell 脚本
- **安全性和可审计性**：两种沙箱方法都提供强大的隔离和审计日志记录

双重沙箱方法意味着您可以使用 WASM 实现跨平台技能，或在需要访问原生库或工具时在 macOS 上利用原生 Python/Shell。

## 限制

OpenSkills 的 WASM 优先方法相比原生执行存在一些限制：

### 当前不支持

1. **非 macOS 平台上的原生脚本**：
   - 原生 Python 和 Shell 脚本仅在 macOS 上支持（seatbelt）
   - Linux seccomp 支持正在规划中

2. **需要构建工作流（对于 WASM）**：
   - JavaScript/TypeScript 技能必须在执行前编译为 WASM 组件
   - 开发人员需要运行 `openskills build` 将源代码编译为 `wasm/skill.wasm`
   - 这相比"即插即用"的原生脚本增加了一个构建步骤

### 为什么存在这些限制

WASM 提供了强大的安全性和跨平台一致性，但它需要：
- **编译步骤**：源代码必须编译为 WASM
- **WASI 兼容性**：代码必须使用 WASI API，而不是原生操作系统 API
- **有限的原生库**：原生 Python 包、Shell 工具等不能直接使用

这些限制对于企业用例是可以接受的，因为：
- 开发人员控制技能开发过程
- 构建工作流是标准实践
- 安全性和跨平台一致性比便利性更重要

## 路线图

OpenSkills 将在保持其 WASM 优先理念的同时不断发展以解决限制：

1. **更多 WASM 就绪脚本**：我们将提供不断扩展的预构建 WASM 组件和模板库，用于常见任务，减少自定义编译的需要。

2. **原生脚本支持**：原生 Python 和 Shell 脚本在 macOS 上通过 seatbelt 支持。Linux seccomp 支持正在规划中，以完成跨平台原生沙箱。

3. **改进的工具**：更好的构建工具和模板，使 WASM 编译对开发人员更加透明。

## 特性

- ✅ **100% Claude Skills 兼容**：完整支持 SKILL.md 格式
- 🔒 **双重沙箱架构**：WASM/WASI 0.3 + macOS seatbelt（生态系统中的独特之处）
- 🧰 **原生脚本支持**：通过 seatbelt 在 macOS 上执行 Python 和 Shell 脚本
- 🤖 **任何智能体框架**：与 LangChain、Vercel AI SDK 或自定义框架集成
- 📊 **渐进式披露**：高效的分层加载（元数据 → 指令 → 资源）
- 🔌 **多语言绑定**：Rust 核心，提供 TypeScript 和 Python 绑定
- 🛡️ **基于能力的安全性**：通过 WASI 和 seatbelt 配置文件实现细粒度权限
- 🏗️ **构建工具**：`openskills build` 用于将 TS/JS 编译为 WASM 组件
- 🌐 **跨平台**：WASM 执行在 macOS、Linux、Windows 上完全相同

## 快速开始

### 安装

```bash
# Rust（从源码）
git clone <repository-url>
cd openskills

# 初始化子模块（测试和示例需要）
git submodule update --init --recursive

cd runtime
cargo build --release

# TypeScript
npm install @finogeek/openskills

# Python
pip install openskills
```

### 构建技能

```bash
# 安装构建依赖
cargo install javy-cli  # 用于 JavaScript → WASM 编译

# 从 TypeScript/JavaScript 构建技能
cd my-skill
openskills build

# 这将编译 src/index.ts → wasm/skill.wasm
```

### 使用技能

```rust
use openskills_runtime::{OpenSkillRuntime, ExecutionOptions};
use serde_json::json;

// 从标准位置发现技能
let mut runtime = OpenSkillRuntime::new();
runtime.discover_skills()?;

// 执行技能
let result = runtime.execute_skill(
    "my-skill",
    ExecutionOptions {
        timeout_ms: Some(5000),
        input: Some(json!({"input": "data"})),
        ..Default::default()
    }
)?;
```

查看 [开发者指南](docs/developers.md) 获取详细的使用示例。

### 与智能体框架集成

OpenSkills 可与**任何智能体框架**配合使用，为智能体提供 Claude 兼容的技能访问。以下是一些示例：

**LangChain (TypeScript/Python)**
```typescript
import { OpenSkillRuntime } from "@finogeek/openskills";
import { DynamicStructuredTool } from "@langchain/core/tools";

const runtime = OpenSkillRuntime.fromDirectory("./skills");
runtime.discoverSkills();

const tool = new DynamicStructuredTool({
  name: "run_skill",
  schema: z.object({ skill_id: z.string(), input: z.string() }),
  func: async ({ skill_id, input }) => {
    const result = runtime.executeSkill(skill_id, { input });
    return result.outputJson;
  },
});
```

**Vercel AI SDK**
```typescript
import { OpenSkillRuntime } from "@finogeek/openskills";
import { tool } from "ai";

const runtime = OpenSkillRuntime.fromDirectory("./skills");
const runSkill = tool({
  inputSchema: z.object({ skill_id: z.string(), input: z.string() }),
  execute: async ({ skill_id, input }) => {
    return runtime.executeSkill(skill_id, { input }).outputJson;
  },
});
```

查看 [examples/agents](examples/agents/) 获取与 LangChain、Vercel AI SDK 等的完整集成示例。

## 架构

OpenSkills 使用 Rust 核心运行时和语言绑定：

```
┌────────────────────┐
│  您的应用程序      │
│  (TS/Python/Rust)  │
└──────────┬──────────┘
           │
    ┌──────▼──────┐
    │   绑定层    │  (napi-rs / PyO3)
    └──────┬──────┘
           │
    ┌──────▼──────┐
    │  Rust 核心  │  (openskills-runtime)
    └──────┬──────┘
           │
    ┌──────▼──────┐
    │   执行层    │  (WASM/WASI 0.3 + macOS seatbelt)
    └─────────────┘
```

### 执行模型

1. **技能发现**：扫描目录中的 SKILL.md 文件
2. **渐进式加载**：按需加载元数据 → 指令 → 资源
3. **执行**：在 Wasmtime 中运行 `wasm/skill.wasm` 或通过 macOS seatbelt 运行原生 `.py/.sh`
4. **权限执行**：从 `allowed-tools` 映射能力到 WASM 或 seatbelt
5. **审计日志**：所有执行都记录输入/输出哈希

## OpenSkills 的独特之处

OpenSkills 是**唯一**结合以下特性的运行时：

1. **WASM/WASI 沙箱**：具有基于能力权限的跨平台安全性
2. **macOS Seatbelt 沙箱**：具有操作系统级别隔离的原生 Python 和 Shell 脚本执行
3. **自动检测**：运行时自动为每个技能选择合适的沙箱
4. **智能体框架无关**：可与任何智能体框架配合使用（LangChain、Vercel AI SDK、自定义）

这种双重方法意味着您将获得：
- **可移植性**：WASM 技能在 macOS、Linux、Windows 上运行完全相同
- **灵活性**：在需要原生库时在 macOS 上使用原生 Python/Shell 脚本
- **安全性**：两种沙箱方法都提供强大的隔离
- **兼容性**：100% 兼容 Claude Skills 规范

## 对比：OpenSkills vs Claude Code

| 方面 | Claude Code | OpenSkills |
|------|-------------|------------|
| **SKILL.md 格式** | ✅ 完整支持 | ✅ 100% 兼容 |
| **沙箱** | seatbelt/seccomp | **WASM/WASI 0.3 + seatbelt (macOS)** ⭐ |
| **跨平台** | 操作系统特定 | WASM 相同，原生仅 macOS |
| **脚本执行** | 原生（Python、shell） | WASM 组件 + 原生（macOS） |
| **需要构建** | 否 | 是（TS/JS → WASM） |
| **原生 Python** | ✅ 支持 | ✅ macOS (seatbelt) |
| **Shell 脚本** | ✅ 支持 | ✅ macOS (seatbelt) |
| **智能体框架** | 仅 Claude Desktop | **任何框架** ⭐ |
| **用例** | 桌面用户，任意技能 | 企业智能体，任何智能体框架 |

## 项目结构

```
openskills/
├── runtime/              # Rust 核心运行时
│   ├── src/
│   │   ├── build.rs      # TS/JS → WASM 构建工具
│   │   ├── wasm_runner.rs # WASI 0.3 执行
│   │   ├── native_runner.rs # Seatbelt 执行 (macOS)
│   │   └── ...
│   └── BUILD.md          # 构建工具文档
├── bindings/             # 语言绑定
│   ├── ts/              # TypeScript (napi-rs)
│   └── python/           # Python (PyO3)
├── docs/                 # 文档
│   ├── developers.md     # 开发者指南
│   ├── contributing.md   # 贡献指南
│   ├── architecture.md   # 架构详情
│   └── spec.md           # 规范
├── examples/             # 示例技能
└── scripts/              # 构建脚本
```

## 文档

- **[开发者指南](docs/developers.md)**：在应用程序中使用 OpenSkills
- **[构建工具指南](runtime/BUILD.md)**：编译 TypeScript/JavaScript 技能
- **[贡献指南](docs/contributing.md)**：如何为 OpenSkills 做出贡献
- **[架构](docs/architecture.md)**：内部架构和设计
- **[规范](docs/spec.md)**：完整的运行时规范

## 构建

```bash
# 克隆并初始化子模块（用于测试和示例）
git clone <repository-url>
cd openskills
git submodule update --init --recursive

# 构建所有内容
./scripts/build_all.sh

# 仅构建运行时
cd runtime
cargo build --release

# 构建绑定
./scripts/build_bindings.sh
```

### 子模块

`examples/claude-official-skills` 目录是一个指向 [anthropics/skills](https://github.com/anthropics/skills) 的 git 子模块。这提供了对官方 Claude Skills 的访问，用于测试和参考。

- **初始克隆**：使用 `git clone --recursive <url>` 或在克隆后运行 `git submodule update --init --recursive`
- **更新**：`cd examples/claude-official-skills && git pull && cd ../.. && git add examples/claude-official-skills && git commit`
- **测试**：如果未初始化子模块，测试套件会优雅地跳过测试

## 状态

- ✅ **Rust 运行时**：完全支持 WASI 0.3
- ✅ **TypeScript 绑定**：正常工作
- ✅ **Python 绑定**：正常工作（需要 Python ≤3.13）
- ✅ **WASM 执行**：完全支持 WASI 0.3 组件模型
- ✅ **构建工具**：`openskills build` 用于 TS/JS 编译
- ✅ **原生脚本**：Seatbelt 沙箱（macOS）
- 🚧 **原生脚本（Linux）**：规划中的 Seccomp 支持

## 许可证

MIT
