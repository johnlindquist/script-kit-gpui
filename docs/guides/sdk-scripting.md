# SDK Scripting

Script Kit scripts are local TypeScript files executed by Bun that talk to the native app through the Script Kit SDK. The SDK is deliberately small: prompts, feedback, and integration points — bring your own utility libraries.

## Where Scripts Live

```bash
~/.scriptkit/plugins/main/scripts/     # executable scripts
~/.scriptkit/plugins/main/scriptlets/  # scriptlet bundles / snippets
```

## Minimal Script

```typescript
import "@scriptkit/sdk";

export const metadata = {
  name: "Today",
  description: "Show today's date",
};

await div(`<h1 class="text-3xl p-8">${new Date().toLocaleDateString()}</h1>`);
```

## Metadata

The primary form is a typed `export const metadata = { ... }` object (typed as `ScriptMetadata`, so you get IDE completion):

```typescript
export const metadata = {
  name: "Deploy Preview",
  description: "Build and open the latest preview",
  author: "Your Name",
  shortcut: "cmd shift d",     // global keyboard shortcut
  alias: "dp",                 // short trigger in the launcher
  keyword: ":dep",             // text-expansion trigger ("snippet"/"expand" are aliases)
  placeholder: "Branch name",  // custom input placeholder
  tags: ["work", "deploy"],
  hidden: false,               // hide from the main list
  cron: "0 9 * * *",           // scheduled execution
  schedule: "every tuesday at 2pm", // natural-language schedule (converted to cron)
  watch: ["~/Downloads/*.zip"],// file-watch triggers
  background: false,           // run without UI
  system: false,               // system-level script
  fallback: false,             // offer this script when no search results match
  fallbackLabel: "Search docs for {input}",
  sdkCapabilities: ["arg", "readFile", "exec"], // optional explicit host contract
  executionTopology: "typescript-script",       // optional explicit SDK transport
};
```

Comment-based metadata is a compatibility-only fallback, read from the top of the file (`// Name:`, `// Description:`, `// Icon:`, `// Alias:`, `// Shortcut:` in the first 20 lines; `// Cron:` and `// Schedule:` in the first 30). Typed values win when both are present.

Avoid duplicate `shortcut`/`alias`/`keyword` values across scripts — colliding entries are excluded so dispatch never races.

Declare `sdkCapabilities` when a script needs a checked compatibility contract.
Unknown, unsupported, duplicate, or malformed capability names prevent ordinary
scripts from entering the launcher. Scriptlets remain visible as disabled rows
with actionable diagnostics; both launcher and legacy execution reject them
before any subprocess or other side effect. The `kit://failed-scripts` resource
lists excluded scripts separately from its `retainedIssues` scriptlet entries.
`executionTopology` accepts
`typescript-script`, `typescript-scriptlet`,
`typescript-scriptlet-interactive`, `shell-scriptlet`, or `python-scriptlet`.
Launcher-opened TypeScript scriptlets use the interactive topology and can use
prompt APIs; the legacy `typescript-scriptlet` topology has no interactive
stdin response pipe. Shell/Python topologies cannot claim SDK globals.

## Prompt APIs

| API | Use |
| --- | --- |
| `await arg(prompt, choices?)` | Text input, optionally with searchable choices |
| `await div(html)` | Rich HTML/Tailwind display |
| `await editor(options?)` | Full multi-line editor |
| `await term(command?)` | Interactive terminal |
| `await drop(options?)` | Drag-and-drop file zone |
| `await template(str)` | Fill in a template string |
| `await fields(defs)` / `await form(html)` | Structured / custom HTML forms |
| `await path(options?)` | File/folder picker |
| `await hotkey(prompt?)` | Capture a keyboard shortcut |
| `await mini(prompt, choices)` | Native compact-choice prompt |
| `await micro(prompt, choices)` | Native minimal-choice prompt |

`fields()` accepts `text`, `password`, `email`, `number`, `date`, `time`,
`datetime-local`, `month`, `week`, `search`, `url`, `tel`, and `color` field
definitions. A supported SDK field type does not imply a native date/color
picker: specialized controls may use the shared text-input treatment.

`arg(...)`, `div(...)`, `editor(...)`, `fields(...)`, `form(...)`, and
`term(...)` also accept optional prompt-scoped `Action[]` definitions where
shown in SDK Reference. Hidden, duplicate, and non-actionable entries are not
serialized; executable callbacks stay in the local script rather than crossing
the JSON protocol.

The GPUI host does **not** implement `mic()`, `webcam()`, `eyeDropper()`,
floating `widget()` windows, global `keyboard.*` / `mouse.*` injection, or
legacy `setPanel()` / `setPreview()` / `setPrompt()` mutations. These APIs fail
before dispatch with `ERR_UNSUPPORTED_SDK_FEATURE` and actionable alternatives.
Use `div(...)` or supported native prompts for presentation, and semantic
`batch(...)` / `getElements(...)` APIs instead of global input injection.

Regular TypeScript scripts receive the interactive SDK stdin/stdout transport.
TypeScript scriptlets can use noninteractive SDK helpers but cannot open prompts
because they do not receive an interactive stdin pipe. Shell and Python
scriptlets do not receive SDK globals.

```typescript
import "@scriptkit/sdk";

export const metadata = { name: "Pick a Service" };

const url = await arg("Pick a service", [
  { name: "Script Kit", value: "https://scriptkit.com", description: "Automation" },
  { name: "GPUI", value: "https://gpui.rs", description: "Native UI" },
]);

await div(`<a class="text-blue-500 underline" href="${url}">${url}</a>`);
```

## System, Clipboard, and Feedback

- `exec(command, args?)` — run an explicit subprocess without invoking a shell;
  returns `{ stdout, stderr, exitCode }`. Prefer `exec(binary, [arg1, arg2])`
  for user-provided values. Pipes, redirection, shell operators, and command
  substitution are rejected instead of silently invoking a shell.
- `clipboard.readText()`, `copy(text)`, `paste()` — clipboard access. `paste()`
  returns the clipboard text; it does not synthesize a paste keystroke or inject
  text into another application.
- `getSelectedText()` / `setSelectedText(text)` — read/replace the selection in the focused app.
- `readFile(path, encoding?)`, `writeFile(path, contents, encoding?)`, and
  `home(...paths)` — explicit-path filesystem helpers.
- `hud(message)` — in-launcher overlay; `notify(message | { title, body })` — macOS Notification Center.
- `beep()` and `say(text)` are **experimental**: they return a dispatch receipt, but audible delivery isn't verified.

## Automation Receipts

Scripts can inspect the app's UI state without screenshots, and run deterministic UI transactions without sleeps:

```typescript
const state = await getState();
const elements = await getElements(100);

await batch([
  { type: "setInput", text: "sdk" },
  { type: "waitFor", condition: "choicesRendered", timeout: 1000 },
  { type: "selectByValue", value: "builtin/sdk-reference", submit: true },
]);
```

## Bring Your Own Packages

```bash
cd ~/.scriptkit
bun add zod date-fns
```

```typescript
import "@scriptkit/sdk";
import { z } from "zod";

const payload = await arg("Paste JSON");
const parsed = z.object({ title: z.string() }).parse(JSON.parse(payload));
await div(`<h1>${parsed.title}</h1>`);
```

## Observation Helpers

`computer.listNativeWindows()` and `computer.captureNativeWindow(...)` give
scripts explicitly scoped access to native macOS windows — see
[Computer Use](./computer-use.md). Listing is observation-only; capturing an
image is a sensitive operation that requires both Accessibility and Screen
Recording permission. Never call capture as a background capability check.
MCP client helpers (`mcp.listTools`, `mcp.call`, ...) are covered in
[MCP and Agent Context](./mcp-and-agent-context.md).

## Compatibility and Author Diagnostics

The versioned `kit://sdk-reference` capability catalog is the host-owned source
of truth for supported, experimental, and unsupported SDK globals and namespace
methods. Each row declares its minimum guaranteed host-contract version,
interactive-prompt requirement, permission prerequisites, platform restrictions,
and migration alternatives where relevant.

Author validation distinguishes:

- `unknown_capability` and `unsupported_capability`: choose a real SDK API or
  follow the supplied supported alternative.
- `missing_sdk_transport` and `interactive_prompt_unavailable`: move the
  command into a regular TypeScript script when its scriptlet topology cannot
  provide SDK globals or prompt responses.
- `unsupported_platform`, `missing_permission`, `host_version_too_old`, and
  `invalid_host_version`: correct the explicitly reported host prerequisite
  before execution.
- `permission_inventory_unavailable`: the command requires a permission, but
  no already-known grant inventory was supplied. Permission is neither assumed
  granted nor reported denied; supply an existing read-only inventory first.

These checks use already-known host facts. They do not request permissions,
open a window, capture the screen, focus another app, or start an AI request.

## AI Context Without Accidental Inference

`aiIsOpen()`, `aiGetActiveChat()`, `aiListChats()`, `aiGetConversation()`, and
`aiGetStreamingStatus()` inspect explicitly scoped chat state without starting a
provider request. `aiAppendMessage()` updates a conversation without requesting
a response. In contrast, `aiSendMessage()` requests inference, and
`aiStartChat()` may request inference unless its options explicitly disable the
response. Use the read-only helpers for authoring, validation, and preflight;
reserve message submission for an intentional user action.

The separate `chat(...)` prompt is UI-only: it never starts built-in provider
inference. Its `chat.addMessage`, `chat.startStream`, `chat.appendChunk`,
`chat.completeStream`, `chat.clear`, `chat.setError`, and `chat.clearError`
controllers require an already-active inline chat session. `chat.getMessages()`
and `chat.getResult()` only inspect script-local conversation state.

## Discover APIs In-App

- Read **`kit://command-doctor`** for a side-effect-free readiness report over
  already-loaded scripts and scriptlets, including real capability support,
  blocked or permission-pending state, real launcher actions, plugin ownership,
  SHA-256-redacted command identity, and suggested repairs. The wire shape is
  available as the SDK's `HostCommandDoctorReport` type. This is a host MCP
  resource, not a `commandDoctor()` SDK function.
- Search **`sdk`**, **`doctor`**, **`diagnostics`**, or **`permissions`** in the
  launcher and open **SDK Reference**. It is generated from the same Rust-owned
  catalog as the `kit://sdk-reference` MCP resource. Its versioned capability
  catalog declares support, experimental status, required permissions, platform
  restrictions, interactive-transport requirements, and migration alternatives;
  the additive `authoringResources` entries identify the Command Doctor and
  Script Issues without inventing callable globals.
- Search **`template`** and use **New Script from Template** to start from a working starter instead of a blank file.

## Related

- [Getting Started](./getting-started.md) — build, hotkey, first script
- [Main Menu Input](./main-menu-input.md) — aliases, keywords, capture handlers, command heads
