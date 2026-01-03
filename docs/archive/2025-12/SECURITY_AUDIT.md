# Script Kit GPUI Security Audit

**Audit Date:** 2025-12-29  
**Auditors:** AI Swarm Security Team (10 specialized agents)  
**Scope:** Full application security review  
**Codebase:** Script Kit GPUI - A Rust/GPUI rewrite of Script Kit

---

## Executive Summary

This comprehensive security audit examined 10 distinct attack surfaces in the Script Kit GPUI application. The application is a developer productivity tool that executes user-authored TypeScript/JavaScript scripts via the Bun runtime, with a GPUI-based Rust shell providing the UI and system integration.

**Overall Security Posture: MEDIUM RISK**

The application demonstrates generally sound security practices for a desktop automation tool. The primary risks stem from the inherent trust model (user scripts have full system access) and several specific areas requiring hardening:

1. **Shell injection vulnerabilities** in scriptlet variable substitution (HIGH)
2. **Unencrypted clipboard history** storage with sensitive data (HIGH)
3. **No path traversal protection** in file operations (MEDIUM-HIGH)
4. **Predictable temp file paths** enabling symlink attacks (MEDIUM)
5. **Environment variable leakage** to child processes (MEDIUM)

The codebase correctly avoids common pitfalls like `eval()`, uses typed JSON protocols, and implements proper process isolation with process groups.

---

## Quick Stats

| Severity | Count |
|----------|-------|
| **Critical** | 1 |
| **High** | 8 |
| **Medium** | 23 |
| **Low** | 25 |
| **Informational** | 10 |
| **TOTAL** | 67 |

### Findings Distribution by Category

| Category | Critical | High | Medium | Low | Info |
|----------|----------|------|--------|-----|------|
| Command Injection | 0 | 2 | 1 | 1 | 1 |
| IPC Protocol | 0 | 0 | 4 | 3 | 2 |
| Clipboard/Accessibility | 0 | 2 | 4 | 3 | 1 |
| File System | 1 | 2 | 3 | 2 | 0 |
| Terminal/PTY | 0 | 1 | 2 | 2 | 1 |
| Input Validation | 0 | 2 | 5 | 8 | 0 |
| Memory Safety | 0 | 0 | 2 | 4 | 1 |
| Dependencies | 0 | 0 | 1 | 1 | 1 |
| Privilege/macOS | 0 | 0 | 2 | 2 | 2 |
| SDK Security | 0 | 0 | 0 | 3 | 7 |

---

## Critical & High Priority Findings

These findings require immediate attention:

| ID | Severity | Category | Finding | Location |
|----|----------|----------|---------|----------|
| **FS-001** | CRITICAL | File System | Arbitrary code execution via config.ts - executes TypeScript with `bun` | `config.rs:188-287` |
| SEC-001 | HIGH | Command Injection | Shell injection via scriptlet variable substitution `{{name}}` | `executor.rs:1319-1365` |
| SEC-002 | HIGH | Command Injection | AppleScript injection via type command - insufficient escaping | `executor.rs:1596-1620` |
| CH-01 | HIGH | Clipboard | No encryption for stored clipboard data in SQLite | `clipboard_history.rs:194-204` |
| CH-04 | HIGH | Clipboard | No content filtering for sensitive data (passwords, API keys) | `clipboard_history.rs:319-334` |
| FS-002 | HIGH | File System | No path traversal protection in `onlyin` parameter | `file_search.rs:152-231` |
| FS-003 | HIGH | File System | Predictable temp file path `/tmp/kit-config.js` - symlink attack | `config.rs:199` |
| PTY-001 | HIGH | Terminal | Environment variables inherited without filtering (secrets leak) | `pty.rs:183-202` |
| IV-SV001 | HIGH | Input Validation | No shell escaping for positional args in scriptlets | `scriptlets.rs:472-518` |
| IV-SV002 | HIGH | Input Validation | No shell escaping for named inputs in scriptlets | `scriptlets.rs:472-518` |

---

## Detailed Audit Reports

Each area was examined by a specialized audit agent:

| # | Report | Risk Rating | Key Concern |
|---|--------|-------------|-------------|
| 1 | [Command Injection & Script Execution](security-audit/01-COMMAND_INJECTION.md) | MEDIUM | Scriptlet variable substitution lacks sanitization |
| 2 | [IPC Protocol Security](security-audit/02-IPC_PROTOCOL.md) | MEDIUM-HIGH | No message size limits, DoS potential |
| 3 | [Clipboard & Accessibility](security-audit/03-CLIPBOARD_ACCESSIBILITY.md) | MEDIUM-HIGH | Unencrypted persistent storage of sensitive clipboard data |
| 4 | [File System Access](security-audit/04-FILESYSTEM.md) | MEDIUM-HIGH | Config loading executes arbitrary TypeScript |
| 5 | [Terminal/PTY Security](security-audit/05-TERMINAL_PTY.md) | MEDIUM | Environment variable leakage to child processes |
| 6 | [Input Validation](security-audit/06-INPUT_VALIDATION.md) | MEDIUM | Shell injection in scriptlet substitution |
| 7 | [Memory Safety](security-audit/07-MEMORY_SAFETY.md) | MEDIUM | AXUIElement cache leaks, potential use-after-free |
| 8 | [Dependencies](security-audit/08-DEPENDENCIES.md) | MEDIUM | GPL-3.0 transitive deps require compliance review |
| 9 | [Privilege & macOS Security](security-audit/09-PRIVILEGE_MACOS.md) | MEDIUM | Runs unsandboxed with Accessibility permission |
| 10 | [SDK Security](security-audit/10-SDK_SECURITY.md) | MEDIUM | Semi-trusted script model is intentional |

---

## Attack Surface Diagram

```
                              ┌─────────────────────────────────────────┐
                              │          SCRIPT KIT GPUI                 │
                              │        (Rust/GPUI Application)           │
                              └─────────────────────────────────────────┘
                                               │
         ┌─────────────────────────────────────┼─────────────────────────────────────┐
         │                                     │                                     │
         ▼                                     ▼                                     ▼
┌─────────────────────┐             ┌─────────────────────┐             ┌─────────────────────┐
│    USER INPUT       │             │   SCRIPT EXECUTION  │             │   SYSTEM APIS       │
│  (Trust Boundary)   │             │   (Trust Boundary)  │             │   (Trust Boundary)  │
├─────────────────────┤             ├─────────────────────┤             ├─────────────────────┤
│ • Filter strings    │             │ • User scripts (bun)│             │ • Clipboard (R/W)   │
│ • Prompt inputs     │             │ • Scriptlets (shell)│             │ • Accessibility API │
│ • stdin JSON msgs   │             │ • Config.ts loading │             │ • File system       │
│ • Theme files       │             │ • SDK preload       │             │ • Process spawning  │
└─────────────────────┘             └─────────────────────┘             └─────────────────────┘
         │                                     │                                     │
         │ LOW RISK                            │ HIGH RISK                           │ MEDIUM RISK
         │ (typed JSON)                        │ (code exec)                         │ (system access)
         ▼                                     ▼                                     ▼
┌─────────────────────┐             ┌─────────────────────┐             ┌─────────────────────┐
│   ATTACK VECTORS    │             │   ATTACK VECTORS    │             │   ATTACK VECTORS    │
├─────────────────────┤             ├─────────────────────┤             ├─────────────────────┤
│ • Message flooding  │             │ • Shell injection   │             │ • Clipboard harvest │
│ • Large payloads    │             │   via {{variables}} │             │ • Window spoofing   │
│ • Type confusion    │             │ • Config.ts replace │             │ • Symlink attacks   │
│ • Deeply nested JSON│             │ • AppleScript inject│             │ • Env var leakage   │
└─────────────────────┘             └─────────────────────┘             └─────────────────────┘

┌────────────────────────────────────────────────────────────────────────────────────────────┐
│                                 DATA PERSISTENCE                                           │
├────────────────────────────────────────────────────────────────────────────────────────────┤
│  ~/.sk/kit/                                                                                  │
│  ├── clipboard-history.db  ← UNENCRYPTED (passwords, API keys may be stored)              │
│  ├── config.ts             ← EXECUTED (arbitrary code execution if modified)              │
│  ├── theme.json            ← PARSED (JSON, low risk)                                      │
│  ├── scripts/              ← USER CODE (trusted by design)                                │
│  └── logs/                 ← JSONL LOGS (potential info disclosure)                       │
└────────────────────────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────────────────────────┐
│                                 EXTERNAL PROCESSES                                         │
├────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                            │
│  GPUI App ──stdin JSON──► bun (user script) ──► FULL SYSTEM ACCESS                        │
│      │                         │                                                          │
│      │                         └──► Inherits: HOME, USER, PATH, SHELL                     │
│      │                              (and potentially secrets in env)                      │
│      │                                                                                    │
│      └──► Scriptlet Execution (shell/python/ruby)                                         │
│                 │                                                                          │
│                 └──► {{variable}} substitution ← SHELL INJECTION RISK                     │
│                                                                                            │
└────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Risk Matrix by Category

```
                    │  Low     │  Medium   │  High     │  Critical │
   ─────────────────┼──────────┼───────────┼───────────┼───────────┤
   Command Injection│          │           │  SEC-001  │           │
                    │          │           │  SEC-002  │           │
   ─────────────────┼──────────┼───────────┼───────────┼───────────┤
   IPC Protocol     │  IPC-004 │  IPC-001  │           │           │
                    │  IPC-005 │  IPC-002  │           │           │
                    │          │  IPC-010  │           │           │
   ─────────────────┼──────────┼───────────┼───────────┼───────────┤
   Clipboard/Access │  ST-01   │  CH-02    │  CH-01    │           │
                    │  ST-02   │  CH-03    │  CH-04    │           │
                    │  ST-03   │  CH-05    │           │           │
   ─────────────────┼──────────┼───────────┼───────────┼───────────┤
   File System      │  FS-007  │  FS-004   │  FS-002   │  FS-001   │
                    │  FS-008  │  FS-005   │  FS-003   │           │
   ─────────────────┼──────────┼───────────┼───────────┼───────────┤
   Terminal/PTY     │  PTY-003 │  PTY-002  │  PTY-001  │           │
                    │  THR-001 │  ESC-001  │           │           │
   ─────────────────┼──────────┼───────────┼───────────┼───────────┤
   Input Validation │  IV-P001 │  IV-PA001 │  IV-SV001 │           │
                    │  IV-P002 │  IV-PA002 │  IV-SV002 │           │
                    │  multiple│  multiple │           │           │
   ─────────────────┴──────────┴───────────┴───────────┴───────────┘
```

---

## Remediation Roadmap

### Phase 1: Critical (Immediate - This Week)

| Priority | Finding | Action | Effort |
|----------|---------|--------|--------|
| P0 | FS-001 | Replace TypeScript config with JSON-only format | Medium |
| P0 | SEC-001/IV-SV001 | Implement shell escaping for scriptlet variables using `shell-escape` crate | Medium |
| P0 | CH-01 | Implement SQLCipher encryption for clipboard-history.db | Medium |
| P0 | FS-003 | Use `tempfile` crate for secure temp file creation | Low |

### Phase 2: High (1-2 Weeks)

| Priority | Finding | Action | Effort |
|----------|---------|--------|--------|
| P1 | SEC-002 | Fix AppleScript escaping - handle all special characters | Low |
| P1 | CH-04 | Add sensitive data filtering (patterns: `*_KEY`, `*SECRET*`, `*PASSWORD*`) | Medium |
| P1 | PTY-001 | Create environment variable allowlist, block known secret patterns | Low |
| P1 | FS-002 | Implement path canonicalization and validation | Medium |
| P1 | IPC-001 | Add message size limits (default 10MB max) | Low |
| P1 | IPC-010 | Add choice count validation (max 10,000) | Low |

### Phase 3: Medium (1 Month)

| Priority | Finding | Action | Effort |
|----------|---------|--------|--------|
| P2 | CH-02 | Add explicit opt-in for clipboard monitoring | Medium |
| P2 | CH-05 | Implement time-based expiry for clipboard entries | Medium |
| P2 | IPC-005 | Add rate limiting (~100 msg/sec) | Low |
| P2 | ESC-001 | Sanitize terminal title strings | Low |
| P2 | PTY-003 | Implement graceful shutdown (SIGTERM before SIGKILL) | Low |
| P2 | FS-005 | Validate watched paths before watching | Low |
| P2 | Dependencies | Pin gpui to specific commit, update rusqlite/resvg | Low |

### Phase 4: Low (As Resources Allow)

| Priority | Finding | Action | Effort |
|----------|---------|--------|--------|
| P3 | Memory | Implement RAII wrappers for CoreFoundation types | Medium |
| P3 | Memory | Add window reference validation before use | Medium |
| P3 | IPC-004 | Replace `serde_json::Value` with typed enums | Medium |
| P3 | IPC-007 | Add depth-limited JSON deserialization | Medium |
| P3 | CH-06 | Add visual indicator for active clipboard monitoring | Low |
| P3 | License | Review GPL-3.0 compliance for distribution | Medium |
| P3 | Documentation | Document security model for users | Low |

---

## Security Model Documentation

### Trust Boundaries

| Component | Trust Level | Notes |
|-----------|-------------|-------|
| User scripts (`~/.sk/kit/scripts/`) | Fully Trusted | By design - user's own code |
| Config file (`~/.sk/kit/config.ts`) | Fully Trusted | Currently executed as code |
| Theme file (`~/.sk/kit/theme.json`) | Semi-Trusted | JSON parsed, not executed |
| Scriptlets (markdown embedded) | Semi-Trusted | Need input sanitization |
| SDK (`kit-sdk.ts`) | Fully Trusted | Embedded at compile time |
| IPC Messages (stdin/stdout) | Validated | Typed JSON protocol |

### Permission Requirements

| Permission | Purpose | Risk if Compromised |
|------------|---------|---------------------|
| Accessibility | Read/write selected text, window control | Full UI automation access |
| Clipboard | Monitor and store clipboard | Capture passwords/secrets |
| File System | `~/.sk/kit/` access | Script execution, data access |
| Process Spawning | Run bun/node/shell | Arbitrary code execution |
| Network | Script-dependent | Data exfiltration possible |

### Positive Security Practices Observed

- No `eval()` or `new Function()` in SDK
- Typed JSON protocol with graceful error handling
- Process group isolation for child cleanup
- Permission checks before Accessibility API use
- No sudo/root operations
- No network listeners or IPC servers

---

## Testing Recommendations

### Security Test Suite

```bash
# Add to CI pipeline
cargo audit                              # Dependency vulnerabilities
cargo clippy -- -D warnings              # Lints including security warnings
cargo test --features system-tests       # Security-sensitive tests

# Manual security testing
# 1. Scriptlet injection test
echo '{"type":"run","path":"test-scriptlet-injection.ts"}' | ./target/debug/script-kit-gpui

# 2. Large message DoS test
# 3. Path traversal test
# 4. Environment variable leakage test
```

### Recommended Fuzz Targets

1. `parse_message_graceful()` - JSON protocol parser
2. `cf_string_to_string()` - CoreFoundation string conversion
3. `format_scriptlet()` - Variable substitution
4. `parse_html_comment_metadata()` - Scriptlet metadata parser

---

## Conclusion

Script Kit GPUI demonstrates reasonable security practices for a developer productivity tool with intentional full system access. The main vulnerabilities exist in the scriptlet execution path where user input is interpolated into shell commands without proper sanitization, and in the clipboard history feature which stores sensitive data unencrypted.

**Recommended Immediate Actions:**
1. Implement shell escaping for scriptlet variable substitution
2. Encrypt clipboard history database
3. Replace TypeScript config with JSON-only format
4. Use secure temp file creation

**For Production Distribution:**
1. Create entitlements file for notarization
2. Implement hardened runtime
3. Document security model clearly for users
4. Consider optional sandboxing for untrusted scripts

---

*Report compiled by Summary Compiler Agent*  
*Cell ID: cell--9bnr5-mjr5bz7b3mo*  
*Epic: cell--9bnr5-mjr5bz6icof*
