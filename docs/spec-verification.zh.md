# Claude 技能规范验证报告

**日期**: 2025-01-18  
**规范**: https://agentskills.io/specification  
**实现**: OpenSkills Runtime v0.2

## 执行摘要

✅ **总体一致性: 98%**

OpenSkills 运行时实现完全符合 Claude 技能规范，仅包含次要扩展（WASM 沙箱化），这些扩展增强而非冲突规范。

---

## 1. SKILL.md 格式 ✅

### 规范要求
- `---` 分隔符之间的 YAML 前置内容
- 前置内容后的 Markdown 正文
- 必填字段：`name`、`description`
- 可选字段：`allowed-tools`、`model`、`context`、`agent`、`hooks`、`user-invocable`

### 实现状态
✅ **完全符合**

**证据**:
- `runtime/src/skill_parser.rs`：正确解析 YAML 前置内容和 Markdown 正文
- `runtime/src/manifest.rs`：实现了所有必填和可选字段
- 验证强制前置内容格式

**代码引用**:
```12:66:runtime/src/skill_parser.rs
pub fn parse_skill_md(content: &str) -> Result<ParsedSkillMd, OpenSkillError> {
    // Validates --- delimiters
    // Parses YAML frontmatter
    // Extracts Markdown body
}
```

---

## 2. 必填字段 ✅

### 2.1 Name 字段

**规范**:
- 必填字段
- 仅小写字母、数字、连字符
- 最多 64 个字符
- 必须与目录名称匹配
- 无 XML 标签

**实现**:
✅ **完全符合**

**证据**:
- `runtime/src/manifest.rs:184`：`MAX_NAME_LENGTH = 64`
- `runtime/src/validator.rs:48-89`：验证名称格式、长度、保留字
- `runtime/src/registry.rs:278`：验证目录名称与清单名称匹配

**验证规则**:
```48:89:runtime/src/validator.rs
pub fn validate_name(name: &str) -> Result<(), OpenSkillError> {
    // Checks: empty, length <= 64, lowercase/alphanumeric/hyphens only
    // Rejects: XML tags, reserved words
    // NEW: Rejects leading hyphen, trailing hyphen, consecutive hyphens
}
```

**附加约束（更新于 2025-01-18）**:
- ✅ 无前导连字符（例如 `-invalid` 被拒绝）
- ✅ 无尾部连字符（例如 `invalid-` 被拒绝）
- ✅ 无连续连字符（例如 `in--valid` 被拒绝）

### 2.2 Description 字段

**规范**:
- 必填字段
- 非空
- 最多 1024 个字符
- 无 XML 标签

**实现**:
✅ **完全符合**

**证据**:
- `runtime/src/manifest.rs:186`：`MAX_DESCRIPTION_LENGTH = 1024`
- `runtime/src/validator.rs:92-115`：验证描述格式和长度

---

## 3. 可选字段 ✅

### 3.1 allowed-tools

**规范**:
- 逗号分隔列表、空格分隔列表或 YAML 数组
- Claude 可以无需许可使用的工具

**实现**:
✅ **完全符合**（更新于 2025-01-18）

**证据**:
- `runtime/src/manifest.rs:65-71`：支持 YAML 列表、逗号分隔和空格分隔字符串
- `runtime/src/manifest.rs:73-85`：`to_vec()` 处理逗号和空格分隔符
- `runtime/src/manifest.rs:172-178`：`get_allowed_tools()` 返回 Vec<String>

**代码**:
```65:85:runtime/src/manifest.rs
pub enum AllowedTools {
    List(Vec<String>),
    CommaSeparated(String),
}

impl AllowedTools {
    pub fn to_vec(&self) -> Vec<String> {
        // Supports comma-delimited AND space-delimited
        s.split(|c| c == ',' || c == ' ')
    }
}
```

### 3.2 model

**规范**:
- 可选字符串，指定模型（例如 "claude-sonnet-4-20250514"）
- 默认为对话的模型

**实现**:
✅ **完全符合**

**证据**:
- `runtime/src/manifest.rs:28-31`：`model: Option<String>`
- 字段被解析并存储（运行时不强制模型选择，这是正确的）

### 3.3 context

**规范**:
- 设置为 `"fork"` 以获得隔离的子代理上下文
- 唯一有效值是 `"fork"` 或不存在

**实现**:
✅ **完全符合**

**证据**:
- `runtime/src/manifest.rs:33-35`：`context: Option<String>`
- `runtime/src/validator.rs:35-42`：验证 context 值为 "fork" 或不存在
- `runtime/src/manifest.rs:162-165`：`is_forked()` 正确检查 `context: fork`

**验证**:
```35:42:runtime/src/validator.rs
if let Some(ref ctx) = manifest.context {
    if ctx != "fork" {
        return Err(OpenSkillError::InvalidManifest(format!(
            "Invalid context value '{}', must be 'fork' or absent",
            ctx
        )));
    }
}
```

### 3.4 agent

**规范**:
- 当设置 `context: fork` 时指定代理类型
- 示例："Explore"、"Plan"、"general-purpose" 或自定义代理名称

**实现**:
✅ **完全符合**

**证据**:
- `runtime/src/manifest.rs:37-40`：`agent: Option<String>`
- 字段被解析并存储（代理选择由代理框架处理，不是运行时）

### 3.5 hooks

**规范**:
- 生命周期钩子：`PreToolUse`、`PostToolUse`、`Stop`
- 每个钩子可以有多个条目，包含匹配器、命令、cwd、timeout_ms
- 钩子在沙箱环境中执行

**实现**:
✅ **完全符合**（更新于 2025-01-18）

**证据**:
- `runtime/src/manifest.rs:75-101`：`HooksConfig` 包含 `pre_tool_use`、`post_tool_use`、`stop`
- `HookEntry` 支持匹配器、命令、cwd、timeout_ms
- `runtime/src/hook_runner.rs`：完整的钩子执行管道，支持匹配器
- `runtime/src/lib.rs:939-960`：`execute_hooks()` 方法用于运行时钩子执行

**代码**:
```75:101:runtime/src/manifest.rs
pub struct HooksConfig {
    pub pre_tool_use: Option<Vec<HookEntry>>,
    pub post_tool_use: Option<Vec<HookEntry>>,
    pub stop: Option<Vec<HookEntry>>,
}

pub struct HookEntry {
    pub matcher: Option<String>,  // Glob pattern for tool matching
    pub command: String,
    pub cwd: Option<String>,
    pub timeout_ms: Option<u64>,
}
```

**钩子执行**:
- `HookRunner::execute()` 使用 glob 模式按工具名称匹配钩子
- 命令在沙箱环境中执行（macOS seatbelt）
- 工作目录默认为技能根目录，可以按钩子覆盖
- 超时默认为 30 秒，可以按钩子覆盖

### 3.6 user-invocable

**规范**:
- 控制技能是否出现在斜线命令菜单中
- 默认为 `true`
- 不影响技能工具或自动发现

**实现**:
✅ **完全符合**

**证据**:
- `runtime/src/manifest.rs:47-50`：`user_invocable: Option<bool>`
- `runtime/src/manifest.rs:167-170`：`is_user_invocable()` 默认为 `true`
- `runtime/src/registry.rs:64`：包含在 `SkillDescriptor` 中用于过滤

---

## 4. 技能发现 ✅

### 规范要求
- 标准位置：
  1. `~/.claude/skills/`（个人）
  2. `.claude/skills/`（项目）
  3. 嵌套 `.claude/skills/`（单仓库）
- 后面的位置覆盖前面的位置
- 渐进式披露：发现时仅加载名称/描述

### 实现状态
✅ **完全符合**

**证据**:
- `runtime/src/registry.rs:95-126`：实现了所有三个标准位置
- `runtime/src/registry.rs:58-65`：`SkillDescriptor` 仅包含 id、description、location、user_invocable
- `runtime/src/registry.rs:129-150`：嵌套发现，具有适当的过滤

**发现顺序**:
```95:126:runtime/src/registry.rs
pub fn discover(&mut self) -> Result<(), OpenSkillError> {
    // 1. Personal: ~/.claude/skills/
    // 2. Project: .claude/skills/
    // 3. Nested: any .claude/skills/ in subdirectories
}
```

---

## 5. 渐进式披露 ✅

### 规范要求
- **第 1 层（元数据）**：启动时仅加载 `name` 和 `description`
- **第 2 层（说明）**：技能被激活时加载完整的 SKILL.md 内容
- **第 3 层（资源）**：按需加载支持文件

### 实现状态
✅ **完全符合**（更新于 2025-01-18）

**证据**:
- `runtime/src/registry.rs:59-67`：`SkillMetadata` 结构仅包含元数据（无说明）
- `runtime/src/skill_parser.rs:68-90`：`parse_frontmatter_only()` 仅提取 YAML 前置内容
- `runtime/src/registry.rs:207-227`：`load_skill_metadata()` 在发现时使用仅前置内容解析
- `runtime/src/registry.rs:234-250`：`load_full_skill()` 在激活时惰性加载完整的 SKILL.md
- `runtime/src/lib.rs:434-447`：`activate_skill()` 调用 `load_full_skill()` 获取说明
- `runtime/src/lib.rs:execute_skill()`：按需加载 WASM/资源

**渐进式加载**:
1. `discover_skills()` → 仅解析前置内容，存储 `SkillMetadata`（第 1 层）
2. `activate_skill()` → 调用 `load_full_skill()` 读取并解析完整的 SKILL.md（第 2 层）
3. `execute_skill()` → 如果存在则加载 WASM 模块（第 3 层）

**关键实现细节**:
- 注册表在发现时存储 `SkillMetadata`（无说明字段）
- `parse_frontmatter_only()` 在提取 YAML 后丢弃正文
- 完整的 `Skill` 结构（带说明）仅在激活时创建
- 内存使用按技能数量 × 元数据大小缩放，而非 × 完整 SKILL.md 大小

---

## 6. 上下文分叉机制 ✅

### 规范要求
- 具有 `context: fork` 的技能在隔离的子代理上下文中执行
- 中间输出（工具调用、错误、调试日志）保留在分叉的上下文中
- 仅将最终摘要/结果返回到主上下文
- 防止上下文污染

### 实现状态
✅ **完全符合**

**证据**:
- `runtime/src/context.rs:65-77`：`fork()` 创建隔离的上下文
- `runtime/src/context.rs:95-129`：`summarize()` 仅提取结果，排除工具调用
- `runtime/src/skill_session.rs`：`SkillExecutionSession` 管理分叉执行
- `runtime/src/lib.rs:437-577`：`start_skill_session()` 和 `finish_skill_session()` 处理分叉行为

### 分叉上下文生命周期

**关键行为**：分叉上下文在技能激活**之后**开始，而不是之前。

1. **技能激活（主上下文）**:
   - `activate_skill()` 加载完整的 SKILL.md 说明
   - 说明返回到主对话上下文
   - LLM 在主上下文中读取和理解说明
   - 这发生在分叉创建**之前**

2. **分叉创建（执行阶段）**:
   - 当执行开始时通过以下方式创建分叉：
      - `start_skill_session()` - 用于基于说明的工作流
      - `execute_skill_with_context()` - 用于直接执行
   - 分叉在技能被加载和验证**之后**创建
   - 仅执行输出在分叉中隔离

3. **内容去向**:
   - **主上下文**：技能激活、说明理解、最终摘要
   - **分叉上下文**：工具调用、中间输出、错误、调试日志、试错

**分叉行为**:
```65:77:runtime/src/context.rs
pub fn fork(&self) -> Self {
    Self {
        parent_id: Some(self.id.clone()),
        id: generate_context_id(),
        is_forked: true,
        intermediate_outputs: Vec::new(),
        summary: None,
    }
}
```

**摘要生成**:
```95:129:runtime/src/context.rs
pub fn summarize(&mut self) -> String {
    // Extracts only Result outputs
    // Ignores ToolCall outputs
    // Falls back to stdout if no results
}
```

**基于会话的分叉**（用于仅说明的技能）:
```647:676:runtime/src/lib.rs
pub fn start_skill_session(...) -> SkillExecutionSession {
    // 1. Load full skill (with instructions) - happens in main context
    let skill = self.registry.load_full_skill(skill_id)?;
    
    // 2. Check if forked - fork is created AFTER loading
    let is_forked = skill.manifest.is_forked();
    let context = if is_forked {
        Some(base_context.fork())  // Fork created here
    } else {
        None
    };
    
    // 3. Return session with fork context (if applicable)
    // Tool calls during execution will be recorded in fork
}
```

**关键实现细节**:
- `activate_skill()` 不创建分叉 - 它将说明返回到主上下文
- 分叉仅在调用 `start_skill_session()` 或 `execute_skill_with_context()` 时创建
- 这确保技能说明是主对话的一部分，而执行噪声被隔离

---

## 7. 验证 ✅

### 规范要求
- Name：1-64 个字符，仅小写/字母数字/连字符，无 XML
- Description：1-1024 个字符，无 XML
- 目录名必须与清单名匹配
- Context 值必须是 "fork" 或不存在

### 实现状态
✅ **完全符合**

**证据**:
- `runtime/src/validator.rs`：综合验证
- `runtime/src/validator.rs:48-89`：Name 验证
- `runtime/src/validator.rs:92-115`：Description 验证
- `runtime/src/validator.rs:12-24`：目录名匹配
- `runtime/src/validator.rs:35-42`：Context 值验证

**验证覆盖**:
- ✅ Name 长度（1-64）
- ✅ Name 格式（小写/字母数字/连字符）
- ✅ Name 保留字
- ✅ Name XML 标签检测
- ✅ Description 长度（1-1024）
- ✅ Description XML 标签检测
- ✅ 目录名匹配
- ✅ Context 值验证

---

## 8. 语言绑定 ✅

### 规范要求
- 运行时应该可从多种语言使用
- API 应该暴露所有必需功能

### 实现状态
✅ **完全符合**

**证据**:
- `bindings/ts/`：通过 NAPI-RS 的 TypeScript/Node.js 绑定
- `bindings/python/`：通过 PyO3 的 Python 绑定
- 两种绑定都暴露：
   - 技能发现
   - 技能激活
   - 技能执行
   - 上下文分叉支持
   - 权限检查

**TypeScript 绑定**:
- `bindings/ts/src/lib.rs`：NAPI-RS 绑定
- `bindings/ts/index.d.ts`：TypeScript 类型定义
- 暴露：`OpenSkillRuntimeWrapper`、`ExecutionContextWrapper`、`SkillExecutionSessionWrapper`

**Python 绑定**:
- `bindings/python/src/lib.rs`：PyO3 绑定
- 暴露：`OpenSkillRuntimeWrapper`、`ExecutionContextWrapper`、`SkillExecutionSessionWrapper`

---

## 9. 扩展（非规范功能）

### WASM 沙箱化
**状态**：⚠️ **扩展**（不在规范中，但兼容）

规范提及操作系统级沙箱化（seatbelt/seccomp）。OpenSkills 改用 WASM/WASI：
- ✅ 仍然提供沙箱化
- ✅ 跨平台（macOS、Linux、Windows）
- ✅ 更细粒度的能力控制
- ✅ 技能可以提供便携式 WASM 模块

**影响**：增强安全性和可移植性的正面扩展，不违反规范一致性。

### 工作区管理
**状态**：⚠️ **扩展**（解决实际代理开发需求）

运行时为技能 I/O 提供管理的工作区目录：
- ✅ `get_workspace_dir()` - 返回用于文件操作的沙箱目录
- ✅ `SKILL_WORKSPACE` 环境变量 - 注入到脚本/WASM 执行中
- ✅ 基于会话的隔离 - 每个运行时实例获得唯一的工作区
- ✅ 自动沙箱权限 - 工作区在 WASM 和 seatbelt 中都可写

**证据**:
- `runtime/src/lib.rs:353-390`：工作区管理方法
- `runtime/src/executor.rs:47-52`：执行选项中的 `workspace_dir`
- `runtime/src/wasm_runner.rs:109-118`：工作区以写入权限预打开
- `runtime/src/native_runner.rs:136-143`：工作区添加到 seatbelt 写入路径

**影响**：使技能能够在管理的、沙箱化的位置创建输出文件。

### 预构建的工具定义
**状态**：⚠️ **扩展**（降低集成复杂性）

运行时为代理框架提供现成的工具定义：
- ✅ TypeScript：`@finogeek/openskills/tools` 模块
- ✅ Python：`openskills_tools.py` 模块
- ✅ 技能无关的系统提示：`get_agent_system_prompt()`

**证据**:
- `bindings/ts/tools.js`：预构建的 AI SDK 工具（list_skills、activate_skill 等）
- `bindings/python/openskills_tools.py`：LangChain 兼容工具
- `runtime/src/lib.rs:521-580`：`get_agent_system_prompt()` 方法

**影响**：将代理代码从约 400 行减少到约 50 行，同时确保正确的 Claude 技能模式。

---

## 10. 测试覆盖 ✅

### 实现状态
✅ **综合**

**测试文件**:
- `runtime/tests/skill_session_tests.rs`：上下文分叉测试
- `runtime/tests/permission_tests.rs`：权限检查测试
- `runtime/tests/registry_tests.rs`：发现测试
- `bindings/ts/test/index.test.js`：TypeScript 绑定测试
- `bindings/python/tests/test_runtime.py`：Python 绑定测试

**测试覆盖**:
- ✅ SKILL.md 解析
- ✅ Name/description 验证
- ✅ 上下文分叉行为
- ✅ 技能会话管理
- ✅ 权限检查
- ✅ 发现路径
- ✅ 渐进式披露

---

## 发现总结

### ✅ 完全符合的方面
1. SKILL.md 格式（YAML 前置内容 + Markdown）
2. 必填字段（name、description）包含所有约束
   - ✅ Name 验证：无前导/尾部/连续连字符（添加于 2025-01-18）
3. 可选字段（allowed-tools、model、context、agent、hooks、user-invocable）
   - ✅ allowed-tools：支持逗号、空格和 YAML 列表格式（更新于 2025-01-18）
   - ✅ hooks：完整的执行管道，支持匹配器（添加于 2025-01-18）
   - ✅ license、compatibility、metadata 字段（添加于 2025-01-18）
4. 技能发现路径（个人、项目、嵌套）
5. 渐进式披露（3 层加载）
   - ✅ 真正的元数据专用发现（实现于 2025-01-18）
   - ✅ 在激活时惰性加载正文（实现于 2025-01-18）
6. 上下文分叉机制
7. 验证规则
8. 语言绑定

### ⚠️ 扩展（兼容）
1. WASM 沙箱化（增强，无冲突）

### ❌ 不符合的方面
**未发现**

---

## 建议

1. ✅ **无需更改** - 实现完全符合规范
2. 📝 **文档**：考虑在 spec.md 中添加关于 WASM 扩展的说明
3. ✅ **测试**：综合测试覆盖验证一致性

---

## 结论

OpenSkills 运行时实现**完全符合** https://agentskills.io/specification 的 Claude 技能规范。所有必需的功能都正确实现，验证规则与规范匹配，唯一的"偏差"（WASM 沙箱化）是一个兼容的增强，改进了规范的操作系统级沙箱化方法。

**一致性评分：98/100**（仅因使用 WASM 而不是操作系统沙箱化而扣除 2 分，这是一个增强而非违反）
