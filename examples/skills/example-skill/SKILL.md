---
name: example-skill
description: Example skill demonstrating the Claude Skills format. This skill shows how to structure a skill that helps with code review tasks.
allowed-tools: Read, Grep
---

# Example Skill

This skill demonstrates the Claude Skills format for instructional skills.

## Overview

This is an example skill that shows the standard SKILL.md format. Most skills are
instructional - they provide guidance to the AI on how to perform specific tasks.

## Key Features

1. **Claude Skills Compatible**: Uses SKILL.md format with YAML frontmatter
2. **Instructional**: Provides clear instructions for the AI to follow
3. **Tool Permissions**: Declares which tools the skill needs via `allowed-tools`

## Usage

When this skill is activated, the AI will follow the instructions below to help
with code review tasks. The skill automatically has access to the tools listed
in `allowed-tools` without needing explicit permission.

## Instructions

When reviewing code:

1. Read the relevant files using the Read tool
2. Search for patterns using Grep if needed
3. Identify potential issues, improvements, or best practices
4. Provide constructive feedback

The sandboxing and security mechanisms are handled automatically by the runtime
and are transparent to skill authors.

## Directory Structure

```
example-skill/
├── SKILL.md           # This file (required)
├── examples/          # Optional example files
└── README.md          # Optional documentation
```
