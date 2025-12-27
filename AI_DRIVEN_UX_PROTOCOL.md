# AI-Driven UX Protocol Reference

This document provides a comprehensive reference for the JSONL protocol used in Script Kit GPUI. The protocol enables bidirectional communication between TypeScript scripts and the Rust GPUI application.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Stdin Commands](#stdin-commands)
3. [Message ID Correlation](#message-id-correlation)
4. [Message Categories](#message-categories)
   - [Core Prompts](#core-prompts)
   - [Text Input Prompts](#text-input-prompts)
   - [Selection Prompts](#selection-prompts)
   - [Form Prompts](#form-prompts)
   - [File/Path Prompts](#filepath-prompts)
   - [Input Capture Prompts](#input-capture-prompts)
   - [Template/Text Prompts](#templatetext-prompts)
   - [Media Prompts](#media-prompts)
   - [Notification/Feedback Messages](#notificationfeedback-messages)
   - [System Control Messages](#system-control-messages)
   - [UI Update Messages](#ui-update-messages)
   - [Selected Text Operations](#selected-text-operations)
   - [Window Information](#window-information)
   - [Clipboard History](#clipboard-history)
   - [Window Management (System Windows)](#window-management-system-windows)
   - [File Search](#file-search)
   - [Screenshot Capture](#screenshot-capture)
   - [Error Reporting](#error-reporting)
5. [Data Types](#data-types)
6. [Graceful Error Handling](#graceful-error-handling)
7. [SDK Integration](#sdk-integration)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      Script Kit GPUI                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    JSONL stdin     ┌──────────────────────┐  │
│  │              │ ─────────────────► │                      │  │
│  │  TypeScript  │                    │    Rust GPUI App     │  │
│  │   Scripts    │ ◄───────────────── │   (UI Rendering)     │  │
│  │   (bun)      │    JSONL stdout    │                      │  │
│  └──────────────┘                    └──────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Communication Flow:**
1. Scripts send JSONL messages to the app via **stdout**
2. App sends responses back via **stdin** to the script
3. Each message has a `"type"` field for discrimination
4. Messages use `"id"` or `"requestId"` for correlation

---

## Stdin Commands

The app accepts these commands via stdin for control and testing:

### `run` - Execute a Script

```json
{"type": "run", "path": "/absolute/path/to/script.ts"}
```

**Purpose:** Runs a TypeScript script with the SDK preloaded.

**Example:**
```bash
echo '{"type": "run", "path": "/Users/me/scripts/hello.ts"}' | ./target/debug/script-kit-gpui
```

### `show` - Show Window

```json
{"type": "show"}
```

**Purpose:** Makes the app window visible and brings it to front.

### `hide` - Hide Window

```json
{"type": "hide"}
```

**Purpose:** Hides the app window.

### `setFilter` - Set Search Filter

```json
{"type": "setFilter", "text": "search term"}
```

**Purpose:** Sets the filter/search text for the current prompt.

---

## Message ID Correlation

Messages use IDs to correlate requests with responses:

| Message Category | ID Field | Purpose |
|-----------------|----------|---------|
| Prompts (arg, div, editor, etc.) | `id` | Links script request to user submission |
| System operations | `requestId` | Links async request to response |

**Request/Response Pattern:**

```
Script → App:   {"type": "getSelectedText", "requestId": "req-123"}
App → Script:   {"type": "selectedText", "text": "Hello", "requestId": "req-123"}
```

**Prompt/Submit Pattern:**

```
Script → App:   {"type": "arg", "id": "1", "placeholder": "Pick", "choices": [...]}
User selects...
App → Script:   {"type": "submit", "id": "1", "value": "apple"}
```

---

## Message Categories

### Core Prompts

#### `arg` - Argument Prompt with Choices

Display a prompt with selectable choices.

**Request (Script → App):**
```json
{
  "type": "arg",
  "id": "prompt-1",
  "placeholder": "Pick a fruit",
  "choices": [
    {"name": "Apple", "value": "apple", "description": "A red fruit"},
    {"name": "Banana", "value": "banana"}
  ]
}
```

**Response (App → Script):**
```json
{"type": "submit", "id": "prompt-1", "value": "apple"}
```

**TypeScript SDK:**
```typescript
const fruit = await arg("Pick a fruit", [
  {name: "Apple", value: "apple", description: "A red fruit"},
  {name: "Banana", value: "banana"}
]);
```

#### `div` - HTML Display

Display HTML content with optional Tailwind styling.

**Request:**
```json
{
  "type": "div",
  "id": "display-1",
  "html": "<h1>Hello World</h1><p>Welcome to Script Kit!</p>",
  "tailwind": "text-2xl font-bold p-4"
}
```

**Response:**
```json
{"type": "submit", "id": "display-1", "value": null}
```

**TypeScript SDK:**
```typescript
await div("<h1>Hello!</h1><p>Press Enter to continue</p>");
// or with markdown:
await div(md("# Hello\n\nPress Enter to continue"));
```

#### `submit` - User Submission

Sent by app when user submits a value.

**Message:**
```json
{"type": "submit", "id": "prompt-1", "value": "selected_value"}
```

**Note:** `value` can be `null` if user cancels.

#### `update` - Live Update

Sent for real-time updates during prompts.

**Message:**
```json
{
  "type": "update",
  "id": "prompt-1",
  "filter": "search text",
  "input": "user typing"
}
```

#### `exit` - Termination Signal

Signal script or app termination.

**Message:**
```json
{"type": "exit", "code": 0, "message": "Success"}
```

---

### Text Input Prompts

#### `editor` - Code/Text Editor

Full-featured code editor with syntax highlighting.

**Request:**
```json
{
  "type": "editor",
  "id": "editor-1",
  "content": "// Initial code\nconst x = 42;",
  "language": "javascript",
  "onInit": "console.log('Editor ready')",
  "onSubmit": "console.log('Submitted')"
}
```

**Response:**
```json
{"type": "submit", "id": "editor-1", "value": "// Edited code\nconst x = 100;"}
```

**TypeScript SDK:**
```typescript
const code = await editor("// Start typing", "typescript");
```

#### `mini` - Compact Prompt

Same as `arg` but with compact display.

**Request:**
```json
{
  "type": "mini",
  "id": "mini-1",
  "placeholder": "Quick pick",
  "choices": [{"name": "A", "value": "a"}]
}
```

#### `micro` - Tiny Prompt

Even smaller than mini.

**Request:**
```json
{
  "type": "micro",
  "id": "micro-1",
  "placeholder": "Tiny",
  "choices": []
}
```

---

### Selection Prompts

#### `select` - Multiple Selection

Select from choices with optional multi-select.

**Request:**
```json
{
  "type": "select",
  "id": "select-1",
  "placeholder": "Select items",
  "choices": [
    {"name": "Red", "value": "red"},
    {"name": "Blue", "value": "blue"},
    {"name": "Green", "value": "green"}
  ],
  "multiple": true
}
```

**Response (multiple=true):**
```json
{"type": "submit", "id": "select-1", "value": "[\"red\",\"blue\"]"}
```

**TypeScript SDK:**
```typescript
const colors = await select("Select colors", ["red", "blue", "green"], {multiple: true});
```

---

### Form Prompts

#### `fields` - Multiple Input Fields

Display multiple form fields.

**Request:**
```json
{
  "type": "fields",
  "id": "form-1",
  "fields": [
    {"name": "username", "label": "Username", "type": "text", "placeholder": "Enter username"},
    {"name": "email", "label": "Email Address", "type": "email"},
    {"name": "password", "label": "Password", "type": "password"}
  ]
}
```

**Response:**
```json
{"type": "submit", "id": "form-1", "value": "{\"username\":\"john\",\"email\":\"john@example.com\",\"password\":\"***\"}"}
```

**TypeScript SDK:**
```typescript
const [username, email, password] = await fields([
  {name: "username", label: "Username"},
  {name: "email", label: "Email", type: "email"},
  {name: "password", label: "Password", type: "password"}
]);
```

#### `form` - Custom HTML Form

Display a custom HTML form.

**Request:**
```json
{
  "type": "form",
  "id": "custom-form-1",
  "html": "<form><input name='field1' /><button type='submit'>Submit</button></form>"
}
```

---

### File/Path Prompts

#### `path` - File/Folder Picker

Native file or folder path picker.

**Request:**
```json
{
  "type": "path",
  "id": "path-1",
  "startPath": "/home/user/Documents",
  "hint": "Select a configuration file"
}
```

**Response:**
```json
{"type": "submit", "id": "path-1", "value": "/home/user/Documents/config.json"}
```

**TypeScript SDK:**
```typescript
const file = await path({startPath: "~/Documents", hint: "Pick a file"});
```

#### `drop` - File Drop Zone

Accept files via drag-and-drop.

**Request:**
```json
{
  "type": "drop",
  "id": "drop-1"
}
```

**Response:**
```json
{"type": "submit", "id": "drop-1", "value": "[\"/path/to/file1.txt\",\"/path/to/file2.pdf\"]"}
```

**TypeScript SDK:**
```typescript
const files = await drop();
// files: FileInfo[] = [{path: "...", name: "...", size: 1234}]
```

---

### Input Capture Prompts

#### `hotkey` - Keyboard Shortcut Capture

Capture a keyboard shortcut.

**Request:**
```json
{
  "type": "hotkey",
  "id": "hotkey-1",
  "placeholder": "Press a key combination"
}
```

**Response:**
```json
{"type": "submit", "id": "hotkey-1", "value": "{\"key\":\"k\",\"command\":true,\"shift\":true,\"option\":false,\"control\":false,\"shortcut\":\"cmd+shift+k\"}"}
```

**TypeScript SDK:**
```typescript
const hk = await hotkey();
// hk: HotkeyInfo = {key: "k", command: true, shift: true, ...}
```

---

### Template/Text Prompts

#### `template` - Template with Placeholders

Fill in template placeholders.

**Request:**
```json
{
  "type": "template",
  "id": "template-1",
  "template": "Hello {{name}}, welcome to {{place}}!"
}
```

**Response:**
```json
{"type": "submit", "id": "template-1", "value": "Hello John, welcome to Script Kit!"}
```

**TypeScript SDK:**
```typescript
const result = await template("Hello {{name}}, welcome to {{place}}!");
```

#### `env` - Environment Variable Prompt

Prompt for an environment variable (optionally secret).

**Request:**
```json
{
  "type": "env",
  "id": "env-1",
  "key": "API_KEY",
  "secret": true
}
```

**Response:**
```json
{"type": "submit", "id": "env-1", "value": "sk-abc123..."}
```

**TypeScript SDK:**
```typescript
const apiKey = await env("API_KEY"); // Prompts if not set, caches value
```

---

### Media Prompts

#### `chat` - Chat Interface

Interactive chat conversation UI.

**Request:**
```json
{
  "type": "chat",
  "id": "chat-1"
}
```

**TypeScript SDK:**
```typescript
const chatController = await chat({
  onInit: async () => { /* setup */ },
  onSubmit: async (input) => { /* handle message */ }
});
chatController.addMessage({text: "Hello!", position: "left"});
```

#### `term` - Terminal Emulator

Embedded terminal with command execution.

**Request:**
```json
{
  "type": "term",
  "id": "term-1",
  "command": "ls -la"
}
```

**TypeScript SDK:**
```typescript
await term("npm install"); // Runs command in terminal
await term(); // Opens empty terminal
```

#### `widget` - Custom Widget

Custom floating widget window.

**Request:**
```json
{
  "type": "widget",
  "id": "widget-1",
  "html": "<div id='app'>Widget Content</div>",
  "options": {
    "transparent": true,
    "alwaysOnTop": true,
    "width": 300,
    "height": 200
  }
}
```

**TypeScript SDK:**
```typescript
const w = await widget("<div>My Widget</div>", {
  transparent: true,
  alwaysOnTop: true
});
w.onClick((event) => console.log(event.targetId));
```

#### `webcam` - Webcam Capture

Capture image from webcam.

**Request:**
```json
{
  "type": "webcam",
  "id": "webcam-1"
}
```

**Response:**
```json
{"type": "submit", "id": "webcam-1", "value": "data:image/jpeg;base64,/9j/4AAQ..."}
```

#### `mic` - Microphone Recording

Record audio from microphone.

**Request:**
```json
{
  "type": "mic",
  "id": "mic-1"
}
```

**Response:**
```json
{"type": "submit", "id": "mic-1", "value": "data:audio/wav;base64,..."}
```

---

### Notification/Feedback Messages

#### `notify` - System Notification

Display a system notification.

**Message:**
```json
{
  "type": "notify",
  "title": "Task Complete",
  "body": "Your script has finished running"
}
```

**TypeScript SDK:**
```typescript
await notify("Task Complete", "Your script has finished");
```

#### `beep` - System Beep

Play system alert sound.

**Message:**
```json
{"type": "beep"}
```

**TypeScript SDK:**
```typescript
await beep();
```

#### `say` - Text-to-Speech

Read text aloud.

**Message:**
```json
{
  "type": "say",
  "text": "Hello, world!",
  "voice": "Samantha"
}
```

**TypeScript SDK:**
```typescript
await say("Hello, world!", "Samantha");
```

#### `setStatus` - Status Bar Update

Update status bar message.

**Message:**
```json
{
  "type": "setStatus",
  "status": "busy",
  "message": "Processing files..."
}
```

---

### System Control Messages

#### `menu` - Menu Bar Control

Configure menu bar icon and scripts.

**Message:**
```json
{
  "type": "menu",
  "icon": "🚀",
  "scripts": ["script1", "script2"]
}
```

#### `clipboard` - Clipboard Operations

Read or write clipboard contents.

**Read Text:**
```json
{
  "type": "clipboard",
  "action": "read",
  "format": "text"
}
```

**Write Text:**
```json
{
  "type": "clipboard",
  "action": "write",
  "format": "text",
  "content": "Hello, clipboard!"
}
```

**Read Image:**
```json
{
  "type": "clipboard",
  "action": "read",
  "format": "image"
}
```

**TypeScript SDK:**
```typescript
const text = await clipboard.readText();
await clipboard.writeText("Hello!");
const image = await clipboard.readImage();
```

#### `keyboard` - Keyboard Simulation

Simulate keyboard input.

**Type Text:**
```json
{
  "type": "keyboard",
  "action": "type",
  "keys": "Hello, World!"
}
```

**Tap Keys (hotkey):**
```json
{
  "type": "keyboard",
  "action": "tap",
  "keys": "cmd+c"
}
```

**TypeScript SDK:**
```typescript
await keyboard.type("Hello!");
await keyboard.tap("cmd+c");
```

#### `mouse` - Mouse Control

Control mouse position and clicks.

**Move Mouse:**
```json
{
  "type": "mouse",
  "action": "move",
  "data": {"x": 100, "y": 200}
}
```

**Click:**
```json
{
  "type": "mouse",
  "action": "click",
  "data": {"button": "left"}
}
```

**Set Position:**
```json
{
  "type": "mouse",
  "action": "setPosition",
  "data": {"x": 500, "y": 300}
}
```

**TypeScript SDK:**
```typescript
await mouse.move({x: 100, y: 200});
await mouse.click();
await mouse.setPosition({x: 500, y: 300});
```

#### `show` - Show Window

Show the app window.

**Message:**
```json
{"type": "show"}
```

**TypeScript SDK:**
```typescript
await show();
```

#### `hide` - Hide Window

Hide the app window.

**Message:**
```json
{"type": "hide"}
```

**TypeScript SDK:**
```typescript
await hide();
```

#### `browse` - Open URL

Open URL in default browser.

**Message:**
```json
{
  "type": "browse",
  "url": "https://scriptkit.com"
}
```

**TypeScript SDK:**
```typescript
await browse("https://scriptkit.com");
```

#### `exec` - Execute Shell Command

Execute a shell command.

**Message:**
```json
{
  "type": "exec",
  "command": "ls -la",
  "options": {"cwd": "/home/user"}
}
```

**TypeScript SDK:**
```typescript
const result = await exec("ls -la");
```

---

### UI Update Messages

#### `setPanel` - Update Panel Content

Set the panel HTML content.

**Message:**
```json
{
  "type": "setPanel",
  "html": "<div class='p-4'>Panel content here</div>"
}
```

**TypeScript SDK:**
```typescript
setPanel("<div>Updated panel</div>");
```

#### `setPreview` - Update Preview Content

Set the preview pane HTML.

**Message:**
```json
{
  "type": "setPreview",
  "html": "<img src='preview.png' />"
}
```

**TypeScript SDK:**
```typescript
setPreview("<img src='preview.png' />");
```

#### `setPrompt` - Update Prompt Content

Set the prompt area HTML.

**Message:**
```json
{
  "type": "setPrompt",
  "html": "<b>Enter your name:</b>"
}
```

---

### Selected Text Operations

Operations for getting/setting selected text in focused applications.

#### `getSelectedText` - Get Selection

**Request:**
```json
{
  "type": "getSelectedText",
  "requestId": "req-123"
}
```

**Response:**
```json
{
  "type": "selectedText",
  "text": "The selected content",
  "requestId": "req-123"
}
```

**TypeScript SDK:**
```typescript
const selected = await getSelectedText();
```

#### `setSelectedText` - Replace Selection

**Request:**
```json
{
  "type": "setSelectedText",
  "text": "Replacement text",
  "requestId": "req-456"
}
```

**Response:**
```json
{
  "type": "textSet",
  "success": true,
  "requestId": "req-456"
}
```

**Error Response:**
```json
{
  "type": "textSet",
  "success": false,
  "error": "Permission denied",
  "requestId": "req-456"
}
```

**TypeScript SDK:**
```typescript
await setSelectedText("New text");
```

#### `checkAccessibility` - Check Permissions

**Request:**
```json
{
  "type": "checkAccessibility",
  "requestId": "req-789"
}
```

**Response:**
```json
{
  "type": "accessibilityStatus",
  "granted": true,
  "requestId": "req-789"
}
```

**TypeScript SDK:**
```typescript
const hasAccess = await checkAccessibility();
```

#### `requestAccessibility` - Request Permissions

**Request:**
```json
{
  "type": "requestAccessibility",
  "requestId": "req-abc"
}
```

**Response:**
```json
{
  "type": "accessibilityStatus",
  "granted": true,
  "requestId": "req-abc"
}
```

---

### Window Information

#### `getWindowBounds` - Get App Window Position

**Request:**
```json
{
  "type": "getWindowBounds",
  "requestId": "req-wb-1"
}
```

**Response:**
```json
{
  "type": "windowBounds",
  "x": 100.5,
  "y": 200.5,
  "width": 800.0,
  "height": 600.0,
  "requestId": "req-wb-1"
}
```

**TypeScript SDK:**
```typescript
const bounds = await getWindowBounds();
// bounds: {x: number, y: number, width: number, height: number}
```

---

### Clipboard History

Operations for managing clipboard history.

#### `clipboardHistory` - History Operations

**List Entries:**
```json
{
  "type": "clipboardHistory",
  "requestId": "req-ch-1",
  "action": "list"
}
```

**Response:**
```json
{
  "type": "clipboardHistoryList",
  "requestId": "req-ch-1",
  "entries": [
    {
      "entryId": "e1",
      "content": "Hello World",
      "contentType": "text",
      "timestamp": "2024-01-15T10:30:00Z",
      "pinned": false
    }
  ]
}
```

**Pin Entry:**
```json
{
  "type": "clipboardHistory",
  "requestId": "req-ch-2",
  "action": "pin",
  "entryId": "e1"
}
```

**Unpin Entry:**
```json
{
  "type": "clipboardHistory",
  "requestId": "req-ch-3",
  "action": "unpin",
  "entryId": "e1"
}
```

**Remove Entry:**
```json
{
  "type": "clipboardHistory",
  "requestId": "req-ch-4",
  "action": "remove",
  "entryId": "e1"
}
```

**Clear All:**
```json
{
  "type": "clipboardHistory",
  "requestId": "req-ch-5",
  "action": "clear"
}
```

**Action Result:**
```json
{
  "type": "clipboardHistoryResult",
  "requestId": "req-ch-2",
  "success": true
}
```

**TypeScript SDK:**
```typescript
// List history
const history = await getClipboardHistory();

// Manage entries
await pinClipboardEntry("entry-id");
await unpinClipboardEntry("entry-id");
await removeClipboardEntry("entry-id");
await clearClipboardHistory();
```

---

### Window Management (System Windows)

Operations for managing system windows (other applications).

#### `windowList` - List All Windows

**Request:**
```json
{
  "type": "windowList",
  "requestId": "req-wl-1"
}
```

**Response:**
```json
{
  "type": "windowListResult",
  "requestId": "req-wl-1",
  "windows": [
    {
      "windowId": 12345,
      "title": "Document.txt - VS Code",
      "appName": "Visual Studio Code",
      "bounds": {"x": 0, "y": 0, "width": 1200, "height": 800},
      "isMinimized": false,
      "isActive": true
    }
  ]
}
```

**TypeScript SDK:**
```typescript
const windows = await getWindows();
```

#### `windowAction` - Perform Window Action

**Focus Window:**
```json
{
  "type": "windowAction",
  "requestId": "req-wa-1",
  "action": "focus",
  "windowId": 12345
}
```

**Close Window:**
```json
{
  "type": "windowAction",
  "requestId": "req-wa-2",
  "action": "close",
  "windowId": 12345
}
```

**Minimize Window:**
```json
{
  "type": "windowAction",
  "requestId": "req-wa-3",
  "action": "minimize",
  "windowId": 12345
}
```

**Maximize Window:**
```json
{
  "type": "windowAction",
  "requestId": "req-wa-4",
  "action": "maximize",
  "windowId": 12345
}
```

**Resize Window:**
```json
{
  "type": "windowAction",
  "requestId": "req-wa-5",
  "action": "resize",
  "windowId": 12345,
  "bounds": {"x": 100, "y": 100, "width": 800, "height": 600}
}
```

**Move Window:**
```json
{
  "type": "windowAction",
  "requestId": "req-wa-6",
  "action": "move",
  "windowId": 12345,
  "bounds": {"x": 200, "y": 200, "width": 800, "height": 600}
}
```

**Action Result:**
```json
{
  "type": "windowActionResult",
  "requestId": "req-wa-1",
  "success": true
}
```

**TypeScript SDK:**
```typescript
await focusWindow(12345);
await closeWindow(12345);
await minimizeWindow(12345);
await maximizeWindow(12345);
await setWindowPosition(12345, {x: 100, y: 100});
await setWindowBounds(12345, {x: 100, y: 100, width: 800, height: 600});
```

---

### File Search

Search for files using system search capabilities.

#### `fileSearch` - Search Files

**Request:**
```json
{
  "type": "fileSearch",
  "requestId": "req-fs-1",
  "query": "*.ts",
  "onlyin": "/home/user/projects"
}
```

**Response:**
```json
{
  "type": "fileSearchResult",
  "requestId": "req-fs-1",
  "files": [
    {
      "path": "/home/user/projects/app/main.ts",
      "name": "main.ts",
      "isDirectory": false,
      "size": 2048,
      "modifiedAt": "2024-01-15T10:30:00Z"
    },
    {
      "path": "/home/user/projects/app/utils",
      "name": "utils",
      "isDirectory": true
    }
  ]
}
```

**TypeScript SDK:**
```typescript
const files = await find("*.ts", {onlyin: "/projects"});
// files: FileSearchResult[]
```

---

### Screenshot Capture

Capture screenshots of the app window.

#### `captureScreenshot` - Capture Window

**Request:**
```json
{
  "type": "captureScreenshot",
  "requestId": "req-ss-1"
}
```

**Response:**
```json
{
  "type": "screenshotResult",
  "requestId": "req-ss-1",
  "data": "iVBORw0KGgoAAAANSUhEUgAA...",
  "width": 800,
  "height": 600
}
```

**Note:** `data` is base64-encoded PNG.

**TypeScript SDK:**
```typescript
const screenshot = await captureScreenshot();
// screenshot: {data: string, width: number, height: number}
```

---

### Error Reporting

Report script execution errors with structured information.

#### `setError` - Script Error

**Message:**
```json
{
  "type": "setError",
  "errorMessage": "Module 'xyz' not found",
  "stderrOutput": "Error: Cannot find module 'xyz'\n    at require (/path/to/script.ts:5)",
  "exitCode": 1,
  "stackTrace": "at require (/path/to/script.ts:5)\n    at main (/path/to/script.ts:10)",
  "scriptPath": "/home/user/scripts/failing-script.ts",
  "suggestions": [
    "Run: npm install xyz",
    "Check the import path"
  ],
  "timestamp": "2024-01-15T10:30:00Z"
}
```

**Minimal Error:**
```json
{
  "type": "setError",
  "errorMessage": "Script crashed",
  "scriptPath": "/path/to/script.ts"
}
```

**Note:** All fields except `errorMessage` and `scriptPath` are optional.

---

## Data Types

### Choice

```typescript
interface Choice {
  name: string;       // Display name
  value: string;      // Return value
  description?: string; // Optional description
}
```

**JSON:**
```json
{"name": "Apple", "value": "apple", "description": "A red fruit"}
```

### Field

```typescript
interface Field {
  name: string;         // Field identifier
  label?: string;       // Display label
  type?: string;        // Input type: text, password, email, etc.
  placeholder?: string; // Placeholder text
  value?: string;       // Initial value
}
```

**JSON:**
```json
{"name": "email", "label": "Email Address", "type": "email", "placeholder": "you@example.com"}
```

### ClipboardAction

Values: `"read"`, `"write"`

### ClipboardFormat

Values: `"text"`, `"image"`

### KeyboardAction

Values: `"type"`, `"tap"`

### MouseAction

Values: `"move"`, `"click"`, `"setPosition"`

### ClipboardEntryType

Values: `"text"`, `"image"`

### ClipboardHistoryAction

Values: `"list"`, `"pin"`, `"unpin"`, `"remove"`, `"clear"`

### WindowActionType

Values: `"focus"`, `"close"`, `"minimize"`, `"maximize"`, `"resize"`, `"move"`

### TargetWindowBounds (System Windows)

```typescript
interface TargetWindowBounds {
  x: number;      // Integer, can be negative
  y: number;      // Integer, can be negative
  width: number;  // Unsigned integer
  height: number; // Unsigned integer
}
```

### SystemWindowInfo

```typescript
interface SystemWindowInfo {
  windowId: number;
  title: string;
  appName: string;
  bounds?: TargetWindowBounds;
  isMinimized?: boolean;
  isActive?: boolean;
}
```

### FileSearchResultEntry

```typescript
interface FileSearchResultEntry {
  path: string;
  name: string;
  isDirectory: boolean;
  size?: number;
  modifiedAt?: string; // ISO 8601 timestamp
}
```

### ClipboardHistoryEntryData

```typescript
interface ClipboardHistoryEntryData {
  entryId: string;
  content: string;
  contentType: "text" | "image";
  timestamp: string;
  pinned: boolean;
}
```

### ScriptErrorData

```typescript
interface ScriptErrorData {
  errorMessage: string;      // Required: User-friendly message
  scriptPath: string;        // Required: Path to failing script
  stderrOutput?: string;     // Raw stderr
  exitCode?: number;         // Process exit code
  stackTrace?: string;       // Parsed stack trace
  suggestions?: string[];    // Fix suggestions
  timestamp?: string;        // ISO 8601 timestamp
}
```

---

## Graceful Error Handling

The protocol supports graceful handling of unknown message types:

### ParseResult

```rust
enum ParseResult {
    Ok(Message),                    // Known message type
    UnknownType { message_type: String, raw: String }, // Unknown type, ignored
    ParseError(serde_json::Error),  // Invalid JSON
}
```

**Behavior:**
- Known message types are parsed normally
- Unknown types are logged as warnings and skipped
- Invalid JSON causes parse errors

**Example - Unknown Type Handling:**
```json
{"type": "futureFeature", "data": "test"}
```

This message is logged and skipped without crashing.

---

## SDK Integration

### Importing the SDK

```typescript
// In test scripts
import '../../scripts/kit-sdk';

// In production (after tsconfig.json path mapping)
import '@johnlindquist/kit';
```

### SDK Version

```typescript
import { SDK_VERSION } from '@johnlindquist/kit';
console.log(SDK_VERSION); // "0.2.0"
```

### Common Patterns

**Simple Prompt:**
```typescript
const name = await arg("What's your name?");
console.log(`Hello, ${name}!`);
```

**Choices from Array:**
```typescript
const fruit = await arg("Pick a fruit", ["Apple", "Banana", "Cherry"]);
```

**Choices with Values:**
```typescript
const action = await arg("What do?", [
  {name: "Create", value: "create", description: "Create new item"},
  {name: "Delete", value: "delete", description: "Remove item"}
]);
```

**Chained Prompts:**
```typescript
const name = await arg("Name?");
const email = await arg("Email?");
const result = await fields([
  {name: "address", label: "Address"},
  {name: "city", label: "City"}
]);
```

**Error Handling:**
```typescript
try {
  const text = await getSelectedText();
  await setSelectedText(text.toUpperCase());
} catch (e) {
  await notify("Error", e.message);
}
```

---

## Message Count Summary

| Category | Count | Message Types |
|----------|-------|---------------|
| Core Prompts | 5 | arg, div, submit, update, exit |
| Text Input | 3 | editor, mini, micro |
| Selection | 1 | select |
| Forms | 2 | fields, form |
| File/Path | 2 | path, drop |
| Input Capture | 1 | hotkey |
| Template/Text | 2 | template, env |
| Media | 5 | chat, term, widget, webcam, mic |
| Notifications | 4 | notify, beep, say, setStatus |
| System Control | 8 | menu, clipboard, keyboard, mouse, show, hide, browse, exec |
| UI Updates | 3 | setPanel, setPreview, setPrompt |
| Selected Text | 8 | getSelectedText, setSelectedText, checkAccessibility, requestAccessibility, selectedText, textSet, accessibilityStatus |
| Window Info | 2 | getWindowBounds, windowBounds |
| Clipboard History | 4 | clipboardHistory, clipboardHistoryEntry, clipboardHistoryList, clipboardHistoryResult |
| Window Management | 4 | windowList, windowAction, windowListResult, windowActionResult |
| File Search | 2 | fileSearch, fileSearchResult |
| Screenshot | 2 | captureScreenshot, screenshotResult |
| Error | 1 | setError |
| **Total** | **59** | |

---

## Quick Reference

### Request → Response Message Pairs

| Request Type | Response Type |
|-------------|---------------|
| `arg`, `div`, `editor`, `mini`, `micro`, `select`, `fields`, `form`, `path`, `drop`, `hotkey`, `template`, `env`, `chat`, `term`, `widget`, `webcam`, `mic` | `submit` |
| `getSelectedText` | `selectedText` |
| `setSelectedText` | `textSet` |
| `checkAccessibility`, `requestAccessibility` | `accessibilityStatus` |
| `getWindowBounds` | `windowBounds` |
| `clipboardHistory` (list) | `clipboardHistoryList` |
| `clipboardHistory` (pin/unpin/remove/clear) | `clipboardHistoryResult` |
| `windowList` | `windowListResult` |
| `windowAction` | `windowActionResult` |
| `fileSearch` | `fileSearchResult` |
| `captureScreenshot` | `screenshotResult` |
