# OpenSkills 架构

本文档描述了 OpenSkills 运行时的内部架构。

## 概述

OpenSkills 使用 Rust 作为核心运行时构建，为执行 Claude Skills 提供原生操作系统级沙箱（macOS seatbelt + Linux Landlock）作为主要执行方法，以及可用于特定用例的实验性基于 WASM 的沙箱。架构强调：

- **安全性**：操作系统级沙箱（macOS seatbelt + Linux Landlock）作为主要方式，实验性的基于能力权限的 WASM 沙箱
- **性能**：高效的技能发现和执行
- **兼容性**：100% 兼容 Claude Skills 格式
- **可扩展性**：多个生态系统的语言绑定

**注意**：WASM 沙箱是实验性的，不是主要的执行方法。大多数技能使用原生 Python 和 Shell 脚本通过操作系统级沙箱。

## 核心组件

### 1. 技能注册表（`registry.rs`）

负责：
- 扫描目录查找技能
- 加载技能元数据（第 1 层）
- 缓存技能描述符
- 验证技能结构

**关键类型：**
- `SkillRegistry`：主注册表接口
- `Skill`：内部技能表示
- `SkillDescriptor`：公共技能元数据

### 2. 清单解析器（`manifest.rs`、`skill_parser.rs`）

负责：
- 从 SKILL.md 解析 YAML 前置元数据
- 验证必需和可选字段
- 提取 Markdown 指令
- 强制执行格式约束

**关键类型：**
- `SkillManifest`：解析的清单数据
- `ExecutionKind`：执行类型（Wasm/Http/Local）
- `Permissions`：权限配置

### 3. WASM 运行器（`wasm_runner.rs`）- 实验性

**状态**：实验性功能，不是主要的执行方法。

负责：
- 通过 Wasmtime 加载 WASM 模块
- 配置 WASI 能力
- 强制执行权限
- 捕获 stdout/stderr
- 超时强制执行

**关键特性：**
- WASI 0.3 (WASIp3) 仅组件模型执行
- 基于能力的文件系统访问
- 网络白名单强制执行
- 确定性执行支持

**用例**：确定性逻辑、策略执行、编排。不适合完整的 Python 生态系统或原生库。

### 4. 原生运行器（`native_runner.rs`）- 主要方式

**状态**：主要执行方法，生产就绪。

负责：
- 通过操作系统级沙箱执行原生 Python 和 shell 脚本（macOS seatbelt + Linux Landlock）
- 从权限构建沙箱配置文件
- 捕获 stdout/stderr
- 超时强制执行

**用例**：完整 Python 生态系统、原生库、ML/量化代码、传统技能。这是大多数技能的推荐方法。

### 5. 权限系统（`permissions.rs`）

负责：
- 将 `allowed-tools` 映射到 WASI 能力
- 强制执行文件系统权限
- 网络访问控制
- 环境变量过滤

**能力映射：**
- `Read` → 文件系统读取访问
- `Write` → 文件系统写入访问
- `Bash` → 完整文件系统访问
- `WebSearch` → 网络访问

### 6. 审计系统（`audit.rs`）

负责：
- 记录执行跟踪
- 哈希输入/输出
- 跟踪资源使用
- 生成审计记录

**审计记录字段：**
- 技能 ID 和版本
- 输入/输出哈希
- 执行时间
- 使用的权限
- 退出状态

## 数据流

### 技能发现

```
1. Registry.scan()
   └─> 读取目录条目
   └─> 对于每个目录：
       ├─> 检查 SKILL.md
       ├─> 解析 YAML 前置元数据（第 1 层）
       ├─> 验证格式
       └─> 缓存 SkillDescriptor

2. Runtime.load_skills()
   └─> 调用 Registry.scan()
   └─> 返回 Vec<SkillDescriptor>
```

### 技能执行

```
1. Runtime.execute_skill()
   ├─> 如果未缓存则加载完整技能（第 2 层）
   ├─> 根据模式验证输入
   ├─> 创建 PermissionEnforcer
   └─> 自动检测执行模式

2. Executor.execute()
   ├─> 对于原生（macOS，主要方式）：
   │   ├─> 构建 seatbelt 配置文件
   │   ├─> 执行 Python/shell 脚本
   │   ├─> 强制执行文件系统/网络权限
   │   └─> 捕获输出
   │
   └─> 对于 WASM（实验性）：
       ├─> 加载 WASM 模块
       ├─> 配置 WASI 上下文
       ├─> 设置权限
       ├─> 执行并超时
       └─> 捕获输出

3. 执行后
   ├─> 根据模式验证输出
   ├─> 创建审计记录
   └─> 返回 ExecutionResult
```

## 渐进式披露

运行时实现三层加载：

1. **第 1 层（元数据）**：启动时加载
   - 仅来自 YAML 前置元数据的 `name` 和 `description`
   - 最小令牌成本
   - 快速发现

2. **第 2 层（指令）**：激活时加载
   - 完整的 SKILL.md 内容（Markdown 主体）
   - 在选择/激活技能时加载
   - 中等令牌成本

3. **第 3 层（资源）**：按需加载
   - 脚本、WASM 模块、数据文件
   - 仅在需要时加载
   - 零令牌成本，直到输出进入上下文

## 安全模型

### 原生 Seatbelt 沙箱（macOS）- 主要方式

**状态**：生产就绪，主要执行方法。

- **隔离**：脚本执行受 seatbelt 配置文件限制
- **文件系统**：来自 `allowed-tools` 的子路径读/写白名单
- **网络**：仅在启用 `WebSearch`/`Fetch` 时允许
- **超时**：用于超时强制执行的 epoch 中断
- **内存**：可配置的内存限制

### WASM 沙箱 - 实验性

**状态**：实验性功能，不是主要的执行方法。

- **隔离**：每次执行在隔离的 WASM 实例中运行
- **能力**：通过 WASI 预开放进行文件系统访问
- **网络**：域白名单强制执行

**限制**：无法访问完整的 Python 生态系统、原生库或操作系统原生行为。最适合确定性逻辑和策略执行。

### 权限强制执行

权限在多个级别强制执行：

1. **清单级别**：`allowed-tools` 定义能力
2. **运行时级别**：PermissionEnforcer 验证操作
3. **WASI 级别**：WASI 上下文配置为最小能力

## 错误处理

错误类型在 `errors.rs` 中定义：

- `SkillNotFound`：技能 ID 不在注册表中
- `InvalidManifest`：SKILL.md 解析/验证失败
- `PermissionDenied`：操作不被允许
- `Timeout`：执行超过时间限制
- `ExecutionFailure`：WASM 执行错误

所有错误实现 `std::error::Error` 并可序列化。

## 性能考虑

### 缓存

- 首次加载后缓存技能元数据
- WASM 模块可以缓存（未来优化）
- 注册表使用 HashMap 进行 O(1) 查找

### 延迟加载

- 仅在需要时加载指令
- 按需加载资源
- 减少初始内存占用

### 并发执行

- 运行时不是线程安全的（如果需要使用 Mutex）
- WASM 执行是单线程的
- 未来：考虑 I/O 的异步执行

## 扩展点

### 自定义执行器

实现 `Executor` trait 用于自定义执行模式：

```rust
pub trait Executor {
    fn execute(
        &self,
        skill: &Skill,
        input: Value,
        ctx: &ExecutionContext
    ) -> Result<ExecutionArtifacts, OpenSkillError>;
}
```

### 自定义审计接收器

实现 `AuditSink` trait 用于自定义审计日志：

```rust
pub trait AuditSink: Send + Sync {
    fn record(&self, audit: &AuditRecord);
}
```

### 语言绑定

绑定遵循通用模式：

1. 在特定于语言的类型中包装 `OpenSkillRuntime`
2. 在语言类型和 Rust 类型之间转换
3. 适当处理错误
4. 为目标语言提供惯用 API

## WASI 执行模型（实验性）

**状态**：实验性功能。原生脚本是主要的执行方法。

OpenSkills 在存在 WASM 模块时通过 Wasmtime 的组件模型**仅执行 WASI 0.3 (WASIp3) 组件**。

- 传统的"核心模块"WASM 工件被**拒绝**。
- WASM 执行是可选的 - 大多数技能使用原生 Python/shell 脚本。

## 未来改进

- **异步执行**：支持 I/O 操作的 async/await
- **WASM 模块缓存**：缓存编译的 WASM 模块
- **组件模型**：完整的 WASI 组件模型支持
- **分布式执行**：支持远程技能执行
- **技能依赖**：支持技能到技能的依赖

## 相关文档

- [规范](spec.zh.md) - 运行时规范
- [开发者指南](developers.zh.md) - 使用运行时
- [贡献指南](contributing.zh.md) - 贡献代码
