/**
 * Initialize a new skill from template
 * 
 * Input: { skill_name: string, path: string }
 * Output: { success: boolean, message: string, skill_dir?: string }
 */

interface InitSkillInput {
  skill_name: string;
  path: string;
}

interface InitSkillOutput {
  success: boolean;
  message: string;
  skill_dir?: string;
  files?: Record<string, string>;
  instructions?: string[];
}

const SKILL_TEMPLATE = `---
name: {skill_name}
description: [TODO: Complete and informative explanation of what the skill does and when to use it. Include WHEN to use this skill - specific scenarios, file types, or tasks that trigger it.]
---

# {skill_title}

## Overview

[TODO: 1-2 sentences explaining what this skill enables]

## Structuring This Skill

[TODO: Choose the structure that best fits this skill's purpose. Common patterns:

**1. Workflow-Based** (best for sequential processes)
- Works well when there are clear step-by-step procedures
- Example: DOCX skill with "Workflow Decision Tree" → "Reading" → "Creating" → "Editing"
- Structure: ## Overview → ## Workflow Decision Tree → ## Step 1 → ## Step 2...

**2. Task-Based** (best for tool collections)
- Works well when the skill offers different operations/capabilities
- Example: PDF skill with "Quick Start" → "Merge PDFs" → "Split PDFs" → "Extract Text"
- Structure: ## Overview → ## Quick Start → ## Task Category 1 → ## Task Category 2...

**3. Reference/Guidelines** (best for standards or specifications)
- Works well for brand guidelines, coding standards, or requirements
- Example: Brand styling with "Brand Guidelines" → "Colors" → "Typography" → "Features"
- Structure: ## Overview → ## Guidelines → ## Specifications → ## Usage...

**4. Capabilities-Based** (best for integrated systems)
- Works well when the skill provides multiple interrelated features
- Example: Product Management with "Core Capabilities" → numbered capability list
- Structure: ## Overview → ## Core Capabilities → ### 1. Feature → ### 2. Feature...

Patterns can be mixed and matched as needed. Most skills combine patterns (e.g., start with task-based, add workflow for complex operations).

Delete this entire "Structuring This Skill" section when done - it's just guidance.]

## [TODO: Replace with the first main section based on chosen structure]

[TODO: Add content here. See examples in existing skills:
- Code samples for technical skills
- Decision trees for complex workflows
- Concrete examples with realistic user requests
- References to scripts/templates/references as needed]

## Resources

This skill includes example resource directories that demonstrate how to organize different types of bundled resources:

### scripts/
Executable code (Python/Bash/etc.) that can be run directly to perform specific operations.

**Examples from other skills:**
- PDF skill: \`fill_fillable_fields.py\`, \`extract_form_field_info.py\` - utilities for PDF manipulation
- DOCX skill: \`document.py\`, \`utilities.py\` - Python modules for document processing

**Appropriate for:** Python scripts, shell scripts, or any executable code that performs automation, data processing, or specific operations.

**Note:** Scripts may be executed without loading into context, but can still be read by Claude for patching or environment adjustments.

### references/
Documentation and reference material intended to be loaded into context to inform Claude's process and thinking.

**Examples from other skills:**
- Product management: \`communication.md\`, \`context_building.md\` - detailed workflow guides
- BigQuery: API reference documentation and query examples
- Finance: Schema documentation, company policies

**Appropriate for:** In-depth documentation, API references, database schemas, comprehensive guides, or any detailed information that Claude should reference while working.

### assets/
Files not intended to be loaded into context, but rather used within the output Claude produces.

**Examples from other skills:**
- Brand styling: PowerPoint template files (.pptx), logo files
- Frontend builder: HTML/React boilerplate project directories
- Typography: Font files (.ttf, .woff2)

**Appropriate for:** Templates, boilerplate code, document templates, images, icons, fonts, or any files meant to be copied or used in the final output.

---

**Any unneeded directories can be deleted.** Not every skill requires all three types of resources.`;

const EXAMPLE_SCRIPT = `#!/usr/bin/env python3
"""
Example helper script for {skill_name}

This is a placeholder script that can be executed directly.
Replace with actual implementation or delete if not needed.

Example real scripts from other skills:
- pdf/scripts/fill_fillable_fields.py - Fills PDF form fields
- pdf/scripts/convert_pdf_to_images.py - Converts PDF pages to images
"""

def main():
    print("This is an example script for {skill_name}")
    # TODO: Add actual script logic here
    # This could be data processing, file conversion, API calls, etc.

if __name__ == "__main__":
    main()
`;

const EXAMPLE_REFERENCE = `# Reference Documentation for {skill_title}

This is a placeholder for detailed reference documentation.
Replace with actual reference content or delete if not needed.

Example real reference docs from other skills:
- product-management/references/communication.md - Comprehensive guide for status updates
- product-management/references/context_building.md - Deep-dive on gathering context
- bigquery/references/ - API references and query examples

## When Reference Docs Are Useful

Reference docs are ideal for:
- Comprehensive API documentation
- Detailed workflow guides
- Complex multi-step processes
- Information too lengthy for main SKILL.md
- Content that's only needed for specific use cases

## Structure Suggestions

### API Reference Example
- Overview
- Authentication
- Endpoints with examples
- Error codes
- Rate limits

### Workflow Guide Example
- Prerequisites
- Step-by-step instructions
- Common patterns
- Troubleshooting
- Best practices
`;

const EXAMPLE_ASSET = `# Example Asset File

This placeholder represents where asset files would be stored.
Replace with actual asset files (templates, images, fonts, etc.) or delete if not needed.

Asset files are NOT intended to be loaded into context, but rather used within
the output Claude produces.

Example asset files from other skills:
- Brand guidelines: logo.png, slides_template.pptx
- Frontend builder: hello-world/ directory with HTML/React boilerplate
- Typography: custom-font.ttf, font-family.woff2
- Data: sample_data.csv, test_dataset.json

## Common Asset Types

- Templates: .pptx, .docx, boilerplate directories
- Images: .png, .jpg, .svg, .gif
- Fonts: .ttf, .otf, .woff, .woff2
- Boilerplate code: Project directories, starter files
- Icons: .ico, .svg
- Data files: .csv, .json, .xml, .yaml

Note: This is a text placeholder. Actual assets can be any file type.
`;

function titleCaseSkillName(skillName: string): string {
  return skillName
    .split('-')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
    .join(' ');
}

export function initSkill(input: InitSkillInput): InitSkillOutput {
  try {
    // Validate skill name
    if (!input.skill_name || !/^[a-z0-9-]+$/.test(input.skill_name)) {
      return {
        success: false,
        message: `Invalid skill name: "${input.skill_name}". Must be hyphen-case (lowercase letters, digits, and hyphens only)`,
      };
    }

    if (input.skill_name.startsWith('-') || input.skill_name.endsWith('-') || input.skill_name.includes('--')) {
      return {
        success: false,
        message: `Invalid skill name: "${input.skill_name}". Cannot start/end with hyphen or contain consecutive hyphens`,
      };
    }

    if (input.skill_name.length > 64) {
      return {
        success: false,
        message: `Skill name is too long (${input.skill_name.length} characters). Maximum is 64 characters.`,
      };
    }

    // Note: In WASM environment, we can't directly access filesystem
    // This function returns instructions for the runtime to execute
    const skillTitle = titleCaseSkillName(input.skill_name);
    const skillContent = SKILL_TEMPLATE
      .replace(/{skill_name}/g, input.skill_name)
      .replace(/{skill_title}/g, skillTitle);

    const exampleScriptContent = EXAMPLE_SCRIPT.replace(/{skill_name}/g, input.skill_name);
    const exampleReferenceContent = EXAMPLE_REFERENCE.replace(/{skill_title}/g, skillTitle);

    return {
      success: true,
      message: `Skill '${input.skill_name}' initialization instructions generated`,
      skill_dir: `${input.path}/${input.skill_name}`,
      // Include file contents for runtime to write
      files: {
        'SKILL.md': skillContent,
        'scripts/example.py': exampleScriptContent,
        'references/api_reference.md': exampleReferenceContent,
        'assets/example_asset.txt': EXAMPLE_ASSET,
      },
      instructions: [
        `Create directory: ${input.path}/${input.skill_name}`,
        `Create SKILL.md with template content`,
        `Create scripts/ directory with example.py`,
        `Create references/ directory with api_reference.md`,
        `Create assets/ directory with example_asset.txt`,
        `Set executable permission on scripts/example.py`,
      ],
    };
  } catch (error) {
    return {
      success: false,
      message: `Error initializing skill: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}
