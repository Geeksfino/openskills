## OpenSkills 代理集成 - 快速开始

在5分钟内开始在您的代理框架中使用 OpenSkills。

### 选择您的路径

#### Python 开发者 (LangChain)
```bash
cd langchain-python
pip install -r requirements.txt
python main.py
```

#### TypeScript 开发者（简单示例 - AI SDK）
```bash
cd simple
npm install
npm run start
```

#### TypeScript 开发者（现有项目 - LangChainJS）
```bash
cd langchain-js
npm install
npm run start
```

#### 高级示例 (LangChainJS)
```bash
cd langchain-js
npm run advanced
```

### 您将获得什么
- 每个框架的最小代理，暴露一个 `run_skill` 工具
- 一个高级示例（LangChainJS）使用每个技能一个工具
- 从 `examples/skills` 加载的技能

### 基本集成 (TypeScript)
```typescript
import { OpenSkillRuntime } from "@finogeek/openskills";

const runtime = OpenSkillRuntime.fromDirectory("./examples/skills");
runtime.discoverSkills();

const skills = runtime.listSkills();
console.log(skills.map((s) => s.id));

const result = runtime.executeSkill("example-skill", {
  input: JSON.stringify({ query: "hello" }),
  timeout_ms: 5000,
});
console.log(result.outputJson);
```

### 基本集成 (Python)
```python
from openskills import OpenSkillRuntime

runtime = OpenSkillRuntime.from_directory("./examples/skills")
runtime.discover_skills()

skills = runtime.list_skills()
print([s["id"] for s in skills])

result = runtime.execute_skill(
    "example-skill",
    input={"query": "hello"},
    timeout_ms=5000,
)
print(result.get("output", ""))
```

### 下一步
- 阅读集成指南：`GUIDE.md`
- 浏览示例：`langchain-js/`、`langchain-python/`、`simple/`
- 在 `examples/skills` 下构建您自己的技能
