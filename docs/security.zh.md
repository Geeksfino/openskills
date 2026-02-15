# 安全政策

本文档描述了 OpenSkills Runtime 实施的安全模型、沙箱机制和权限政策。

## 概述

OpenSkills Runtime 实现了一个**纵深防御**安全模型，包括多个层次：

1. **本机脚本沙箱**（主要方式）- OS 级别的沙箱（macOS seatbelt + Linux Landlock）用于 Python/Shell 脚本 - 生产就绪
2. **WASM 沙箱**（实验性）- 针对 WASM 模块的基于能力的隔离（WASI 0.2/0.3）- 用于特定用例
3. **主机策略** - 位于技能声明和沙箱强制执行之间的主机级工具治理
4. **权限执行** - 通过 `allowed-tools`、风险工具检测和主机策略解析进行的基于工具的访问控制
5. **上下文隔离** - Forked 上下文防止技能输出污染

**注意**：通过操作系统级沙箱的原生脚本是主要和推荐的执行方法。WASM 沙箱是实验性的。

---

## WASM 沙箱（WASI 0.2/0.3）- 实验性

**状态**：实验性功能。原生脚本是主要的执行方法。

WASM 模块在基于能力的沙箱中执行，使用 WASI 组件模型。运行时同时支持 WASI 0.2 (p2) 和 0.3 (p3) —— 它首先尝试 p3 实例化，并在兼容性方面回退到 p2。当前的构建工具链通过 `wasi_snapshot_preview1` 适配器生成 WASI 0.2 组件。这可用于需要确定性的特定用例，但不适合完整的 Python 生态系统或原生库。

查看 [README.md](../README.md#wasm-support-long-term-vision) 了解有关 WASM 角色和限制的详细讨论。

### 文件系统访问

**读取访问：**
- 默认：无读取访问
- 基于 `allowed-tools` 授予：
  - `Read`、`Grep`、`Glob`、`LS` → 读取技能根目录
  - `Bash`、`Terminal` → 读取技能根目录
- 可通过技能清单 `wasm.filesystem.read` 配置进行扩展

**写入访问：**
- 默认：无写入访问
- 基于 `allowed-tools` 授予：
  - `Write`、`Edit`、`MultiEdit` → 写入技能根目录
  - `Bash`、`Terminal` → 写入技能根目录
- 可通过技能清单 `wasm.filesystem.write` 配置进行扩展

**路径解析：**
- 相对路径相对于技能根目录解析
- 绝对路径按原样使用（如果允许）

### 网络访问

**默认：** 无网络访问

**授予时：**
- `allowed-tools` 包含 `WebSearch` 或 `Fetch`
- 技能清单 `wasm.network.allow` 指定允许的主机

**主机匹配：**
- 精确主机匹配：`api.example.com`
- 子域匹配：`*.example.com` 匹配 `sub.api.example.com`
- 通配符 `*` 允许所有主机（使用 `WebSearch` 或 `Fetch` 工具时）

### 环境变量

**默认：** 不暴露环境变量

**通过以下方式授予：**
- 技能清单 `wasm.env.allow` 列表

### 资源限制

- **内存：** 默认 128MB，可通过 `wasm.memory_mb` 配置
- **超时：** 默认 30 秒，可通过 `wasm.timeout_ms` 配置
- **随机种子：** 可选确定性种子通过 `wasm.random_seed`

---

## 本机脚本沙箱（macOS/Linux）- 主要方式

**状态**：生产就绪，主要执行方法。

本机 Python 和 Shell 脚本在操作系统级沙箱（macOS seatbelt 和 Linux Landlock）下执行，遵循 Claude Code 的安全模型。这是大多数技能的推荐方法，提供对完整 Python 生态系统和原生工具的访问。

### 安全模型

沙箱配置文件使用**"允许宽泛读取，拒绝特定敏感路径"**方法（在 macOS 上；Linux Landlock 使用基于能力的路径限制）：

1. **默认拒绝** - 默认拒绝所有操作
2. **允许宽泛文件读取** - Python 和解释器需要系统库访问（macOS）
3. **拒绝特定敏感路径** - 凭证和配置文件被显式阻止
4. **仅允许写入特定路径** - 临时目录、技能根目录、配置路径

### 核心权限

所有本机脚本都获得这些基础权限：
- `sysctl-read` - 系统信息查询
- `process-exec` - 执行解释器二进制文件
- `process-fork` - Fork 子进程
- `mach-lookup` - Mach 端口查询（进程执行所需）
- `signal` - 信号处理

### 文件读取访问

**允许：**
- **宽泛读取访问** (`allow file-read*`) - Python/解释器需要访问：
  - 系统库（`/usr/lib`、`/System/Library`）
  - Python 框架（`/Library/Frameworks/Python.framework`）
  - Homebrew 安装（`/opt/homebrew`）
  - 标准系统路径（`/usr/bin`、`/bin`、`/sbin`）
  - 用户目录（`/Users`）
  - 临时目录（`/tmp`、`/private/tmp`）

**显式拒绝（敏感路径）：**
```
~/.ssh              # SSH 密钥
~/.gnupg            # GPG 密钥
~/.aws              # AWS 凭证
~/.azure            # Azure 凭证
~/.config/gcloud    # Google Cloud 凭证
~/.kube             # Kubernetes 配置
~/.docker           # Docker 配置
~/.npmrc            # npm 凭证
~/.pypirc           # PyPI 凭证
~/.netrc            # 网络凭证
~/.gitconfig        # Git 配置
~/.git-credentials  # Git 凭证
~/.bashrc           # Shell 配置
~/.zshrc            # Shell 配置
~/.profile          # Shell 配置
~/.bash_profile     # Shell 配置
~/.zprofile         # Shell 配置
```

**注意：** `~` 在运行时扩展为用户的主目录。

### 文件写入访问

**允许：**
- `/dev/null` - 输出重定向
- 临时目录：
  - `/tmp`
  - `/private/tmp`
  - `/private/var/tmp`
  - `/private/var/folders`
- 技能根目录（技能所在位置）
- 显式配置的写入路径（来自技能清单）

**拒绝：**
- 所有其他路径（包括系统目录、用户主目录等）

### 进程执行

**默认：** 无进程执行

**授予时：**
- 脚本类型为 `Shell` 或 `Python`（需要解释器执行）
- `allowed-tools` 包含 `Bash` 或 `Terminal`
- 授予完整 `process*` 权限（允许子进程生成）

### 网络访问

**默认：** 无网络访问

**授予时：**
- `allowed-tools` 包含 `WebSearch` 或 `Fetch`
- `allow network*` 添加到 seatbelt 配置文件

---

## 权限执行

### 允许的工具

技能可以通过其清单中的 `allowed-tools` 字段限制允许使用的工具：

```yaml
allowed-tools:
  - Read
  - Grep
  - Glob
```

**行为：**
- **空列表** = 允许所有工具（无限制）
- **非空列表** = 仅允许列出的工具
- 未列出的工具调用被**拒绝**并返回 `PermissionDenied` 错误

### 风险工具

某些工具被分类为"风险"工具，需要通过回调进行显式权限：

**低风险：**
- `Read`、`Grep`、`Glob`、`LS` - 仅读操作

**中风险：**
- `Write`、`Edit`、`MultiEdit` - 文件修改
- `WebSearch`、`Fetch` - 网络访问

**高风险：**
- `Bash`、`Terminal` - 任意命令执行
- `Delete` - 文件删除

**权限流程：**
1. Agent 调用 `check_tool_permission(skill_id, tool, description)`
2. Runtime 检查工具是否在 `allowed-tools` 中（如果列表非空）
3. 如果工具有风险，runtime 调用权限回调：
   - `DenyAllCallback` - 总是拒绝（严格模式）
   - `CliPermissionCallback` - 提示用户审批
   - 自定义回调 - 用户定义的逻辑
4. 如果允许返回 `true`，如果拒绝返回 `false` 或错误

### 工具到能力映射

工具映射到 WASI 能力如下：

| 工具 | 文件系统读取 | 文件系统写入 | 网络 |
|------|-----------|----------|------|
| `Read`、`Grep`、`Glob`、`LS` | ✅ 技能根目录 | ❌ | ❌ |
| `Write`、`Edit`、`MultiEdit` | ✅ 技能根目录 | ✅ 技能根目录 | ❌ |
| `Bash`、`Terminal` | ✅ 技能根目录 | ✅ 技能根目录 | ❌ |
| `WebSearch`、`Fetch` | ❌ | ❌ | ✅ 所有主机 |

---

## 上下文隔离

### Forked 上下文

使用 `context: fork` 的技能在隔离上下文中执行：

**隔离：**
- Forked 上下文中记录的工具调用
- 中间输出（stdout、stderr）分别捕获
- 仅**摘要**返回到父上下文
- 防止冗长工具输出的上下文污染

**摘要生成：**
- 仅提取 `Result` 类型输出
- 从摘要中排除 `ToolCall`、`Stdout`、`Stderr`
- 如果未记录显式结果，则回退到 stdout

**用例：**
- 仅指令技能（无 WASM 模块或本机脚本）
- Agent 执行工具并记录输出
- 最终摘要返回到父 agent

---

## 安全边界

### 受保护内容

✅ **凭证** - SSH 密钥、AWS/Azure/GCP 凭证、API 密钥  
✅ **配置文件** - Git 配置、shell 配置、Docker/Kubernetes 配置  
✅ **系统目录** - 拒绝系统路径的写入访问  
✅ **网络** - 除非显式允许，否则无网络访问  
✅ **进程执行** - 仅限于所需的解释器，除非允许 `Bash`/`Terminal`  

### 允许内容

✅ **系统库** - 用于解释器执行的读取访问  
✅ **技能目录** - 对技能根目录的读/写访问  
✅ **临时文件** - 对 `/tmp` 和变体的写入访问  
✅ **标准输入/输出** - `/dev/null` 用于重定向  

---

## 审计日志

所有技能执行都记录有：

- **技能 ID** 和版本
- **输入/输出哈希值**（SHA-256）
- **使用的权限**（工具、文件系统路径、网络主机）
- **执行状态**（成功、超时、权限拒绝、失败）
- **计时**（开始时间、持续时间）
- **输出**（stdout、stderr）

审计记录被发送到配置的审计接收器（默认：无操作接收器）。

---

## 最佳实践

### 对于技能作者

1. **最小化 `allowed-tools`** - 仅请求你实际需要的工具
2. **避免风险工具** - 如果可能，优先使用 `Read` 而非 `Bash`
3. **使用原生脚本** - 推荐用于大多数技能；完整 Python 生态系统访问
4. **指定文件系统路径** - 对于 WASM（实验性）：使用 `wasm.filesystem.read/write` 进行最小访问
5. **限制网络** - 对于 WASM（实验性）：使用 `wasm.network.allow` 指定特定主机，而不是 `*`
6. **使用 `context: fork`** - 对于仅指令技能以防止上下文污染

### 对于 Runtime 用户

1. **检查 `allowed-tools`** - 理解每个技能能做什么
2. **配置权限回调** - 使用 `CliPermissionCallback` 进行交互式审批
3. **监控审计日志** - 查看执行记录以找出可疑活动
4. **相信技能来源** - 仅从受信任的仓库加载技能

---

## 实施细节

### Seatbelt 配置文件生成

Seatbelt 配置文件基于以下内容动态生成：
- 技能根目录
- 配置的读/写路径
- `allowed-tools`（用于网络/进程权限）
- 脚本类型（Python/Shell）

配置文件遵循此结构：
```
(version 1)
(deny default)
(allow sysctl-read)
(allow process-exec)
(allow process-fork)
(allow mach-lookup)
(allow signal)
(allow file-read*)
(deny file-read* (subpath "~/.ssh"))
... (更多敏感路径拒绝)
(allow file-write* (subpath "/tmp"))
... (更多写入路径允许)
(allow process*)  # 如果允许 Bash/Terminal
(allow network*) # 如果允许 WebSearch/Fetch
```

### WASI 能力预开启

WASM 模块通过 WASI 0.3 接收预开启的目录：
- 只读目录：映射到技能根目录或配置的读取路径
- 写入目录：映射到技能根目录或配置的写入路径
- 无法访问父目录或系统路径

---

## 相关文档

- [架构](./architecture.md) - 整体系统设计
- [技能流](./skill-flow.md) - 执行工作流
- [开发者指南](./developers.md) - API 使用示例

---

最后更新：2026-01-18
