# SDK Security Audit

**Audit Date:** December 29, 2025  
**Audited File:** `scripts/kit-sdk.ts` (3,733 lines)  
**SDK Version:** 0.2.0  
**Risk Rating:** MEDIUM  

---

## Executive Summary

The Script Kit SDK (`scripts/kit-sdk.ts`) is a TypeScript preload module that provides global functions for user-authored scripts. It handles stdin/stdout JSON protocol communication with the GPUI Rust host, exposes system APIs (clipboard, keyboard, mouse), and manages prompts.

**Key Findings:**
- **No eval/Function() usage** - The SDK does not use `eval()` or `new Function()` for dynamic code execution
- **No direct innerHTML assignment** - HTML is passed to GPUI host, not rendered by SDK itself
- **JSON.parse without validation** - Multiple instances of parsing untrusted JSON without schema validation
- **Prototype pollution risk is LOW** - No `Object.assign` from user input or deep merging patterns
- **stdin/stdout handling is secure** - Uses buffered line-based JSON protocol
- **Semi-trusted script model** - Scripts run with full Node.js capabilities (by design)

**Overall Assessment:** The SDK follows reasonable security practices for its threat model (user-authored scripts running locally). The main concerns are around JSON parsing robustness and the inherent trust model where scripts have full system access.

---

## Findings Table

| ID | Finding | Severity | Location | Status |
|----|---------|----------|----------|--------|
| SDK-001 | JSON.parse without schema validation | LOW | Multiple locations | Open |
| SDK-002 | No input sanitization for HTML content | INFO | `div()`, `form()`, `widget()` | By Design |
| SDK-003 | Unrestricted shell command execution | INFO | `exec()` function | By Design |
| SDK-004 | Full clipboard access | INFO | `clipboard.*` APIs | By Design |
| SDK-005 | Keyboard/mouse automation | INFO | `keyboard.*`, `mouse.*` | By Design |
| SDK-006 | Process environment access | LOW | `env()` function | Open |
| SDK-007 | File system operations | INFO | `isFile()`, `isDir()`, `trash()` | By Design |
| SDK-008 | Arbitrary script execution | INFO | `run()` function | By Design |
| SDK-009 | No rate limiting on API calls | LOW | All IPC functions | Open |
| SDK-010 | Accessibility API access | INFO | `getSelectedText()`, `setSelectedText()` | By Design |

---

## Detailed Analysis

### 1. Stdin/Stdout Protocol Handling

**Location:** Lines 891-945

```typescript
process.stdin.on('data', (chunk: string) => {
  stdinBuffer += chunk;
  
  while ((newlineIndex = stdinBuffer.indexOf('\n')) !== -1) {
    const line = stdinBuffer.substring(0, newlineIndex);
    stdinBuffer = stdinBuffer.substring(newlineIndex + 1);
    
    if (line.trim()) {
      try {
        const msg = JSON.parse(line) as ResponseMessage;
        // ... handle message
      } catch (e) {
        // Ignore parse errors
      }
    }
  }
});
```

**Assessment:** SECURE
- Uses line-based buffering to handle chunked input
- JSON parsing is wrapped in try/catch
- Parse errors are silently ignored (appropriate for robustness)
- No buffer overflow risk (JavaScript strings auto-resize)

**Recommendation:** Consider logging parse errors in debug mode for troubleshooting.

---

### 2. Dynamic Code Execution Analysis

**Search Results:** No instances of:
- `eval()`
- `new Function()`
- `vm.runInContext()`
- `vm.runInNewContext()`
- `require()` with dynamic paths
- `import()` with user-controlled paths

**Assessment:** SECURE - No dynamic code execution vectors found.

---

### 3. HTML/XSS Vector Analysis

**Locations:**
- `div(html, tailwind?)` - Line 1733
- `form(html)` - Line 1924
- `widget(html, options?)` - Line 2518
- `setPanel(html)` - Line 2955
- `setPreview(html)` - Line 2960
- `setPrompt(html)` - Line 2965
- `md(markdown)` - Line 1752

**How HTML is Handled:**

```typescript
globalThis.div = async function div(html: string, tailwind?: string): Promise<void> {
  const message: DivMessage = {
    type: 'div',
    id,
    html,  // Raw HTML passed through
    tailwind,
  };
  send(message);
};
```

**Assessment:** BY DESIGN (NOT A VULNERABILITY)
- HTML content is passed directly to the GPUI host via JSON protocol
- The SDK does NOT render HTML itself - it delegates to Rust/GPUI
- XSS prevention is the responsibility of the GPUI renderer
- Scripts are user-authored and trusted to provide their own HTML

**Recommendation:** Document that script authors are responsible for sanitizing any user-generated content they include in HTML.

**md() Function (Markdown to HTML):**
```typescript
globalThis.md = function md(markdown: string): string {
  let html = markdown;
  html = html.replace(/^### (.+)$/gm, '<h3>$1</h3>');
  html = html.replace(/^## (.+)$/gm, '<h2>$1</h2>');
  // ... more replacements
  return html;
};
```

This is a simple regex-based markdown parser. It does NOT sanitize input - if the markdown contains `<script>` tags, they pass through. This is acceptable because:
1. Input is from script authors (trusted)
2. Output goes to GPUI host (which should handle sanitization)

---

### 4. Prototype Pollution Analysis

**Search for Risky Patterns:**
- `Object.assign()` - Not found
- Deep merge patterns - Not found
- `obj[key] = value` with user input - Not found
- `JSON.parse()` return used directly - Multiple instances

**JSON.parse Usage Instances:**

| Location | Context | Risk |
|----------|---------|------|
| Line 918 | stdin message parsing | LOW - typed as ResponseMessage |
| Line 1870 | select() response parsing | LOW - result is array |
| Line 1906 | fields() response parsing | LOW - result is array |
| Line 1934 | form() response parsing | LOW - result is object |
| Line 1982 | hotkey() response parsing | LOW - typed properties |
| Line 2021 | drop() response parsing | LOW - result is array |
| Line 2661 | eyeDropper() response | LOW - typed properties |
| Line 2727 | exec() result parsing | LOW - typed properties |
| Line 2871 | getWindowBounds() parsing | LOW - typed properties |
| Line 3059 | db() data parsing | LOW - isolated to db instance |
| Line 3115 | store.get() parsing | LOW - returned as-is |

**Assessment:** LOW RISK
- All JSON.parse calls have try/catch handlers
- Results are assigned to typed interfaces, not spread onto existing objects
- No pattern of `Object.prototype` modification

**Recommendation:** Consider using a schema validation library (e.g., zod) for critical message parsing.

---

### 5. Semi-Trusted Script Sandboxing

**Current Model:**
Scripts run via `bun run --preload ~/.sk/kit/sdk/kit-sdk.ts <script>` with:
- Full file system access
- Full network access
- Full process spawning capability
- Clipboard read/write
- Keyboard/mouse automation
- System notifications
- Accessibility API access

**Assessment:** BY DESIGN
This is the intended trust model - Script Kit scripts are user-authored automation tools that need system access to be useful.

**Comparison to Similar Tools:**
| Tool | Trust Model |
|------|-------------|
| Script Kit | Semi-trusted (user-authored scripts) |
| Raycast Extensions | Sandboxed (App Store review) |
| Alfred Workflows | Semi-trusted (user-installed) |
| Hammerspoon | Trusted (Lua scripts with full access) |

---

### 6. IPC Security

**Message Protocol:**
All SDK functions communicate with GPUI via JSONL messages on stdin/stdout.

**Example Message Flow:**
```typescript
// Script sends:
{"type":"arg","id":"1","placeholder":"Name?","choices":[]}

// GPUI responds:
{"type":"submit","id":"1","value":"John"}
```

**Security Properties:**
- Messages are strictly typed (TypeScript interfaces)
- No command injection possible (JSON encoding)
- Message IDs prevent response spoofing
- No authentication (unnecessary for local IPC)

**Pending Map Pattern:**
```typescript
const pending = new Map<string, (msg: ResponseMessage) => void>();

// Store resolver
pending.set(id, (msg) => { resolve(msg.value); });

// Resolve when response arrives
if (id && pending.has(id)) {
  const resolver = pending.get(id);
  pending.delete(id);
  resolver(msg);
}
```

**Assessment:** SECURE
- Promise-based request/response with unique IDs
- Resolvers are removed after use (no accumulation)
- No risk of response replay or injection

---

### 7. Environment Variable Handling

**Location:** Lines 2067-2106

```typescript
globalThis.env = async function env(
  key: string,
  promptFn?: () => Promise<string>
): Promise<string> {
  const existingValue = process.env[key];
  if (existingValue !== undefined && existingValue !== '') {
    return existingValue;
  }
  // ... prompt for value if not set
};
```

**Assessment:** LOW RISK
- Reads from `process.env` (standard Node.js API)
- Secret detection for prompt masking:
  ```typescript
  secret: key.toLowerCase().includes('secret') || 
          key.toLowerCase().includes('password') ||
          key.toLowerCase().includes('token') ||
          key.toLowerCase().includes('key'),
  ```
- Values stored back to `process.env` (only affects current process)

**Recommendation:** Consider adding more patterns to secret detection (e.g., 'credential', 'api_key', 'private').

---

### 8. Shell Command Execution

**Location:** Lines 2718-2753

```typescript
globalThis.exec = async function exec(
  command: string,
  options?: ExecOptions
): Promise<ExecResult> {
  // Sends command to GPUI for execution
  const message: ExecMessage = {
    type: 'exec',
    id,
    command,
    options,
  };
  send(message);
};
```

**Assessment:** BY DESIGN
- Commands are sent to GPUI host for execution
- No shell injection possible at SDK level (command is a single string)
- Actual execution security depends on GPUI implementation
- Users explicitly invoke `exec()` knowing they're running shell commands

---

### 9. Accessibility API Access

**Location:** Lines 2153-2240

```typescript
globalThis.setSelectedText = async function(text: string): Promise<void> {
  // Replaces selected text in focused application
};

globalThis.getSelectedText = async function(): Promise<string> {
  await globalThis.hide();  // Auto-hides Script Kit window
  await new Promise(resolve => setTimeout(resolve, 50));  // Focus delay
  // ... sends request to GPUI
};
```

**Assessment:** SECURE IMPLEMENTATION
- Auto-hides window before accessing selection (correct behavior)
- 50ms delay allows focus transfer (prevents race condition)
- Permission checking available (`hasAccessibilityPermission()`)
- Permission request available (`requestAccessibilityPermission()`)

---

### 10. Rate Limiting

**Current State:** No rate limiting on any SDK functions.

**Potential Concerns:**
- Rapid `clipboard.writeText()` calls could flood clipboard history
- Rapid `keyboard.type()` calls could overwhelm system
- Rapid IPC messages could strain GPUI host

**Assessment:** LOW PRIORITY
- Scripts are user-authored (self-rate-limiting)
- GPUI host can implement throttling if needed
- No known DoS vectors for local automation

---

## Risk Matrix

| Category | Risk Level | Mitigation |
|----------|------------|------------|
| Remote Code Execution | NONE | No eval/Function, no dynamic imports |
| Cross-Site Scripting | N/A | HTML rendered by GPUI, not SDK |
| Prototype Pollution | LOW | No deep merge patterns |
| Command Injection | LOW | Commands go through JSON protocol |
| Privilege Escalation | N/A | Already runs with user privileges |
| Information Disclosure | BY DESIGN | Clipboard/file access is intended |

---

## Recommendations

### High Priority
1. **None** - No critical vulnerabilities found

### Medium Priority
1. Add JSON schema validation for incoming messages using zod or similar
2. Expand secret detection patterns in `env()` function
3. Add debug logging option for JSON parse failures

### Low Priority
1. Consider optional rate limiting for automation APIs
2. Document security model for script authors
3. Add content-security-policy documentation for widget HTML

---

## Conclusion

The Script Kit SDK is well-designed for its threat model. The semi-trusted script execution model is appropriate for a local automation tool. The main security boundary is between the SDK and the GPUI host, which is maintained via strict JSON IPC protocol.

**No critical or high-severity vulnerabilities were identified.**

The SDK correctly avoids dangerous patterns like `eval()`, dynamic imports, and prototype pollution. HTML rendering is delegated to the GPUI host, which should implement appropriate sanitization.

Script authors should be aware that scripts have full system access and should not run untrusted scripts.
