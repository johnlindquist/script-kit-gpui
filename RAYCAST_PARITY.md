# Raycast Feature Parity Roadmap

## Executive Summary

This document provides a comprehensive roadmap for achieving feature parity between Script Kit GPUI and Raycast. The goal is to make Script Kit a compelling alternative for users who need Raycast-like functionality with the power of scripting.

### Project Status

| Metric | Value |
|--------|-------|
| **Current SDK Methods** | 55+ implemented |
| **Raycast Extensions to Match** | 12 core extensions |
| **API Surface to Cover** | ~40 Raycast API concepts |
| **Estimated Total Effort** | 16-24 weeks |

### Key Gaps to Address

1. **ActionPanel with keyboard shortcuts per list item** (Critical)
2. **List.Item.Detail split-view** (Critical)
3. **Grid component for image layouts** (High)
4. **Navigation stack (push/pop views)** (High)
5. **Toast states (loading/success/failure)** (Medium)

---

## Table of Contents

1. [Feature Matrix](#1-feature-matrix)
2. [Implementation Tiers](#2-implementation-tiers)
3. [API Mapping Table](#3-api-mapping-table)
4. [Detailed Feature Specs](#4-detailed-feature-specs)
5. [Technical Architecture Notes](#5-technical-architecture-notes)
6. [Testing Strategy](#6-testing-strategy)
7. [Dependencies](#7-dependencies)

---

## 1. Feature Matrix

### Core Extension Comparison

| Raycast Extension | Script Kit Status | Gap Analysis | Priority |
|-------------------|------------------|--------------|----------|
| **Clipboard History** | Partial (copy/paste only) | Needs: history storage, search, pinning | P1 |
| **Window Management** | Not implemented | Needs: Accessibility API integration | P2 |
| **Snippets** | Partial (template exists) | Needs: auto-expand triggers, sync | P2 |
| **File Search** | Partial (find, path) | Needs: Quick Look preview | P1 |
| **Quicklinks** | Not implemented | Needs: URL shortcuts, placeholders | P3 |
| **Calculator** | Not implemented | Needs: inline math, units, currency | P2 |
| **Calendar** | Not implemented | Needs: EventKit integration | P3 |
| **System** | Partial (basic) | Needs: dark mode toggle, more controls | P2 |
| **Emoji Picker** | Not implemented | Needs: emoji search, skin tones | P3 |
| **Raycast Notes** | Not implemented | Needs: floating window, markdown, sync | P3 |
| **Raycast AI** | Partial (chat exists) | Needs: quick AI, tool calling | P1 |
| **Raycast Focus** | Not implemented | Needs: app blocking, sessions | P3 |

### Prompt Type Comparison

| Raycast Prompt | Script Kit Equivalent | Status | Gap |
|----------------|----------------------|--------|-----|
| `List` | `arg()` | ✅ Implemented | Needs List.Item.Detail |
| `List` (multi-select) | `select()` | ✅ Implemented | - |
| `Form` | `fields()`, `form()` | ✅ Implemented | Needs DatePicker, FilePicker |
| `Detail` | `div()`, `md()` | ✅ Implemented | Needs metadata panel |
| `Grid` | Not implemented | ❌ Missing | New component needed |
| `Alert` | `notify()` | Partial | Needs confirmation dialogs |
| `ActionPanel` | `actions` property | Partial | Needs per-item actions |
| `MenuBarExtra` | `menu()` | ✅ Basic | Needs rich menu items |

### SDK Method Inventory (Current State)

**Prompt APIs (20+ methods):**
- ✅ `arg()` - Text input with choices
- ✅ `div()` - Display HTML content
- ✅ `md()` - Markdown to HTML
- ✅ `mini()` - Compact prompt
- ✅ `micro()` - Tiny prompt
- ✅ `select()` - Multi-select
- ✅ `fields()` - Multi-field form
- ✅ `form()` - Custom HTML form
- ✅ `path()` - File/folder browser
- ✅ `hotkey()` - Capture keyboard shortcut
- ✅ `drop()` - Drag and drop zone
- ✅ `template()` - Tab-through template
- ✅ `env()` - Environment variable prompt
- ✅ `editor()` - Code editor (in progress)
- ✅ `chat()` - Conversational UI
- ✅ `widget()` - Floating HTML widget
- ✅ `term()` - Terminal window
- ✅ `webcam()` - Webcam capture
- ✅ `mic()` - Audio recording
- ✅ `eyeDropper()` - Color picker
- ✅ `find()` - Spotlight file search

**System APIs (15+ methods):**
- ✅ `beep()` - System beep
- ✅ `say()` - Text-to-speech
- ✅ `notify()` - System notification
- ✅ `setStatus()` - App status
- ✅ `menu()` - Menu bar
- ✅ `copy()` / `paste()` - Clipboard text
- ✅ `clipboard.readImage()` / `writeImage()` - Clipboard images
- ✅ `setSelectedText()` / `getSelectedText()` - Accessibility
- ✅ `keyboard.type()` / `keyboard.tap()` - Keyboard simulation
- ✅ `mouse.move()` / `click()` / `setPosition()` - Mouse control

**Utility APIs (20+ methods):**
- ✅ `exec()` - Shell commands
- ✅ `get()` / `post()` / `put()` / `patch()` / `del()` - HTTP
- ✅ `download()` - Download files
- ✅ `trash()` - Move to trash
- ✅ `show()` / `hide()` / `blur()` - Window control
- ✅ `submit()` / `exit()` / `wait()` - Flow control
- ✅ `setPanel()` / `setPreview()` / `setPrompt()` - Content setters
- ✅ `home()` / `skPath()` / `kitPath()` / `tmpPath()` - Path utilities
- ✅ `isFile()` / `isDir()` / `isBin()` - File checks
- ✅ `db()` / `store` / `memoryMap` - Storage
- ✅ `browse()` / `editFile()` / `run()` - App utilities
- ✅ `uuid()` / `compile()` - Misc utilities

---

## 2. Implementation Tiers

### Tier 1: Quick Wins (1-2 weeks each)

| Feature | Description | Effort | Value |
|---------|-------------|--------|-------|
| **Toast States** | Loading/success/failure states for notify() | 3 days | High |
| **Alert Confirmation** | Blocking confirmation dialogs | 3 days | High |
| **Clipboard History UI** | Search and select from history | 1 week | High |
| **Quicklinks** | URL shortcuts with placeholders | 3 days | Medium |
| **System Toggles** | Dark mode, volume, Do Not Disturb | 3 days | Medium |

### Tier 2: Core Features (2-4 weeks each)

| Feature | Description | Effort | Value |
|---------|-------------|--------|-------|
| **ActionPanel** | Per-item keyboard shortcuts and actions | 2 weeks | Critical |
| **List.Item.Detail** | Split-view with detail pane | 2 weeks | Critical |
| **Form.DatePicker** | Native date/time picker | 1 week | High |
| **Form.FilePicker** | Native file picker | 1 week | High |
| **Navigation Stack** | Push/pop views like iOS | 2 weeks | High |
| **Calculator** | Inline math, units, currency | 2 weeks | Medium |

### Tier 3: Advanced Features (4-8 weeks each)

| Feature | Description | Effort | Value |
|---------|-------------|--------|-------|
| **Grid Component** | Image grid layout | 4 weeks | High |
| **Snippets Manager** | Text expansion with triggers | 4 weeks | High |
| **AI Tools** | Tool calling, function execution | 4 weeks | High |
| **File Preview** | Quick Look integration | 3 weeks | Medium |
| **OAuth PKCE** | Secure OAuth flow | 2 weeks | Medium |

### Tier 4: Complex Features (8+ weeks each)

| Feature | Description | Effort | Value |
|---------|-------------|--------|-------|
| **Window Management** | Move/resize windows, snapping | 8 weeks | High |
| **Raycast Notes** | Floating window, markdown, sync | 8 weeks | Medium |
| **Raycast Focus** | App/website blocking | 6 weeks | Medium |
| **Calendar Integration** | EventKit, event management | 6 weeks | Medium |

---

## 3. API Mapping Table

### Raycast → Script Kit API Mapping

| Raycast API | Script Kit Equivalent | Status |
|-------------|----------------------|--------|
| `List` | `arg(placeholder, choices)` | ✅ |
| `List.Item` | Choice object `{name, value, description}` | ✅ |
| `List.Item.Detail` | - | ❌ NEEDS IMPL |
| `List.Item.Accessories` | Choice metadata | Partial |
| `List.EmptyView` | Empty state in arg() | ✅ |
| `List.Dropdown` | - | ❌ NEEDS IMPL |
| `Grid` | - | ❌ NEEDS IMPL |
| `Grid.Item` | - | ❌ NEEDS IMPL |
| `Detail` | `div(html)` | ✅ |
| `Detail.Metadata` | - | ❌ NEEDS IMPL |
| `Form` | `fields(fieldDefs)` | ✅ |
| `Form.TextField` | FieldDef with type: 'text' | ✅ |
| `Form.PasswordField` | FieldDef with type: 'password' | ✅ |
| `Form.TextArea` | - | ❌ NEEDS IMPL |
| `Form.Checkbox` | - | ❌ NEEDS IMPL |
| `Form.DatePicker` | - | ❌ NEEDS IMPL |
| `Form.FilePicker` | - | ❌ NEEDS IMPL |
| `Form.Dropdown` | - | ❌ NEEDS IMPL |
| `Form.TagPicker` | - | ❌ NEEDS IMPL |
| `ActionPanel` | Actions in arg config | Partial |
| `Action` | Shortcut in arg config | Partial |
| `Action.CopyToClipboard` | `copy(text)` | ✅ |
| `Action.OpenInBrowser` | `browse(url)` | ✅ |
| `Action.Push` | - | ❌ NEEDS IMPL |
| `Action.Pop` | - | ❌ NEEDS IMPL |
| `Toast` | `notify(options)` | Partial |
| `Toast.Style.Animated` | - | ❌ NEEDS IMPL |
| `Toast.Style.Success` | - | ❌ NEEDS IMPL |
| `Toast.Style.Failure` | - | ❌ NEEDS IMPL |
| `Alert` | - | ❌ NEEDS IMPL |
| `Clipboard.copy` | `copy(text)` | ✅ |
| `Clipboard.paste` | `paste()` | ✅ |
| `Clipboard.readText` | `clipboard.readText()` | ✅ |
| `environment` | `process.env`, `env()` | ✅ |
| `getPreferenceValues` | - | ❌ NEEDS IMPL |
| `LocalStorage` | `store` API | ✅ |
| `Cache` | - | ❌ NEEDS IMPL |
| `useNavigation` | - | ❌ NEEDS IMPL |
| `useFrecencySorting` | - | ❌ NEEDS IMPL |
| `usePromise` | Native async/await | ✅ |
| `useForm` | `fields()` / `form()` | ✅ |
| `MenuBarExtra` | `menu()` | Partial |
| `OAuth.PKCEClient` | - | ❌ NEEDS IMPL |
| `AI` | `chat()` | Partial |
| `AI.ask` | - | ❌ NEEDS IMPL |

---

## 4. Detailed Feature Specs

### 4.1 ActionPanel with Per-Item Actions

**Description:** Each list item can have its own set of actions with keyboard shortcuts, displayed in a popup panel.

**User Stories:**
- As a user, I want to press Enter on a list item and see available actions
- As a user, I want to use Cmd+K to open the action panel
- As a user, I want each action to have a visible keyboard shortcut
- As a user, I want to execute actions without closing the main list

**UI Mockup (ASCII):**
```
┌─────────────────────────────────────────┐
│ 🔍 Search scripts...                    │
├─────────────────────────────────────────┤
│ ▸ hello-world.ts                        │
│   greeting-script.ts                    │
│   file-organizer.ts                     │
└─────────────────────────────────────────┘
         │
         ▼ (Cmd+K or Enter)
┌─────────────────────────────────────────┐
│ Actions for: hello-world.ts             │
├─────────────────────────────────────────┤
│ ▸ Run Script              ⏎ Enter       │
│   Edit Script             ⌘E            │
│   Copy Path               ⌘C            │
│   Reveal in Finder        ⌘⇧F           │
│   Delete                  ⌘⌫            │
└─────────────────────────────────────────┘
```

**Technical Requirements:**
1. Extend `Choice` interface with `actions` array
2. Create `ActionsPanel` component in `src/actions.rs` (exists, needs enhancement)
3. Add `Cmd+K` global shortcut handler
4. Render action shortcuts with proper key icons
5. Support action groups with separators

**Files to Modify:**
- `src/actions.rs` - Enhance ActionsPanel component
- `src/prompts.rs` - Wire actions to ArgPrompt
- `scripts/kit-sdk.ts` - Extend Choice interface

**API Design:**
```typescript
interface Choice {
  name: string;
  value: string;
  description?: string;
  icon?: string;
  actions?: Action[];  // NEW
}

interface Action {
  title: string;
  shortcut?: string;  // e.g., "cmd+e", "cmd+shift+f"
  icon?: string;
  onAction: () => void | Promise<void>;
}

// Usage:
await arg("Select script", [
  {
    name: "hello-world.ts",
    value: "/path/to/hello-world.ts",
    actions: [
      { title: "Run", shortcut: "enter", onAction: () => run("hello-world") },
      { title: "Edit", shortcut: "cmd+e", onAction: () => editFile(path) },
      { title: "Copy Path", shortcut: "cmd+c", onAction: () => copy(path) },
    ]
  }
]);
```

**Estimated Effort:** 2 weeks

**Dependencies:** None

---

### 4.2 List.Item.Detail (Split-View)

**Description:** Display detailed information in a right-side panel while browsing a list.

**User Stories:**
- As a user, I want to see a preview of the selected item without opening it
- As a user, I want the detail pane to update as I navigate the list
- As a user, I want to toggle the detail pane on/off

**UI Mockup (ASCII):**
```
┌─────────────────────────────────────────────────────────────────────┐
│ 🔍 Search files...                                                  │
├──────────────────────────────┬──────────────────────────────────────┤
│ ▸ README.md                  │  # README                            │
│   package.json               │                                      │
│   src/                       │  Welcome to the project!             │
│   tests/                     │                                      │
│                              │  ## Installation                     │
│                              │  ```bash                             │
│                              │  npm install                         │
│                              │  ```                                 │
│                              │                                      │
│                              │  ## Usage                            │
│                              │  See examples directory.             │
└──────────────────────────────┴──────────────────────────────────────┘
```

**Technical Requirements:**
1. Add `detail` property to Choice interface
2. Create split-view layout in ArgPrompt
3. Support markdown rendering in detail pane
4. Support metadata key-value display
5. Add `Cmd+D` to toggle detail pane

**Files to Modify:**
- `src/prompts.rs` - Add detail pane rendering
- `scripts/kit-sdk.ts` - Extend Choice interface

**API Design:**
```typescript
interface Choice {
  name: string;
  value: string;
  detail?: {
    markdown?: string;
    metadata?: Array<{
      label: string;
      value: string;
      icon?: string;
    }>;
  };
}

// Usage:
await arg("Select file", files.map(f => ({
  name: f.name,
  value: f.path,
  detail: {
    markdown: await fs.readFile(f.path, 'utf8'),
    metadata: [
      { label: "Size", value: formatBytes(f.size) },
      { label: "Modified", value: f.mtime.toLocaleString() },
    ]
  }
})));
```

**Estimated Effort:** 2 weeks

**Dependencies:** None

---

### 4.3 Grid Component

**Description:** Display items in a grid layout, ideal for images, icons, and visual content.

**User Stories:**
- As a user, I want to browse images in a grid layout
- As a user, I want to select items by clicking or keyboard navigation
- As a user, I want adjustable column count

**UI Mockup (ASCII):**
```
┌─────────────────────────────────────────────────────────────────────┐
│ 🔍 Search wallpapers...                                             │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐                 │
│  │         │  │         │  │         │  │         │                 │
│  │  IMG 1  │  │  IMG 2  │  │  IMG 3  │  │  IMG 4  │                 │
│  │         │  │         │  │         │  │         │                 │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘                 │
│  Mountain      Beach        Forest       Desert                     │
│                                                                     │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐                 │
│  │         │  │         │  │    ▸    │  │         │                 │
│  │  IMG 5  │  │  IMG 6  │  │  IMG 7  │  │  IMG 8  │                 │
│  │         │  │         │  │         │  │         │                 │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘                 │
│  City          Lake         [Selected]   Valley                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Technical Requirements:**
1. Create `GridPrompt` component
2. Support image loading and caching
3. Implement grid keyboard navigation (arrows, home, end)
4. Support variable column counts
5. Support item sizing (small, medium, large)

**Files to Modify:**
- `src/grid.rs` - New file for GridPrompt
- `src/main.rs` - Add message handler
- `src/protocol.rs` - Add Grid message type
- `scripts/kit-sdk.ts` - Add grid() function

**API Design:**
```typescript
interface GridItem {
  title: string;
  subtitle?: string;
  value: string;
  image?: string;  // URL or base64
  actions?: Action[];
}

interface GridOptions {
  columns?: number;  // Default: 4
  itemSize?: 'small' | 'medium' | 'large';
  fit?: 'contain' | 'cover';
}

function grid(placeholder: string, items: GridItem[], options?: GridOptions): Promise<string>;

// Usage:
const wallpaper = await grid("Select wallpaper", [
  { title: "Mountain", value: "/wallpapers/mountain.jpg", image: "/wallpapers/mountain.jpg" },
  { title: "Beach", value: "/wallpapers/beach.jpg", image: "/wallpapers/beach.jpg" },
], { columns: 4, itemSize: 'medium' });
```

**Estimated Effort:** 4 weeks

**Dependencies:** Image loading in GPUI

---

### 4.4 Navigation Stack (Push/Pop)

**Description:** Allow scripts to push new views onto a navigation stack and pop back.

**User Stories:**
- As a user, I want to drill down into nested lists
- As a user, I want to press Escape to go back
- As a user, I want a breadcrumb showing my navigation path

**UI Mockup (ASCII):**
```
┌─────────────────────────────────────────┐
│ ← Back │ Projects > script-kit > src    │
├─────────────────────────────────────────┤
│ 🔍 Search in src...                     │
├─────────────────────────────────────────┤
│ ▸ main.rs                               │
│   prompts.rs                            │
│   actions.rs                            │
│   theme.rs                              │
└─────────────────────────────────────────┘
```

**Technical Requirements:**
1. Create navigation state stack in main app
2. Implement `push()` and `pop()` functions
3. Add back button and keyboard shortcut (Escape)
4. Render breadcrumb navigation
5. Handle state cleanup on pop

**Files to Modify:**
- `src/main.rs` - Add navigation stack state
- `src/prompts.rs` - Add breadcrumb rendering
- `scripts/kit-sdk.ts` - Add push/pop functions

**API Design:**
```typescript
interface NavigationContext {
  push(title: string, element: Promise<void>): Promise<void>;
  pop(): void;
}

// Usage:
await arg("Select project", projects, {
  onSelect: async (project) => {
    await push("Select file", arg("Select file in " + project.name, project.files));
  }
});

// Or with explicit push/pop:
const project = await arg("Select project", projects);
push("Files");  // Push breadcrumb
const file = await arg("Select file", getFiles(project));
pop();  // Return to previous view
```

**Estimated Effort:** 2 weeks

**Dependencies:** None

---

### 4.5 Toast States

**Description:** Show animated toast notifications with loading, success, and failure states.

**User Stories:**
- As a user, I want to see a loading spinner while an operation is in progress
- As a user, I want the toast to change to success/failure when complete
- As a user, I want toasts to stack if multiple are shown

**UI Mockup (ASCII):**
```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│                         Main Content                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
            ┌───────────────────┐
            │ ⏳ Uploading...   │  ← Loading state
            └───────────────────┘
                    │
                    ▼ (operation completes)
            ┌───────────────────┐
            │ ✅ Upload complete │  ← Success state
            └───────────────────┘
```

**Technical Requirements:**
1. Create `Toast` component with animation support
2. Support loading spinner animation
3. Support state transitions (loading → success/failure)
4. Implement toast stacking
5. Auto-dismiss with configurable duration

**Files to Modify:**
- `src/toast.rs` - New file for Toast component
- `src/main.rs` - Add toast rendering layer
- `scripts/kit-sdk.ts` - Extend notify() or add toast()

**API Design:**
```typescript
interface ToastOptions {
  title: string;
  message?: string;
  style?: 'animated' | 'success' | 'failure';
  duration?: number;  // ms, default 3000
}

interface ToastHandle {
  updateTitle(title: string): void;
  updateMessage(message: string): void;
  success(title: string, message?: string): void;
  failure(title: string, message?: string): void;
  dismiss(): void;
}

function toast(options: ToastOptions): ToastHandle;

// Usage:
const t = toast({ title: "Uploading...", style: "animated" });
try {
  await uploadFile(file);
  t.success("Upload complete");
} catch (e) {
  t.failure("Upload failed", e.message);
}
```

**Estimated Effort:** 3 days

**Dependencies:** None

---

### 4.6 Alert Confirmation

**Description:** Show blocking confirmation dialogs before destructive actions.

**User Stories:**
- As a user, I want to confirm before deleting files
- As a user, I want clear primary and secondary action buttons
- As a user, I want keyboard shortcuts (Enter for primary, Escape for cancel)

**UI Mockup (ASCII):**
```
┌─────────────────────────────────────────┐
│                                         │
│    ⚠️  Delete "important-file.txt"?    │
│                                         │
│    This action cannot be undone.        │
│                                         │
│    ┌──────────┐    ┌──────────────┐     │
│    │  Cancel  │    │   Delete ⌘D  │     │
│    └──────────┘    └──────────────┘     │
│                                         │
└─────────────────────────────────────────┘
```

**Technical Requirements:**
1. Create `Alert` component
2. Support primary/secondary/destructive button styles
3. Implement modal blocking behavior
4. Support custom icons
5. Return boolean or custom action value

**Files to Modify:**
- `src/alert.rs` - New file for Alert component
- `src/main.rs` - Add alert message handler
- `src/protocol.rs` - Add Alert message type
- `scripts/kit-sdk.ts` - Add alert() function

**API Design:**
```typescript
interface AlertOptions {
  title: string;
  message?: string;
  icon?: 'warning' | 'error' | 'info' | 'success';
  primaryAction: {
    title: string;
    style?: 'default' | 'destructive';
    shortcut?: string;
  };
  secondaryAction?: {
    title: string;
    shortcut?: string;
  };
}

function alert(options: AlertOptions): Promise<boolean>;

// Usage:
const confirmed = await alert({
  title: "Delete file?",
  message: "This action cannot be undone.",
  icon: "warning",
  primaryAction: { title: "Delete", style: "destructive", shortcut: "cmd+d" },
  secondaryAction: { title: "Cancel" }
});

if (confirmed) {
  await trash(file);
}
```

**Estimated Effort:** 3 days

**Dependencies:** None

---

### 4.7 Clipboard History Manager

**Description:** Store, search, and reuse clipboard history.

**User Stories:**
- As a user, I want to access my clipboard history with a hotkey
- As a user, I want to search through past clipboard entries
- As a user, I want to pin frequently used entries
- As a user, I want images and text stored separately

**UI Mockup (ASCII):**
```
┌─────────────────────────────────────────┐
│ 🔍 Search clipboard history...          │
├─────────────────────────────────────────┤
│ ⭐ API_KEY=sk-abc123...     📌 Pinned   │
│ ▸ Hello, World!             10m ago     │
│   https://github.com/...    25m ago     │
│   {                         1h ago      │
│     "name": "John",                     │
│     "email": "john@..."                 │
│   }                                     │
│   [IMAGE: Screenshot.png]   2h ago      │
└─────────────────────────────────────────┘
```

**Technical Requirements:**
1. Create persistent clipboard history storage (SQLite or JSON)
2. Monitor clipboard changes in background
3. Support text and image entries
4. Implement pinning functionality
5. Add encryption for sensitive entries
6. Create search/filter UI

**Files to Modify:**
- `src/clipboard_history.rs` - New file for storage and monitoring
- `src/main.rs` - Add clipboard watcher
- `scripts/kit-sdk.ts` - Add clipboardHistory() function

**API Design:**
```typescript
interface ClipboardEntry {
  id: string;
  type: 'text' | 'image';
  content: string;  // text or base64 image
  timestamp: Date;
  pinned: boolean;
  application?: string;  // source app
}

interface ClipboardHistoryOptions {
  filter?: 'text' | 'image' | 'all';
  limit?: number;
}

function clipboardHistory(options?: ClipboardHistoryOptions): Promise<ClipboardEntry>;

// Usage:
const entry = await clipboardHistory();
await clipboard.writeText(entry.content);
```

**Estimated Effort:** 1 week

**Dependencies:** Background process for monitoring

---

### 4.8 Calculator with Units and Currency

**Description:** Inline calculator supporting math, unit conversions, and currency.

**User Stories:**
- As a user, I want to type math expressions and see results inline
- As a user, I want to convert between units (km to miles, kg to lbs)
- As a user, I want to convert currencies with live rates
- As a user, I want to copy results to clipboard

**UI Mockup (ASCII):**
```
┌─────────────────────────────────────────┐
│ 🔢 Calculator                           │
├─────────────────────────────────────────┤
│ > 42 * 3.14159                          │
│   = 131.94678                     ⌘C    │
│                                         │
│ > 100 km to miles                       │
│   = 62.137 miles                  ⌘C    │
│                                         │
│ > 50 USD to EUR                         │
│   = €46.25 (rate: 0.925)          ⌘C    │
│                                         │
│ > sin(45 deg)                           │
│   = 0.7071                        ⌘C    │
└─────────────────────────────────────────┘
```

**Technical Requirements:**
1. Integrate math expression parser (meval or similar)
2. Add unit conversion library
3. Fetch live currency rates (API integration)
4. Create inline result display
5. Support history of calculations

**Files to Modify:**
- `src/calculator.rs` - New file for calculator logic
- `Cargo.toml` - Add meval dependency
- `scripts/kit-sdk.ts` - Add calc() function

**API Design:**
```typescript
interface CalcResult {
  input: string;
  result: number | string;
  unit?: string;
  formatted: string;
}

function calc(expression: string): Promise<CalcResult>;

// Or as a prompt:
const result = await calculator();  // Opens calculator UI
console.log(result);  // "131.94678"
```

**Estimated Effort:** 2 weeks

**Dependencies:** Math parsing library, currency API

---

## 5. Technical Architecture Notes

### GPUI Patterns to Follow

Based on the existing codebase and `AGENTS.md`, follow these patterns:

#### Layout Order
```rust
div()
    .flex()           // 1. Layout direction
    .flex_col()
    .w_full()         // 2. Sizing
    .h(px(52.))
    .px(px(16.))      // 3. Spacing
    .gap_3()
    .bg(rgb(colors.background.main))  // 4. Visual
    .child(...)       // 5. Children
```

#### Theme Colors (NEVER hardcode)
```rust
// CORRECT
div().bg(rgb(colors.background.main))

// WRONG
div().bg(rgb(0x2d2d2d))  // Breaks theme switching
```

#### State Updates
```rust
fn set_filter(&mut self, filter: String, cx: &mut Context<Self>) {
    self.filter = filter;
    self.update_filtered_results();
    cx.notify();  // REQUIRED - triggers re-render
}
```

#### Event Handling
```rust
div()
    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
        match event.key.as_ref().map(|k| k.as_str()).unwrap_or("") {
            "ArrowDown" => this.move_selection_down(cx),
            "Enter" => this.submit(cx),
            _ => {}
        }
    }))
```

### SDK Protocol Pattern

All new features should follow the existing message protocol:

```typescript
// 1. Define message type in scripts/kit-sdk.ts
interface GridMessage {
  type: 'grid';
  id: string;
  items: GridItem[];
  options?: GridOptions;
}

// 2. Add to protocol.rs
#[serde(rename = "grid")]
Grid {
    id: String,
    items: Vec<GridItem>,
    options: Option<GridOptions>,
}

// 3. Handle in main.rs
Message::Grid { id, items, options } => {
    let grid = GridPrompt::new(id, items, options, ...);
    *current_prompt = Some(PromptState::Grid(grid));
    cx.notify();
}
```

### Performance Considerations

1. **List Virtualization**: Use `uniform_list` for any list > 100 items
2. **Event Coalescing**: Use 20ms coalescing window for rapid keyboard events
3. **Image Loading**: Use async loading with placeholder thumbnails
4. **State Updates**: Batch multiple updates with single `cx.notify()`

---

## 6. Testing Strategy

### Test Categories

| Category | Location | Purpose |
|----------|----------|---------|
| **Smoke Tests** | `tests/smoke/` | Full E2E flows |
| **SDK Tests** | `tests/sdk/` | Individual API methods |
| **Rust Unit Tests** | `src/*.rs` | Internal Rust functions |

### Test Pattern for New Features

```typescript
// tests/sdk/test-grid.ts
import '../../scripts/kit-sdk';

interface TestResult {
  test: string;
  status: 'running' | 'pass' | 'fail' | 'skip';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
}

function logTest(name: string, status: TestResult['status'], extra?: Partial<TestResult>) {
  console.log(JSON.stringify({ test: name, status, timestamp: new Date().toISOString(), ...extra }));
}

// Test: grid-basic
const testName = 'grid-basic';
logTest(testName, 'running');
const start = Date.now();

try {
  const result = await grid("Select item", [
    { title: "Item 1", value: "1", image: "data:image/..." },
    { title: "Item 2", value: "2", image: "data:image/..." },
  ]);
  
  logTest(testName, 'pass', { result, duration_ms: Date.now() - start });
} catch (err) {
  logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
}
```

### Verification Commands

```bash
# Before every commit
cargo check && cargo clippy --all-targets -- -D warnings && cargo test

# Run SDK tests
bun run scripts/test-runner.ts

# Run specific test
bun run scripts/test-runner.ts tests/sdk/test-grid.ts

# Full GPUI integration test
cargo build && ./target/debug/script-kit-gpui tests/sdk/test-grid.ts
```

### Performance Thresholds

| Metric | Threshold |
|--------|-----------|
| P95 Key Latency | < 50ms |
| Single Key Event | < 16.67ms (60fps) |
| Scroll Operation | < 8ms |
| Grid Image Load | < 200ms |

---

## 7. Dependencies

### Rust Crates to Add

| Crate | Purpose | License |
|-------|---------|---------|
| `meval` | Math expression parsing | MIT |
| `rusqlite` | Clipboard history storage | MIT |
| `directories` | XDG-compliant paths | MIT/Apache-2.0 |

### External APIs

| API | Purpose | Authentication |
|-----|---------|----------------|
| exchangerate-api.com | Currency conversion | API key |
| macOS EventKit | Calendar integration | System permissions |
| macOS Accessibility | Window management | System permissions |

### Cargo.toml Additions

```toml
[dependencies]
# Calculator
meval = "0.2"

# Clipboard history storage
rusqlite = { version = "0.31", features = ["bundled"] }

# XDG paths
directories = "5.0"
```

---

## Appendix A: Raycast Extension API Reference

### Core APIs

```typescript
// List
import { List, Action, ActionPanel, Icon } from "@raycast/api";

<List>
  <List.Item
    title="Item"
    subtitle="Description"
    accessories={[{ text: "Meta" }]}
    actions={
      <ActionPanel>
        <Action title="Open" onAction={() => {}} />
        <Action.CopyToClipboard content="Hello" />
      </ActionPanel>
    }
    detail={
      <List.Item.Detail markdown="# Hello" />
    }
  />
</List>

// Grid
<Grid columns={4}>
  <Grid.Item
    title="Image"
    content={{ source: "image.png" }}
  />
</Grid>

// Form
<Form>
  <Form.TextField id="name" title="Name" />
  <Form.DatePicker id="date" title="Date" />
  <Form.FilePicker id="file" title="File" />
</Form>

// Toast
await showToast({
  style: Toast.Style.Animated,
  title: "Loading..."
});
toast.style = Toast.Style.Success;
toast.title = "Done!";

// Alert
const confirmed = await confirmAlert({
  title: "Delete?",
  primaryAction: { title: "Delete", style: Alert.ActionStyle.Destructive }
});

// Navigation
const { push, pop } = useNavigation();
push(<SecondView />);
```

---

## Appendix B: Migration Guide from Raycast

For Raycast extension developers wanting to port to Script Kit:

### Component Mapping

| Raycast | Script Kit |
|---------|-----------|
| `<List>` | `await arg()` |
| `<List.Item>` | Choice object |
| `<Grid>` | `await grid()` (when implemented) |
| `<Form>` | `await fields()` |
| `<Detail>` | `await div()` |
| `showToast()` | `toast()` (when implemented) |
| `confirmAlert()` | `await alert()` (when implemented) |

### Code Example

**Raycast:**
```typescript
import { List, ActionPanel, Action } from "@raycast/api";

export default function Command() {
  return (
    <List>
      <List.Item
        title="Hello"
        actions={
          <ActionPanel>
            <Action.CopyToClipboard content="Hello" />
          </ActionPanel>
        }
      />
    </List>
  );
}
```

**Script Kit:**
```typescript
const result = await arg("Select", [
  {
    name: "Hello",
    value: "hello",
    actions: [
      { title: "Copy", shortcut: "cmd+c", onAction: () => copy("Hello") }
    ]
  }
]);
```

---

## Summary

This roadmap provides a comprehensive plan to achieve Raycast feature parity in Script Kit GPUI. The implementation is organized into tiers based on effort and value, with detailed specifications for each feature.

**Quick Reference:**

| Tier | Timeline | Key Features |
|------|----------|--------------|
| **Tier 1** | 1-2 weeks each | Toast states, alerts, clipboard history |
| **Tier 2** | 2-4 weeks each | ActionPanel, split-view, navigation |
| **Tier 3** | 4-8 weeks each | Grid, snippets, AI tools |
| **Tier 4** | 8+ weeks each | Window management, notes, focus |

**Next Steps:**
1. Prioritize Tier 1 features for quick wins
2. Start Tier 2 ActionPanel development in parallel
3. Gather user feedback on priority ordering
4. Create tracking issues in `.hive/issues.jsonl`
