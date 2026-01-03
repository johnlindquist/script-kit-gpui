# Security Audit: File System Access

**Audit Date:** 2024-12-29  
**Auditor:** filesystem-auditor  
**Scope:** `src/file_search.rs`, `src/config.rs`, `src/watcher.rs`  
**Risk Level:** MEDIUM-HIGH

---

## Executive Summary

This audit examines file system access patterns in three critical modules:
1. **file_search.rs** - Spotlight-based file search using `mdfind`
2. **config.rs** - Configuration loading via TypeScript execution
3. **watcher.rs** - Directory watching for config/theme/script changes

**Key Findings:**
- **CRITICAL:** Config loading executes arbitrary TypeScript code via `bun`
- **HIGH:** No path canonicalization or symlink resolution in file operations
- **MEDIUM:** File watchers don't validate paths before emitting events
- **LOW:** TOCTOU race conditions in file metadata operations

---

## Findings Summary Table

| ID | Severity | Component | Finding | Status |
|----|----------|-----------|---------|--------|
| FS-001 | CRITICAL | config.rs | Arbitrary code execution via config.ts | Open |
| FS-002 | HIGH | file_search.rs | No path traversal protection in onlyin parameter | Open |
| FS-003 | HIGH | config.rs | Predictable temp file path (/tmp/kit-config.js) | Open |
| FS-004 | MEDIUM | file_search.rs | Symlink following without validation | Open |
| FS-005 | MEDIUM | watcher.rs | No path validation on watched directories | Open |
| FS-006 | MEDIUM | watcher.rs | Potential symlink race in script watching | Open |
| FS-007 | LOW | file_search.rs | TOCTOU race in metadata retrieval | Open |
| FS-008 | LOW | file_search.rs | Unbounded mdfind output processing | Open |

---

## Detailed Findings

### FS-001: Arbitrary Code Execution via Config Loading (CRITICAL)

**Location:** `src/config.rs`, lines 188-287

**Description:**  
The `load_config()` function executes arbitrary TypeScript code from `~/.sk/kit/config.ts` via two separate `bun` invocations:

```rust
// Step 1: Transpile TypeScript
let build_output = Command::new("bun")
    .arg("build")
    .arg("--target=bun")
    .arg(config_path.to_string_lossy().to_string())
    .arg(format!("--outfile={}", tmp_js_path))
    .output();

// Step 2: Execute the transpiled code
let json_output = Command::new("bun")
    .arg("-e")
    .arg(format!(
        "console.log(JSON.stringify(require('{}').default))",
        tmp_js_path
    ))
    .output();
```

**Attack Vectors:**
1. **Malicious config.ts:** If an attacker can write to `~/.sk/kit/config.ts`, they gain arbitrary code execution with user privileges
2. **Import chain attacks:** config.ts can import other modules, allowing code injection through dependencies
3. **Tilde expansion bypass:** The path uses `shellexpand::tilde()` which could be manipulated in non-standard HOME scenarios

**Impact:** Complete system compromise with user privileges

**Recommendation:**
- Consider a JSON-only config format that doesn't require code execution
- If TypeScript is required, sandbox the bun execution
- Add integrity checking (e.g., checksum validation) before execution
- Validate the config file ownership matches the running user

---

### FS-002: No Path Traversal Protection in File Search (HIGH)

**Location:** `src/file_search.rs`, lines 152-231

**Description:**  
The `search_files()` function passes the `onlyin` parameter directly to `mdfind` without validation:

```rust
pub fn search_files(query: &str, onlyin: Option<&str>, limit: usize) -> Vec<FileResult> {
    // ...
    if let Some(dir) = onlyin {
        cmd.arg("-onlyin").arg(dir);  // No validation!
    }
    cmd.arg(query);  // Query passed directly
    // ...
}
```

**Attack Vectors:**
1. **Directory traversal:** A malicious `onlyin` value like `/etc` or `/private` could expose system files
2. **Symlink traversal:** If `onlyin` points to a symlink, it could resolve outside intended boundaries
3. **Query injection:** While `Command` handles argument escaping, the query could contain Spotlight-specific operators that expand search scope

**Impact:** Information disclosure, access to files outside intended scope

**Recommendation:**
- Canonicalize `onlyin` path and verify it's under allowed directories
- Implement an allowlist of searchable directories
- Add path prefix checking after canonicalization
- Consider sandboxing mdfind results to the user's home directory

---

### FS-003: Predictable Temp File Path (HIGH)

**Location:** `src/config.rs`, line 199

**Description:**  
The transpiled JavaScript is written to a hardcoded, predictable path:

```rust
let tmp_js_path = "/tmp/kit-config.js";
```

**Attack Vectors:**
1. **Symlink attack:** An attacker could create `/tmp/kit-config.js` as a symlink to a sensitive file, causing bun to overwrite it
2. **Race condition:** Between writing and reading the temp file, another process could modify it
3. **Multi-user systems:** Any user can read/write to `/tmp`, potentially injecting malicious code

**Impact:** Arbitrary file overwrite, code injection

**Recommendation:**
- Use `tempfile` crate for secure temporary file creation
- Generate unique, unpredictable filenames
- Set restrictive permissions (mode 0600) on the temp file
- Clean up temp files immediately after use

```rust
// Recommended approach
use tempfile::NamedTempFile;
let tmp_file = NamedTempFile::new()?;
// Use tmp_file.path() instead of hardcoded path
```

---

### FS-004: Symlink Following in File Metadata (MEDIUM)

**Location:** `src/file_search.rs`, lines 247-296

**Description:**  
The `get_file_metadata()` function uses `std::fs::metadata()` which follows symlinks:

```rust
pub fn get_file_metadata(path: &str) -> Option<FileMetadata> {
    let path_obj = Path::new(path);
    let metadata = match std::fs::metadata(path_obj) {  // Follows symlinks!
        Ok(m) => m,
        // ...
    };
    // ...
}
```

**Attack Vectors:**
1. **Symlink dereference:** A symlink in search results could point to sensitive files, leaking metadata
2. **Symlink to device files:** Could cause hangs or resource exhaustion if symlinks point to `/dev/` entries
3. **Circular symlinks:** Could cause infinite loops (though metadata() handles this)

**Impact:** Information disclosure, potential DoS

**Recommendation:**
- Use `symlink_metadata()` first to check if path is a symlink
- Implement `lstat()` equivalent checks before following
- Consider using `canonicalize()` and validating the resolved path is within allowed boundaries

---

### FS-005: No Path Validation in File Watchers (MEDIUM)

**Location:** `src/watcher.rs`, lines 82-167, 222-307, 362-449

**Description:**  
File watchers accept paths from `shellexpand::tilde()` without validation:

```rust
fn watch_loop(tx: Sender<ConfigReloadEvent>) -> NotifyResult<()> {
    let config_path = PathBuf::from(
        shellexpand::tilde("~/.sk/kit/config.ts").as_ref()
    );
    
    let watch_path = config_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));  // Fallback to current dir!
    
    // ...
    watcher.watch(watch_path, RecursiveMode::NonRecursive)?;
}
```

**Attack Vectors:**
1. **HOME manipulation:** If `$HOME` is set to an attacker-controlled directory, the watcher monitors unintended paths
2. **Fallback to current directory:** The `.` fallback could watch arbitrary directories
3. **Missing directory handling:** If `~/.sk/kit` doesn't exist, no error is raised

**Impact:** Monitoring of unintended directories, potential trigger for malicious reloads

**Recommendation:**
- Validate that expanded paths are within expected boundaries
- Don't fall back to current directory - fail explicitly
- Check directory existence before watching
- Canonicalize paths before watching

---

### FS-006: Symlink Race in Script Watching (MEDIUM)

**Location:** `src/watcher.rs`, lines 362-449

**Description:**  
The `ScriptWatcher` uses `RecursiveMode::Recursive` which follows symlinks into directories:

```rust
// Watch the scripts directory recursively
watcher.watch(&scripts_path, RecursiveMode::Recursive)?;

// Watch the scriptlets directory recursively (for *.md files)
if scriptlets_path.exists() {  // TOCTOU: exists() check before watch()
    watcher.watch(&scriptlets_path, RecursiveMode::Recursive)?;
}
```

**Attack Vectors:**
1. **Symlink injection:** An attacker could create a symlink in scripts/ pointing to a large directory tree, causing resource exhaustion
2. **TOCTOU race:** Between `exists()` check and `watch()`, the directory state could change
3. **Symlink to external directories:** Watching could expose file system activity outside `~/.sk/kit`

**Impact:** Resource exhaustion, information leakage about external file activity

**Recommendation:**
- Don't follow symlinks in recursive watches
- Validate all paths within watched directories are regular files/directories
- Use `symlink_metadata()` to detect and handle symlinks explicitly
- Implement depth limits for recursive watching

---

### FS-007: TOCTOU Race in Metadata Retrieval (LOW)

**Location:** `src/file_search.rs`, lines 197-210

**Description:**  
File metadata is retrieved after path construction without atomicity:

```rust
for line in stdout.lines().take(limit) {
    let path = Path::new(line);
    
    // Time passes between mdfind output and metadata retrieval
    let (size, modified) = match std::fs::metadata(path) {
        Ok(meta) => { /* ... */ }
        Err(_) => (0, 0),  // Silent failure
    };
    // ...
}
```

**Attack Vectors:**
1. **File modification race:** File could be modified/deleted between search and metadata retrieval
2. **Symlink swap:** A regular file could be swapped with a symlink between operations
3. **Silent failure masking:** Metadata errors return (0, 0) instead of indicating the file is inaccessible

**Impact:** Inaccurate metadata, potential security decisions based on stale data

**Recommendation:**
- Document that metadata may be stale
- Consider using `openat()` patterns for atomic operations
- Propagate metadata errors instead of silently returning defaults
- Add `accessed_at` timestamp to indicate when metadata was retrieved

---

### FS-008: Unbounded mdfind Output Processing (LOW)

**Location:** `src/file_search.rs`, lines 186-227

**Description:**  
While there's a `limit` parameter for results, the entire mdfind output is captured before limiting:

```rust
let output = match cmd.output() {  // Captures ALL output into memory
    Ok(output) => output,
    // ...
};

let stdout = String::from_utf8_lossy(&output.stdout);
let mut results = Vec::new();

for line in stdout.lines().take(limit) {  // Limit applied AFTER full capture
    // ...
}
```

**Attack Vectors:**
1. **Memory exhaustion:** A query matching millions of files would load all paths into memory before limiting
2. **DoS via crafted query:** Spotlight queries like `kMDItemContentType == *` could return massive result sets

**Impact:** Memory exhaustion, DoS

**Recommendation:**
- Use piped output with streaming processing
- Implement timeout for mdfind execution
- Add memory limits or use `take()` on a BufReader line iterator
- Consider using `-count` flag first to check result size

```rust
// Recommended streaming approach
let mut child = cmd.stdout(Stdio::piped()).spawn()?;
let reader = BufReader::new(child.stdout.take().unwrap());
for line in reader.lines().take(limit) {
    // Process line
}
child.kill()?;  // Kill if we have enough results
```

---

## Path Traversal Analysis

### Current State

The codebase has **minimal path traversal protections**:

| File | Function | Protection | Risk |
|------|----------|------------|------|
| file_search.rs | `search_files()` | None | HIGH |
| file_search.rs | `get_file_metadata()` | None | MEDIUM |
| config.rs | `load_config()` | Tilde expansion only | HIGH |
| watcher.rs | `watch_loop()` | Tilde expansion only | MEDIUM |

### Missing Protections

1. **No path canonicalization** - Paths like `~/../../../etc/passwd` are not normalized
2. **No allowlist checking** - Any path can be accessed
3. **No path prefix validation** - Results aren't verified to be within expected directories
4. **No symlink resolution validation** - Symlinks can escape directory boundaries

### Recommended Path Validation Function

```rust
use std::path::{Path, PathBuf};

fn validate_path(path: &Path, allowed_base: &Path) -> Result<PathBuf, &'static str> {
    // Canonicalize both paths
    let canonical_path = path.canonicalize()
        .map_err(|_| "Failed to canonicalize path")?;
    let canonical_base = allowed_base.canonicalize()
        .map_err(|_| "Failed to canonicalize base")?;
    
    // Check that path is under allowed base
    if !canonical_path.starts_with(&canonical_base) {
        return Err("Path escapes allowed directory");
    }
    
    Ok(canonical_path)
}
```

---

## Symlink Attack Vectors

### Attack Surface Summary

| Component | Symlink Handling | Attack Vector |
|-----------|------------------|---------------|
| `search_files()` | Follows (mdfind default) | Results include symlink targets |
| `get_file_metadata()` | Follows (metadata()) | Metadata of symlink target |
| `detect_file_type()` | Follows (is_dir()) | Type of symlink target |
| ConfigWatcher | Follows (notify default) | Events from symlink target changes |
| ScriptWatcher | Follows recursively | Watch entire symlinked directory trees |

### High-Risk Symlink Scenarios

1. **Scripts directory symlink attack:**
   ```
   ~/.sk/kit/scripts/evil -> /etc
   # ScriptWatcher now monitors /etc for changes
   ```

2. **Config file symlink swap:**
   ```
   rm ~/.sk/kit/config.ts
   ln -s /path/to/malicious.ts ~/.sk/kit/config.ts
   # Next config reload executes attacker code
   ```

3. **Theme file race condition:**
   ```
   ln -s /dev/urandom ~/.sk/kit/theme.json
   # Theme loader may hang or crash
   ```

---

## Config Injection Risks

### Current Threat Model

The config loading system has a **trusted file model** that assumes:
- `~/.sk/kit/config.ts` is always trustworthy
- The user controls their home directory
- No other processes can write to config files

This model **breaks** in:
- Multi-user systems with shared $HOME
- Systems with malware that has user write access
- Misconfigured permission scenarios
- Development environments with untrusted code

### Injection Points

1. **Direct config.ts injection:**
   - Write malicious TypeScript to config.ts
   - Code executes with full user privileges

2. **Import statement injection:**
   - config.ts can import any module
   - Malicious npm packages could be loaded

3. **Environment variable injection:**
   - TypeScript code can read/modify environment
   - Could affect subsequent process spawns

4. **Temp file injection:**
   - Race condition in /tmp/kit-config.js
   - Potential for code injection between transpile and execute

### Mitigation Strategies

| Strategy | Effectiveness | Implementation Effort |
|----------|--------------|----------------------|
| JSON-only config | High | Medium |
| Config file integrity checking | Medium | Low |
| Sandbox bun execution | High | High |
| Config schema validation | Medium | Medium |
| File permission validation | Low | Low |

---

## Risk Rating Matrix

| Risk | Likelihood | Impact | Overall |
|------|-----------|--------|---------|
| Config code execution | Medium | Critical | HIGH |
| Path traversal in search | Medium | High | MEDIUM-HIGH |
| Temp file race | Low | High | MEDIUM |
| Symlink attacks | Low | Medium | LOW-MEDIUM |
| TOCTOU races | Low | Low | LOW |
| DoS via mdfind | Low | Medium | LOW |

---

## Recommendations by Priority

### Critical (Address Immediately)

1. **Replace TypeScript config with JSON**
   - Remove arbitrary code execution
   - Use JSON schema validation for config structure

2. **Secure temp file handling**
   - Use `tempfile` crate for unique, secure temp files
   - Set appropriate permissions
   - Clean up immediately after use

### High Priority

3. **Implement path validation**
   - Add canonicalization before all file operations
   - Validate paths are within allowed directories
   - Implement allowlist for searchable paths

4. **Add symlink handling**
   - Use `symlink_metadata()` to detect symlinks
   - Optionally refuse to follow symlinks outside home directory
   - Document symlink behavior for users

### Medium Priority

5. **Improve error handling**
   - Don't silently ignore metadata errors
   - Log security-relevant failures
   - Propagate errors to callers

6. **Add resource limits**
   - Stream mdfind output instead of buffering
   - Implement timeouts for external commands
   - Add depth limits for directory watching

### Low Priority

7. **Add file permission validation**
   - Check config file ownership
   - Warn on world-writable config files
   - Validate home directory permissions

---

## Appendix: Code References

### Vulnerable Code Locations

```
src/config.rs:189-287  - Config loading with code execution
src/config.rs:199      - Hardcoded temp file path
src/file_search.rs:152-231 - File search without path validation
src/file_search.rs:247-296 - Metadata retrieval with symlink following
src/watcher.rs:82-167  - Config watcher path handling
src/watcher.rs:222-307 - Theme watcher path handling
src/watcher.rs:362-449 - Script watcher with recursive symlink following
```

### Secure Pattern Examples

```rust
// Secure path validation
fn safe_path(user_input: &str, base: &Path) -> Option<PathBuf> {
    let path = Path::new(user_input);
    let canonical = path.canonicalize().ok()?;
    let base_canonical = base.canonicalize().ok()?;
    
    if canonical.starts_with(&base_canonical) {
        Some(canonical)
    } else {
        None
    }
}

// Secure temp file
use tempfile::NamedTempFile;
let tmp = NamedTempFile::new()?;
// Use tmp.path() - automatically cleaned up on drop
```

---

*End of Security Audit Report*
