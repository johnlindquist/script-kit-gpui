# Script Kit GPUI Glossary

> A reference mapping user-facing concepts to code terminology.

---

## Quick Reference Table

| User Term | Code Term | Location | Description |
|-----------|-----------|----------|-------------|
| Main Window | `ScriptListApp` | `src/main.rs` | Root application struct |
| List Item | `ListItem` | `src/list_item.rs` | A row showing script name, description, icon |
| Selected Item | `is_selected: bool` | - | Item with accent bar highlight |
| Search Box | `filter_text: String` | `src/main.rs` | Input field for filtering scripts |
| Preview Panel | `preview_cache_lines` | - | Syntax-highlighted code preview |
| Actions Menu | `ActionsDialog` | `src/actions.rs` | Cmd+K popup for quick actions |
| Toast | `Toast` | `src/toast.rs` | Auto-dismissing notification |
| Design Variant | `DesignVariant` | `src/designs/` | Visual theme (11 variants) |
| Prompt | `*Prompt` structs | `src/prompts/` | Interactive UI for user input |
| Choice | `Choice` | SDK types | Option in a list `{name, value}` |
| Scrollbar | `Scrollbar` | `src/components/` | Draggable scroll indicator |
| Button | `Button` | `src/components/` | Clickable UI element |
| Theme | `Theme` | `src/theme.rs` | Color and styling configuration |
| Shortcut Badge | - | `ListItem` | Right-aligned keyboard hint |
| Accent Bar | - | `ListItem` | 3px vertical selection indicator |

---

## UI Components

### Window Structure

| Component | Description | Code Location |
|-----------|-------------|---------------|
| **Main Window** | The primary Script Kit interface | `ScriptListApp` in `src/main.rs` |
| **Title Bar** | Window drag area with controls | Configured via `WindowBounds` |
| **Search Box** | Filter input at top of window | `filter_text` field |
| **Script List** | Scrollable list of available scripts | `uniform_list` rendering |
| **Preview Panel** | Code preview on the right side | `preview_cache_lines` |
| **Actions Dialog** | Quick actions popup (Cmd+K) | `ActionsDialog` in `src/actions.rs` |

### List Components

| Component | Description | Dimensions |
|-----------|-------------|------------|
| **List Item** | Single script row | 48px height |
| **Section Header** | Non-selectable group label ("RECENT", "MAIN") | `GroupedListItem::SectionHeader` |
| **Accent Bar** | Selection indicator on left | 3px wide, accent color |
| **Shortcut Badge** | Keyboard hint on right | Right-aligned text |
| **Icon** | Script/app icon | 24x24px |

### Feedback Components

| Component | Variants | Usage |
|-----------|----------|-------|
| **Toast** | Error (red), Success (green), Warning (yellow), Info (blue) | Auto-dismiss notifications |
| **Button** | Primary (filled), Ghost (text), Icon (compact) | User actions |
| **Scrollbar** | Thumb (draggable), Track (background) | List navigation |

---

## SDK Functions

### Prompts (User Input)

```typescript
// Text input with optional choices
await arg("Search scripts", ["option1", "option2"])

// Display HTML content
await div(`<div class="p-4">Hello</div>`)

// Code editor
await editor("const x = 1", "typescript")

// Terminal emulator
await term("npm install")

// Multi-select
await select("Choose items", choices)

// File browser
await path({ startPath: "~" })

// Secure input (stored in keychain)
await env("API_KEY")

// Drag-and-drop zone
await drop()

// Tab-through placeholders
await template("Hello {{name}}")

// Multiple form fields
await fields([
  { name: "email", label: "Email", type: "email" },
  { name: "password", label: "Password", type: "password" }
])

// Keyboard shortcut capture
await hotkey("Press a key combo")

// Compact variants
await mini("Quick input", choices)  // Smaller window
await micro("Tiny", choices)        // Minimal window
```

### Window Control

```typescript
show()                    // Show window
hide()                    // Hide window
blur()                    // Remove focus
getWindowBounds()         // Get position/size
captureScreenshot()       // Capture window as PNG
submit(value)             // Submit and close prompt
exit()                    // Close Script Kit
```

### Content Setters

```typescript
setPanel(html)           // Set bottom panel content
setPreview(html)         // Set right preview content
setPrompt(options)       // Update prompt configuration
```

### Clipboard

```typescript
copy(text)                      // Copy to clipboard
paste()                         // Paste from clipboard
clipboard.readText()            // Read clipboard text
clipboard.writeText(text)       // Write clipboard text
clipboard.readImage()           // Read clipboard image
clipboard.writeImage(buffer)    // Write clipboard image
```

### System

```typescript
beep()                   // System beep sound
say(text)                // Text-to-speech
notify(title, body)      // System notification
setStatus(text)          // Set menu bar status
menu(items)              // Create menu bar menu
```

### Utilities

```typescript
md(markdown)             // Markdown to HTML
uuid()                   // Generate UUID
wait(ms)                 // Delay execution
run(script)              // Run another script
home()                   // Home directory path
skPath(...)            // ~/.sk/kit path helper
kitPath(...)             // Kit installation path
tmpPath(...)             // Temp directory path
```

---

## Prompt Types

### SDK to Rust Mapping

| SDK Function | Rust Type | UI Appearance |
|--------------|-----------|---------------|
| `arg()` | `ArgPrompt` | Input field + choice list |
| `div()` | `DivPrompt` | HTML content (no input) |
| `editor()` | `EditorPrompt` | Full code editor |
| `term()` | `TermPrompt` | Interactive terminal |
| `select()` | `SelectPrompt` | Multi-select with checkboxes |
| `path()` | `PathPrompt` | File/folder browser |
| `env()` | `EnvPrompt` | Secure input + keychain |
| `drop()` | `DropPrompt` | Drag-and-drop zone |
| `template()` | `TemplatePrompt` | Tab-through placeholders |
| `fields()` | `FormPrompt` | Multiple labeled inputs |
| `form()` | `FormPrompt` | Custom HTML form |
| `hotkey()` | (inline) | Shortcut capture |
| `mini()` | `ArgPrompt` variant | Compact prompt |
| `micro()` | `ArgPrompt` variant | Tiny prompt |

### Built-in Views (Not SDK Prompts)

| View | Purpose | Trigger |
|------|---------|---------|
| `ScriptList` | Main script browser | Default view |
| `ClipboardHistoryView` | Clipboard history with pinning | Built-in hotkey |
| `AppLauncherView` | Launch applications | Built-in hotkey |
| `WindowSwitcherView` | Switch windows | Built-in hotkey |
| `DesignGalleryView` | Browse design themes | Settings |

---

## Common Scenarios

### "When I want X, say Y"

| I want to... | SDK Function | Example |
|--------------|--------------|---------|
| Ask user for text | `arg()` | `await arg("Enter name")` |
| Show a list of options | `arg()` with choices | `await arg("Pick", ["A", "B"])` |
| Display HTML | `div()` | `await div("<h1>Hello</h1>")` |
| Edit code | `editor()` | `await editor(code, "js")` |
| Run shell command | `term()` | `await term("ls -la")` |
| Pick multiple items | `select()` | `await select("Choose", items)` |
| Browse files | `path()` | `await path()` |
| Store a secret | `env()` | `await env("API_KEY")` |
| Accept file drops | `drop()` | `await drop()` |
| Fill template | `template()` | `await template("Hi {{name}}")` |
| Show a form | `fields()` | `await fields([...])` |
| Capture hotkey | `hotkey()` | `await hotkey()` |
| Show notification | `notify()` | `notify("Done!")` |
| Copy to clipboard | `copy()` | `copy("text")` |

### Actions Dialog (Cmd+K)

| Action | Shortcut | Description |
|--------|----------|-------------|
| Run Script | `↵` | Execute selected script |
| Edit Script | `⌘E` | Open in editor |
| View Logs | `⌘L` | Show script logs |
| Reveal in Finder | `⌘⇧F` | Open in Finder |
| Copy Path | `⌘⇧C` | Copy script path |
| Create Script | `⌘N` | New script wizard |
| Reload | `⌘R` | Refresh script list |
| Settings | `⌘,` | Open preferences |
| Quit | `⌘Q` | Exit Script Kit |

---

## Key Types

### Choice

```typescript
interface Choice {
  name: string       // Display text
  value: string      // Return value
  description?: string
  icon?: string
  shortcut?: string
}
```

### FieldDef

```typescript
interface FieldDef {
  name: string       // Field identifier
  label: string      // Display label
  type: string       // "text" | "email" | "password" | etc.
  placeholder?: string
  value?: string     // Default value
}
```

### HotkeyInfo

```typescript
interface HotkeyInfo {
  key: string        // Key pressed
  command: boolean   // Cmd/Ctrl held
  shift: boolean
  option: boolean    // Alt
  control: boolean
  shortcut: string   // Human-readable
  keyCode: number
}
```

### FileInfo

```typescript
interface FileInfo {
  path: string
  name: string
  size: number
}
```

---

## Design Variants

Script Kit GPUI includes 11 design variants:

| Variant | Description |
|---------|-------------|
| Default | Clean, modern appearance |
| Minimal | Reduced visual elements |
| Retro Terminal | CRT/terminal aesthetic |
| Brutalist | Bold, stark design |
| Compact | Dense information layout |
| Glassmorphism | Frosted glass effects |
| Apple HIG | macOS Human Interface Guidelines |
| Neo Brutalist | Modern brutalist style |
| Newspaper | Print-inspired typography |
| Windows 95 | Classic Windows aesthetic |
| Outline | Border-focused design |

Access via `DesignVariant` enum in `src/designs/`.

---

## File Locations

| Concept | Path |
|---------|------|
| Main app | `src/main.rs` |
| Prompts | `src/prompts/*.rs` |
| Components | `src/components/*.rs` |
| Design variants | `src/designs/*.rs` |
| Theme system | `src/theme.rs` |
| Actions dialog | `src/actions.rs` |
| List item | `src/list_item.rs` |
| SDK source | `scripts/kit-sdk.ts` |
| Test scripts | `tests/smoke/`, `tests/sdk/` |
| User config | `~/.sk/kit/config.ts` |
| User theme | `~/.sk/kit/theme.json` |
| Logs | `~/.sk/kit/logs/script-kit-gpui.jsonl` |
