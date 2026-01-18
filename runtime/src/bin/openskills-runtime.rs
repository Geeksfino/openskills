//! OpenSkills Runtime CLI
//!
//! Claude Skills compatible runtime with WASM sandbox.

use openskills_runtime::{
    analyze_skill_tokens, validate_skill_path, ExecutionOptions, OpenSkillRuntime,
};
use serde_json::Value;
use std::env;
use std::fs;
use std::process;

fn print_usage() {
    eprintln!("OpenSkills Runtime - Claude Skills compatible with WASM sandbox");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  openskills discover [--project-root <path>]");
    eprintln!("  openskills list [--dir <path>]");
    eprintln!("  openskills activate <skill-id> [--dir <path>]");
    eprintln!("  openskills execute <skill-id> [options]");
    eprintln!("  openskills validate <skill-path> [options]");
    eprintln!("  openskills analyze <skill-path> [options]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  discover      Discover skills from standard locations (~/.claude/skills/, .claude/skills/)");
    eprintln!("  list          List skills from a specific directory");
    eprintln!("  activate      Load full skill content (SKILL.md instructions)");
    eprintln!("  execute       Execute a skill's WASM module in sandbox");
    eprintln!("  validate      Validate a skill's format and structure");
    eprintln!("  analyze       Analyze token usage for a skill");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --project-root, -p   Project root for relative path resolution");
    eprintln!("  --dir, -d            Skills directory (for list/activate)");
    eprintln!("  --input, -i          Input JSON string (for execute)");
    eprintln!("  --input-file, -f     Input JSON file path (for execute)");
    eprintln!("  --timeout-ms, -t     Timeout in ms (for execute)");
    eprintln!("  --warnings           Show validation warnings");
    eprintln!("  --json               Output as JSON");
    eprintln!("  --help, -h           Show help");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let command = &args[1];

    match command.as_str() {
        "discover" => cmd_discover(&args[2..]),
        "list" => cmd_list(&args[2..]),
        "activate" => cmd_activate(&args[2..]),
        "execute" => cmd_execute(&args[2..]),
        "validate" => cmd_validate(&args[2..]),
        "analyze" => cmd_analyze(&args[2..]),
        "--help" | "-h" => {
            print_usage();
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
            process::exit(1);
        }
    }
}

fn cmd_discover(args: &[String]) {
    let mut project_root: Option<String> = None;
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--project-root" | "-p" => {
                i += 1;
                project_root = args.get(i).cloned();
            }
            "--json" => {
                json_output = true;
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let mut runtime = match project_root {
        Some(ref root) => OpenSkillRuntime::with_project_root(root),
        None => OpenSkillRuntime::new(),
    };

    match runtime.discover_skills() {
        Ok(skills) => {
            if json_output {
                println!("{}", serde_json::to_string_pretty(&skills).unwrap_or_default());
            } else {
                if skills.is_empty() {
                    println!("No skills found.");
                    println!("Skills are discovered from:");
                    println!("  - ~/.claude/skills/ (personal)");
                    println!("  - .claude/skills/ (project)");
                    println!("  - Nested .claude/skills/ directories");
                } else {
                    println!("Discovered {} skill(s):", skills.len());
                    for s in skills {
                        let invocable = if s.user_invocable { "" } else { " [hidden]" };
                        println!("  {} ({:?}){}", s.id, s.location, invocable);
                        println!("    {}", s.description);
                    }
                }
            }
        }
        Err(err) => {
            eprintln!("Error discovering skills: {}", err);
            process::exit(1);
        }
    }
}

fn cmd_list(args: &[String]) {
    let mut dir = ".".to_string();
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" | "-d" => {
                i += 1;
                dir = args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("Missing value for --dir");
                    process::exit(1);
                });
            }
            "--json" => {
                json_output = true;
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let runtime = OpenSkillRuntime::from_directory(&dir);
    let skills = runtime.list_skills();

    if json_output {
        println!("{}", serde_json::to_string_pretty(&skills).unwrap_or_default());
    } else {
        if skills.is_empty() {
            println!("No skills found in {}", dir);
        } else {
            println!("Skills in {}:", dir);
            for s in skills {
                println!("  {}: {}", s.id, s.description);
            }
        }
    }
}

fn cmd_activate(args: &[String]) {
    let mut skill_id: Option<String> = None;
    let mut dir: Option<String> = None;
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" | "-d" => {
                i += 1;
                dir = args.get(i).cloned();
            }
            "--json" => {
                json_output = true;
            }
            arg if !arg.starts_with('-') && skill_id.is_none() => {
                skill_id = Some(arg.to_string());
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let skill_id = skill_id.unwrap_or_else(|| {
        eprintln!("Missing skill ID");
        print_usage();
        process::exit(1);
    });

    let mut runtime = match dir {
        Some(ref d) => OpenSkillRuntime::from_directory(d),
        None => OpenSkillRuntime::new(),
    };

    // Discover if using standard locations
    if dir.is_none() {
        if let Err(e) = runtime.discover_skills() {
            eprintln!("Error discovering skills: {}", e);
            process::exit(1);
        }
    }

    match runtime.activate_skill(&skill_id) {
        Ok(loaded) => {
            if json_output {
                let output = serde_json::json!({
                    "id": loaded.id,
                    "name": loaded.manifest.name,
                    "description": loaded.manifest.description,
                    "allowed_tools": loaded.manifest.get_allowed_tools(),
                    "model": loaded.manifest.model,
                    "context": loaded.manifest.context,
                    "agent": loaded.manifest.agent,
                    "user_invocable": loaded.manifest.is_user_invocable(),
                    "location": loaded.location,
                    "instructions": loaded.instructions
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
            } else {
                println!("Skill: {}", loaded.id);
                println!("Description: {}", loaded.manifest.description);
                if !loaded.manifest.get_allowed_tools().is_empty() {
                    println!("Allowed tools: {}", loaded.manifest.get_allowed_tools().join(", "));
                }
                if let Some(ref model) = loaded.manifest.model {
                    println!("Model: {}", model);
                }
                if loaded.manifest.is_forked() {
                    println!("Context: fork");
                    if let Some(ref agent) = loaded.manifest.agent {
                        println!("Agent: {}", agent);
                    }
                }
                println!();
                println!("--- Instructions ---");
                println!("{}", loaded.instructions);
            }
        }
        Err(err) => {
            eprintln!("Error activating skill '{}': {}", skill_id, err);
            process::exit(1);
        }
    }
}

fn cmd_execute(args: &[String]) {
    let mut skill_id: Option<String> = None;
    let mut dir: Option<String> = None;
    let mut input_json: Option<String> = None;
    let mut input_file: Option<String> = None;
    let mut timeout_ms: Option<u64> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" | "-d" => {
                i += 1;
                dir = args.get(i).cloned();
            }
            "--input" | "-i" => {
                i += 1;
                input_json = args.get(i).cloned();
            }
            "--input-file" | "-f" => {
                i += 1;
                input_file = args.get(i).cloned();
            }
            "--timeout-ms" | "-t" => {
                i += 1;
                timeout_ms = args.get(i).and_then(|v| v.parse().ok());
            }
            arg if !arg.starts_with('-') && skill_id.is_none() => {
                skill_id = Some(arg.to_string());
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let skill_id = skill_id.unwrap_or_else(|| {
        eprintln!("Missing skill ID");
        print_usage();
        process::exit(1);
    });

    // Parse input
    let input_str = if let Some(file) = input_file {
        fs::read_to_string(&file).unwrap_or_else(|err| {
            eprintln!("Failed to read input file: {}", err);
            process::exit(1);
        })
    } else {
        input_json.unwrap_or_else(|| "{}".to_string())
    };

    let input: Value = serde_json::from_str(&input_str).unwrap_or_else(|err| {
        eprintln!("Invalid input JSON: {}", err);
        process::exit(1);
    });

    let mut runtime = match dir {
        Some(ref d) => OpenSkillRuntime::from_directory(d),
        None => OpenSkillRuntime::new(),
    };

    // Discover if using standard locations
    if dir.is_none() {
        if let Err(e) = runtime.discover_skills() {
            eprintln!("Error discovering skills: {}", e);
            process::exit(1);
        }
    }

    let options = ExecutionOptions {
        timeout_ms,
        memory_mb: None,
        input: Some(input),
    };

    match runtime.execute_skill(&skill_id, options) {
        Ok(result) => {
            println!("{}", serde_json::to_string_pretty(&result.output).unwrap_or_default());
            if !result.stdout.is_empty() {
                eprintln!("[stdout]\n{}", result.stdout);
            }
            if !result.stderr.is_empty() {
                eprintln!("[stderr]\n{}", result.stderr);
            }
        }
        Err(err) => {
            eprintln!("Execution failed: {}", err);
            process::exit(1);
        }
    }
}

fn cmd_validate(args: &[String]) {
    let mut skill_path: Option<String> = None;
    let mut json_output = false;
    let mut show_warnings = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_output = true;
            }
            "--warnings" => {
                show_warnings = true;
            }
            arg if !arg.starts_with('-') && skill_path.is_none() => {
                skill_path = Some(arg.to_string());
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let skill_path = skill_path.unwrap_or_else(|| {
        eprintln!("Missing skill path");
        print_usage();
        process::exit(1);
    });

    let result = validate_skill_path(std::path::Path::new(&skill_path));

    if json_output {
        let output = serde_json::json!({
            "path": skill_path,
            "valid": result.errors.is_empty(),
            "errors": result.errors,
            "warnings": result.warnings,
            "stats": result.stats,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
    } else {
        if result.errors.is_empty() {
            println!("Validation passed: {}", skill_path);
        } else {
            println!("Validation failed: {}", skill_path);
        }

        if let Some(ref stats) = result.stats {
            println!("Name: {} ({} chars)", stats.name, stats.name_len);
            println!(
                "Description: {} chars",
                stats.description_len
            );
            println!(
                "Instructions: {} chars",
                stats.instructions_len
            );
        }

        if !result.errors.is_empty() {
            println!();
            println!("Errors:");
            for err in &result.errors {
                println!("  - {}", err);
            }
        }

        if show_warnings && !result.warnings.is_empty() {
            println!();
            println!("Warnings:");
            for warn in &result.warnings {
                println!("  - {}", warn);
            }
        }
    }

    if !result.errors.is_empty() {
        process::exit(1);
    }
}

fn cmd_analyze(args: &[String]) {
    let mut skill_path: Option<String> = None;
    let mut json_output = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_output = true;
            }
            arg if !arg.starts_with('-') && skill_path.is_none() => {
                skill_path = Some(arg.to_string());
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let skill_path = skill_path.unwrap_or_else(|| {
        eprintln!("Missing skill path");
        print_usage();
        process::exit(1);
    });

    let analysis = analyze_skill_tokens(std::path::Path::new(&skill_path));

    if json_output {
        println!("{}", serde_json::to_string_pretty(&analysis).unwrap_or_default());
    } else {
        println!("Token Analysis: {}", skill_path);
        println!();
        if let Some(error) = analysis.error.as_ref() {
            println!("Error: {}", error);
            process::exit(1);
        }

        println!("Tier 1 (Metadata):");
        println!("  Name:        {} chars", analysis.name_len);
        println!("  Description: {} chars", analysis.description_len);
        println!("  Estimated:   ~{} tokens", analysis.tier1_tokens);
        println!();
        println!("Tier 2 (Instructions):");
        println!("  Length:      {} chars", analysis.instructions_len);
        println!("  Estimated:   ~{} tokens", analysis.tier2_tokens);
        println!();
        println!("Total:");
        println!("  Estimated:   ~{} tokens", analysis.total_tokens);

        if analysis.tier1_tokens > 150 {
            println!();
            println!("Warning: Tier 1 is large, consider shortening description.");
        }
        if analysis.tier2_tokens > 1500 {
            println!("Warning: Tier 2 is large, consider moving content to references.");
        }
    }
}
