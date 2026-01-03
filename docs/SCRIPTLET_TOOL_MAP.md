# Scriptlet Tool Map

This document describes all available codefence tools for scriptlets and how they map to SDK functions and executor behaviors.

## Overview

Scriptlets are defined in markdown files using fenced code blocks. The language identifier on the code fence determines which tool executes the scriptlet content.

**Example:**
```md
## My Snippet

```bash
echo "Hello, world!"
```
```

## Tool Categories

### Shell Tools

These execute in a shell environment.

| Tool | Executor | Description |
|------|----------|-------------|
| `bash` | `bash` | Bash shell (default on macOS/Linux) |
| `zsh` | `zsh` | Z shell |
| `sh` | `sh` | POSIX shell |
| `fish` | `fish` | Fish shell |
| `cmd` | `cmd.exe` | Windows CMD |
| `powershell` | `powershell.exe` | Windows PowerShell |
| `pwsh` | `pwsh` | PowerShell Core (cross-platform) |

### Interpreter Tools

These execute via language interpreters.

| Tool | Executor | File Extension | Description |
|------|----------|----------------|-------------|
| `python` | `python3` | `.py` | Python 3 script |
| `ruby` | `ruby` | `.rb` | Ruby script |
| `perl` | `perl` | `.pl` | Perl script |
| `php` | `php` | `.php` | PHP script |
| `node` | `node` | `.js` | JavaScript (Node.js) |
| `js` | `node` | `.js` | Alias for `node` |
| `applescript` | `osascript` | `.scpt` | macOS AppleScript |

### TypeScript Tools

These run via `bun` with SDK preload (interactive mode).

| Tool | Executor | SDK Available | Description |
|------|----------|---------------|-------------|
| `kit` | `bun` | Yes | Script Kit TypeScript (preferred) |
| `ts` | `bun` | Yes | TypeScript |
| `bun` | `bun` | Yes | Bun runtime TypeScript |
| `deno` | `bun` | Yes | Currently runs via bun (Deno support planned) |

**Note:** TypeScript tools run interactively and have full access to the Script Kit SDK. They can use prompts like `arg()`, `editor()`, `div()`, etc.

### Special Tools

These perform specific actions without running code through an interpreter.

| Tool | SDK Function | Behavior |
|------|--------------|----------|
| `template` | `template()` | Opens template prompt with VSCode-style tabstops |
| `transform` | N/A (custom) | Gets selected text, processes via script, replaces selection |
| `paste` | `setSelectedText()` | Pastes content directly to current cursor position |
| `type` | N/A (keyboard sim) | Types content character-by-character via keyboard simulation |
| `submit` | N/A | Like `paste` but also sends Enter key |
| `open` | `open()` | Opens URL or file path with system default handler |
| `edit` | `edit()` | Opens file in `$EDITOR` or VS Code |

## Tool-to-SDK Mapping Table

| Codefence Tool | SDK Function | Notes |
|----------------|--------------|-------|
| `template` | `template(content)` | Shows template editor with `{{placeholders}}` and `$1, $2` tabstops |
| `transform` | `getSelectedText()` + script + `setSelectedText()` | macOS only |
| `paste` | `setSelectedText(content)` | macOS only |
| `type` | Keyboard simulation | macOS only, simulates typing |
| `submit` | `setSelectedText(content)` + Enter | macOS only |
| `open` | `open(url)` or system `open`/`xdg-open` | Cross-platform |
| `edit` | `edit(path)` | Opens in editor |

## Template Tool Details

The `template` tool creates an interactive template editor where users can fill in placeholders.

### Syntax

**Mustache-style placeholders:**
```template
Hello {{name}}, welcome to {{company}}!
```

**VSCode-style tabstops:**
```template
function ${1:name}(${2:params}) {
  ${3:// body}
}
```

### Variable Substitution

Templates support automatic variable substitution:

| Variable | Replacement |
|----------|-------------|
| `$SELECTION` | Currently selected text (from any app) |
| `$CLIPBOARD` | Current clipboard contents |
| `$HOME` | User's home directory path |
| `$$` | Literal `$` character |

### Example Scriptlet

```md
## Email Template

```template
Subject: {{subject}}

Hi {{recipient}},

$SELECTION

Best regards,
{{sender}}
```
```

When triggered:
1. Content is processed (variables substituted)
2. Template prompt opens with tabstop navigation
3. User fills in placeholders using Tab/Shift+Tab
4. Enter submits the filled template

## Transform Tool Details

The `transform` tool creates a text processing pipeline:

1. Get selected text from any application
2. Run through the scriptlet code
3. Replace selection with output

### Example

```md
## Uppercase Selection

```transform
#!/bin/bash
tr '[:lower:]' '[:upper:]'
```
```

When triggered on selected text "hello", outputs "HELLO".

## Implementation Details

### Template Tool Flow

When a scriptlet with `template` codefence is executed:

1. **Executor** (`src/executor.rs`): Returns the processed content as `stdout`
2. **App** (`src/app_impl.rs`): Detects `tool == "template"` and calls `handle_prompt_message(ShowTemplate { ... })`
3. **Prompt Handler** (`src/prompt_handler.rs`): Creates a `TemplatePrompt` entity
4. **Template Prompt** (`src/prompts/template.rs`): Renders the tab-through editor UI
5. **On Submit**: The filled template is returned to the caller

This ensures scriptlet templates behave identically to the SDK's `template()` function.

## Source Code References

- Tool definitions: `src/scriptlets.rs:25-52`
- Executor dispatch: `src/executor.rs:1508-1557`
- Scriptlet execution: `src/app_impl.rs:execute_scriptlet()`
- Template handling: `src/app_impl.rs` (template tool special case)
- SDK `template()`: `scripts/kit-sdk.ts:3328-3362`
- Template prompt: `src/prompts/template.rs`
- Expand manager: `src/expand_manager.rs:319-331`

## Platform Support

| Tool | macOS | Linux | Windows |
|------|-------|-------|---------|
| `bash` | Yes | Yes | WSL/Git Bash |
| `zsh` | Yes | Yes | WSL |
| `sh` | Yes | Yes | WSL/Git Bash |
| `fish` | Yes | Yes | WSL |
| `cmd` | No | No | Yes |
| `powershell` | Via pwsh | Via pwsh | Yes |
| `pwsh` | Yes | Yes | Yes |
| `python` | Yes | Yes | Yes |
| `ruby` | Yes | Yes | Yes |
| `perl` | Yes | Yes | Yes |
| `php` | Yes | Yes | Yes |
| `node`/`js` | Yes | Yes | Yes |
| `kit`/`ts`/`bun` | Yes | Yes | Yes |
| `deno` | Yes | Yes | Yes |
| `applescript` | Yes | No | No |
| `template` | Yes | Yes | Yes |
| `transform` | Yes | No | No |
| `paste` | Yes | Partial | Partial |
| `type` | Yes | No | No |
| `submit` | Yes | No | No |
| `open` | Yes | Yes | Yes |
| `edit` | Yes | Yes | Yes |
