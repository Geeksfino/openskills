# 为 OpenSkills 做出贡献

感谢你对为 OpenSkills 做出贡献的兴趣！本指南将帮助你开始。

## 开发设置

### 先决条件

- **Rust**：1.70+（通过 [rustup](https://rustup.rs/) 安装）
- **Node.js**：18+（用于 TypeScript 绑定）
- **Python**：3.8+（用于 Python 绑定，可选）
- **Git**：用于版本控制

### 从源码构建

```bash
# 克隆仓库
git clone <repository-url>
cd openskills

# 构建 Rust 运行时
cd runtime
cargo build --release

# 构建 TypeScript 绑定
cd ../bindings/ts
npm install
npm run build

# 构建 Python 绑定（可选）
cd ../python
pip install maturin
maturin develop
```

### 运行测试

```bash
# Rust 测试
cd runtime
cargo test

# TypeScript 测试（可用时）
cd bindings/ts
npm test

# Python 测试（可用时）
cd bindings/python
pytest
```

## 项目结构

```
openskills/
├── runtime/              # Rust 核心运行时
│   ├── src/
│   │   ├── lib.rs        # 公共 API
│   │   ├── registry.rs   # 技能发现
│   │   ├── manifest.rs   # SKILL.md 解析
│   │   ├── wasm_runner.rs # WASM 执行
│   │   └── ...
│   └── tests/            # 单元测试
├── bindings/
│   ├── ts/               # TypeScript 绑定
│   └── python/           # Python 绑定
├── examples/             # 示例技能
├── docs/                 # 文档
└── spec/                 # 规范
```

## 开发工作流

1. **Fork 和克隆**：Fork 仓库并克隆你的 fork
2. **创建分支**：创建功能分支（`git checkout -b feature/amazing-feature`）
3. **进行更改**：实现你的更改并附带测试
4. **测试**：确保所有测试通过（`cargo test`）
5. **文档**：根据需要更新文档
6. **提交**：编写清晰的提交消息
7. **推送**：推送到你的 fork（`git push origin feature/amazing-feature`）
8. **Pull Request**：打开 Pull Request

## 代码风格

### Rust

- 遵循 [Rust API 指南](https://rust-lang.github.io/api-guidelines/)
- 使用 `cargo fmt` 进行格式化
- 使用 `cargo clippy` 进行代码检查
- 为公共 API 编写文档注释

### TypeScript

- 使用 TypeScript 严格模式
- 遵循现有代码风格
- 为新 API 添加类型定义

### Python

- 遵循 PEP 8
- 使用类型提示
- 记录公共 API

## 测试指南

- 为新功能编写单元测试
- 提交 PR 前确保测试通过
- 为复杂功能添加集成测试
- 测试错误情况和边缘条件

## 文档

- 添加功能时更新相关文档
- 为新 API 添加代码示例
- 保持规范最新
- 为面向用户的更改更新 CHANGELOG.md

## 提交消息

遵循 conventional commits 格式：

```
type(scope): subject

body (optional)

footer (optional)
```

类型：`feat`、`fix`、`docs`、`style`、`refactor`、`test`、`chore`

示例：
```
feat(runtime): add support for custom skill directories

Allow users to specify custom skill directories beyond
standard locations. This enables better monorepo support.

Closes #123
```

## Pull Request 流程

1. **更新文档**：确保文档反映你的更改
2. **添加测试**：包含新功能的测试
3. **更新 CHANGELOG**：如果是面向用户的更改则添加条目
4. **检查 CI**：确保所有 CI 检查通过
5. **请求审查**：向维护者请求审查

## 贡献领域

### 高优先级

- **WASI Linker 集成**：完成 WASM 执行支持
- **测试覆盖率**：提高测试覆盖率
- **文档**：改进开发者文档
- **示例**：添加更多示例技能

### 中优先级

- **性能**：优化技能发现和执行
- **错误消息**：改进错误消息和诊断
- **CLI 改进**：增强 CLI 工具
- **绑定功能**：向绑定添加缺失的功能

### 低优先级

- **工具**：开发工具改进
- **CI/CD**：增强 CI/CD 管道
- **基准测试**：添加性能基准测试

## 获取帮助

- **问题**：为错误或功能请求开启问题
- **讨论**：使用 GitHub Discussions 提问
- **文档**：查看 [docs/](.) 获取详细指南

## 行为准则

在所有互动中保持尊重、包容和建设性。

## 许可证

通过贡献，你同意你的贡献将在 MIT 许可证下授权。
