"""
Pre-built tools for OpenSkills runtime integration with Python agent frameworks.

This module provides ready-to-use tool definitions for LangChain and other frameworks.

Usage:
    from openskills import OpenSkillRuntime
    from openskills_tools import create_langchain_tools, get_agent_system_prompt
    
    runtime = OpenSkillRuntime.from_directory("./skills")
    runtime.discover_skills()
    
    tools = create_langchain_tools(runtime, workspace_dir="./output")
    # Use with LangChain agent
"""

import os
import json
import mimetypes
import fnmatch
from pathlib import Path
from typing import Optional, List, Any, Dict, Callable


# ============================================================================
# Shared Helper Functions
# ============================================================================

def is_path_within_workspace(workspace: Path, relative_path: str) -> bool:
    """
    Validate that a path is within the workspace directory.
    
    This prevents directory traversal attacks by ensuring the resolved path
    is actually within the workspace, not just a string prefix match.
    
    Args:
        workspace: The workspace directory Path
        relative_path: The relative path to validate
        
    Returns:
        True if the path is within the workspace, False otherwise
    """
    resolved_workspace = workspace.resolve()
    resolved_path = (workspace / relative_path).resolve()
    
    # If paths are equal, it's valid
    if resolved_path == resolved_workspace:
        return True
    
    # Use Path.relative_to() or is_relative_to() to check if path is within workspace
    # This is more robust than string prefix matching
    try:
        # Python 3.9+ has is_relative_to() which is cleaner and more efficient
        if hasattr(resolved_path, 'is_relative_to'):
            return resolved_path.is_relative_to(resolved_workspace)
        else:
            # Fallback for older Python: use relative_to()
            # If it succeeds, the path is within the workspace
            # If it raises ValueError, the path is outside the workspace
            try:
                resolved_path.relative_to(resolved_workspace)
                return True
            except ValueError:
                # Path is not relative to workspace (outside or different root)
                return False
    except Exception:
        # Safety fallback: use normalized string comparison with separator
        # Ensure workspace path ends with separator for proper prefix check
        workspace_str = str(resolved_workspace) + os.sep
        path_str = str(resolved_path)
        return path_str.startswith(workspace_str)


def walk_directory(
    dir_path: Path,
    base_path: Path,
    recursive: bool,
    pattern: Optional[str],
    files: List[Dict[str, Any]]
) -> None:
    """
    Recursively walk a directory and collect file information.
    
    Args:
        dir_path: The directory to walk
        base_path: Base path for relative path calculation
        recursive: Whether to walk recursively
        pattern: Optional glob pattern to filter files
        files: List to append file information to
    """
    try:
        for entry in dir_path.iterdir():
            rel_path = base_path / entry.name if base_path else Path(entry.name)
            if entry.is_dir():
                if recursive:
                    walk_directory(entry, rel_path, recursive, pattern, files)
            else:
                # Apply pattern filter if provided
                if pattern and not fnmatch.fnmatch(entry.name, pattern):
                    continue
                
                stat = entry.stat()
                files.append({
                    "path": str(rel_path),
                    "size": stat.st_size,
                    "modified": stat.st_mtime,
                })
    except PermissionError:
        pass  # Skip directories we can't access


def get_mime_types() -> Dict[str, str]:
    """
    Get the MIME types dictionary for common file extensions.
    
    Returns:
        Dictionary mapping file extensions to MIME types
    """
    return {
        '.docx': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
        '.xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
        '.pptx': 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
        '.pdf': 'application/pdf',
        '.png': 'image/png',
        '.jpg': 'image/jpeg',
        '.jpeg': 'image/jpeg',
        '.gif': 'image/gif',
        '.svg': 'image/svg+xml',
        '.txt': 'text/plain',
        '.md': 'text/markdown',
        '.json': 'application/json',
        '.html': 'text/html',
        '.css': 'text/css',
        '.js': 'application/javascript',
        '.ts': 'application/typescript',
    }


def format_bytes(bytes_size: int) -> str:
    """
    Format bytes into human-readable string (Bytes, KB, MB, GB).
    
    Args:
        bytes_size: Size in bytes
        
    Returns:
        Human-readable size string
    """
    if bytes_size == 0:
        return "0 Bytes"
    
    k = 1024
    sizes = ["Bytes", "KB", "MB", "GB"]
    i = 0
    while bytes_size >= k and i < len(sizes) - 1:
        bytes_size /= k
        i += 1
    return f"{round(bytes_size * 100) / 100} {sizes[i]}"


def get_file_info_impl(workspace: Path, path: str) -> Dict[str, Any]:
    """
    Get file information (shared implementation).
    
    Args:
        workspace: The workspace directory Path
        path: Relative path within the workspace
        
    Returns:
        Dictionary with file information
        
    Raises:
        ValueError: If path escapes workspace directory
        FileNotFoundError: If file doesn't exist
    """
    # Security: ensure path is within workspace (prevents directory traversal)
    if not is_path_within_workspace(workspace, path):
        raise ValueError(f"Path {path} escapes workspace directory")
    
    full_path = workspace / path
    if not full_path.exists():
        raise FileNotFoundError(f"File not found: {path}")
    
    stat = full_path.stat()
    ext = full_path.suffix.lower()
    mime_types = get_mime_types()
    
    return {
        "path": path,
        "fullPath": str(full_path),
        "size": stat.st_size,
        "sizeHuman": format_bytes(stat.st_size),
        "type": mime_types.get(ext, mimetypes.guess_type(str(full_path))[0] or 'application/octet-stream'),
        "extension": ext,
        "modified": stat.st_mtime,
    }


# ============================================================================
# Public API Functions
# ============================================================================

def get_agent_system_prompt(runtime) -> str:
    """
    Get a skill-agnostic system prompt from the runtime.
    
    This returns a complete system prompt that teaches the agent how to use
    Claude Skills without any skill-specific knowledge.
    
    Args:
        runtime: The OpenSkills runtime instance
        
    Returns:
        A complete system prompt for skill-based agents
    """
    skills = runtime.list_skills()
    
    if not skills:
        return "No skills are currently available."
    
    prompt = """You have access to Claude Skills that provide specialized capabilities.

## Available Skills

"""
    
    for skill in skills:
        prompt += f"- **{skill['id']}**: {skill['description']}\n"
    
    prompt += """
## How to Use Skills

When a user's request matches a skill's capabilities:

1. **Activate the skill**: Call `activate_skill(skill_id)` to load the full SKILL.md instructions
2. **Read the instructions carefully**: The SKILL.md contains everything you need to know
3. **Follow the instructions exactly**: Execute the steps as described in SKILL.md
4. **Use helper files if referenced**: Call `read_skill_file(skill_id, path)` to read referenced docs
5. **Run scripts as instructed**: Call `run_skill_script(skill_id, script_path, args)` when needed

## Important

- Each skill's SKILL.md contains all the knowledge you need - do NOT assume prior knowledge
- Output files are written to the workspace directory

## File Output and Delivery

When you generate files (documents, images, etc.):

1. **Files are written to the workspace directory** (available as SKILL_WORKSPACE environment variable)
2. **After generating files**, use `list_workspace_files()` to discover what was created
3. **Use `get_file_info(path)`** to get file details (size, type, MIME type) for your response
4. **Mention files in your response** so the user knows what was created
5. **Include file paths and types** in your final response

Example response:
"I've created a Word document for you: 'output/document.docx' (45 KB, Word document)"
"""
    
    return prompt


def create_langchain_tools(runtime, workspace_dir: Optional[str] = None) -> List[Any]:
    """
    Create LangChain-compatible tools for OpenSkills runtime.
    
    Args:
        runtime: The OpenSkills runtime instance
        workspace_dir: Directory for file I/O operations (default: current directory)
        
    Returns:
        List of LangChain Tool objects
        
    Raises:
        ImportError: If langchain is not installed
    """
    try:
        from langchain.tools import Tool, StructuredTool  # type: ignore[import-untyped]
        from langchain.pydantic_v1 import BaseModel, Field  # type: ignore[import-untyped]
    except ImportError:
        raise ImportError(
            "create_langchain_tools requires 'langchain' package. "
            "Install with: pip install langchain"
        )
    
    workspace = Path(workspace_dir) if workspace_dir else Path.cwd()
    workspace.mkdir(parents=True, exist_ok=True)
    
    # Schema definitions
    class ListSkillsInput(BaseModel):
        query: Optional[str] = Field(default=None, description="Optional search query to filter skills")
    
    class ActivateSkillInput(BaseModel):
        skill_id: str = Field(description="The skill ID to activate")
    
    class ReadSkillFileInput(BaseModel):
        skill_id: str = Field(description="The skill ID")
        path: str = Field(description="Relative path within the skill directory")
    
    class ListSkillFilesInput(BaseModel):
        skill_id: str = Field(description="The skill ID")
        subdir: Optional[str] = Field(default=None, description="Optional subdirectory")
        recursive: bool = Field(default=False, description="List recursively")
    
    class RunSkillScriptInput(BaseModel):
        skill_id: str = Field(description="The skill ID")
        script_path: str = Field(description="Path to the script relative to skill root")
        args: List[str] = Field(default=[], description="Arguments to pass to the script")
        timeout_ms: int = Field(default=30000, description="Timeout in milliseconds")
    
    class WriteFileInput(BaseModel):
        path: str = Field(description="Relative path within the workspace")
        content: str = Field(description="File content to write")
    
    class ReadFileInput(BaseModel):
        path: str = Field(description="Relative path within the workspace")
    
    class ListWorkspaceFilesInput(BaseModel):
        subdir: Optional[str] = Field(default=None, description="Optional subdirectory to list")
        recursive: bool = Field(default=False, description="List recursively")
        pattern: Optional[str] = Field(default=None, description="Optional glob pattern to filter files (e.g., '*.docx')")
    
    class GetFileInfoInput(BaseModel):
        path: str = Field(description="Relative path within the workspace")
    
    # Tool implementations
    def list_skills(query: Optional[str] = None) -> str:
        skills = runtime.list_skills()
        if query:
            skills = [s for s in skills if query.lower() in s['id'].lower() or query.lower() in s['description'].lower()]
        return json.dumps([{"id": s['id'], "description": s['description']} for s in skills], indent=2)
    
    def activate_skill(skill_id: str) -> str:
        try:
            loaded = runtime.activate_skill(skill_id)
            return json.dumps({
                "id": loaded['id'],
                "name": loaded.get('name', loaded['id']),
                "allowed_tools": loaded.get('allowed_tools', []),
                "instructions": loaded['instructions'],
            })
        except Exception as e:
            return f"Error activating skill {skill_id}: {e}"
    
    def read_skill_file(skill_id: str, path: str) -> str:
        try:
            return runtime.read_skill_file(skill_id, path)
        except Exception as e:
            return f"Error reading {path} from skill {skill_id}: {e}"
    
    def list_skill_files(skill_id: str, subdir: Optional[str] = None, recursive: bool = False) -> str:
        try:
            files = runtime.list_skill_files(skill_id, subdir, recursive)
            return json.dumps(files, indent=2)
        except Exception as e:
            return f"Error listing files in skill {skill_id}: {e}"
    
    def run_skill_script(skill_id: str, script_path: str, args: List[str] = None, timeout_ms: int = 30000) -> str:
        try:
            result = runtime.run_skill_target(skill_id, {
                "target_type": "script",
                "path": script_path,
                "args": args or [],
                "timeout_ms": timeout_ms,
            })
            return json.dumps({
                "stdout": result.get('stdout', ''),
                "stderr": result.get('stderr', ''),
                "output": result.get('output', {}),
            }, indent=2)
        except Exception as e:
            return f"Error running {script_path} from skill {skill_id}: {e}"
    
    def write_file(path: str, content: str) -> str:
        try:
            # Security: ensure path is within workspace (prevents directory traversal)
            if not is_path_within_workspace(workspace, path):
                return f"Error: Path {path} escapes workspace directory"
            full_path = workspace / path
            full_path.parent.mkdir(parents=True, exist_ok=True)
            full_path.write_text(content, encoding='utf-8')
            return f"Successfully wrote {len(content)} bytes to {path}"
        except Exception as e:
            return f"Error writing file: {e}"
    
    def read_file(path: str) -> str:
        try:
            # Security: ensure path is within workspace (prevents directory traversal)
            if not is_path_within_workspace(workspace, path):
                return f"Error: Path {path} escapes workspace directory"
            full_path = workspace / path
            if not full_path.exists():
                return f"Error: File not found: {path}"
            return full_path.read_text(encoding='utf-8')
        except Exception as e:
            return f"Error reading file: {e}"
    
    def list_workspace_files(subdir: Optional[str] = None, recursive: bool = False, pattern: Optional[str] = None) -> str:
        try:
            target_dir = workspace / subdir if subdir else workspace
            if not target_dir.exists():
                return json.dumps({"files": [], "error": "Directory not found"})
            
            files = []
            walk_directory(
                target_dir,
                Path(subdir) if subdir else Path(""),
                recursive,
                pattern,
                files
            )
            return json.dumps({"files": files}, indent=2)
        except Exception as e:
            return f"Error listing workspace files: {e}"
    
    def get_file_info(path: str) -> str:
        try:
            file_info = get_file_info_impl(workspace, path)
            return json.dumps(file_info, indent=2)
        except (ValueError, FileNotFoundError) as e:
            return f"Error: {e}"
        except Exception as e:
            return f"Error getting file info: {e}"
    
    # Create and return tools
    return [
        StructuredTool.from_function(
            func=list_skills,
            name="list_skills",
            description="List all available Claude Skills with their IDs and descriptions.",
            args_schema=ListSkillsInput,
        ),
        StructuredTool.from_function(
            func=activate_skill,
            name="activate_skill",
            description="Activate a Claude Skill to get its full SKILL.md instructions.",
            args_schema=ActivateSkillInput,
        ),
        StructuredTool.from_function(
            func=read_skill_file,
            name="read_skill_file",
            description="Read a file from a skill directory. Use this to read helper files referenced in SKILL.md.",
            args_schema=ReadSkillFileInput,
        ),
        StructuredTool.from_function(
            func=list_skill_files,
            name="list_skill_files",
            description="List files in a skill directory to discover available resources.",
            args_schema=ListSkillFilesInput,
        ),
        StructuredTool.from_function(
            func=run_skill_script,
            name="run_skill_script",
            description="Run a Python or Shell script from a skill directory in a sandbox.",
            args_schema=RunSkillScriptInput,
        ),
        StructuredTool.from_function(
            func=write_file,
            name="write_file",
            description="Write a file to the workspace directory.",
            args_schema=WriteFileInput,
        ),
        StructuredTool.from_function(
            func=read_file,
            name="read_file",
            description="Read a file from the workspace directory.",
            args_schema=ReadFileInput,
        ),
        StructuredTool.from_function(
            func=list_workspace_files,
            name="list_workspace_files",
            description="List all files in the workspace directory. Use this to discover files generated by skills.",
            args_schema=ListWorkspaceFilesInput,
        ),
        StructuredTool.from_function(
            func=get_file_info,
            name="get_file_info",
            description="Get information about a file in the workspace (size, type, path). Use this to reference files in your response.",
            args_schema=GetFileInfoInput,
        ),
    ]


def create_simple_tools(runtime, workspace_dir: Optional[str] = None) -> Dict[str, callable]:
    """
    Create simple callable tools for OpenSkills runtime.
    
    This returns a dict of functions that can be used with any agent framework.
    
    Args:
        runtime: The OpenSkills runtime instance
        workspace_dir: Directory for file I/O operations (default: current directory)
        
    Returns:
        Dict mapping tool names to callable functions
    """
    workspace = Path(workspace_dir) if workspace_dir else Path.cwd()
    workspace.mkdir(parents=True, exist_ok=True)
    
    def list_skills(query: Optional[str] = None) -> List[Dict]:
        skills = runtime.list_skills()
        if query:
            skills = [s for s in skills if query.lower() in s['id'].lower() or query.lower() in s['description'].lower()]
        return [{"id": s['id'], "description": s['description']} for s in skills]
    
    def activate_skill(skill_id: str) -> Dict:
        loaded = runtime.activate_skill(skill_id)
        return {
            "id": loaded['id'],
            "name": loaded.get('name', loaded['id']),
            "allowed_tools": loaded.get('allowed_tools', []),
            "instructions": loaded['instructions'],
        }
    
    def read_skill_file(skill_id: str, path: str) -> str:
        return runtime.read_skill_file(skill_id, path)
    
    def list_skill_files(skill_id: str, subdir: Optional[str] = None, recursive: bool = False) -> List[str]:
        return runtime.list_skill_files(skill_id, subdir, recursive)
    
    def write_file(path: str, content: str) -> str:
        # Security: ensure path is within workspace (prevents directory traversal)
        if not is_path_within_workspace(workspace, path):
            raise ValueError(f"Path {path} escapes workspace directory")
        full_path = workspace / path
        full_path.parent.mkdir(parents=True, exist_ok=True)
        full_path.write_text(content, encoding='utf-8')
        return f"Wrote {len(content)} bytes to {path}"
    
    def read_file(path: str) -> str:
        # Security: ensure path is within workspace (prevents directory traversal)
        if not is_path_within_workspace(workspace, path):
            raise ValueError(f"Path {path} escapes workspace directory")
        full_path = workspace / path
        return full_path.read_text(encoding='utf-8')
    
    def list_workspace_files(subdir: Optional[str] = None, recursive: bool = False, pattern: Optional[str] = None) -> List[Dict]:
        target_dir = workspace / subdir if subdir else workspace
        if not target_dir.exists():
            return []
        
        files = []
        walk_directory(
            target_dir,
            Path(subdir) if subdir else Path(""),
            recursive,
            pattern,
            files
        )
        return files
    
    def get_file_info(path: str) -> Dict:
        return get_file_info_impl(workspace, path)
    
    return {
        "list_skills": list_skills,
        "activate_skill": activate_skill,
        "read_skill_file": read_skill_file,
        "list_skill_files": list_skill_files,
        "write_file": write_file,
        "read_file": read_file,
        "list_workspace_files": list_workspace_files,
        "get_file_info": get_file_info,
    }
