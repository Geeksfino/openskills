# 技能类型处理完整指南

本文档整合了技能类型检测、缓存优化和工具函数优化的完整方案。

## 目录

1. [问题背景](#问题背景)
2. [技能类型定义](#技能类型定义)
3. [技能类型检测](#技能类型检测)
4. [性能优化：类型缓存](#性能优化类型缓存)
5. [工具函数优化](#工具函数优化)
6. [完整工作流程](#完整工作流程)
7. [总结](#总结)

---

## 问题背景

OpenSkills 支持两种类型的技能：

- **指令型技能（Instruction-Only）**：只有 `SKILL.md` 文件，没有可执行文件
- **可执行型技能（Executable）**：包含可执行文件（WASM、Python、Shell 脚本）

之前的实现存在以下问题：

1. ❌ **执行错误**：尝试执行指令型技能时会报错 `No executable artifact found`
2. ❌ **性能问题**：每次调用都要检测技能类型，重复访问文件系统
3. ❌ **逻辑混乱**：通过捕获错误来判断技能类型，不够优雅

## 技能类型定义

### 指令型技能（Instruction-Only）

- **特征**：只有 `SKILL.md` 文件，没有可执行文件（.wasm, .py, .sh）
- **用途**：AI 模型需要遵循技能中的指令来完成任务
- **示例**：`explaining-code`、`code-review`（纯指令）

```
explaining-code/
└── SKILL.md  ← 只有这个文件
```

### 可执行型技能（Executable）

- **特征**：有可执行文件（WASM、Python、Shell 脚本）
- **用途**：可以执行脚本或 WASM 模块来处理任务
- **示例**：`skill-creator`（有 `wasm/skill.wasm`）

```
skill-creator/
├── SKILL.md
├── src/
│   └── index.ts
└── wasm/
    └── skill.wasm  ← 找到这个，是可执行型
```

---

## 技能类型检测

### 检测函数

我们使用 `detectSkillType()` 函数来检测技能类型：

```typescript
function detectSkillType(skillId: string, skillsDir: string): "instruction-only" | "executable"
```

### 检测逻辑

#### 1. 检查 WASM 文件

按以下优先级查找：
- `skill.wasm`
- `wasm/skill.wasm`
- `module.wasm`
- `main.wasm`
- 任何 `.wasm` 文件（递归搜索）

如果找到任一 WASM 文件 → **可执行型**

#### 2. 检查原生脚本文件

按以下优先级查找：
- `script.py`, `main.py`, `index.py`
- `src/main.py`, `src/index.py`
- `script.sh`, `main.sh`, `index.sh`
- `src/main.sh`, `src/index.sh`
- `script.bash`, `main.bash`, `index.bash`
- `src/main.bash`, `src/index.bash`
- 在 `scripts/` 或 `src/` 目录中的任何 `.py`, `.sh`, `.bash` 文件

如果找到任一脚本文件 → **可执行型**

#### 3. 默认判断

如果以上都没找到 → **指令型**

### 检测流程图

```
开始检测技能类型
    ↓
检查 WASM 文件？
    ├─ 找到 → 返回 "executable"
    └─ 未找到 ↓
        检查脚本文件（.py/.sh/.bash）？
            ├─ 找到 → 返回 "executable"
            └─ 未找到 ↓
                返回 "instruction-only"
```

### 与 Rust 运行时的一致性

这个检测逻辑与 Rust 运行时的 `detect_execution_mode()` 函数保持一致：

```rust
fn detect_execution_mode(skill_root: &PathBuf) -> Result<ExecutionMode> {
    if find_wasm_module(skill_root).is_some() {
        return Ok(ExecutionMode::Wasm);
    }
    if find_native_script(skill_root).is_some() {
        return Ok(ExecutionMode::Native);
    }
    Err("No executable artifact found")
}
```

确保 JavaScript/TypeScript 绑定与 Rust 核心的行为一致。

---

## 性能优化：类型缓存

### 优化前后对比

#### 优化前（每次调用都检测）

```typescript
func: async ({ skill_id, input }) => {
  // ❌ 每次调用都要检测文件系统
  const skillType = detectSkillType(skill_id, skillsDir);
  // ...
}
```

**问题**：
- 每次调用都要访问文件系统
- 重复检测相同技能
- 性能开销大

#### 优化后（注册时检测并缓存）

```typescript
// ✅ 在注册时一次性检测所有技能
runtime.discoverSkills();
const skills = runtime.listSkills();
const skillTypeCache = new Map<string, "instruction-only" | "executable">();

for (const skill of skills) {
  const skillType = detectSkillType(skill.id, skillsDir);
  skillTypeCache.set(skill.id, skillType);
}

// 调用时直接从缓存获取
func: async ({ skill_id, input }) => {
  const skillType = skillTypeCache.get(skill_id);  // ✅ 从缓存获取
  // ...
}
```

**优点**：
- ✅ **只检测一次**：注册时检测，后续直接使用
- ✅ **性能更好**：避免重复的文件系统访问
- ✅ **逻辑清晰**：明确知道哪些技能是指令型，哪些是可执行型
- ✅ **可扩展**：可以在技能列表中显示类型信息

### 实现细节

#### 1. 基础示例（index-alibaba.ts）

```typescript
// 在技能发现后立即检测并缓存
runtime.discoverSkills();
const skills = runtime.listSkills();
const skillTypeCache = new Map<string, "instruction-only" | "executable">();

for (const skill of skills) {
  const skillType = detectSkillType(skill.id, skillsDir);
  skillTypeCache.set(skill.id, skillType);
}

// 在工具函数中使用缓存
func: async ({ skill_id, input }) => {
  const skillType = skillTypeCache.get(skill_id) || detectSkillType(skill_id, skillsDir);
  // ...
}
```

#### 2. 高级示例（openskills-tool.ts）

```typescript
// 使用模块级缓存，支持多个技能目录
const skillTypeCache = new Map<string, Map<string, SkillType>>();

function getOrCreateSkillTypeCache(skillsDir: string): Map<string, SkillType> {
  if (!skillTypeCache.has(skillsDir)) {
    skillTypeCache.set(skillsDir, new Map());
  }
  return skillTypeCache.get(skillsDir)!;
}

export function createOpenSkillsTools(options: ToolOptions) {
  // 在创建工具时检测并缓存
  const typeCache = getOrCreateSkillTypeCache(options.skillsDir);
  for (const skill of skills) {
    if (!typeCache.has(skill.id)) {
      const skillType = detectSkillType(skill.id, options.skillsDir);
      typeCache.set(skill.id, skillType);
    }
  }
  // ...
}
```

### 性能对比

#### 优化前
- 注册时：0 次文件系统检测
- 每次调用：1 次文件系统检测
- 10 次调用：10 次文件系统检测

#### 优化后
- 注册时：N 次文件系统检测（N = 技能数量）
- 每次调用：0 次文件系统检测（从缓存获取）
- 10 次调用：0 次额外的文件系统检测

**总开销**：
- 优化前：0 + (1 × 调用次数) = 调用次数次
- 优化后：N + 0 = N 次（仅注册时）

**示例**：如果技能数量为 4，调用 10 次：
- 优化前：10 次检测
- 优化后：4 次检测（减少了 60%）

### 额外收益：技能列表显示类型

现在技能列表中也会显示类型信息：

```
Available skills:
- explaining-code [Instruction]: Explains code clearly...
- code-review [Instruction]: Reviews code for quality...
- skill-creator [Executable]: Guide for creating effective skills...
```

这样 Agent 在调用前就能知道技能类型，有助于：
- 更好的决策：知道哪些技能需要执行，哪些需要遵循指令
- 更清晰的提示：工具描述中已经说明了类型

---

## 工具函数优化

### 问题描述

之前的代码在处理**指令型技能**时会报错：

```
Error: native execution failed: No executable artifact found (expected .wasm, .py, or .sh)
```

这是因为 `runtime.executeSkill()` 只能执行有可执行文件的技能（WASM、Python、Shell 脚本）。

### 优化方案

优化了 `runSkillTool` 和 `openskills-tool.ts` 中的工具函数，添加了基于类型检测的处理逻辑：

1. **检测技能类型**（从缓存获取或实时检测）
2. **根据类型选择处理方式**：
   - 可执行型：执行技能并返回结果
   - 指令型：激活技能并返回格式化指令

### 代码逻辑

```typescript
func: async ({ skill_id, input }) => {
  // 1. 从缓存获取技能类型（或实时检测）
  const skillType = skillTypeCache.get(skill_id) || detectSkillType(skill_id, skillsDir);
  
  if (skillType === "instruction-only") {
    // 2. 对于指令型技能，激活并返回技能说明
    try {
      const loadedSkill = runtime.activateSkill(skill_id);
      return `[Instruction-Only Skill: ${skill_id}]\n\n` +
             `Description: ${loadedSkill.description}\n\n` +
             `Instructions:\n${loadedSkill.instructions}\n\n` +
             `User Request: ${input}\n\n` +
             `Please follow the above instructions to help the user.`;
    } catch (activateError: any) {
      return `Error: Failed to activate skill "${skill_id}": ${activateError.message}`;
    }
  } else {
    // 3. 对于可执行型技能，执行并返回结果
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
}
```

### 返回格式

#### 可执行型技能

返回技能执行的结果（JSON 字符串）

#### 指令型技能

返回格式化的文本，包含：
- 技能 ID 和类型标识
- 技能描述
- 完整的技能指令（从 SKILL.md 的 Markdown body 获取）
- 用户请求
- 提示 Agent 遵循指令

**示例返回**：
```
[Instruction-Only Skill: explaining-code]

Description: Explains code clearly and thoroughly...

Instructions:
# Explaining Code

When explaining code, you should:
1. Break down the code into logical sections
2. Explain what each part does
...

User Request: Explain this TypeScript function

Please follow the above instructions to help the user.
```

---

## 完整工作流程

### 初始化阶段

```
1. 创建 runtime
   ↓
2. 发现技能：runtime.discoverSkills()
   ↓
3. 获取技能列表：runtime.listSkills()
   ↓
4. 检测并缓存所有技能类型
   for (skill of skills) {
     skillTypeCache.set(skill.id, detectSkillType(skill.id))
   }
   ↓
5. 创建 Agent 和工具
```

### 执行阶段：可执行型技能

```
Agent 调用 run_skill(skill_id, input)
    ↓
从缓存获取技能类型 → "executable"
    ↓
runtime.executeSkill() 执行技能
    ↓
返回执行结果（JSON）
```

### 执行阶段：指令型技能

```
Agent 调用 run_skill(skill_id, input)
    ↓
从缓存获取技能类型 → "instruction-only"
    ↓
runtime.activateSkill() 激活技能
    ↓
返回格式化的指令内容
    ↓
Agent 收到指令，遵循指令帮助用户
```

---

## 优势总结

### 相比之前的实现

1. ✅ **性能优化**：
   - 注册时一次性检测，避免重复文件系统访问
   - 调用时直接从缓存获取，零开销

2. ✅ **逻辑清晰**：
   - 明确区分技能类型，不再依赖错误捕获
   - 代码更易理解和维护

3. ✅ **错误处理完善**：
   - 区分真正的执行错误和类型错误
   - 提供更精确的错误信息

4. ✅ **用户体验好**：
   - Agent 能正确理解和使用指令型技能
   - 技能列表中显示类型信息，便于决策

5. ✅ **向后兼容**：
   - 可执行型技能正常工作
   - 不影响现有功能

### 性能提升

- **文件系统访问**：从每次调用 1 次减少到注册时 N 次（N = 技能数量）
- **响应时间**：调用时无需等待文件系统检测
- **可扩展性**：支持大量技能和频繁调用

---

## 进一步的优化建议

如果想在 Rust 核心层实现，可以：

1. **修改 `SkillDescriptor` 结构**：
   ```rust
   pub struct SkillDescriptor {
       pub id: String,
       pub description: String,
       pub location: SkillLocation,
       pub user_invocable: bool,
       pub skill_type: Option<SkillType>,  // 新增字段
   }
   ```

2. **在 `registry.rs` 的 `list()` 方法中检测**：
   ```rust
   pub fn list(&self) -> Vec<SkillDescriptor> {
       self.skills.values().map(|s| {
           let skill_type = detect_execution_mode(&s.root, None)
               .ok()
               .map(|mode| match mode {
                   ExecutionMode::Wasm { .. } | ExecutionMode::Native { .. } => SkillType::Executable,
               });
           SkillDescriptor {
               // ...
               skill_type,
           }
       }).collect()
   }
   ```

但目前的 JavaScript/TypeScript 层缓存方案已经足够好，无需修改 Rust 核心代码。

---

## 测试

运行示例验证优化：

```bash
npm run start:alibaba
# 或
npm run web
```

现在调用 `explaining-code` 或其他指令型技能应该：
- ✅ 不再报错
- ✅ 正确返回技能指令让 Agent 遵循
- ✅ 性能更好（使用缓存）

---

## 总结

✅ **优化完成**：
- 实现了技能类型检测功能
- 在技能注册时检测并缓存类型
- 调用时直接从缓存获取，避免重复检测
- 根据类型选择正确的处理方式
- 性能显著提升，逻辑更清晰

✅ **额外收益**：
- 技能列表中显示类型信息
- 更清晰的技能描述
- 便于 Agent 做更好的决策
- 代码更易维护和扩展
