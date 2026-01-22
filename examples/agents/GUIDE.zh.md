## OpenSkills 代理集成指南

本指南展示了如何将 OpenSkills 运行时与流行的代理框架集成，以及如何将技能作为工具暴露给您的代理。

### 概述
OpenSkills：
- 从包含 `SKILL.md` 的目录发现技能
- 支持渐进式信息披露（元数据 → 说明 → 资源）
- 在双沙箱方案中执行技能（WASM，以及 macOS 上可用的原生执行）
- 适用于 TypeScript 和 Python 绑定

### 集成模式

#### 模式 A：单一工具（简单）
暴露一个工具，可以按 ID 执行任何技能。这是 `langchain-python` 和 `simple` 中最小示例使用的模式。

**优点：** 代码更少，接线更快  
**缺点：** 代理必须在工具调用中提供 `skill_id`

#### 模式 B：每个技能一个工具（推荐）
为每个技能暴露一个具有清晰描述的工具。这改进了代理推理和工具选择。LangChainJS 高级示例使用这种方法。

**优点：** 更好的工具选择和提示  
**缺点：** 稍多的设置代码

#### 模式 C：提示注入（推荐）
将技能元数据注入系统提示，以便代理能决定何时使用技能。

### 常见集成步骤

#### 1) 初始化运行时
**TypeScript**
```typescript
import { OpenSkillRuntime } from "@finogeek/openskills";

const runtime = OpenSkillRuntime.fromDirectory("./examples/skills");
runtime.discoverSkills();
```

**Python**
```python
from openskills import OpenSkillRuntime

runtime = OpenSkillRuntime.from_directory("./examples/skills")
runtime.discover_skills()
```

#### 2) 列出可用技能
**TypeScript**
```typescript
const skills = runtime.listSkills();
skills.forEach((skill) => {
  console.log(`${skill.id}: ${skill.description}`);
});
```

**Python**
```python
skills = runtime.list_skills()
for skill in skills:
    print(f"{skill['id']}: {skill['description']}")
```

#### 3) 执行技能
**TypeScript**
```typescript
const result = runtime.executeSkill("example-skill", {
  input: JSON.stringify({ query: "hello" }),
  timeout_ms: 5000,
});
console.log(result.outputJson);
```

**Python**
```python
result = runtime.execute_skill(
    "example-skill",
    input={"query": "hello"},
    timeout_ms=5000,
)
print(result.get("output", ""))
```

### LangChainJS 高级模式（每个技能一个工具）
高级示例为每个技能构建一个工具，并将技能元数据注入系统提示：

- 工具创建助手：`langchain-js/src/openskills-tool.ts`
- 代理示例：`langchain-js/src/advanced-agent.ts`

### 最佳实践
- 在启动时发现技能一次
- 在提示中使用技能元数据以改进选择
- 对于生产代理，倾向于使用每个技能的工具
- 在 `examples/skills` 中保存技能，构建后的工件放在 `wasm/` 中

### 故障排除
- **"技能未找到"**：检查 `examples/skills/<skill>/SKILL.md`
- **"WASM 模块未找到"**：在技能文件夹中运行 `openskills build`
- **缺少 API 密钥**：设置 `OPENAI_API_KEY` 或 `ANTHROPIC_API_KEY`

### 资源
- 每个框架文件夹中的 `README.md` 获取设置详情
- `docs/developers.md` 用于构建技能
