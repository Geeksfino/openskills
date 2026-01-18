# OpenSkills Documentation

Welcome to the OpenSkills documentation. This directory contains comprehensive documentation for developers, contributors, and users.

## Documentation Index

### For Users & Developers

- **[Developer Guide](developers.md)**: Complete guide to using OpenSkills Runtime in your applications
  - Quick start examples
  - API reference
  - Building skills
  - Best practices
  - Troubleshooting

- **[Specification](spec.md)**: Complete runtime specification
  - Skill format (SKILL.md)
  - Discovery locations
  - Progressive disclosure
  - WASM sandbox model
  - API contracts

### For Contributors

- **[Contributing Guide](contributing.md)**: How to contribute to OpenSkills
  - Development setup
  - Code style guidelines
  - Testing requirements
  - Pull request process

- **[Architecture](architecture.md)**: Internal architecture and design
  - Core components
  - Data flow
  - Security model
  - Extension points
  - Current implementation status

### Architecture Analysis

- **[Claude Skills Comparison](claude-skills-comparison.md)**: Detailed comparison with Claude Skills official architecture
  - Progressive disclosure implementation analysis
  - Feature-by-feature comparison
  - Gaps and recommendations
  - Implementation scores

- **[Architecture Comparison Diagrams](architecture-comparison.md)**: Visual architecture diagrams
  - Progressive disclosure flow diagrams
  - Context fork mechanism
  - Permission model comparison
  - Token optimization comparison

- **[Enhancement Proposals](enhancement-proposals.md)**: Concrete implementation proposals
  - System prompt metadata injection
  - Ask-before-act permission system
  - Context fork mechanism
  - Validation CLI tooling

## Quick Links

- **Getting Started**: See [Developer Guide - Quick Start](developers.md#quick-start)
- **Creating Skills**: See [Developer Guide - Building Skills](developers.md#building-skills)
- **API Reference**: See [Developer Guide - API Reference](developers.md#api-reference)
- **Contributing**: See [Contributing Guide](contributing.md)

## Additional Resources

- **Main README**: See [../README.md](../README.md) for project overview
- **Example Skills**: See [../examples/skills/](../examples/skills/) for example implementations
- **Build Scripts**: See [../scripts/](../scripts/) for build automation
