# Security Audit: Privilege Escalation & macOS Security

**Audit Date:** 2024-12-29  
**Auditor:** Security Audit Agent  
**Scope:** macOS-specific code including NSWindow/NSPanel, entitlements, TCC permissions, sandboxing, and IPC

## Executive Summary

Script Kit GPUI is a macOS desktop application that requires elevated privileges to function as intended. The application operates **without App Sandbox** and requires **Accessibility permissions** for core functionality. While the current implementation follows reasonable security practices for a developer productivity tool, there are several areas that warrant attention for a production deployment.

### Risk Rating: **MEDIUM**

| Category | Risk Level | Notes |
|----------|------------|-------|
| **Entitlements** | LOW | No entitlements file exists; unsigned development build |
| **TCC Permissions** | MEDIUM | Requires Accessibility permission with broad system access |
| **Sandboxing** | HIGH | App runs unsandboxed with full disk and network access |
| **Privilege Escalation** | LOW | No sudo/root operations; reasonable process isolation |
| **IPC Security** | MEDIUM | Script execution uses stdin/stdout pipes; process groups |

---

## 1. Entitlements Analysis

### Current Status: No Entitlements File Found

The application does not have a `.entitlements` file in the repository. This means:

- **Development builds** run without code signing or entitlements
- **Distribution via App Store** would require creating entitlements
- **Notarization** for direct distribution would require hardened runtime

### Implicit Capabilities Used

Based on code analysis, the application implicitly uses these capabilities:

| Capability | Source File | Purpose |
|------------|-------------|---------|
| Accessibility API | `selected_text.rs` | Read/write selected text from other apps |
| Accessibility API | `window_control.rs` | List, move, resize windows across apps |
| Clipboard Access | `clipboard_history.rs` | Monitor and store clipboard contents |
| App Launching | `app_launcher.rs` | Scan and launch applications via `open -a` |
| Process Spawning | `executor.rs` | Execute scripts via bun/node |
| Keyboard Simulation | `selected_text.rs` | Simulate Cmd+V for paste operations |
| System Tray | `tray.rs` | Menu bar icon and menu |
| Global Hotkeys | `main.rs` | Global keyboard shortcut capture |

### Recommended Entitlements for Distribution

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Required for notarization with hardened runtime -->
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    
    <!-- Required for spawning bun/node processes -->
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
    
    <!-- If sandboxed (not currently) -->
    <key>com.apple.security.automation.apple-events</key>
    <true/>
</dict>
</plist>
```

### Finding E1: Missing Entitlements File
**Severity:** LOW  
**Impact:** Cannot distribute via App Store or notarize without proper entitlements  
**Recommendation:** Create `script-kit-gpui.entitlements` for distribution builds

---

## 2. TCC (Transparency, Consent, and Control) Permissions

### Required Permissions

#### 2.1 Accessibility Permission (REQUIRED)

**Location:** System Preferences > Privacy & Security > Accessibility

**Used For:**
- `get_selected_text()` - Reading selected text from any application
- `set_selected_text()` - Writing/replacing selected text
- `list_windows()` - Enumerating windows across all applications
- `tile_window()` / `move_window()` / `resize_window()` - Window management
- Keyboard event simulation (Cmd+V paste)

**Code References:**
```rust
// src/selected_text.rs:37-40
pub fn has_accessibility_permission() -> bool {
    let result = accessibility::application_is_trusted();
    ...
}

// src/selected_text.rs:52-60
pub fn request_accessibility_permission() -> bool {
    let result = accessibility::application_is_trusted_with_prompt();
    ...
}
```

**Security Implication:** Accessibility permission grants the application ability to:
- Read text from any application (passwords, sensitive data)
- Control windows of other applications
- Simulate keyboard input system-wide

### Finding T1: Broad Accessibility Access
**Severity:** MEDIUM  
**Impact:** Application can read/modify content in any application including password managers, banking apps  
**Mitigation:** This is inherent to the product's functionality. Document clearly in user-facing materials.

#### 2.2 Clipboard Access (IMPLICIT)

**No explicit TCC prompt** - Clipboard access does not require user permission on macOS.

**Code Reference:**
```rust
// src/clipboard_history.rs:300-383
fn clipboard_monitor_loop(stop_flag: Arc<Mutex<bool>>) -> Result<()> {
    let mut clipboard = Clipboard::new()...
    loop {
        // Polls clipboard every 500ms
        if let Ok(text) = clipboard.get_text() { ... }
        if let Ok(image_data) = clipboard.get_image() { ... }
    }
}
```

**Security Implication:** 
- Background thread monitors clipboard continuously
- Stores up to 1000 entries in SQLite database at `~/.sk/kit/clipboard-history.db`
- Images are base64-encoded and stored

### Finding T2: Clipboard History Privacy
**Severity:** MEDIUM  
**Impact:** Sensitive clipboard contents (passwords, API keys) may be persisted to disk  
**Recommendation:** 
1. Add option to exclude certain apps from clipboard capture
2. Consider encryption at rest for clipboard database
3. Add time-based auto-deletion for sensitive data

---

## 3. Sandboxing Status

### Current Status: **NOT SANDBOXED**

The application runs with full user privileges and is not contained by App Sandbox.

### Evidence

1. **No sandbox entitlement** - No `com.apple.security.app-sandbox` key
2. **Direct file system access:**
   - Writes to `~/.sk/kit/` directory
   - Scans `/Applications`, `/System/Applications`, `~/Applications`
   - Creates temp files in system temp directory
3. **Network access:**
   - No restrictions on network operations
   - Scripts can make arbitrary network requests
4. **Process spawning:**
   - Spawns arbitrary child processes (bun, node, shell scripts)
   - Creates new process groups for child isolation

### File System Access Patterns

| Path | Access | Purpose |
|------|--------|---------|
| `~/.sk/kit/` | R/W | User data, scripts, config |
| `~/.sk/kit/clipboard-history.db` | R/W | Clipboard database |
| `~/.sk/kit/sdk/` | R/W | SDK files |
| `~/.sk/kit/logs/` | R/W | Application logs |
| `~/.sk/kit/cache/app-icons/` | R/W | Cached app icons |
| `/Applications/` | R | App scanning |
| `/System/Applications/` | R | System app scanning |
| `~/Applications/` | R | User app scanning |
| `/tmp/` | R/W | Temp script execution |

### Finding S1: Unsandboxed Execution
**Severity:** HIGH (for App Store distribution)  
**Impact:** Application has unrestricted access to file system and network  
**Note:** This is intentional for a developer tool. Sandboxing would severely limit functionality.  
**Recommendation for Distribution:** If not App Store, document security model clearly. If App Store target, evaluate minimal sandbox profile.

---

## 4. Privilege Escalation Vectors

### 4.1 Script Execution

**Risk:** User scripts execute with same privileges as the application.

**Code Reference:**
```rust
// src/executor.rs:752-811
fn spawn_script(cmd: &str, args: &[&str]) -> Result<ScriptSession, String> {
    let mut command = Command::new(&executable);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // On Unix, spawn in a new process group
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    ...
}
```

**Mitigations Present:**
- Scripts run as the current user, not elevated
- Process group isolation allows clean termination
- No `sudo` or privilege elevation mechanisms

### Finding P1: Script Execution Without Isolation
**Severity:** LOW  
**Impact:** Malicious scripts could access user data (by design)  
**Note:** This is the intended behavior for a scripting tool  
**Recommendation:** Consider optional sandboxed execution mode for untrusted scripts

### 4.2 No Root/Admin Operations

The codebase does **not** contain:
- `sudo` invocations
- `AuthorizationCreate` calls
- `SMJobBless` privileged helper installation
- Keychain password operations

### 4.3 Process Cleanup

**Positive Finding:** Proper process group management ensures child processes are cleaned up:

```rust
// src/executor.rs:461-504
impl ProcessHandle {
    fn kill(&mut self) {
        #[cfg(unix)]
        {
            // Kill entire process group using negative PID
            let negative_pgid = format!("-{}", self.pid);
            Command::new("kill").args(["-9", &negative_pgid]).output()
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        self.kill();
    }
}
```

---

## 5. NSWindow/NSPanel Security

### 5.1 Window Configuration

**Code Reference:**
```rust
// src/main.rs:8256-8294
fn configure_as_floating_panel() {
    unsafe {
        let app: id = NSApp();
        let window: id = msg_send![app, keyWindow];
        
        if window != nil {
            // NSFloatingWindowLevel = 3
            let floating_level: i32 = 3;
            let _: () = msg_send![window, setLevel:floating_level];
            
            // MoveToActiveSpace behavior
            let collection_behavior: u64 = 2;
            let _: () = msg_send![window, setCollectionBehavior:collection_behavior];
            
            // Disable state restoration
            let _: () = msg_send![window, setRestorable:false];
            let _: () = msg_send![window, setFrameAutosaveName:empty_string];
        }
    }
}
```

**Security Implications:**
- `NSFloatingWindowLevel` (3): Window floats above normal windows but below system panels
- `setRestorable:false`: Prevents macOS from caching window state
- No `NSWindowCollectionBehaviorCanJoinAllSpaces`: Window stays on current space

### 5.2 Raw Pointer Usage

The code uses `unsafe` blocks with raw Objective-C message sending:

```rust
// src/window_manager.rs:125-148
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct WindowId(usize);

unsafe impl Send for WindowId {}
unsafe impl Sync for WindowId {}
```

**Finding W1: Unsafe Pointer Management
**Severity:** LOW  
**Impact:** Memory safety depends on correct usage  
**Note:** This is standard for Cocoa interop in Rust. The implementation follows safe patterns.

---

## 6. Inter-Process Communication (IPC)

### 6.1 Script Communication Protocol

Scripts communicate via stdin/stdout using JSONL (JSON Lines):

```rust
// src/executor.rs - ScriptSession struct
pub struct ScriptSession {
    pub stdin: ChildStdin,
    stdout_reader: JsonlReader<BufReader<ChildStdout>>,
    pub stderr: Option<ChildStderr>,
    child: Child,
    process_handle: ProcessHandle,
}
```

**Protocol Messages:** (from `src/protocol.rs`)
- Arg prompts, Div displays, Editor content
- File operations, clipboard operations
- Selected text get/set
- Window control commands

### 6.2 No Network IPC

The application does **not** expose:
- Local socket servers
- HTTP endpoints
- Mach ports
- XPC services

All IPC is via:
- stdin/stdout pipes to child processes
- System clipboard (arboard crate)
- Accessibility APIs (AXUIElement)

### Finding I1: Script Communication Channel
**Severity:** LOW  
**Impact:** Scripts can send arbitrary protocol messages  
**Mitigation:** Protocol parser uses typed messages, unknown types are handled gracefully

---

## 7. External Dependencies Security

### Key Native Dependencies

| Crate | Version | Purpose | Risk |
|-------|---------|---------|------|
| `cocoa` | 0.26 | Cocoa bindings | LOW - Well-maintained |
| `objc` | 0.2 | Obj-C runtime | LOW - Standard FFI |
| `core-graphics` | 0.24 | Graphics/events | LOW - System binding |
| `macos-accessibility-client` | 0.0.1 | Accessibility API | LOW - Wrapper only |
| `arboard` | 3.6 | Clipboard | LOW - Maintained |
| `rusqlite` | 0.31 | SQLite (bundled) | LOW - Bundled SQLite |
| `global-hotkey` | 0.7 | Global shortcuts | MEDIUM - System hooks |

### Finding D1: Pinned Dependency Versions
**Severity:** LOW  
**Recommendation:** Set up automated dependency scanning (cargo-audit) in CI

---

## 8. Recommendations Summary

### Immediate Actions

1. **Document Security Model** - Create user-facing documentation explaining:
   - Why Accessibility permission is required
   - What clipboard history stores and how to clear it
   - How script execution works

2. **Add Clipboard Privacy Controls** - Options for:
   - Disabling clipboard history
   - Excluding specific apps
   - Auto-expiring sensitive entries

### For Production Distribution

3. **Create Entitlements File** - Required for notarization:
   ```bash
   touch script-kit-gpui.entitlements
   ```

4. **Add Code Signing** - Sign with Developer ID for distribution outside App Store

5. **Implement Hardened Runtime** - Required for notarization:
   ```bash
   codesign --options runtime ...
   ```

### Future Enhancements

6. **Optional Script Sandboxing** - Consider `sandbox-exec` for untrusted scripts

7. **Audit Logging** - Log sensitive operations (accessibility access, clipboard reads)

8. **Encrypted Storage** - Consider encrypting clipboard database at rest

---

## Appendix A: TCC Permission Categories

| Permission | Database | Required | Prompted |
|------------|----------|----------|----------|
| Accessibility | TCC.db | Yes | Yes |
| Full Disk Access | TCC.db | No | - |
| Automation | TCC.db | No* | - |
| Screen Recording | TCC.db | No | - |
| Microphone | TCC.db | No | - |
| Camera | TCC.db | No | - |

*Automation may be required for AppleScript scriptlets

## Appendix B: File System Footprint

```
~/.sk/kit/
├── clipboard-history.db    # SQLite - clipboard data
├── config.ts               # User configuration
├── logs/
│   └── script-kit-gpui.jsonl  # Application logs
├── sdk/
│   └── kit-sdk.ts          # Embedded SDK
├── cache/
│   └── app-icons/          # Cached app icons (PNG)
├── scripts/                # User scripts
└── tsconfig.json           # TypeScript config
```

## Appendix C: Attack Surface Summary

| Surface | Exposure | Mitigation |
|---------|----------|------------|
| Script Execution | User-controlled scripts | Process groups, no elevation |
| Clipboard | Continuous monitoring | User-initiated feature |
| Accessibility | Cross-app text/window | TCC permission required |
| File System | ~/.sk/kit/ writes | User data only |
| Network | Script-dependent | No app-level restrictions |
| IPC | stdin/stdout pipes | Typed protocol, no network |
