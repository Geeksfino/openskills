/**
 * Main entry point for skill-creator WASM module
 *
 * Reads input from stdin (primary) or SKILL_INPUT environment variable (fallback)
 * Outputs JSON result to stdout
 *
 * Note: In WASI/javy environment, stdin is the preferred input method
 */

import { initSkill } from './init_skill';
import { validateSkill } from './validate_skill';
import { packageSkill } from './package_skill';

interface SkillInput {
  action: 'init_skill' | 'validate_skill' | 'package_skill';
  [key: string]: any;
}

// Helper to read all input from stdin (works in javy WASI environment)
function readStdin(): string {
  try {
    // @ts-ignore - Javy global is available in javy-compiled WASM
    if (typeof Javy !== 'undefined' && Javy.IO && Javy.IO.readSync) {
      // Read stdin using Javy.IO.readSync(fd, buffer) where fd=0 is stdin
      const chunks: Uint8Array[] = [];
      const chunkSize = 1024;
      let totalBytes = 0;

      while (true) {
        const buffer = new Uint8Array(chunkSize);
        // @ts-ignore
        const bytesRead = Javy.IO.readSync(0, buffer);
        if (bytesRead === 0) break;
        chunks.push(buffer.subarray(0, bytesRead));
        totalBytes += bytesRead;
      }

      if (totalBytes > 0) {
        // Combine all chunks
        const result = new Uint8Array(totalBytes);
        let offset = 0;
        for (const chunk of chunks) {
          result.set(chunk, offset);
          offset += chunk.length;
        }
        // Decode as UTF-8
        const decoder = new TextDecoder();
        return decoder.decode(result).trim();
      }
    }
  } catch (e) {
    // Javy.IO not available, continue
  }

  // Fallback: try QuickJS std.in
  try {
    // @ts-ignore - std module might be available in some QuickJS environments
    const std = require('std');
    if (std && std.in) {
      const input = std.in.readAsString();
      if (input && input.trim()) {
        return input.trim();
      }
    }
  } catch (e) {
    // std module not available, continue
  }

  return '';
}

// Helper to get environment variable (fallback for testing)
function getEnv(name: string): string | undefined {
  // Try Node.js style first (for development/testing)
  if (typeof process !== 'undefined' && process.env) {
    return process.env[name];
  }
  // Try QuickJS/javy style (if std module is available)
  try {
    // @ts-ignore - std module may not be in TypeScript definitions
    const std = require('std');
    if (std && std.getenv) {
      return std.getenv(name);
    }
  } catch (e) {
    // std module not available, continue
  }
  return undefined;
}

function main() {
  try {
    // Try stdin first (preferred for WASI), then fall back to env var
    let skillInput = readStdin();
    if (!skillInput) {
      skillInput = getEnv('SKILL_INPUT') || '{}';
    }
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
