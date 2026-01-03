# Security Audit: Command Injection & Script Execution

**Audit Date:** 2024-12-29  
**Auditor:** Security Audit Worker  
**Files Reviewed:**
- `src/executor.rs` (2683 lines)
- `src/scripts.rs` (2000+ lines)
- `src/scriptlets.rs` (1425 lines)

**Risk Rating:** MEDIUM (with specific HIGH-severity findings)

---

## Executive Summary

This security audit examines the command injection and script execution attack surface in Script Kit GPUI. The application executes user TypeScript/JavaScript scripts via `bun` runtime with an SDK preload mechanism. The architecture involves IPC via stdin JSON messages between the GPUI Rust app and spawned `bun` processes.

**Key findings:** The codebase demonstrates generally sound security practices for a desktop application designed to execute user scripts. However, several areas require attention: (1) scriptlet variable substitution lacks input sanitization, enabling shell injection; (2) AppleScript execution passes unsanitized content; (3) environment variable handling could expose sensitive data; (4) temp file creation uses predictable paths. The design assumption that users execute their own trusted scripts is reasonable but should be documented, and untrusted scriptlet execution paths need hardening.

---

## Findings Summary

| ID | Severity | Title | Location | Status |
|----|----------|-------|----------|--------|
| SEC-001 | **HIGH** | Shell Injection via Scriptlet Variable Substitution | `executor.rs:1319-1365` | Open |
| SEC-002 | **HIGH** | AppleScript Injection via Type Command | `executor.rs:1596-1620` | Open |
| SEC-003 | **MEDIUM** | Predictable Temp File Paths | `executor.rs:1323-1324`, `1371-1373`, `1431-1432` | Open |
| SEC-004 | **MEDIUM** | Environment Variable Exposure | `executor.rs:32-70` | Open |
| SEC-005 | **LOW** | Path Traversal in Script Execution | `executor.rs:662-748` | Mitigated |
| SEC-006 | **INFO** | SDK Extraction to Fixed Path | `executor.rs:349-393` | Acceptable |
| SEC-007 | **MEDIUM** | Argument Escaping Insufficient for Shell Contexts | `scriptlets.rs:472-518` | Open |

---

## Detailed Findings

### SEC-001: Shell Injection via Scriptlet Variable Substitution

**Severity:** HIGH  
**Location:** `src/executor.rs:1319-1365` (`execute_shell_scriptlet`)  
**CWE:** CWE-78 (Improper Neutralization of Special Elements used in an OS Command)

#### Description

The `execute_shell_scriptlet` function writes user-provided content directly to a temporary shell script file and executes it. When scriptlet content contains user-controlled variables (via `{{variableName}}` substitution from `format_scriptlet`), these values are not sanitized before being interpolated into shell commands.

#### Vulnerable Code Pattern

```rust
// executor.rs:1319-1327
fn execute_shell_scriptlet(shell: &str, content: &str, options: &ScriptletExecOptions) -> Result<ScriptletResult, String> {
    logging::log("EXEC", &format!("Executing shell scriptlet with {}", shell));
    
    // Create temp file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("scriptlet-{}.sh", std::process::id()));
    
    std::fs::write(&temp_file, content)  // content is NOT sanitized
        .map_err(|e| format!("Failed to write temp script: {}", e))?;
```

The `content` parameter comes from scriptlet processing where user inputs are directly substituted:

```rust
// scriptlets.rs:480-484
fn format_scriptlet(...) -> String {
    let mut result = content.to_string();
    
    // Replace named inputs {{variableName}}
    for (name, value) in inputs {
        let placeholder = format!("{{{{{}}}}}", name);
        result = result.replace(&placeholder, value);  // NO SANITIZATION
    }
```

#### Attack Scenario

1. User creates a scriptlet: `echo Hello {{name}}!`
2. Attacker provides input: `; rm -rf / #` for the `name` variable
3. Resulting script: `echo Hello ; rm -rf / #!`
4. Shell executes destructive command

#### Remediation

```rust
/// Escape a string for safe use in shell commands
fn shell_escape(s: &str) -> String {
    // Use single quotes and escape any embedded single quotes
    format!("'{}'", s.replace('\'', "'\\''"))
}

// In format_scriptlet, apply escaping for shell tools:
fn format_scriptlet(
    content: &str,
    inputs: &HashMap<String, String>,
    positional_args: &[String],
    windows: bool,
    escape_for_shell: bool,  // NEW parameter
) -> String {
    let mut result = content.to_string();
    
    for (name, value) in inputs {
        let safe_value = if escape_for_shell {
            shell_escape(value)
        } else {
            value.clone()
        };
        let placeholder = format!("{{{{{}}}}}", name);
        result = result.replace(&placeholder, &safe_value);
    }
    // ...
}
```

---

### SEC-002: AppleScript Injection via Type Command

**Severity:** HIGH  
**Location:** `src/executor.rs:1596-1620`  
**CWE:** CWE-78 (OS Command Injection)

#### Description

The `execute_type` function constructs AppleScript commands by string concatenation with minimal escaping. The current escaping only handles backslashes and double quotes, but AppleScript has additional special characters and escape sequences that could be exploited.

#### Vulnerable Code Pattern

```rust
// executor.rs:1603-1612
fn execute_type(content: &str) -> Result<ScriptletResult, String> {
    let text = content.trim();
    
    // Use AppleScript to simulate typing
    let script = format!(
        r#"tell application "System Events" to keystroke "{}""#,
        text.replace('\\', "\\\\").replace('"', "\\\"")  // Insufficient escaping
    );
    
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
```

#### Attack Scenario

Content containing AppleScript control characters or multi-statement injection:

```
test" & (do shell script "malicious command") & "
```

Could result in:
```applescript
tell application "System Events" to keystroke "test" & (do shell script "malicious command") & ""
```

#### Remediation

```rust
/// Properly escape text for AppleScript string literals
fn applescript_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            _ if c.is_control() => {
                // Skip or escape control characters
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => result.push(c),
        }
    }
    result
}

// Apply in execute_type:
let safe_text = applescript_escape(text);
let script = format!(
    r#"tell application "System Events" to keystroke "{}""#,
    safe_text
);
```

Additionally, consider using AppleScript's `-s` flag for safer script handling or using Accessibility APIs directly instead of AppleScript.

---

### SEC-003: Predictable Temp File Paths

**Severity:** MEDIUM  
**Location:** `src/executor.rs:1323-1324`, `1371-1373`, `1431-1432`  
**CWE:** CWE-377 (Insecure Temporary File)

#### Description

Temporary script files are created with predictable paths based on the process ID:

```rust
let temp_file = temp_dir.join(format!("scriptlet-{}.sh", std::process::id()));
```

On multi-user systems or in race conditions, this could allow:
1. Symlink attacks where an attacker pre-creates a symlink at the predictable path
2. Information disclosure if temp files aren't properly cleaned up
3. Code injection if an attacker can predict and write to the file before execution

#### Remediation

Use secure temporary file creation:

```rust
use tempfile::NamedTempFile;

fn execute_shell_scriptlet(shell: &str, content: &str, options: &ScriptletExecOptions) -> Result<ScriptletResult, String> {
    // Create temp file with random name
    let temp_file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    
    std::fs::write(temp_file.path(), content)
        .map_err(|e| format!("Failed to write temp script: {}", e))?;
    
    // ... execute ...
    
    // File is automatically deleted when temp_file goes out of scope
}
```

Add `tempfile` to `Cargo.toml`:
```toml
[dependencies]
tempfile = "3.10"
```

---

### SEC-004: Environment Variable Exposure

**Severity:** MEDIUM  
**Location:** `src/executor.rs:32-70`  
**CWE:** CWE-526 (Exposure of Sensitive Information Through Environmental Variables)

#### Description

The AUTO_SUBMIT mode reads configuration from environment variables and could expose sensitive information in logs. More critically, spawned script processes inherit the full environment including potentially sensitive variables.

#### Vulnerable Pattern

```rust
// Scripts inherit full environment
let mut command = Command::new(&executable);
command
    .args(args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
// No .env_clear() or selective env passing
```

User scripts have access to all environment variables, which may include:
- API keys
- Database credentials
- AWS tokens
- Other sensitive configuration

#### Remediation

Consider providing an option to run scripts in a sanitized environment:

```rust
fn spawn_script(cmd: &str, args: &[&str], sanitize_env: bool) -> Result<ScriptSession, String> {
    let mut command = Command::new(&executable);
    command.args(args);
    
    if sanitize_env {
        // Only pass necessary variables
        command.env_clear();
        command.env("PATH", std::env::var("PATH").unwrap_or_default());
        command.env("HOME", std::env::var("HOME").unwrap_or_default());
        command.env("LANG", std::env::var("LANG").unwrap_or_default());
        // Add SCRIPT_KIT specific vars
        command.env("SCRIPT_KIT", "1");
    }
    
    // ...
}
```

Document the security implications for users who execute scripts from untrusted sources.

---

### SEC-005: Path Traversal in Script Execution

**Severity:** LOW (Mitigated by Design)  
**Location:** `src/executor.rs:662-748`  
**CWE:** CWE-22 (Path Traversal)

#### Description

The `execute_script_interactive` function accepts a `Path` parameter and executes it. However, this is largely mitigated by the application's design:

1. Scripts come from user input via stdin JSON messages
2. The primary attack vector would require compromising the JSON message source
3. The application is designed to run user-provided scripts by definition

#### Current Code

```rust
pub fn execute_script_interactive(path: &Path) -> Result<ScriptSession, String> {
    let path_str = path
        .to_str()
        .ok_or_else(|| "Invalid path encoding".to_string())?;
    // No path validation - by design
```

#### Analysis

This is an **acceptable risk** because:
1. Script Kit is explicitly designed to execute user scripts
2. Users are expected to run their own scripts from `~/.sk/kit/scripts/`
3. Adding path restrictions would break legitimate use cases

#### Recommendation (Defense in Depth)

For enhanced security in future versions, consider:

```rust
/// Optional: Validate script path is within allowed directories
fn is_allowed_script_path(path: &Path) -> bool {
    let allowed_bases = [
        dirs::home_dir().map(|h| h.join(".kenv")),
        Some(std::env::current_dir().unwrap_or_default()),
    ];
    
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    
    allowed_bases.iter().flatten().any(|base| {
        canonical.starts_with(base)
    })
}
```

---

### SEC-006: SDK Extraction to Fixed Path

**Severity:** INFO  
**Location:** `src/executor.rs:349-393`  
**CWE:** N/A (Informational)

#### Description

The embedded SDK is extracted to `~/.sk/kit/sdk/kit-sdk.ts` on every startup. This is a fixed, known location within the user's home directory.

```rust
fn ensure_sdk_extracted() -> Option<PathBuf> {
    let kenv_dir = dirs::home_dir()?.join(".kenv");
    let kenv_sdk = kenv_dir.join("sdk");
    let sdk_path = kenv_sdk.join("kit-sdk.ts");
    // ...
    if let Err(e) = std::fs::write(&sdk_path, EMBEDDED_SDK) {
```

#### Analysis

This is **acceptable** because:
1. The path is within the user's home directory
2. Directory permissions should protect against other users
3. The SDK content is compiled into the binary (trusted source)

#### Recommendation

Ensure the `~/.sk/kit` directory has appropriate permissions (700 or 750):

```rust
#[cfg(unix)]
fn ensure_kenv_permissions(kenv_dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(kenv_dir) {
        let mut perms = metadata.permissions();
        perms.set_mode(0o700);
        let _ = std::fs::set_permissions(kenv_dir, perms);
    }
}
```

---

### SEC-007: Argument Escaping Insufficient for Shell Contexts

**Severity:** MEDIUM  
**Location:** `src/scriptlets.rs:472-518`  
**CWE:** CWE-88 (Improper Neutralization of Argument Delimiters)

#### Description

The `format_scriptlet` function handles positional arguments and `$@` expansion, but the escaping is insufficient for complex shell contexts:

```rust
// scriptlets.rs:508-514
// Replace $@ with all args quoted
let all_args = positional_args
    .iter()
    .map(|a| format!("\"{}\"", a.replace('\"', "\\\"")))
    .collect::<Vec<_>>()
    .join(" ");
result = result.replace("$@", &all_args);
```

This escaping only handles double quotes, but doesn't account for:
- Backticks (\`)
- Dollar signs ($)
- Backslashes
- Newlines
- Null bytes

#### Attack Scenario

Input: `$(malicious_command)`

After "escaping": `"$(malicious_command)"`

When placed in a shell script with double quotes, command substitution still occurs.

#### Remediation

Use proper shell escaping:

```rust
/// Escape a string for use within double-quoted shell strings
fn escape_for_double_quotes(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '$' => result.push_str("\\$"),
            '`' => result.push_str("\\`"),
            '!' => result.push_str("\\!"),  // History expansion in bash
            '\n' => result.push_str("\\n"),
            '\0' => {} // Drop null bytes
            _ => result.push(c),
        }
    }
    result
}

// Or better, use single quotes where possible:
fn shell_quote(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
```

---

## Architecture Analysis

### Trust Model

Script Kit GPUI operates under the following trust assumptions:

1. **User scripts are trusted** - Scripts in `~/.sk/kit/scripts/` are created by the user
2. **SDK is trusted** - Embedded at compile time from the repository
3. **Scriptlets may be semi-trusted** - Could come from shared markdown files

### Attack Surface

```
┌─────────────────────────────────────────────────────────────┐
│                        User Interface                        │
│                    (GPUI Rust Application)                   │
└────────────────────────────┬────────────────────────────────┘
                             │ stdin JSON
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                      Script Execution                        │
│                   (bun --preload SDK)                        │
│   ┌─────────────────────────────────────────────────────┐   │
│   │  User Scripts (.ts/.js)                             │   │
│   │  - Full access to system                            │   │
│   │  - Inherit environment                              │   │
│   └─────────────────────────────────────────────────────┘   │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                     Scriptlet Execution                      │
│                (Shell/Python/Ruby/etc.)                      │
│   ┌─────────────────────────────────────────────────────┐   │
│   │  Variable Substitution ({{name}})  ← INJECTION RISK │   │
│   │  Conditionals ({{#if}})                             │   │
│   │  Positional Args ($1, $@)          ← INJECTION RISK │   │
│   └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Positive Security Practices Observed

1. **No shell=true** - Commands are executed directly without shell interpretation at the spawn level
2. **Process groups** - Proper cleanup via `process_group(0)` for orphan prevention
3. **Piped I/O** - stdin/stdout/stderr are properly captured
4. **Type-safe protocol** - JSON messages are parsed through defined structures
5. **No eval()** - No dynamic code evaluation in Rust layer

---

## Recommendations Summary

### Immediate Actions (HIGH Priority)

1. **Implement shell escaping** for scriptlet variable substitution
2. **Fix AppleScript escaping** in `execute_type` function
3. **Use secure temp file creation** with `tempfile` crate

### Short-Term Actions (MEDIUM Priority)

4. **Document security model** - Make trust assumptions explicit
5. **Add environment sanitization option** for sensitive deployments
6. **Review positional argument escaping** for edge cases

### Long-Term Considerations

7. **Consider sandboxing** for untrusted scriptlet execution
8. **Add content security policies** for scriptlets from external sources
9. **Implement audit logging** for script executions

---

## Conclusion

Script Kit GPUI demonstrates reasonable security practices for its intended purpose as a user script execution environment. The primary vulnerabilities exist in the scriptlet execution path where user-controlled input is interpolated into shell commands without proper sanitization. These issues should be addressed before accepting scriptlets from untrusted sources (e.g., shared scriptlet repositories).

The application's design correctly avoids common pitfalls like shell injection at the process spawn level by using `Command::new()` with separate arguments rather than shell string concatenation. However, the scriptlet templating system re-introduces these risks and requires hardening.

---

*Report generated by Security Audit Worker*
*Cell ID: cell--9bnr5-mjr5bz6oklm*
