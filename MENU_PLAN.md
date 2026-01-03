# Menu Tray Enhancement Plan

## Current State (v0.1)

The menu tray currently has three basic items:
- **Open Script Kit** - Shows/focuses the main window
- **Settings** - Opens `~/.kit/config.ts` in editor
- **Quit** - Exits the application

---

## Phase 1: Essential Menu Items

### 1.1 Recent Scripts Submenu
**Priority: High** | **Complexity: Medium**

Display the 5-10 most recently run scripts for quick access.

```
Script Kit
├── Open Script Kit          ⌘;
├── ─────────────────────
├── Recent Scripts           ▸
│   ├── hello-world.ts
│   ├── screenshot.ts
│   ├── clipboard-history.ts
│   └── Clear Recent...
├── ─────────────────────
├── Settings
└── Quit
```

**Implementation Steps:**
1. Add `recent_scripts: Vec<String>` tracking in `ScriptListApp`
2. Store recent scripts in `~/.kit/db/recent.json` (persist across restarts)
3. Create `Submenu` in `TrayManager::create_menu()` using muda's `Submenu::new()`
4. Dynamically rebuild menu when scripts are run
5. Add "Clear Recent" menu item at bottom of submenu

**Files to Modify:**
- `src/tray.rs` - Add submenu support, dynamic menu rebuilding
- `src/main.rs` - Track script execution, update menu
- `src/scripts.rs` - Add recent scripts storage/retrieval

---

### 1.2 New Script Action
**Priority: High** | **Complexity: Low**

Quick way to create a new script without opening the main window.

```
├── New Script...            ⌘N
```

**Implementation Steps:**
1. Add `MenuItem::new("New Script...", true, Some(Accelerator::new(Modifiers::META, Code::KeyN)))`
2. Handler opens editor with a new script template at `~/.sk/kit/scripts/`
3. Generate unique filename: `script-{timestamp}.ts`
4. Pre-populate with Script Kit template:
   ```typescript
   // Name: New Script
   // Description: 
   
   import "@johnlindquist/kit"
   
   
   ```

**Files to Modify:**
- `src/tray.rs` - Add menu item
- `src/main.rs` - Add handler to create and open new script

---

### 1.3 Edit Scripts Folder
**Priority: Medium** | **Complexity: Low**

Open the scripts folder in Finder/editor.

```
├── Edit Scripts Folder      ⌘⇧E
```

**Implementation Steps:**
1. Add menu item with accelerator
2. Handler uses `open ~/.sk/kit/scripts` (macOS `open` command)
3. Alternative: open in configured editor

**Files to Modify:**
- `src/tray.rs` - Add menu item
- `src/main.rs` - Add handler

---

## Phase 2: Script Management

### 2.1 Pinned/Favorite Scripts
**Priority: Medium** | **Complexity: Medium**

Allow users to pin frequently-used scripts to the menu.

```
├── ─────────────────────
├── ⭐ screenshot.ts
├── ⭐ clipboard-history.ts  
├── ─────────────────────
```

**Implementation Steps:**
1. Add `pinned` field to script metadata in `~/.kit/db/scripts.json`
2. Create pinned scripts section in menu (between Recent and Settings)
3. Add "Pin to Menu" / "Unpin" in main window's actions dialog
4. Limit to 5-7 pinned scripts max
5. Allow reordering via drag in main UI (future)

**Files to Modify:**
- `src/tray.rs` - Add pinned section
- `src/scripts.rs` - Add pinned script storage
- `src/actions.rs` - Add pin/unpin actions
- `src/main.rs` - Sync pinned scripts to menu

---

### 2.2 Run Script by Name
**Priority: Low** | **Complexity: High**

Type to search and run a script directly from menu bar.

```
├── Run Script...            ⌘⇧R
    [Opens mini search popup near tray icon]
```

**Implementation Steps:**
1. Create mini floating window (like Spotlight)
2. Position near tray icon location
3. Fuzzy search through scripts
4. Enter to run, Escape to dismiss
5. Consider using existing search logic from `ScriptListApp`

**Files to Modify:**
- `src/tray.rs` - Add menu item
- `src/main.rs` - Create mini search window
- New file: `src/mini_search.rs` - Mini search UI component

---

## Phase 3: System Integration

### 3.1 Start at Login
**Priority: High** | **Complexity: Medium**

Toggle to automatically start Script Kit on login.

```
├── ─────────────────────
├── ☑ Start at Login
```

**Implementation Steps:**
1. Add `CheckMenuItem` (muda supports this)
2. macOS: Use `SMAppService` or LaunchAgent plist
3. Store preference in config
4. Create/remove `~/Library/LaunchAgents/com.scriptkit.gpui.plist`

**LaunchAgent plist template:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.scriptkit.gpui</string>
    <key>ProgramArguments</key>
    <array>
        <string>/path/to/script-kit-gpui</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
```

**Files to Modify:**
- `src/tray.rs` - Add CheckMenuItem
- `src/config.rs` - Add `start_at_login: bool`
- New file: `src/login_item.rs` - LaunchAgent management

---

### 3.2 Check for Updates
**Priority: Medium** | **Complexity: Medium**

Check if a newer version is available.

```
├── Check for Updates...
```

**Implementation Steps:**
1. Add menu item
2. Fetch latest release from GitHub API
3. Compare with current version (from `Cargo.toml`)
4. Show notification if update available
5. Optional: Open releases page in browser

**Files to Modify:**
- `src/tray.rs` - Add menu item
- New file: `src/updates.rs` - Update checking logic

---

### 3.3 Show/Hide Dock Icon
**Priority: Low** | **Complexity: Medium**

Toggle whether the app appears in the Dock.

```
├── ☐ Show in Dock
```

**Implementation Steps:**
1. Add CheckMenuItem
2. macOS: Use `NSApp.setActivationPolicy()`
   - `.regular` = show in dock
   - `.accessory` = hide from dock (menu bar only)
3. Store preference in config
4. Apply on startup

**Files to Modify:**
- `src/tray.rs` - Add CheckMenuItem
- `src/config.rs` - Add `show_in_dock: bool`
- `src/main.rs` - Apply activation policy

---

## Phase 4: Developer Features

### 4.1 View Logs
**Priority: Medium** | **Complexity: Low**

Quick access to application logs.

```
├── View Logs               ⌘L
```

**Implementation Steps:**
1. Add menu item
2. Open `~/.kit/logs/script-kit-gpui.jsonl` in Console.app or editor
3. Or: open the in-app log panel (Cmd+L already works in window)

**Files to Modify:**
- `src/tray.rs` - Add menu item
- `src/main.rs` - Add handler

---

### 4.2 Reload Scripts
**Priority: Medium** | **Complexity: Low**

Force reload all scripts without restarting.

```
├── Reload Scripts          ⌘R
```

**Implementation Steps:**
1. Add menu item
2. Call existing `reload_scripts()` in `ScriptListApp`
3. Show brief notification on completion

**Files to Modify:**
- `src/tray.rs` - Add menu item
- `src/main.rs` - Add handler

---

### 4.3 Debug Menu (Development Only)
**Priority: Low** | **Complexity: Low**

Submenu with debugging tools (only in debug builds).

```
├── Debug                    ▸
│   ├── Show Window Bounds
│   ├── Log Focus State
│   ├── Dump Script Cache
│   └── Force GC
```

**Implementation Steps:**
1. Wrap in `#[cfg(debug_assertions)]`
2. Add submenu with diagnostic actions
3. Each action logs to JSONL or shows alert

**Files to Modify:**
- `src/tray.rs` - Add debug submenu (conditional)
- `src/main.rs` - Add debug handlers

---

## Implementation Order

### Sprint 1: Core Functionality
1. ✅ Basic menu (Open, Settings, Quit) - DONE
2. New Script action (1.2)
3. Edit Scripts Folder (1.3)
4. Recent Scripts submenu (1.1)

### Sprint 2: Polish
5. Start at Login (3.1)
6. Reload Scripts (4.2)
7. View Logs (4.1)

### Sprint 3: Power User
8. Pinned Scripts (2.1)
9. Show/Hide Dock Icon (3.3)
10. Check for Updates (3.2)

### Sprint 4: Advanced
11. Run Script by Name (2.2)
12. Debug Menu (4.3)

---

## Technical Notes

### Menu Rebuilding
The tray-icon/muda crate doesn't support dynamic menu updates easily. Options:
1. **Recreate entire TrayIcon** - Works but may flicker
2. **Use menu item enable/disable** - Limited functionality
3. **Native NSMenu integration** - Most flexible, requires unsafe Objective-C

Recommendation: Start with option 1 (recreate) for MVP, optimize later.

### Keyboard Shortcuts in Menu
muda supports accelerators via `Accelerator::new(modifiers, key)`:
```rust
MenuItem::new("New Script", true, Some(Accelerator::new(
    Modifiers::META, 
    Code::KeyN
)))
```

Note: These are display-only hints. Global hotkeys need separate `global-hotkey` registration.

### State Synchronization
Menu state (recent scripts, pinned, etc.) should sync with:
1. Main window UI
2. Persisted storage (`~/.kit/db/`)
3. File system watchers (for external changes)

Use `Arc<Mutex<MenuState>>` or channels for thread-safe updates.

---

## Menu Structure (Final Vision)

```
Script Kit
├── Open Script Kit              ⌘;
├── New Script...                ⌘N
├── Run Script...                ⌘⇧R
├── ─────────────────────────────
├── Recent Scripts               ▸
│   ├── hello-world.ts
│   ├── screenshot.ts
│   └── Clear Recent...
├── ─────────────────────────────
├── ⭐ clipboard-history.ts
├── ⭐ quick-note.ts
├── ─────────────────────────────
├── Edit Scripts Folder          ⌘⇧E
├── Reload Scripts               ⌘R
├── View Logs                    ⌘L
├── ─────────────────────────────
├── ☑ Start at Login
├── ☐ Show in Dock
├── Check for Updates...
├── ─────────────────────────────
├── Settings                     ⌘,
└── Quit Script Kit              ⌘Q
```
