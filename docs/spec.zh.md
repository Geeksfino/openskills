# OpenSkills Runtime 规范 (v0.2)

OpenSkills 是一个 Claude Skills 兼容的运行时，它使用**操作系统级别的沙箱（macOS 上的 seatbelt，Linux 上的 seccomp）作为主要执行方法**，并提供**实验性的基于 WASM 的沙箱**用于特定用例。

## Claude Skills 兼容性

此运行时实现了 [Claude Code Agent Skills 规范](https://code.claude.com/docs/en/skills)。

### Skill 格式

Skills 是包含 `SKILL.md` 文件的目录，该文件带有 YAML frontmatter 和 Markdown 说明：

```
my-skill/
├── SKILL.md           # 必需: YAML frontmatter + Markdown 说明
├── scripts/           # 可选: 支持脚本
├── examples/          # 可选: 示例文件
├── wasm/              # 可选: 用于沙箱执行的 WASM 模块
│   └── skill.wasm
└── README.md          # 可选: 文档
```

### SKILL.md 格式

```yaml
---
name: my-skill
description: Skill 的功能及使用场景。Claude 使用此字段决定何时应用该 Skill。
allowed-tools: Read, Write, Bash
model: claude-sonnet-4-20250514
context: fork
agent: Explore
user-invocable: true
---

# 说明

Claude 在 Skill 激活时遵循的 Markdown 内容。
根据需要引用支持文件。
```

### 必需字段

| 字段 | 约束条件 |
|------|---------|
| `name` | 仅限小写字母、数字、连字符。最多 64 个字符。必须与目录名称匹配。 |
| `description` | 非空。最多 1024 个字符。不含 XML 标签。 |

### 可选字段

| 字段 | 描述 |
|------|------|
| `allowed-tools` | Skill 激活时 Claude 可以使用的工具，无需请求权限。 |
| `model` | 使用的模型（例如 `claude-sonnet-4-20250514`）。 |
| `context` | 设置为 `fork` 以获得隔离的子代理上下文。 |
| `agent` | 当 `context: fork` 时的代理类型（例如 `Explore`、`Plan`）。 |
| `hooks` | 生命周期钩子（`PreToolUse`、`PostToolUse`、`Stop`）。 |
| `user-invocable` | Skill 是否出现在斜杠命令菜单中（默认值：true）。 |

## 发现位置

Skills 从以下位置发现（按顺序，后面的覆盖前面的）：

1. **个人**：`~/.claude/skills/`
2. **项目**：`.claude/skills/`（相对于项目根目录）
3. **嵌套**：任何子目录中的 `.claude/skills/`（monorepo 支持）
4. **自定义**：代理配置的目录（通过 `with_custom_directory` 或 `RuntimeConfig`）

### 代理配置的目录

代理可以配置自定义 skill 目录，作为标准位置的补充或替代：

- **多个目录**：代理可以指定多个自定义目录
- **覆盖标准位置**：代理可以禁用标准位置发现
- **灵活配置**：支持构建器模式或配置结构体

这允许代理：
- 从代理特定的目录加载 skills
- 在多个代理之间共享 skills
- 在需要时覆盖标准 Claude Skills 位置

## 渐进式披露

1. **发现**：启动时，仅加载 `name` 和 `description`。
2. **激活**：当 Skill 被触发时，加载完整的 `SKILL.md` 内容。
3. **执行**：支持文件和 WASM 模块按需加载。

## WASM 沙箱（OpenSkills 扩展）- 实验性

**状态**：实验性功能。通过操作系统级沙箱（seatbelt/seccomp）的原生脚本是主要的执行方法。

OpenSkills 提供**实验性的** WASM/WASI 沙箱作为特定用例的可选执行方法。

### 为什么选择 WASM？（长期愿景）

WASM 支持用于需要以下特性的用例：
- **确定性**：相同输入 → 相同输出，对审计和合规性至关重要
- **快速启动**：毫秒级启动时间
- **跨平台一致性**：在 macOS、Linux、Windows 上的沙箱行为相同
- **能力基安全**：通过 WASI 能力实现细粒度控制
- **隔离**：强大的内存和执行隔离

**最适合**：策略逻辑、编排、验证、评分、推理粘合剂。

**不适合**：完整 Python 生态系统、ML 库（NumPy、PyTorch）、原生库、操作系统原生行为。

查看 [README.md](../README.md#wasm-support-long-term-vision) 了解有关 WASM 角色和限制的详细讨论。

### WASM 执行模型（实验性）

**注意**：WASM 执行是实验性的。大多数技能使用原生 Python/shell 脚本。

Skills 可以可选地包含用于沙箱脚本执行的 WASM 模块：

```
my-skill/
├── SKILL.md
└── wasm/
    └── skill.wasm     # 兼容 WASI 的模块（可选）
```

如果存在 WASM 模块，运行时：
1. 使用 Wasmtime 加载 WASM 模块
2. 根据 `allowed-tools` 配置 WASI 能力
3. 以适当的权限预打开文件系统路径
4. 以超时和内存限制执行
5. 捕获 stdout/stderr 用于审计

**如果不存在 WASM 模块**，运行时使用原生 Python/shell 脚本通过操作系统级沙箱（macOS 上的 seatbelt）。

### 能力映射

`allowed-tools` 值映射到 WASI 能力：

| 工具 | WASI 能力 |
|------|----------|
| `Read`、`Grep`、`Glob`、`LS` | 文件系统读取 |
| `Write`、`Edit`、`MultiEdit` | 文件系统写入 |
| `Bash`、`Terminal` | 完整文件系统 |
| `WebSearch`、`Fetch` | 网络访问 |

### WASM 模块接口

WASM 模块应该与 WASI 兼容。运行时提供：

**环境变量：**
- `SKILL_ID`：Skill 标识符
- `SKILL_NAME`：manifest 中的 Skill 名称
- `SKILL_INPUT`：JSON 输入数据
- `TIMEOUT_MS`：执行超时
- `RANDOM_SEED`：确定性种子（如果已配置）

**预打开的目录：**
- `/skill`：Skill 根目录（只读）
- 基于 `allowed-tools` 的其他路径

**输出：**
- 将 JSON 写入 stdout 以获得结构化输出
- stderr 被捕获用于日志记录/调试

### 约束

```yaml
# 默认值
timeout_ms: 30000    # 30 秒
memory_mb: 128       # 128 MB
```

## 审计模型

每次执行都会生成一条审计记录：

```
skill_id: string
version: string
input_hash: sha256
output_hash: sha256
start_time_ms: timestamp
duration_ms: number
permissions_used: [string]
exit_status: success | failed | timeout
stdout: string
stderr: string
```

## API

### Rust

```rust
use openskills_runtime::{OpenSkillRuntime, ExecutionOptions};

// 从标准位置发现
let mut runtime = OpenSkillRuntime::new();
let skills = runtime.discover_skills()?;

// 列出可用 skills（渐进式披露）
for skill in runtime.list_skills() {
    println!("{}: {}", skill.id, skill.description);
}

// 激活 skill（加载完整说明）
let loaded = runtime.activate_skill("my-skill")?;
println!("{}", loaded.instructions);

// 执行 WASM 模块（如果存在）
let result = runtime.execute_skill("my-skill", ExecutionOptions::default())?;
println!("{}", result.output);
```

### CLI

```bash
# 从标准位置发现 skills
openskills discover

# 列出特定目录中的 skills
openskills list --dir ./skills

# 激活（加载完整内容）
openskills activate my-skill --json

# 执行 WASM 模块
openskills execute my-skill --input '{"query": "hello"}'
```

## 兼容性说明

### 支持的功能

- 完整的 SKILL.md 格式支持（YAML frontmatter + Markdown）
- 所有元数据字段（`allowed-tools`、`model`、`context`、`agent`、`hooks`、`user-invocable`）
- 标准发现路径
- 渐进式披露
- 名称/描述约束的验证

### 差异之处

- **沙箱**：WASM/WASI 而不是 seatbelt/seccomp
- **脚本执行**：脚本必须编译为 WASM 或通过 WASM 解释器运行
- **环境**：WASI 环境而不是本机操作系统

### 从本机脚本迁移

具有本机脚本（`.sh`、`.py`）的 Skills 需要 WASM 兼容的替代方案：

1. **编译为 WASM**：使用 Rust、Go 或其他具有 WASM 目标的语言
2. **使用 WASM 解释器**：装载编译为 WASM 的解释器
3. **保持说明性**：大多数 skills 是说明性的，不需要修改

## 错误分类

- `SkillNotFound`：注册表中不存在 Skill ID
- `InvalidManifest`：SKILL.md 解析或验证失败
- `PermissionDenied`：操作不被 skill 配置允许
- `Timeout`：执行超过时间限制
- `ToolNotAllowed`：工具不在 `allowed-tools` 列表中
- `WasmError`：WASM 模块加载或执行失败
