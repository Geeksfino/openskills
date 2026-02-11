# Git Submodules - Do Not Modify Directly

## Rule

**NEVER modify files or directories within git submodules directly in the openskills repository.**

## Affected Directories

The following directories are git submodules and should **NOT** be modified directly:

- `examples/finskills` - External FinSkills repository
- `examples/claude-official-skills` - External Claude official skills repository
- Any other directory that is a git submodule (check with `git submodule status`)

## Why

Git submodules are pointers to external repositories. Modifying them directly:
- Creates local changes that don't belong to the openskills project
- Can cause confusion and merge conflicts
- Breaks the separation of concerns between repositories
- Makes it difficult to track changes properly

## Correct Workflow

1. **Identify the submodule**: Check which repository the submodule points to
2. **Navigate to the actual repository**: Work in the standalone repository (not the submodule directory)
3. **Make changes there**: Commit changes in the external repository
4. **Update the submodule reference**: In openskills, update the submodule to point to the new commit

## Example

If you need to modify `examples/finskills`:

```bash
# ❌ WRONG - Don't do this:
cd examples/finskills
# make changes and commit

# ✅ CORRECT - Do this instead:
cd /path/to/finskills/repo  # The actual repository
# make changes and commit
# Then in openskills:
cd examples/finskills
git pull  # Update submodule to latest
```

## Verification

Before making changes, always check:
- `git submodule status` - Lists all submodules
- `git status` in the directory - Shows if it's a submodule
- The directory's `.git` file - If it exists and points to a parent `.git/modules/`, it's a submodule
