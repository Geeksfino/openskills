/**
 * Main entry point for skill-creator WASM module
 * 
 * Reads input from SKILL_INPUT environment variable and routes to appropriate function
 * Outputs JSON result to stdout
 * 
 * Note: In javy/QuickJS environment, environment variables are accessed via std.env
 */

import { initSkill } from './init_skill';
import { validateSkill } from './validate_skill';
import { packageSkill } from './package_skill';

interface SkillInput {
  action: 'init_skill' | 'validate_skill' | 'package_skill';
  [key: string]: any;
}

// Helper to get environment variable (works in both Node.js and QuickJS/javy)
function getEnv(name: string): string | undefined {
  // Try Node.js style first (for development/testing)
  if (typeof process !== 'undefined' && process.env) {
    return process.env[name];
  }
  // Try QuickJS/javy style (if std module is available)
  try {
    // @ts-ignore - std module may not be in TypeScript definitions
    const std = require('std');
    if (std && std.env) {
      return std.env[name];
    }
  } catch (e) {
    // std module not available, continue
  }
  return undefined;
}

function main() {
  try {
    // Read input from environment variable (set by OpenSkills runtime)
    const skillInput = getEnv('SKILL_INPUT') || '{}';
    const input: SkillInput = JSON.parse(skillInput);

    if (!input.action) {
      const error = {
        success: false,
        error: 'Missing "action" field. Must be one of: init_skill, validate_skill, package_skill',
      };
      console.log(JSON.stringify(error));
      return;
    }

    let result: any;

    switch (input.action) {
      case 'init_skill':
        if (!input.skill_name || !input.path) {
          result = {
            success: false,
            error: 'Missing required fields: skill_name, path',
          };
        } else {
          result = initSkill({
            skill_name: input.skill_name,
            path: input.path,
          });
        }
        break;

      case 'validate_skill':
        if (!input.skill_path) {
          result = {
            valid: false,
            error: 'Missing required field: skill_path',
          };
        } else {
          result = validateSkill({
            skill_path: input.skill_path,
          });
        }
        break;

      case 'package_skill':
        if (!input.skill_path) {
          result = {
            success: false,
            error: 'Missing required field: skill_path',
          };
        } else {
          result = packageSkill({
            skill_path: input.skill_path,
            output_dir: input.output_dir,
          });
        }
        break;

      default:
        result = {
          success: false,
          error: `Unknown action: ${input.action}. Must be one of: init_skill, validate_skill, package_skill`,
        };
    }

    // Output JSON result to stdout (captured by OpenSkills runtime)
    console.log(JSON.stringify(result));
  } catch (error) {
    const errorResult = {
      success: false,
      error: error instanceof Error ? error.message : String(error),
    };
    console.log(JSON.stringify(errorResult));
  }
}

// Execute main function
main();
