/**
 * Validate a skill's structure and metadata
 * 
 * Input: { skill_path: string, skill_md_content?: string, frontmatter?: object }
 * Output: { valid: boolean, message: string, errors?: string[] }
 */

interface ValidateSkillInput {
  skill_path: string;
  skill_md_content?: string;
  frontmatter?: Record<string, any>;
}

interface ValidateSkillOutput {
  valid: boolean;
  message: string;
  errors?: string[];
}

const ALLOWED_PROPERTIES = new Set(['name', 'description', 'license', 'allowed-tools', 'metadata']);

function extractFrontmatter(content: string): { frontmatter?: Record<string, any>; error?: string } {
  if (!content.startsWith('---')) {
    return { error: 'SKILL.md must start with --- (YAML frontmatter)' };
  }

  const endMatch = content.match(/^---\n(.*?)\n---/s);
  if (!endMatch) {
    return { error: 'Invalid frontmatter format: must be enclosed in --- markers' };
  }

  const frontmatterText = endMatch[1];
  
  // Simple YAML parsing (for demo - in production, use a proper YAML parser)
  // This is a simplified parser for the WASM environment
  try {
    const frontmatter: Record<string, any> = {};
    const lines = frontmatterText.split('\n');
    let currentKey = '';
    let currentValue = '';
    let inMultiline = false;

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith('#')) continue;

      if (trimmed.includes(':')) {
        if (currentKey) {
          frontmatter[currentKey] = currentValue.trim();
        }
        const colonIndex = trimmed.indexOf(':');
        currentKey = trimmed.substring(0, colonIndex).trim();
        currentValue = trimmed.substring(colonIndex + 1).trim();
        inMultiline = false;
      } else if (currentKey && (trimmed.startsWith('-') || trimmed.startsWith('|'))) {
        // Multiline value
        currentValue += '\n' + trimmed;
        inMultiline = true;
      } else if (currentKey) {
        currentValue += ' ' + trimmed;
      }
    }

    if (currentKey) {
      frontmatter[currentKey] = currentValue.trim();
    }

    return { frontmatter };
  } catch (error) {
    return { error: `Failed to parse YAML frontmatter: ${error instanceof Error ? error.message : String(error)}` };
  }
}

export function validateSkill(input: ValidateSkillInput): ValidateSkillOutput {
  const errors: string[] = [];

  try {
    let frontmatter: Record<string, any> | undefined = input.frontmatter;

    // If skill_md_content is provided, extract frontmatter from it
    if (input.skill_md_content && !frontmatter) {
      const extracted = extractFrontmatter(input.skill_md_content);
      if (extracted.error) {
        errors.push(extracted.error);
        return {
          valid: false,
          message: 'Validation failed',
          errors,
        };
      }
      frontmatter = extracted.frontmatter;
    }

    if (!frontmatter) {
      return {
        valid: false,
        message: 'Cannot validate: frontmatter not provided. Provide either skill_md_content or frontmatter in input.',
        errors: ['Missing frontmatter data'],
      };
    }

    // Check required fields
    if (!frontmatter.name) {
      errors.push('Missing required field: name');
    } else {
      if (typeof frontmatter.name !== 'string') {
        errors.push('Field "name" must be a string');
      } else {
        const name = frontmatter.name.trim();
        if (!/^[a-z0-9-]+$/.test(name)) {
          errors.push(`Name "${name}" should be hyphen-case (lowercase letters, digits, and hyphens only)`);
        }
        if (name.startsWith('-') || name.endsWith('-') || name.includes('--')) {
          errors.push(`Name "${name}" cannot start/end with hyphen or contain consecutive hyphens`);
        }
        if (name.length > 64) {
          errors.push(`Name is too long (${name.length} characters). Maximum is 64 characters.`);
        }
      }
    }

    if (!frontmatter.description) {
      errors.push('Missing required field: description');
    } else {
      if (typeof frontmatter.description !== 'string') {
        errors.push('Field "description" must be a string');
      } else {
        const desc = frontmatter.description.trim();
        if (desc.includes('<') || desc.includes('>')) {
          errors.push('Description cannot contain angle brackets (< or >)');
        }
        if (desc.length > 1024) {
          errors.push(`Description is too long (${desc.length} characters). Maximum is 1024 characters.`);
        }
      }
    }

    // Check for unexpected properties
    for (const key of Object.keys(frontmatter)) {
      if (!ALLOWED_PROPERTIES.has(key)) {
        errors.push(`Unexpected key "${key}" in frontmatter. Allowed properties are: ${Array.from(ALLOWED_PROPERTIES).join(', ')}`);
      }
    }

    if (errors.length > 0) {
      return {
        valid: false,
        message: 'Validation failed',
        errors,
      };
    }

    return {
      valid: true,
      message: 'Skill is valid!',
    };
  } catch (error) {
    return {
      valid: false,
      message: `Error during validation: ${error instanceof Error ? error.message : String(error)}`,
      errors: [error instanceof Error ? error.message : String(error)],
    };
  }
}
