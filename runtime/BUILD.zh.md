# OpenSkills 构建工具

`openskills build` 命令将 TypeScript/JavaScript 技能编译为 WASM 组件，以便在 OpenSkills 运行时中执行。

## 工作原理：基于插件的构建后端

OpenSkills 使用**基于插件的构建系统**将 JavaScript/TypeScript 编译为 WASM。默认插件是 **`javy`**（通过 `javy-codegen`），但随着时间推移可以添加其他插件。这种方法：

- **无需 CLI**：插件可以通过 Rust 库进行编译，无需外部二进制文件
- **编程式编译**：JavaScript → WASM 编译通过 Rust API 调用完成
- **可插拔后端**：开发者可以选择使用哪个编译器后端

### 理解默认插件（javy）

**javy 插件**（`plugin.wasm`）是一个包含 QuickJS 运行时和编译逻辑的 WASM 模块。它之所以需要是因为：

1. **javy-codegen** 是协调编译过程的 Rust 库
2. **plugin.wasm** 包含实际的 JavaScript 引擎（QuickJS）和 WASM 生成代码
3. 插件必须在使用前进行"wizened"（初始化），这会生成 `plugin_wizened.wasm`

这种分离允许：
- **库的灵活性**：Rust 代码可以通过 crates.io 进行版本控制和分发
- **插件更新**：插件可以根据需要独立更新
- **无 CLI 依赖**：一切都在 OpenSkills 构建过程中以编程方式完成

## 设置

**在构建你的第一个技能之前**，运行设置脚本以安装所需的工具：

```bash
# 从 OpenSkills 仓库根目录
./scripts/setup_build_tools.sh
```

此脚本将：
- ✅ 下载 WASI preview1 适配器（WASI 0.3 组件所需）
- ✅ 安装 `javy` CLI（可用时下载预构建二进制文件，否则从源码构建）
- ✅ 安装 `wasm-tools`（用于组件转换）
- ✅ 检查可选工具，如 AssemblyScript

设置脚本会自动检测你的操作系统和架构，在可用时下载预构建二进制文件，如果需要则回退到从源码构建。

## 快速开始

运行设置脚本后，你可以构建技能：

```bash
# 从当前目录构建技能（自动检测插件）
openskills build

# 构建指定的技能目录
openskills build my-skill

# 使用详细输出构建
openskills build --verbose

# 强制重新构建（忽略最新检查）
openskills build --force

# 列出可用的插件
openskills build --list-plugins

# 明确选择一个插件
openskills build --plugin quickjs  # 推荐：使用预构建工具

# QuickJS 通过 javy CLI + wasm-tools 组件转换
openskills build --plugin quickjs

# AssemblyScript 通过 asc + wasm-tools 组件转换
openskills build --plugin assemblyscript

# 提供插件选项（示例：覆盖适配器路径）
openskills build --plugin quickjs \
  --plugin-option adapter_path=/path/to/wasi_preview1_adapter.wasm
```

### 配置文件（可选）

在技能目录中放置 `.openskills.toml` 或 `openskills.toml` 以设置默认值：

```toml
[build]
plugin = "javy"

[build.plugin_options]
plugin_path = "/tmp/javy/plugin_wizened.wasm"
```

CLI 标志会覆盖配置文件值。

## 要求

### 必需：javy 插件（默认后端）

OpenSkills 使用 **`javy-codegen`** 作为库依赖项（无需安装 CLI），但需要 **`plugin.wasm`** 文件来执行实际的 JavaScript → WASM 编译。

### QuickJS 插件（javy CLI + wasm-tools）

**快速设置**（推荐）：
```bash
./scripts/setup_build_tools.sh
openskills build --plugin quickjs
```

设置脚本会下载 WASI 适配器并检查所需的工具。如果未找到，适配器也会在首次构建时**自动下载**。

**手动设置**：
1. 安装 `javy` CLI：
   ```bash
   git clone https://github.com/bytecodealliance/javy.git /tmp/javy
   cd /tmp/javy && cargo install --path crates/cli
   ```
2. 安装 `wasm-tools`：
   ```bash
   cargo install wasm-tools
   ```
3. WASI 适配器会自动下载到 `~/.cache/openskills/`，或手动提供：
   ```bash
   export WASI_ADAPTER_PATH=/path/to/wasi_snapshot_preview1.command.wasm
   ```

### AssemblyScript 插件（asc + wasm-tools）

**快速设置**（推荐）：
```bash
./scripts/setup_build_tools.sh
openskills build --plugin assemblyscript
```

**手动设置**：
1. 安装 AssemblyScript：
   ```bash
   npm install -g assemblyscript
   ```
2. 安装 `wasm-tools`：
   ```bash
   cargo install wasm-tools
   ```
3. WASI 适配器会自动下载，或设置 `WASI_ADAPTER_PATH` 环境变量。

### 适配器自动检测

QuickJS 和 AssemblyScript 插件会自动在以下位置搜索 WASI 适配器：
1. 明确的 `--plugin-option adapter_path=...`
2. `WASI_ADAPTER_PATH` 环境变量
3. `~/.cache/openskills/wasi_preview1_adapter.wasm`
4. `~/.wasmtime/wasi_snapshot_preview1.command.wasm`
5. 当前目录（`wasi_preview1_adapter.wasm`）

如果未找到，适配器会从 Bytecode Alliance wasmtime 发布版**自动下载**。

#### 什么是插件？

javy 插件是一个包含以下内容的 WASM 模块：
- **QuickJS 运行时**：用于执行和编译 JS 代码的 JavaScript 引擎
- **编译逻辑**：将 JavaScript 转换为 WebAssembly 字节码的代码
- **WASI 绑定**：用于 WASM 执行的系统接口绑定

插件必须在使用前进行**"wizened"**（初始化），这会处理原始插件并生成 `plugin_wizened.wasm`。

#### 获取插件

插件可以通过以下方式提供：

1. **辅助脚本**（推荐）：
   ```bash
   ./scripts/build_javy_plugin.sh
   export JAVY_PLUGIN_PATH=/tmp/javy/target/wasm32-wasip1/release/plugin_wizened.wasm
   ```

2. **环境变量**：设置 `JAVY_PLUGIN_PATH` 指向现有的 `plugin_wizened.wasm` 文件

3. **当前目录**：将 `plugin_wizened.wasm` 放在当前目录（会自动检测）

4. **手动构建**（如果需要自定义）：
   ```bash
   git clone https://github.com/bytecodealliance/javy.git
   cd javy
   rustup target add wasm32-wasip1
   cargo build --release --target wasm32-wasip1 -p javy-plugin
   cargo run -p javy-cli -- init-plugin \
     target/wasm32-wasip1/release/plugin.wasm \
     --out target/wasm32-wasip1/release/plugin_wizened.wasm
   export JAVY_PLUGIN_PATH=$(pwd)/target/wasm32-wasip1/release/plugin_wizened.wasm
   ```

#### 为什么不捆绑插件？

插件不与 OpenSkills 捆绑是因为：
- **大小**：插件很大（~几 MB），会使 OpenSkills 二进制文件膨胀
- **灵活性**：用户可以自己构建插件或使用预构建版本
- **版本控制**：插件版本可以独立于 OpenSkills 版本
- **一次性设置**：一旦构建，插件可以重复用于所有技能构建

### 可选（用于 TypeScript）
- **esbuild**（推荐，更快）：如果不存在，会通过 `npx` 自动安装
- **TypeScript 编译器（tsc）**：esbuild 的替代方案
  ```bash
  npm install -g typescript
  ```

## 支持的源文件

构建工具按以下顺序自动检测源文件：
1. `src/index.ts`
2. `src/index.js`
3. `index.ts`
4. `index.js`
5. `src/main.ts`
6. `src/main.js`

## 构建过程

### TypeScript
1. **转译**：TypeScript → JavaScript（使用 esbuild 或 tsc）
2. **编译**：JavaScript → WASM 组件（使用选定的构建插件）
3. **输出**：`wasm/skill.wasm`

### JavaScript
1. **编译**：JavaScript → WASM 组件（使用选定的构建插件）
2. **输出**：`wasm/skill.wasm`

### 底层原理

当你运行 `openskills build` 时，会发生以下情况：

1. **源检测**：查找你的 TypeScript/JavaScript 源文件
2. **TypeScript 转译**（如果需要）：使用 esbuild 或 tsc 将 TS → JS 转换
3. **插件加载**：解析选定的构建插件及其依赖项
4. **WASM 生成**：使用插件后端：
   - 读取 JavaScript 源代码
   - 在 JavaScript 运行时中执行它（默认 javy 插件使用 QuickJS）
   - 生成 WASM 字节码
   - 将字节码嵌入 WASM 组件中
5. **输出**：将编译后的 WASM 写入 `wasm/skill.wasm`

所有这些都以编程方式完成——在构建过程中不会调用 CLI 工具。

## 示例技能结构

```
my-skill/
├── SKILL.md              # 技能清单
├── src/
│   └── index.ts         # TypeScript 源代码
└── wasm/
    └── skill.wasm       # 编译后的 WASM（生成的）
```

## 增量构建

构建工具检查文件修改时间：
- 如果 `wasm/skill.wasm` 比源代码新，则跳过构建
- 使用 `--force` 强制重新构建

## 输出

默认情况下，编译后的 WASM 会写入相对于技能目录的 `wasm/skill.wasm`。

你可以指定自定义输出路径：
```bash
openskills build --output custom/path/skill.wasm
```

## 错误处理

构建工具提供清晰的错误消息：
- 缺少源文件
- 缺少 javy 插件（附带如何获取的说明）
- TypeScript 编译错误
- JavaScript 到 WASM 编译错误

## 与 Git 集成

**推荐**：同时提交源代码和编译后的 WASM：
```bash
git add src/index.ts wasm/skill.wasm
git commit -m "Add skill implementation"
```

这允许：
- 源代码审查
- 无需构建工具链即可立即使用
- CI/CD 验证（重新构建并比较哈希）

## CI/CD 集成

示例 GitHub Actions 工作流：
```yaml
- name: Build skill
  run: |
    # 首先构建 javy 插件
    scripts/build_javy_plugin.sh
    openskills build --verbose
    
- name: Verify WASM matches source
  run: |
    # 重新构建并比较哈希
    openskills build --force
    # 如果哈希与提交的版本不同则失败
```
