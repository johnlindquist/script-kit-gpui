# Input Validation Security Audit

## Executive Summary

This audit examines all user input handling across the Script Kit GPUI codebase, focusing on protocol message validation, user-provided paths, script metadata parsing, theme/config JSON parsing, and filter string handling.

**Overall Risk Level: MEDIUM**

The codebase demonstrates generally good practices with serde-based parsing and type safety, but several areas lack sufficient input validation and could be vulnerable to injection attacks or unexpected behavior.

| Category | Risk Level | Key Findings |
|----------|------------|--------------|
| Protocol Message Validation | LOW | Strong type system with serde, graceful unknown type handling |
| User-Provided Paths | MEDIUM | No path traversal protection, no length limits |
| Script Metadata Parsing | MEDIUM | Basic HTML comment parsing, no size limits |
| Theme/Config JSON Parsing | LOW | Good defaults, but shell command execution risk in config |
| Filter String Handling | LOW | Simple string operations, minimal risk |
| Scriptlet Variable Substitution | HIGH | Direct shell injection possible via positional args |

---

## 1. Protocol Message Validation

### Location: `src/protocol.rs`

### Analysis

The protocol layer uses serde's tagged enum deserialization with the `#[serde(tag = "type")]` attribute, which provides strong type safety.

#### Strengths

1. **Type-Safe Deserialization** (Lines 633-1323)
   - All message types are explicitly defined as enum variants
   - Unknown message types are gracefully handled via `ParseResult::UnknownType`
   - No arbitrary code execution from malformed JSON

2. **Graceful Error Handling** (Lines 2186-2234)
   ```rust
   pub fn parse_message_graceful(line: &str) -> ParseResult {
       // Single parse - parse to Value first, then convert
       // Handles unknown types without crashing
   }
   ```

3. **Semantic ID Generation** (Lines 557-627)
   - Input sanitization via `value_to_slug()` function
   - Removes non-alphanumeric characters
   - Truncates to 20 characters
   - Prevents injection via semantic IDs

#### Gaps

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| IV-P001 | LOW | `Choice.name` | No length validation on choice names |
| IV-P002 | LOW | `Choice.description` | No length validation on descriptions |
| IV-P003 | LOW | `Message::Div.html` | HTML content passed without sanitization |
| IV-P004 | MEDIUM | `Message::Exec.command` | Shell command passed directly to execution |
| IV-P005 | LOW | `Message::Browse.url` | URL passed to system open without validation |

#### Recommendations

1. Add length limits to string fields in `Choice`, `Field`, and HTML content
2. Consider URL validation for `browse` messages
3. Document that `exec` is intentionally powerful and requires trust in scripts

---

## 2. User-Provided Paths

### Location: `src/executor.rs`, `src/protocol.rs`

### Analysis

Path handling appears in several contexts: script execution, file search, and path prompts.

#### Identified Path Input Points

| Input Point | Location | Validation | Risk |
|-------------|----------|------------|------|
| Script path | `execute_script_interactive()` | Extension check only | MEDIUM |
| File search `only_in` | `Message::FileSearch` | None | MEDIUM |
| Path prompt `start_path` | `Message::Path` | None | LOW |
| Drop prompt paths | `Message::Drop` | None | LOW |
| SDK path | `find_sdk_path()` | Existence check only | LOW |
| Temp file paths | `execute_shell_scriptlet()` | Generated internally | LOW |

#### Gaps

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| IV-PA001 | MEDIUM | `execute_script_interactive()` | No path traversal protection (e.g., `../../etc/passwd`) |
| IV-PA002 | MEDIUM | `FileSearch.query` | Glob pattern passed to system without sanitization |
| IV-PA003 | LOW | `Message::Path.start_path` | No validation of path existence or permissions |
| IV-PA004 | LOW | All paths | No maximum path length enforcement |
| IV-PA005 | MEDIUM | `execute_open()` | Direct pass-through to `open` command |

#### Code Examples

```rust
// executor.rs:1518-1533 - execute_open() passes content directly to shell
fn execute_open(content: &str, _options: &ScriptletExecOptions) -> Result<...> {
    let target = content.trim();
    // No validation of target - could be file:// URL or shell metacharacters
    let output = Command::new(cmd_name)
        .arg(target)  // Direct pass-through
        .output()
```

#### Recommendations

1. Implement path canonicalization before processing
2. Add path traversal checks (reject paths containing `..`)
3. Enforce maximum path length (e.g., 4096 characters)
4. Validate URL schemes in `browse` and `open` commands

---

## 3. Script Metadata Parsing

### Location: `src/scriptlets.rs`

### Analysis

Scriptlets are parsed from markdown files with HTML comment metadata. The parsing is custom-built without a formal parser.

#### Parsing Functions

1. **`parse_html_comment_metadata()`** (Lines 179-226)
   - Extracts key-value pairs from HTML comments
   - Uses simple string splitting on `:`

2. **`extract_code_block_nested()`** (Lines 237-278)
   - Handles code fences (``` and ~~~)
   - Supports nested fences

3. **`parse_markdown_as_scriptlets()`** (Lines 323-392)
   - Main entry point for scriptlet parsing
   - Splits markdown by headers

#### Gaps

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| IV-SM001 | LOW | `parse_html_comment_metadata()` | No limit on comment size |
| IV-SM002 | LOW | `ScriptletMetadata.extra` | Unbounded HashMap for extra fields |
| IV-SM003 | MEDIUM | `extract_named_inputs()` | No limit on number of inputs extracted |
| IV-SM004 | LOW | `parse_markdown_as_scriptlets()` | No limit on file size or number of scriptlets |
| IV-SM005 | MEDIUM | Code content | Extracted code is not validated before execution |

#### Code Example

```rust
// scriptlets.rs:179-226 - No size limits on metadata parsing
pub fn parse_html_comment_metadata(text: &str) -> ScriptletMetadata {
    let mut metadata = ScriptletMetadata::default();
    
    while let Some(start) = remaining.find("<!--") {
        // No limit on iterations or content size
        if let Some(end) = remaining[start..].find("-->") {
            let comment_content = &remaining[start + 4..start + end];
            // Could be arbitrarily large
```

#### Recommendations

1. Add maximum size limits for markdown files (e.g., 1MB)
2. Limit the number of scriptlets per file
3. Limit the number of extra metadata fields
4. Add maximum length for metadata values

---

## 4. Theme/Config JSON Parsing

### Location: `src/theme.rs`, `src/config.rs`

### Analysis

Both theme and config files use serde JSON deserialization with sensible defaults.

#### Theme Loading (`src/theme.rs`)

**Strengths:**
- All color values are strongly typed as `HexColor` (u32)
- Defaults are provided for all fields via `#[serde(default)]`
- Invalid JSON falls back to system appearance detection
- Good error logging

**Code Example (Lines 786-865):**
```rust
pub fn load_theme() -> Theme {
    // Falls back gracefully on any error
    match serde_json::from_str::<Theme>(&contents) {
        Ok(theme) => { ... }
        Err(e) => {
            // Uses defaults, doesn't crash
            Theme { colors: ColorScheme::dark_default(), ... }
        }
    }
}
```

#### Config Loading (`src/config.rs`)

**Concern: Shell Command Execution (Lines 198-286)**

The config loading process executes `bun build` and `bun -e` with user-controlled file paths:

```rust
// config.rs:200-205 - Potential command injection via config path
let build_output = Command::new("bun")
    .arg("build")
    .arg("--target=bun")
    .arg(config_path.to_string_lossy().to_string())  // User-controlled path
    .arg(format!("--outfile={}", tmp_js_path))
    .output();
```

While the path is fixed (`~/.sk/kit/config.ts`), a malicious symlink or modified config.ts could execute arbitrary code.

#### Gaps

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| IV-TC001 | LOW | Theme opacity values | No clamping of opacity to 0.0-1.0 during load |
| IV-TC002 | MEDIUM | Config loading | Executes bun with config file - trusts file content |
| IV-TC003 | LOW | Theme file | No maximum file size check |
| IV-TC004 | LOW | `VibrancySettings.material` | String field could contain unexpected values |

#### Recommendations

1. Add explicit bounds checking for numeric values (opacity, padding)
2. Consider sandboxing config evaluation or using JSON-only config
3. Add file size limits before reading
4. Validate vibrancy material against known values

---

## 5. Filter String Handling

### Location: `src/prompts.rs`

### Analysis

The `ArgPrompt` implements a simple filter for searching choices. The implementation is safe.

#### Filter Implementation (Lines 84-94)

```rust
fn refilter(&mut self) {
    let filter_lower = self.input_text.to_lowercase();
    self.filtered_choices = self
        .choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| choice.name.to_lowercase().contains(&filter_lower))
        .map(|(idx, _)| idx)
        .collect();
    self.selected_index = 0;
}
```

**Strengths:**
- Uses simple string `contains()` operation
- No regex or pattern matching that could be exploited
- Case-insensitive comparison is safe

#### Gaps

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| IV-FS001 | LOW | `input_text` | No maximum length on filter input |
| IV-FS002 | LOW | Character input | No filtering of control characters |

#### Recommendations

1. Consider limiting filter string length to prevent memory issues with very long inputs
2. Filter out non-printable control characters

---

## 6. Scriptlet Variable Substitution (CRITICAL)

### Location: `src/scriptlets.rs`

### Analysis

The scriptlet system performs variable substitution that could lead to shell injection vulnerabilities.

#### Variable Substitution (Lines 472-518)

```rust
pub fn format_scriptlet(
    content: &str,
    inputs: &HashMap<String, String>,
    positional_args: &[String],
    windows: bool,
) -> String {
    // Named inputs - direct replacement, no escaping
    for (name, value) in inputs {
        let placeholder = format!("{{{{{}}}}}", name);
        result = result.replace(&placeholder, value);  // UNSAFE
    }
    
    // Positional args - direct replacement
    for (i, arg) in positional_args.iter().enumerate() {
        let placeholder = format!("${}", i + 1);
        result = result.replace(&placeholder, arg);  // UNSAFE
    }
```

#### Critical Vulnerability: Shell Injection

If a user provides input containing shell metacharacters, they will be interpreted by the shell:

**Attack Vector:**
1. Scriptlet contains: `echo "Hello $1"`
2. User provides arg: `"; rm -rf /; echo "`
3. Result: `echo "Hello "; rm -rf /; echo ""`

#### Gaps

| Finding | Severity | Location | Description |
|---------|----------|----------|-------------|
| IV-SV001 | **HIGH** | `format_scriptlet()` | No shell escaping for positional args |
| IV-SV002 | **HIGH** | `format_scriptlet()` | No shell escaping for named inputs |
| IV-SV003 | MEDIUM | `$@` / `%*` expansion | Quote escaping is minimal, not comprehensive |
| IV-SV004 | MEDIUM | Conditional processing | No limits on recursion depth |

#### The "Escaping" Code (Lines 495-500, 508-514)

```rust
// This escaping is insufficient for shell safety
let all_args = positional_args
    .iter()
    .map(|a| format!("\"{}\"", a.replace('\"', "\\\"")))
    .collect::<Vec<_>>()
    .join(" ");
```

This only escapes double quotes, not:
- Single quotes
- Backticks
- `$()` command substitution
- `;`, `|`, `&`, `>`, `<` metacharacters
- Newlines

#### Recommendations

1. **Implement proper shell escaping** using a library like `shell-escape`
2. Consider using a sandboxed execution environment
3. Add input validation to reject dangerous characters
4. Provide alternative substitution modes (safe vs. raw)

---

## 7. Risk Summary Matrix

| ID | Category | Severity | Description | Status |
|----|----------|----------|-------------|--------|
| IV-P001 | Protocol | LOW | No length validation on choice names | Open |
| IV-P002 | Protocol | LOW | No length validation on descriptions | Open |
| IV-P003 | Protocol | LOW | HTML content passed without sanitization | Open |
| IV-P004 | Protocol | MEDIUM | Shell command in exec message | By Design |
| IV-P005 | Protocol | LOW | URL passed without validation | Open |
| IV-PA001 | Paths | MEDIUM | No path traversal protection | Open |
| IV-PA002 | Paths | MEDIUM | Unsanitized glob patterns | Open |
| IV-PA003 | Paths | LOW | No path existence validation | Open |
| IV-PA004 | Paths | LOW | No path length limits | Open |
| IV-PA005 | Paths | MEDIUM | Direct shell pass-through in open | Open |
| IV-SM001 | Scriptlets | LOW | Unbounded comment size | Open |
| IV-SM002 | Scriptlets | LOW | Unbounded extra fields | Open |
| IV-SM003 | Scriptlets | MEDIUM | Unbounded input extraction | Open |
| IV-SM004 | Scriptlets | LOW | No file/scriptlet limits | Open |
| IV-SM005 | Scriptlets | MEDIUM | Unvalidated code content | By Design |
| IV-TC001 | Theme/Config | LOW | No opacity bounds checking | Open |
| IV-TC002 | Theme/Config | MEDIUM | Code execution in config load | By Design |
| IV-TC003 | Theme/Config | LOW | No theme file size limit | Open |
| IV-TC004 | Theme/Config | LOW | Unvalidated vibrancy material | Open |
| IV-FS001 | Filter | LOW | No filter length limit | Open |
| IV-FS002 | Filter | LOW | No control char filtering | Open |
| IV-SV001 | Substitution | **HIGH** | No shell escaping (positional) | **Critical** |
| IV-SV002 | Substitution | **HIGH** | No shell escaping (named) | **Critical** |
| IV-SV003 | Substitution | MEDIUM | Incomplete quote escaping | Open |
| IV-SV004 | Substitution | MEDIUM | Unbounded recursion | Open |

---

## 8. Immediate Action Items

### Critical (P0)

1. **IV-SV001/IV-SV002: Shell Injection in Scriptlets**
   - Implement proper shell escaping for all variable substitution
   - Consider using `shell-escape` crate or similar
   - Add tests for shell metacharacter handling

### High Priority (P1)

2. **IV-PA001: Path Traversal**
   - Add path canonicalization and validation
   - Reject paths containing `..` or starting with `/etc`, `/root`, etc.

3. **IV-PA002: Glob Pattern Injection**
   - Sanitize or validate glob patterns in file search

### Medium Priority (P2)

4. Add length limits to string inputs across the protocol
5. Implement file size limits for theme and scriptlet files
6. Add proper URL validation for browse/open commands

---

## 9. Security Design Considerations

### Trust Model

The current design implicitly trusts:
1. Scripts executed via `run` command
2. Config files in `~/.sk/kit/`
3. Theme files in `~/.sk/kit/`
4. Scriptlet files

This is reasonable for a power-user tool, but should be documented clearly.

### Recommended Future Improvements

1. **Content Security Policy for div/HTML prompts**
   - Consider sandboxing HTML content
   - Prevent script execution in displayed HTML

2. **Rate Limiting**
   - Consider rate limits on protocol messages
   - Prevent denial-of-service via message flooding

3. **Audit Logging**
   - Log all script executions with full paths
   - Log all system commands (open, exec, keyboard, mouse)

---

## Audit Metadata

| Field | Value |
|-------|-------|
| Audit Date | 2024-12-29 |
| Auditor | Security Audit Swarm Agent |
| Files Reviewed | protocol.rs, theme.rs, config.rs, prompts.rs, scriptlets.rs, executor.rs, utils.rs |
| Codebase Version | Current HEAD |
| Risk Rating | MEDIUM (with HIGH scriptlet injection vulnerability) |
