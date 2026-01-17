# Contributing to OpenSkills

Thank you for your interest in contributing to OpenSkills! This guide will help you get started.

## Development Setup

### Prerequisites

- **Rust**: 1.70+ (install via [rustup](https://rustup.rs/))
- **Node.js**: 18+ (for TypeScript bindings)
- **Python**: 3.8+ (for Python bindings, optional)
- **Git**: For version control

### Building from Source

```bash
# Clone the repository
git clone <repository-url>
cd openskills

# Build Rust runtime
cd runtime
cargo build --release

# Build TypeScript bindings
cd ../bindings/ts
npm install
npm run build

# Build Python bindings (optional)
cd ../python
pip install maturin
maturin develop
```

### Running Tests

```bash
# Rust tests
cd runtime
cargo test

# TypeScript tests (when available)
cd bindings/ts
npm test

# Python tests (when available)
cd bindings/python
pytest
```

## Project Structure

```
openskills/
├── runtime/              # Rust core runtime
│   ├── src/
│   │   ├── lib.rs        # Public API
│   │   ├── registry.rs   # Skill discovery
│   │   ├── manifest.rs   # SKILL.md parsing
│   │   ├── wasm_runner.rs # WASM execution
│   │   └── ...
│   └── tests/            # Unit tests
├── bindings/
│   ├── ts/               # TypeScript bindings
│   └── python/           # Python bindings
├── examples/              # Example skills
├── docs/                  # Documentation
└── spec/                  # Specification
```

## Development Workflow

1. **Fork and Clone**: Fork the repository and clone your fork
2. **Create Branch**: Create a feature branch (`git checkout -b feature/amazing-feature`)
3. **Make Changes**: Implement your changes with tests
4. **Test**: Ensure all tests pass (`cargo test`)
5. **Document**: Update documentation as needed
6. **Commit**: Write clear commit messages
7. **Push**: Push to your fork (`git push origin feature/amazing-feature`)
8. **Pull Request**: Open a pull request

## Code Style

### Rust

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Write doc comments for public APIs

### TypeScript

- Use TypeScript strict mode
- Follow existing code style
- Add type definitions for new APIs

### Python

- Follow PEP 8
- Use type hints
- Document public APIs

## Testing Guidelines

- Write unit tests for new features
- Ensure tests pass before submitting PR
- Add integration tests for complex features
- Test error cases and edge conditions

## Documentation

- Update relevant documentation when adding features
- Add code examples for new APIs
- Keep specification up to date
- Update CHANGELOG.md for user-facing changes

## Commit Messages

Follow conventional commits format:

```
type(scope): subject

body (optional)

footer (optional)
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

Example:
```
feat(runtime): add support for custom skill directories

Allow users to specify custom skill directories beyond
standard locations. This enables better monorepo support.

Closes #123
```

## Pull Request Process

1. **Update Documentation**: Ensure docs reflect your changes
2. **Add Tests**: Include tests for new functionality
3. **Update CHANGELOG**: Add entry if user-facing
4. **Check CI**: Ensure all CI checks pass
5. **Request Review**: Request review from maintainers

## Areas for Contribution

### High Priority

- **WASI Linker Integration**: Complete WASM execution support
- **Test Coverage**: Increase test coverage
- **Documentation**: Improve developer documentation
- **Examples**: Add more example skills

### Medium Priority

- **Performance**: Optimize skill discovery and execution
- **Error Messages**: Improve error messages and diagnostics
- **CLI Improvements**: Enhance CLI tooling
- **Binding Features**: Add missing features to bindings

### Low Priority

- **Tooling**: Development tooling improvements
- **CI/CD**: Enhance CI/CD pipeline
- **Benchmarks**: Add performance benchmarks

## Getting Help

- **Issues**: Open an issue for bugs or feature requests
- **Discussions**: Use GitHub Discussions for questions
- **Documentation**: Check [docs/](.) for detailed guides

## Code of Conduct

Be respectful, inclusive, and constructive in all interactions.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
