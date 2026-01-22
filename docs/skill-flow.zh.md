# 技能执行流文档

本文档说明了在 OpenSkills 运行时中如何发现、匹配、激活和执行技能的完整端到端流程。提供了两个完整工作流程:

1. **代码审查技能 (WASM 执行)** - 演示 WASM 基础沙箱处理
2. **PDF 技能 (本地 Python 执行)** - 演示 macOS 上的本地 seatbelt 沙箱处理

## 工作流 1: 代码审查技能 (WASM 执行)

### 阶段 1: 发现 (第1层 - 仅元数据)

```
用户查询: "Can you review my code?"
     │
     ▼
Agent 调用: runtime.discover_skills()
     │
     ├─► 扫描目录
     │   ├─ ~/.claude/skills/
     │   ├─ .claude/skills/
     │   └─ examples/skills/ (自定义目录)
     │
     ├─► 对每个找到的目录:
     │   ├─ 遍历子目录
     │   ├─ 检查 SKILL.md 文件
     │   └─ 如果找到: 仅加载第1层
     │
     ▼
文件 I/O: 读取 examples/skills/code-review/SKILL.md
     │
     ├─► 解析 YAML 前置事项 (第1-7行):
     │   ├─ name: "code-review"
     │   ├─ description: "Reviews code for quality..."
     │   ├─ allowed-tools: ["Read", "Grep", "Glob", "LS"]
     │   ├─ context: "fork"
     │   └─ agent: "Explore"
     │
     ├─► 跳过 Markdown 正文 (第2层 - 暂不加载)
     │
     └─► 创建 SkillDescriptor:
         {
           id: "code-review",
           description: "Reviews code for quality...",
           location: Custom,
           user_invocable: true
         }
     │
     ▼
返回: Vec<SkillDescriptor> (仅元数据，~100 tokens)
     │
     └─► Agent 接收技能列表用于语义匹配
```

### 阶段 2: 技能匹配

```
Agent 接收技能描述符:
   - code-review: "Reviews code for quality..."
   - test-generator: "Generates test cases..."
   - pdf: "PDF manipulation toolkit..."

Agent 执行语义匹配:
   查询: "Can you review my code?"
   匹配: "code-review" (高置信度)
      │
      ▼
Agent 决定使用: "code-review"
```

### 阶段 3: 激活 (第2层 - 指令)

```
Agent 调用: runtime.get_skill("code-review")
     │
     ├─► Registry.get("code-review")
     │   └─ 返回: Skill (已在发现期间加载)
     │
     ▼
文件 I/O: SKILL.md 已解析，现在我们使用完整内容
     │
     ├─► 第1层 (已加载):
     │   └─ Manifest 元数据
     │
     ├─► 第2层 (现在访问):
     │   └─ 完整的 SKILL.md markdown 正文 (第9-67行)
     │       ├─ "# Code Review Skill"
     │       ├─ "## Review Checklist"
     │       ├─ "### 1. Correctness"
     │       ├─ "### 2. Security"
     │       ├─ "### 3. Performance"
     │       ├─ "### 4. Maintainability"
     │       ├─ "### 5. Testing"
     │       ├─ "## Output Format"
     │       └─ "## Guidelines"
     │
     └─► 返回: LoadedSkill {
           id: "code-review",
           manifest: SkillManifest {...},
           instructions: "# Code Review Skill\n\nPerform thorough...",
           location: Custom
         }
     │
     ▼
Agent 接收完整指令 (~2000 tokens)
     │
     └─► Agent 现在具有完整的技能上下文
```

### 阶段 4: 执行 (第3层 - WASM 模块)

```
Agent 调用: runtime.execute_skill("code-review", ExecutionOptions {
     input: json!({"code": "...", "file": "src/main.rs"}),
     timeout_ms: Some(30000)
})
     │
     ├─► Registry.get("code-review") → Skill
     │
     ├─► 验证技能结构
     │
     ├─► 检查危险工具权限
     │   └─ allowed-tools: ["Read", "Grep", "Glob", "LS"]
     │       └─ 全部低风险，无需权限提示
     │
     ├─► 将 allowed-tools 映射到 WASI 能力:
     │   ├─ Read → 文件系统读访问
     │   ├─ Grep → 文件系统读访问
     │   ├─ Glob → 文件系统读访问
     │   └─ LS → 文件系统读访问
     │
     ├─► 创建 PermissionEnforcer:
     │   ├─ allowed_tools: ["Read", "Grep", "Glob", "LS"]
     │   ├─ wasm_config: { filesystem: { read: [...] }, ... }
     │   └─ skill_root: "examples/skills/code-review"
     │
     ▼
检测执行模式: detect_execution_mode()
     │
     ├─► 检查 WASM 模块:
     │   ├─ examples/skills/code-review/wasm/skill.wasm ✓ 找到
     │   └─ 返回: ExecutionMode::Wasm { wasm_module: "wasm/skill.wasm" }
     │
     ▼
文件 I/O: 加载 WASM 模块 (第3层 - 按需)
     │
     ├─► 读取: examples/skills/code-review/wasm/skill.wasm
     │   └─ 二进制文件读入内存
     │
     ▼
WASM 执行: execute_wasm()
     │
     ├─► 配置 Wasmtime:
     │   ├─ 带 WASI 0.3 的 Engine (组件模型)
     │   ├─ Epoch 中断 (用于超时)
     │   └─ 异步支持已启用
     │
     ├─► 构建 WASI 上下文:
     │   ├─ 预打开技能根目录: /skill (仅读)
     │   ├─ 预打开读路径 (来自 allowed-tools 的任何)
     │   ├─ 注入环境变量:
     │   │   ├─ SKILL_ID="code-review"
     │   │   ├─ SKILL_NAME="code-review"
     │   │   ├─ SKILL_INPUT='{"code":"...","file":"src/main.rs"}'
     │   │   └─ TIMEOUT_MS="30000"
     │   └─ 捕获 stdout/stderr (内存缓冲区)
     │
     ├─► 加载 WASM 组件:
     │   ├─ 解析 wasm/skill.wasm
     │   ├─ 验证 WASI 0.3 兼容性
     │   └─ 创建组件实例
     │
     ├─► 在 WASM 沙箱中执行:
     │   ├─ 运行时: 带 WASI 0.3 的 Wasmtime
     │   ├─ 内存: 隔离线性内存
     │   ├─ 文件系统: 基于能力 (仅 /skill 和预打开目录)
     │   ├─ 网络: 已拒绝 (不在 allowed-tools 中)
     │   ├─ 过程: 已拒绝 (不在 allowed-tools 中)
     │   └─ 超时: 30 秒 (基于 epoch 的中断)
     │
     ├─► WASM 模块执行:
     │   ├─ 通过 SKILL_INPUT 环境变量接收输入
     │   ├─ 可从 /skill 目录读取 (技能资源)
     │   ├─ 可从预打开路径读取 (如果任何)
     │   ├─ 处理代码审查逻辑
     │   ├─ 将输出写入 stdout (已捕获)
     │   └─ 返回 JSON 结果
     │
     ├─► 捕获输出:
     │   ├─ stdout: "Review results..."
     │   ├─ stderr: "" (空)
     │   └─ exit_status: Success
     │
     └─► 返回: ExecutionArtifacts {
           output: json!({"review": "...", "issues": [...]}),
           stdout: "Review results...",
           stderr: "",
           permissions_used: ["Read", "Grep", "Glob", "LS"],
           exit_status: Success
         }
     │
     ▼
Agent 接收执行结果
     │
     └─► Agent 在对用户的响应中使用审查结果
```

---

## 工作流 2: PDF 技能 (本地 Python 执行)

### 阶段 1: 发现 (第1层 - 仅元数据)

```
用户查询: "Extract text from this PDF file"
     │
     ▼
Agent 调用: runtime.discover_skills()
     │
     ├─► 扫描目录
     │   ├─ ~/.claude/skills/
     │   ├─ .claude/skills/
     │   └─ examples/claude-official-skills/skills/ (子模块)
     │
     ├─► 对每个找到的目录:
     │   ├─ 遍历子目录
     │   ├─ 检查 SKILL.md 文件
     │   └─ 如果找到: 仅加载第1层
     │
     ▼
文件 I/O: 读取 examples/claude-official-skills/skills/pdf/SKILL.md
     │
     ├─► 解析 YAML 前置事项 (第1-5行):
     │   ├─ name: "pdf"
     │   ├─ description: "Comprehensive PDF manipulation toolkit..."
     │   └─ license: "Proprietary. LICENSE.txt has complete terms"
     │
     ├─► 跳过 Markdown 正文 (第2层 - 暂不加载)
     │
     ├─► 跳过 scripts/ 目录 (第3层 - 暂不加载)
     │
     └─► 创建 SkillDescriptor:
         {
           id: "pdf",
           description: "Comprehensive PDF manipulation toolkit...",
           location: Custom,
           user_invocable: true
         }
     │
     ▼
返回: Vec<SkillDescriptor> (仅元数据，~100 tokens)
     │
     └─► Agent 接收技能列表用于语义匹配
```

### 阶段 2: 技能匹配

```
Agent 接收技能描述符:
   - code-review: "Reviews code..."
   - pdf: "Comprehensive PDF manipulation toolkit..."
   - docx: "Word document processing..."

Agent 执行语义匹配:
   查询: "Extract text from this PDF file"
   匹配: "pdf" (高置信度)
      │
      ▼
Agent 决定使用: "pdf"
```

### 阶段 3: 激活 (第2层 - 指令)

```
Agent 调用: runtime.get_skill("pdf")
     │
     ├─► Registry.get("pdf")
     │   └─ 返回: Skill (已在发现期间加载)
     │
     ▼
文件 I/O: SKILL.md 已解析，现在我们使用完整内容
     │
     ├─► 第1层 (已加载):
     │   └─ Manifest 元数据
     │
     ├─► 第2层 (现在访问):
     │   └─ 完整的 SKILL.md markdown 正文 (第7-295行)
     │       ├─ "# PDF Processing Guide"
     │       ├─ "## Overview"
     │       ├─ "## Quick Start"
     │       ├─ "## Python Libraries"
     │       │   ├─ "### pypdf - Basic Operations"
     │       │   ├─ "### pdfplumber - Text and Table Extraction"
     │       │   └─ "### pdf2image - Convert to Images"
     │       ├─ "## Command-Line Tools"
     │       ├─ 参考:
     │       │   ├─ forms.md (用于表单填充)
     │       │   └─ reference.md (用于高级功能)
     │       └─ 脚本示例和使用模式
     │
     └─► 返回: LoadedSkill {
           id: "pdf",
           manifest: SkillManifest {...},
           instructions: "# PDF Processing Guide\n\n## Overview...",
           location: Custom
         }
     │
     ▼
Agent 接收完整指令 (~5000+ tokens)
     │
     └─► Agent 现在具有完整的技能上下文
```

### 阶段 4: 执行 (第3层 - 本地 Python 脚本)

```
Agent 调用: runtime.execute_skill("pdf", ExecutionOptions {
     input: json!({
         "action": "extract_text",
         "file": "/path/to/document.pdf"
     }),
     timeout_ms: Some(10000)
})
     │
     ├─► Registry.get("pdf") → Skill
     │
     ├─► 验证技能结构
     │
     ├─► 检查危险工具权限
     │   └─ allowed-tools: [] (空 = 所有工具允许)
     │       └─ 无显式限制，继续
     │
     ├─► 将 allowed-tools 映射到能力:
     │   └─ 空列表 → 默认权限 (技能根目录中的读/写)
     │
     ├─► 创建 PermissionEnforcer:
     │   ├─ allowed_tools: [] (全部允许)
     │   ├─ wasm_config: 默认 (用于本地，用于路径映射)
     │   └─ skill_root: "examples/claude-official-skills/skills/pdf"
     │
     ▼
检测执行模式: detect_execution_mode()
     │
     ├─► 检查 WASM 模块:
     │   ├─ examples/.../pdf/wasm/skill.wasm ✗ 未找到
     │   └─ 继续本地脚本检测
     │
     ├─► 检查本地脚本:
     │   ├─ examples/.../pdf/scripts/extract_form_field_info.py ✓ 找到
     │   ├─ examples/.../pdf/scripts/fill_fillable_fields.py
     │   ├─ examples/.../pdf/scripts/convert_pdf_to_images.py
     │   └─ ... (共 8 个 Python 脚本)
     │
     ├─► 基于输入 action="extract_text":
     │   └─ 选择: scripts/extract_form_field_info.py
     │       (或 Agent 可基于指令选择)
     │
     └─► 返回: ExecutionMode::Native {
           script_path: "scripts/extract_form_field_info.py",
           script_type: ScriptType::Python
         }
     │
     ▼
文件 I/O: 加载 Python 脚本 (第3层 - 按需)
     │
     ├─► 读取: examples/.../pdf/scripts/extract_form_field_info.py
     │   └─ 源代码已读 (但不加载到 Agent 上下文中)
     │       └─ 仅脚本输出将返回
     │
     ▼
本地执行: execute_native() [macOS seatbelt]
     │
     ├─► 检测平台: macOS ✓
     │
     ├─► 准备 seatbelt 配置文件:
     │   ├─ 规范化 skill_root 路径
     │   ├─ 从 PermissionEnforcer 获取读路径
     │   ├─ 从 PermissionEnforcer 获取写路径
     │   ├─ 确定网络访问: false (allowed-tools 中无 WebSearch/Fetch)
     │   ├─ 确定过程访问: false (allowed-tools 中无 Bash/Terminal)
     │   └─ 获取 Python 可执行路径: /usr/bin/python3
     │
     ├─► 构建 seatbelt 配置文件字符串:
     │   ├─ "(version 1)"
     │   ├─ "(deny default)"  ← 从拒绝所有开始
     │   ├─ "(allow sysctl-read)"
     │   ├─ 系统读路径:
     │   │   ├─ "/System", "/usr/lib", "/usr/bin" 等。
     │   │   └─ "(allow file-read* file-map-executable (subpath \"/usr/bin\"))"
     │   ├─ 临时路径:
     │   │   └─ "(allow file-read* file-write* (subpath \"/tmp\"))"
     │   ├─ 技能根 (仅读):
     │   │   └─ "(allow file-read* (subpath \".../pdf\"))"
     │   ├─ 读路径 (来自 allowed-tools 的任何):
     │   │   └─ "(allow file-read* (subpath \"...\"))"
     │   └─ 写路径 (来自 allowed-tools 的任何):
     │       └─ "(allow file-write* (subpath \"...\"))"
     │
     ├─► 将 seatbelt 配置文件写入临时文件:
     │   └─ /tmp/openskills-seatbelt-{pid}-{attempt}.sb
     │
     ├─► 准备命令:
     │   ├─ 程序: "sandbox-exec"
     │   ├─ 参数: ["-f", profile_path, "--", "python3", script_path]
     │   ├─ 工作目录: skill_root
     │   ├─ stdin: 管道 (用于输入 JSON)
     │   ├─ stdout: 管道 (用于捕获)
     │   └─ stderr: 管道 (用于捕获)
     │
     ├─► 设置环境变量:
     │   ├─ SKILL_ID="pdf"
     │   ├─ SKILL_NAME="pdf"
     │   ├─ SKILL_INPUT='{"action":"extract_text","file":"/path/to/document.pdf"}'
     │   ├─ TIMEOUT_MS="10000"
     │   ├─ PYTHONNOUSERSITE="1" (从用户 site-packages 隔离)
     │   └─ PATH、PYTHONPATH (最小化、沙箱化)
     │
     ├─► 用 seatbelt 生成过程:
     │   └─ sandbox-exec -f /tmp/openskills-seatbelt-12345-0.sb -- \
     │       python3 scripts/extract_form_field_info.py
     │
     ├─► 将输入 JSON 写入 stdin
     │
     ├─► 使用超时监控执行:
     │   ├─ 启动超时线程 (10 秒)
     │   ├─ 等待过程完成
     │   └─ 如果超时: 杀死过程，返回超时错误
     │
     ├─► 读取 stdout/stderr:
     │   ├─ stdout: 从 Python 脚本捕获的输出
     │   └─ stderr: 任何错误消息
     │
     ├─► Python 脚本执行 (在 seatbelt 沙箱中):
     │   ├─ 从环境读取 SKILL_INPUT
     │   ├─ 解析 JSON: {"action": "extract_text", "file": "..."}
     │   ├─ 可从技能根读取: scripts/、forms.md、reference.md
     │   ├─ 可从允许的读路径读取 (如果任何)
     │   ├─ 可写入允许的写路径 (如果任何)
     │   ├─ 无法访问网络 (被 seatbelt 拒绝)
     │   ├─ 无法生成过程 (被 seatbelt 拒绝)
     │   ├─ 无法访问允许路径外的文件
     │   ├─ 执行 PDF 提取逻辑
     │   ├─ 使用 pypdf 或 pdfplumber (如果已安装)
     │   └─ 将结果输出到 stdout (JSON)
     │
     ├─► 清理:
     │   └─ 删除临时 seatbelt 配置文件
     │
     └─► 返回: ExecutionArtifacts {
           output: json!({"text": "Extracted text content...", "pages": 10}),
           stdout: "Extracted text content...",
           stderr: "",
           permissions_used: [],
           exit_status: Success
         }
     │
     ▼
Agent 接收执行结果
     │
     └─► Agent 在对用户的响应中使用提取的文本
```

---

## 沙箱环境支持

OpenSkills 提供两个互补的沙箱环境，各自针对不同用例优化:

### 1. WASM/WASI 0.3 沙箱 (主要)

**目的**: 用于编译技能的跨平台、基于能力的沙箱处理

**技术栈**:
- **运行时**: Wasmtime 40+ 带 WASI 0.3 (组件模型)
- **隔离**: 线性内存模型，基于能力的文件系统访问
- **平台支持**: macOS、Linux、Windows (相同行为)

**工作原理**:

1. **技能编译**: JavaScript/TypeScript 技能使用 `javy` (QuickJS 基础) 或 `wasm-pack` 等工具编译为 WASM 组件
2. **模块加载**: WASM 模块从技能目录中的 `wasm/skill.wasm` 加载
3. **WASI 上下文设置**:
   - 基于 `allowed-tools` 映射预打开文件系统路径
   - 仅向明确允许的目录授予读/写权限
   - 注入环境变量 (SKILL_ID、SKILL_INPUT 等。)
   - 在内存缓冲区中捕获 stdout/stderr
4. **能力映射**: `allowed-tools` 值映射到 WASI 能力:
   - `Read`、`Grep`、`Glob`、`LS` → 文件系统读访问
   - `Write` → 文件系统写访问
   - `WebSearch`、`Fetch` → 网络访问 (带主机允许列表)
   - `Bash`、`Terminal` → 过程生成 (如果支持)
5. **执行**: 组件在隔离 WASM 实例中运行，具有:
   - 通过 epoch 中断的超时强制
   - 内存限制 (可配置)
   - 无法访问主机文件系统，除了预打开路径
   - 无网络访问，除非明确允许
   - 跨平台的确定性执行

**优势**:
- ✅ **跨平台一致性**: 相同的安全模型无处不在
- ✅ **内存安全**: 线性内存防止缓冲区溢出
- ✅ **可移植性**: 技能可以发布预编译的 WASM 模块
- ✅ **细粒度权限**: 基于能力的访问控制
- ✅ **无本地依赖**: 纯 WASM 执行

**限制**:
- 需要编译步骤 (TS/JS → WASM)
- 仅限于 WASI 兼容 API
- 无法直接使用本地 Python 包
- 与本地执行相比性能开销

**用例**:
- 基于 TypeScript/JavaScript 的技能
- 需要跨平台一致性的技能
- 可以编译为 WASM 的技能
- 对安全要求严格的企业部署

---

### 2. 本地 Seatbelt 沙箱 (仅限 macOS)

**目的**: 用于本地 Python 和 shell 脚本的 OS 级沙箱处理

**技术栈**:
- **运行时**: macOS `sandbox-exec` 与 seatbelt 配置文件
- **隔离**: 过程级沙箱处理，带文件系统和网络限制
- **平台支持**: 仅限 macOS (已计划 Linux seccomp 支持)

**工作原理**:

1. **脚本检测**: 运行时在 `scripts/` 目录中检测本地脚本 (`.py`、`.sh`)
2. **Seatbelt 配置文件生成**:
   - 从 `(deny default)` 开始 - 默认拒绝全部
   - 允许系统读路径: `/System`、`/usr/bin`、`/usr/lib` 等。
   - 允许临时路径: `/tmp`、`/private/tmp` (读/写)
   - 允许技能根目录 (默认仅读)
   - 基于 `allowed-tools` 映射添加读/写路径
   - 为 Python 解释器和脚本父目录授予 `file-map-executable` 权限
   - 如果 `allowed-tools` 中有 `WebSearch` 或 `Fetch`，有条件地允许网络访问
   - 如果 `allowed-tools` 中有 `Bash` 或 `Terminal`，有条件地允许过程生成
3. **配置文件写入**: Seatbelt 配置文件写入临时文件 (`/tmp/openskills-seatbelt-{pid}-{attempt}.sb`)
4. **过程执行**:
   - 生成 `sandbox-exec -f {profile} -- python3 {script}`
   - 将工作目录设置为技能根目录
   - 将输入 JSON 管道传输到 stdin
   - 捕获 stdout/stderr
   - 通过单独的监控线程强制超时
5. **环境隔离**:
   - 设置 `PYTHONNOUSERSITE=1` 以防止加载用户 site-packages
   - 最小化 PATH 和 PYTHONPATH
   - 将技能元数据作为环境变量注入
6. **清理**: 执行后删除临时 seatbelt 配置文件

**优势**:
- ✅ **本地 Python 支持**: 可以使用任何 Python 包 (pypdf、pdfplumber 等。)
- ✅ **Shell 脚本支持**: 可以执行 bash 脚本
- ✅ **完整 OS API 访问**: 在沙箱约束内
- ✅ **无需编译**: 直接脚本执行
- ✅ **强隔离**: 过程级沙箱处理

**限制**:
- 仅限 macOS (已计划 Linux seccomp 支持)
- 特定于平台的行为
- 需要本地 Python/系统依赖
- 不如 WASM 可移植

**用例**:
- 基于 Python 的技能 (PDF 处理、文档操作)
- 需要本地库的技能
- 无法编译为 WASM 的遗留脚本
- 仅限 macOS 的部署

---

## 执行模式检测

运行时自动检测使用哪个执行模式:

```rust
fn detect_execution_mode(skill_root: &PathBuf, wasm_override: Option<String>) 
    -> Result<ExecutionMode, OpenSkillError>
```

**检测优先级**:
1. **WASM 覆盖**: 如果在 ExecutionOptions 中指定了 `wasm_module`，使用 WASM
2. **WASM 模块**: 查找 `wasm/skill.wasm`、`skill.wasm`、`module.wasm` 或任何 `.wasm` 文件
3. **本地脚本**: 在 `scripts/` 目录中查找 `.py` 或 `.sh` 文件
4. **错误**: 如果未找到，返回错误

**技能目录结构**:

```
my-skill/
├── SKILL.md              # 第1 & 2层: 元数据 + 指令
├── wasm/
│   └── skill.wasm        # 第3层: WASM 模块 (如果使用 WASM)
└── scripts/
    └── process.py        # 第3层: 本地脚本 (如果使用本地)
```

**注**: 一个技能可以同时有 WASM 和本地脚本，但根据检测优先级只有一个将被执行。

---

## 权限映射

两个沙箱环境使用相同的 `allowed-tools` → 能力映射:

| allowed-tools | WASM 能力 | 本地 Seatbelt |
|---------------|----------------|-----------------|
| `Read`、`Grep`、`Glob`、`LS` | 文件系统读 (预打开目录) | 文件系统读 (配置文件路径) |
| `Write` | 文件系统写 (预打开目录) | 文件系统写 (配置文件路径) |
| `WebSearch`、`Fetch` | 网络访问 (主机允许列表) | 网络访问 (配置文件允许) |
| `Bash`、`Terminal` | 过程生成 (如果支持) | 过程生成 (配置文件允许) |

**空 `allowed-tools`**: 表示允许所有工具 (无限制)

**危险工具**: `Write`、`Bash`、`Terminal`、`WebSearch` 等工具可能在执行前通过 `PermissionManager` 回调触发权限提示。

---

## 文件 I/O 操作摘要

### 代码审查 (WASM):
1. **发现**: 读取 `SKILL.md` (仅前置事项) - 第1层
2. **激活**: 使用已解析的 `SKILL.md` (完整内容) - 第2层
3. **执行**: 读取 `wasm/skill.wasm` (二进制，按需) - 第3层

### PDF (本地 Python):
1. **发现**: 读取 `SKILL.md` (仅前置事项) - 第1层
2. **激活**: 使用已解析的 `SKILL.md` (完整内容) - 第2层
3. **执行**: 读取 `scripts/extract_form_field_info.py` (源，按需，不在上下文中) - 第3层

两个工作流都遵循**渐进披露模式**: 元数据 → 指令 → 资源，资源仅在执行时需要时加载。脚本源代码从不加载到 Agent 的上下文窗口中 - 仅返回它们的输出。
