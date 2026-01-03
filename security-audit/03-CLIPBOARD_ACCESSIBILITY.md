# Security Audit: Clipboard & Accessibility APIs

**Audit Date:** 2024-12-29  
**Auditor:** clipboard-accessibility-auditor (swarm agent)  
**Cell ID:** cell--9bnr5-mjr5bz6t5np  
**Files Reviewed:**
- `src/clipboard_history.rs` (819 lines)
- `src/selected_text.rs` (374 lines)

---

## Executive Summary

This audit examines the clipboard history and selected text capture functionality in Script Kit GPUI. These features have significant privacy implications as they involve:
- **Continuous clipboard monitoring** (every 500ms)
- **Persistent storage of clipboard data** (up to 1000 entries in SQLite)
- **macOS Accessibility API access** for cross-application text capture
- **Keyboard simulation** for paste operations

**Overall Risk Rating: MEDIUM-HIGH**

The implementation follows reasonable security practices but has several areas requiring attention, particularly around data retention, sensitive data exposure, and implicit consent for clipboard monitoring.

---

## Findings Summary

| ID | Finding | Severity | Status | Location |
|----|---------|----------|--------|----------|
| CH-01 | No encryption for stored clipboard data | HIGH | Open | clipboard_history.rs:194-204 |
| CH-02 | Clipboard monitoring runs automatically | MEDIUM | Open | clipboard_history.rs:221-261 |
| CH-03 | Images stored as base64 in plain SQLite | MEDIUM | Open | clipboard_history.rs:404-409 |
| CH-04 | No content filtering for sensitive data | HIGH | Open | clipboard_history.rs:319-334 |
| CH-05 | Long-term data retention (1000 entries) | MEDIUM | Open | clipboard_history.rs:38 |
| CH-06 | No user notification of clipboard capture | MEDIUM | Open | clipboard_history.rs:221-261 |
| ST-01 | Accessibility permission required (proper) | LOW | Mitigated | selected_text.rs:37-41 |
| ST-02 | Clipboard contents temporarily exposed | LOW | Open | selected_text.rs:204-238 |
| ST-03 | Original clipboard restoration is best-effort | LOW | Open | selected_text.rs:225-234 |
| ST-04 | Keyboard simulation can affect other apps | MEDIUM | Open | selected_text.rs:246-276 |

---

## Detailed Findings

### CH-01: No Encryption for Stored Clipboard Data

**Severity:** HIGH  
**Location:** `clipboard_history.rs:194-204`

**Description:**
Clipboard history is stored in a plain SQLite database at `~/.sk/kit/clipboard-history.db` without any encryption. This means:
- Any process with file system access can read clipboard history
- Sensitive data (passwords, API keys, private messages) is exposed
- Database survives application restarts and persists indefinitely

**Vulnerable Code:**
```rust
conn.execute(
    "CREATE TABLE IF NOT EXISTS history (
        id TEXT PRIMARY KEY,
        content TEXT NOT NULL,         // Plain text storage
        content_type TEXT NOT NULL DEFAULT 'text',
        timestamp INTEGER NOT NULL,
        pinned INTEGER DEFAULT 0
    )",
    [],
)
```

**Risk:**
- Malware or unauthorized applications could harvest sensitive data
- Shared computers expose clipboard history to other users
- Backup systems may inadvertently store sensitive clipboard data

**Recommendations:**
1. Implement SQLCipher encryption for the database
2. Consider memory-only storage option for sensitive environments
3. Add per-entry encryption with user-derived key
4. Use macOS Keychain for encryption key storage

---

### CH-02: Clipboard Monitoring Runs Automatically

**Severity:** MEDIUM  
**Location:** `clipboard_history.rs:221-261`

**Description:**
The `init_clipboard_history()` function automatically starts a background thread that monitors the clipboard every 500ms. This happens at application startup without explicit user consent or notification.

**Vulnerable Code:**
```rust
pub fn init_clipboard_history() -> Result<()> {
    // ...
    // Start the monitoring thread (no user consent check)
    thread::spawn(move || {
        if let Err(e) = clipboard_monitor_loop(stop_flag_clone) {
            error!(error = %e, "Clipboard monitor thread failed");
        }
    });
    // ...
}
```

**Risk:**
- Users may not be aware their clipboard is being monitored
- Privacy expectations violated without informed consent
- Could capture sensitive data users don't intend to store

**Recommendations:**
1. Add explicit user opt-in for clipboard monitoring
2. Show visual indicator when clipboard monitoring is active
3. Add configuration option to disable clipboard history
4. Implement first-run consent dialog

---

### CH-03: Images Stored as Base64 in Plain SQLite

**Severity:** MEDIUM  
**Location:** `clipboard_history.rs:404-409`

**Description:**
Clipboard images are encoded as base64 and stored in the SQLite database without compression or encryption. The format is `rgba:{width}:{height}:{base64_data}`.

**Vulnerable Code:**
```rust
fn encode_image_as_base64(image: &arboard::ImageData) -> Result<String> {
    let base64_data = BASE64.encode(&image.bytes);
    Ok(format!("rgba:{}:{}:{}", image.width, image.height, base64_data))
}
```

**Risk:**
- Sensitive screenshots (passwords, private documents) stored unencrypted
- Database size can grow rapidly with images (up to 400MB image cache)
- Image metadata may contain location/timestamp information

**Recommendations:**
1. Allow users to disable image capture
2. Add size limits for individual images
3. Implement automatic expiry for image entries
4. Consider not storing images by default

---

### CH-04: No Content Filtering for Sensitive Data

**Severity:** HIGH  
**Location:** `clipboard_history.rs:319-334`

**Description:**
All clipboard text is stored without any filtering for potentially sensitive data. Common sensitive patterns include:
- Passwords and API keys
- Credit card numbers
- Social security numbers
- Private keys
- Authentication tokens

**Vulnerable Code:**
```rust
if let Ok(text) = clipboard.get_text() {
    if !text.is_empty() {
        let is_new = match &last_text {
            Some(last) => last != &text,
            None => true,
        };
        if is_new {
            // No filtering - all text is stored
            if let Err(e) = add_entry(&text, ContentType::Text) {
                warn!(error = %e, "Failed to add text entry to history");
            }
            last_text = Some(text);
        }
    }
}
```

**Risk:**
- Password managers often use clipboard for passwords
- API keys and tokens copied from documentation
- Sensitive credentials exposed in persistent storage

**Recommendations:**
1. Implement pattern detection for sensitive data (regex for API keys, passwords)
2. Add option to exclude password manager applications
3. Implement "incognito mode" to temporarily disable capture
4. Mark entries as "sensitive" and auto-delete after short period
5. Integrate with macOS "Handoff" secure clipboard for sensitive items

---

### CH-05: Long-term Data Retention (1000 Entries)

**Severity:** MEDIUM  
**Location:** `clipboard_history.rs:38`

**Description:**
The clipboard history retains up to 1000 entries with LRU eviction. Pinned entries are never evicted. There is no time-based expiry.

**Configuration:**
```rust
const MAX_HISTORY_ENTRIES: usize = 1000;
```

**Risk:**
- Old sensitive data persists indefinitely
- Users may forget what data is stored
- No automatic cleanup mechanism
- Pinned entries never expire

**Recommendations:**
1. Add configurable retention period (e.g., 7 days, 30 days)
2. Implement time-based expiry for entries
3. Add "clear all" button prominently in UI
4. Limit pinned entries count
5. Add periodic reminder about stored data

---

### CH-06: No User Notification of Clipboard Capture

**Severity:** MEDIUM  
**Location:** `clipboard_history.rs:221-261`

**Description:**
There is no visual indicator or notification when clipboard data is captured. Users have no way to know:
- That monitoring is active
- When an item was captured
- What items are stored

**Risk:**
- Privacy expectations violated
- Users may not realize sensitive data is being stored
- No opportunity to prevent capture of specific items

**Recommendations:**
1. Add menu bar icon indicator when monitoring is active
2. Optional toast notification on capture
3. Keyboard shortcut to view recent captures
4. System notification for first-time capture

---

### ST-01: Accessibility Permission Required (Proper Implementation)

**Severity:** LOW (Mitigated)  
**Location:** `selected_text.rs:37-41`

**Description:**
The `get_selected_text()` and `set_selected_text()` functions properly check for accessibility permissions before proceeding. This is a **positive security pattern**.

**Good Code:**
```rust
pub fn has_accessibility_permission() -> bool {
    let result = accessibility::application_is_trusted();
    debug!(granted = result, "Checked accessibility permission");
    result
}

pub fn get_selected_text() -> Result<String> {
    // Check permissions first
    if !has_accessibility_permission() {
        bail!("Accessibility permission required...");
    }
    // ...
}
```

**Positive Notes:**
- Permission check before every operation
- Clear error message directing users to settings
- Uses macOS system permission dialog
- Cannot bypass permission requirement

---

### ST-02: Clipboard Contents Temporarily Exposed

**Severity:** LOW  
**Location:** `selected_text.rs:204-238`

**Description:**
When setting selected text, the function temporarily overwrites the system clipboard. Although it attempts to restore the original content, there's a window where:
1. Original clipboard is read
2. New text is written to clipboard
3. Cmd+V is simulated
4. Original is restored (best effort)

**Vulnerable Code:**
```rust
fn set_via_clipboard_fallback(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    
    // Save original clipboard contents
    let original = clipboard.get_text().ok();
    
    // Set new text to clipboard (WINDOW OF EXPOSURE)
    clipboard.set_text(text).context("Failed to set clipboard text")?;
    
    // Paste...
    
    // Restore (best effort)
    if let Some(original_text) = original {
        thread::sleep(Duration::from_millis(100));
        if let Err(e) = clipboard.set_text(&original_text) {
            warn!(error = %e, "Failed to restore original clipboard");
        }
    }
}
```

**Risk:**
- Other applications polling clipboard may capture the temporary content
- Race condition if user copies something during operation
- Original clipboard may not be restored on error

**Recommendations:**
1. Minimize exposure window duration
2. Document this behavior for users
3. Consider using macOS NSPasteboard private types

---

### ST-03: Original Clipboard Restoration is Best-Effort

**Severity:** LOW  
**Location:** `selected_text.rs:225-234`

**Description:**
If the clipboard restoration fails, the warning is logged but the operation continues. The user's original clipboard content may be lost.

**Vulnerable Code:**
```rust
if let Err(e) = clipboard.set_text(&original_text) {
    warn!(error = %e, "Failed to restore original clipboard");
    // No retry, no user notification
}
```

**Risk:**
- User's clipboard content can be silently lost
- No notification to user about the loss
- May contain sensitive or important data

**Recommendations:**
1. Implement retry logic for restoration
2. Notify user if restoration fails
3. Keep backup in memory and retry on next operation

---

### ST-04: Keyboard Simulation Can Affect Other Apps

**Severity:** MEDIUM  
**Location:** `selected_text.rs:246-276`

**Description:**
The `simulate_paste_with_cg()` function posts keyboard events at the system level (HID). This can affect any focused application, potentially:
- Pasting into wrong application if focus changes
- Triggering unintended actions if app has different keybindings
- Interfering with other automation tools

**Vulnerable Code:**
```rust
pub fn simulate_paste_with_cg() -> Result<()> {
    // Posts to HID (system-wide)
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(5));
    key_up.post(CGEventTapLocation::HID);
}
```

**Risk:**
- Paste into wrong application (security sensitive)
- Data disclosure to unintended recipient
- Race condition with user actions

**Recommendations:**
1. Verify focused application before posting events
2. Use targeted event posting if possible
3. Add timing guards to detect focus changes
4. Document the behavior clearly

---

## Privacy Risk Analysis

### Data Flow Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                         CLIPBOARD FLOW                            │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  [User Action]                                                    │
│       │                                                           │
│       ▼                                                           │
│  ┌─────────┐    500ms     ┌───────────────┐    SQL      ┌──────┐ │
│  │ System  │────poll────▶│ Clipboard     │───insert───▶│SQLite│ │
│  │Clipboard│              │ Monitor Thread│              │  DB  │ │
│  └─────────┘              └───────────────┘              └──────┘ │
│                                                              │    │
│                                                              │    │
│  ╔═══════════════════════════════════════════════════════════╝    │
│  ║ DATA AT REST (UNENCRYPTED):                                    │
│  ║ - ~/.sk/kit/clipboard-history.db                                 │
│  ║ - Up to 1000 entries                                           │
│  ║ - No expiry                                                    │
│  ║ - Plain text and base64 images                                 │
│  ╚════════════════════════════════════════════════════════════    │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                    SELECTED TEXT FLOW                             │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  [Script Request]                                                 │
│       │                                                           │
│       ▼                                                           │
│  ┌─────────────┐   AX API   ┌──────────────┐                     │
│  │ Script Kit  │───────────▶│  Target App  │                     │
│  │    GPUI     │◀───────────│  (via AX)    │                     │
│  └─────────────┘            └──────────────┘                     │
│       │                                                           │
│       │ (Fallback: Clipboard simulation)                          │
│       ▼                                                           │
│  ┌─────────────┐  Cmd+C/V   ┌──────────────┐                     │
│  │  Clipboard  │◀──────────▶│  Target App  │                     │
│  │  (arboard)  │            │              │                     │
│  └─────────────┘            └──────────────┘                     │
│                                                                   │
│  ╔═══════════════════════════════════════════════════════════    │
│  ║ PERMISSION GATE: Accessibility permission required             │
│  ║ - Checked before every operation                               │
│  ║ - macOS enforced                                               │
│  ╚════════════════════════════════════════════════════════════    │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### Privacy Implications

| Aspect | Risk Level | Details |
|--------|------------|---------|
| Data Collection | HIGH | All clipboard data captured automatically |
| Data Storage | HIGH | Plain text SQLite, no encryption |
| Data Retention | MEDIUM | 1000 entries, no time-based expiry |
| Consent | MEDIUM | No explicit opt-in for clipboard monitoring |
| Transparency | MEDIUM | No visual indicator of active monitoring |
| Access Control | LOW | File system permissions only |
| Cross-App Access | MEDIUM | Accessibility API can read any app's selection |

---

## Data Retention Concerns

### Current Implementation

| Aspect | Current State | Risk |
|--------|---------------|------|
| Max entries | 1000 | Long history of sensitive data |
| Time expiry | None | Data persists indefinitely |
| Image storage | Up to 100MB cache + DB | Large potential data exposure |
| Pinned entries | Never expire | Could accumulate sensitive data |
| Clear mechanism | Manual only | Users may forget to clear |

### Recommended Data Retention Policy

```rust
// Suggested configuration structure
struct ClipboardRetentionPolicy {
    max_entries: usize,           // Current: 1000
    max_age_days: u32,            // Suggested: 30
    max_image_entries: usize,     // Suggested: 50
    max_image_size_mb: f32,       // Suggested: 5.0
    auto_delete_sensitive: bool,  // Suggested: true
    sensitive_item_max_age_minutes: u32, // Suggested: 5
}
```

---

## Permission Model Review

### Clipboard History

| Permission | Required | Implementation | Notes |
|------------|----------|----------------|-------|
| File System | Yes | ~/.sk/kit/ directory | Standard user home |
| Clipboard Read | Yes | arboard crate | No special permission on macOS |
| Clipboard Write | Yes | arboard crate | No special permission on macOS |
| Background Execution | Yes | std::thread | No special permission |

**Finding:** Clipboard access on macOS does **not** require user permission. This is a platform limitation that means:
- Any application can read clipboard contents
- Users have no control over which apps access clipboard
- This is standard macOS behavior (not a bug in Script Kit)

### Selected Text

| Permission | Required | Implementation | Notes |
|------------|----------|----------------|-------|
| Accessibility | Yes | macOS AX API | System permission dialog |
| Clipboard (fallback) | Yes | arboard crate | No special permission |
| Keyboard Events | Yes | Core Graphics | Requires Accessibility permission |

**Finding:** The selected text module properly gates all operations behind macOS Accessibility permission. This is the correct approach.

---

## Risk Rating

### Overall Assessment: MEDIUM-HIGH

| Category | Score (1-5) | Weight | Weighted |
|----------|-------------|--------|----------|
| Data Exposure | 4 | 30% | 1.2 |
| Sensitive Data Handling | 4 | 25% | 1.0 |
| Permission Model | 2 | 20% | 0.4 |
| Data Retention | 3 | 15% | 0.45 |
| Transparency | 3 | 10% | 0.3 |
| **Total** | | **100%** | **3.35/5** |

### Risk Matrix

```
           │ Low Impact │ Medium Impact │ High Impact │
───────────┼────────────┼───────────────┼─────────────┤
High Prob  │            │ CH-02, CH-06  │ CH-01, CH-04│
───────────┼────────────┼───────────────┼─────────────┤
Med Prob   │ ST-02,ST-03│ CH-03, CH-05  │             │
───────────┼────────────┼───────────────┼─────────────┤
Low Prob   │ ST-01      │ ST-04         │             │
───────────┴────────────┴───────────────┴─────────────┘
```

---

## Recommendations Summary

### Priority 1 (Critical)
1. **Implement database encryption** using SQLCipher or similar
2. **Add sensitive data filtering** to prevent capturing passwords/tokens
3. **Add explicit user consent** before enabling clipboard monitoring

### Priority 2 (High)
4. **Implement time-based expiry** for clipboard entries (default: 30 days)
5. **Add visual indicator** for active clipboard monitoring
6. **Add option to disable image capture** (default: off)

### Priority 3 (Medium)
7. **Add configuration options** for retention policy
8. **Implement "incognito mode"** to temporarily disable capture
9. **Add clear history button** prominently in UI
10. **Document privacy implications** in user-facing documentation

### Priority 4 (Low)
11. **Improve clipboard restoration** reliability in selected_text operations
12. **Add focus verification** before keyboard simulation
13. **Consider integration** with macOS Keychain for encryption keys

---

## Appendix: Code References

### Key Functions Reviewed

| Function | File | Line | Purpose |
|----------|------|------|---------|
| `init_clipboard_history` | clipboard_history.rs | 231 | Initializes monitoring |
| `clipboard_monitor_loop` | clipboard_history.rs | 299 | Main polling loop |
| `add_entry` | clipboard_history.rs | 413 | Stores clipboard content |
| `get_connection` | clipboard_history.rs | 184 | Database connection |
| `get_selected_text` | selected_text.rs | 133 | Reads selected text |
| `set_selected_text` | selected_text.rs | 186 | Writes selected text |
| `simulate_paste_with_cg` | selected_text.rs | 246 | Keyboard simulation |
| `has_accessibility_permission` | selected_text.rs | 37 | Permission check |

### External Dependencies

| Crate | Version | Purpose | Security Notes |
|-------|---------|---------|----------------|
| arboard | * | Clipboard access | Well-maintained |
| rusqlite | * | SQLite database | Does not include encryption |
| get_selected_text | * | AX API wrapper | Handles fallback securely |
| macos_accessibility_client | * | Permission checking | Official Apple API wrapper |
| core-graphics | * | Keyboard simulation | Low-level but necessary |

---

*End of Security Audit Report*
