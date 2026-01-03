/**
 * ╔═══════════════════════════════════════════════════════════════════════════╗
 * ║                    SCRIPT KIT CONFIGURATION SCHEMA                         ║
 * ║                                                                             ║
 * ║  This is the AUTHORITATIVE REFERENCE for AI agents modifying config.ts     ║
 * ║  READ THIS FILE FIRST before making any configuration changes.             ║
 * ╚═══════════════════════════════════════════════════════════════════════════╝
 *
 * @fileoverview AI Agent Configuration Reference for Script Kit
 * @version 1.1.0
 * @license MIT
 *
 * ┌───────────────────────────────────────────────────────────────────────────┐
 * │                      COMMAND ID SYSTEM (NEW IN 1.1)                        │
 * └───────────────────────────────────────────────────────────────────────────┘
 *
 * Script Kit uses a unified Command ID system to identify all executable items
 * in the main menu. This includes built-in features, applications, user scripts,
 * and scriptlets (inline scripts).
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * COMMAND ID FORMAT
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * Command IDs use a category-prefixed format: `{category}/{identifier}`
 *
 * CATEGORIES:
 * ───────────────────────────────────────────────────────────────────────────
 *
 * 1. BUILTIN - Built-in Script Kit features
 *    Format: `builtin/{feature-name}`
 *    Examples:
 *      - "builtin/clipboard-history"  → Clipboard history manager
 *      - "builtin/app-launcher"       → Application launcher
 *      - "builtin/window-switcher"    → Window switcher
 *      - "builtin/file-search"        → File search
 *      - "builtin/emoji-picker"       → Emoji picker
 *
 * 2. APP - macOS Applications (by bundle identifier)
 *    Format: `app/{bundle-id}`
 *    Examples:
 *      - "app/com.apple.Safari"       → Safari
 *      - "app/com.google.Chrome"      → Google Chrome
 *      - "app/com.microsoft.VSCode"   → Visual Studio Code
 *      - "app/com.apple.finder"       → Finder
 *      - "app/com.spotify.client"     → Spotify
 *
 * 3. SCRIPT - User scripts (by filename without extension)
 *    Format: `script/{script-name}`
 *    Examples:
 *      - "script/my-custom-script"    → ~/.sk/kit/scripts/my-custom-script.ts
 *      - "script/daily-standup"       → ~/.sk/kit/scripts/daily-standup.ts
 *      - "script/git-commit-helper"   → ~/.sk/kit/scripts/git-commit-helper.ts
 *
 * 4. SCRIPTLET - Inline scriptlets (by UUID)
 *    Format: `scriptlet/{uuid}`
 *    Examples:
 *      - "scriptlet/a1b2c3d4-e5f6-7890-abcd-ef1234567890"
 *      - "scriptlet/clipboard-to-uppercase"
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * DEEPLINK MAPPING
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * Command IDs map 1:1 to deeplinks with the `kit://commands/` prefix:
 *
 * | Config Key                    | Deeplink URL                                  |
 * |-------------------------------|-----------------------------------------------|
 * | builtin/clipboard-history     | kit://commands/builtin/clipboard-history      |
 * | app/com.apple.Safari          | kit://commands/app/com.apple.Safari           |
 * | script/my-script              | kit://commands/script/my-script               |
 * | scriptlet/abc123              | kit://commands/scriptlet/abc123               |
 *
 * IMPORTANT: The `commands/` prefix is part of the URL namespace, NOT the config key.
 * In config.ts, use the ID without the prefix.
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * COMMAND CONFIGURATION OPTIONS
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * Each command can be configured with:
 *
 * 1. SHORTCUT - Global keyboard shortcut
 *    Type: HotkeyConfig (same as the main hotkey)
 *    Purpose: Open this command directly without going through main menu
 *    Example: { modifiers: ["meta", "shift"], key: "KeyV" }
 *
 * 2. HIDDEN - Hide from main menu
 *    Type: boolean
 *    Purpose: Keep the command available via shortcut but hide from list
 *    Default: false
 *    Example: true (hidden) or false (visible)
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * CONFIGURATION EXAMPLES
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * EXAMPLE 1: Add shortcuts to built-in features
 * ```typescript
 * commands: {
 *   "builtin/clipboard-history": {
 *     shortcut: { modifiers: ["meta", "shift"], key: "KeyV" }
 *   },
 *   "builtin/emoji-picker": {
 *     shortcut: { modifiers: ["meta", "ctrl"], key: "Space" }
 *   }
 * }
 * ```
 *
 * EXAMPLE 2: Add shortcuts to applications
 * ```typescript
 * commands: {
 *   "app/com.apple.Safari": {
 *     shortcut: { modifiers: ["meta", "shift"], key: "KeyS" }
 *   },
 *   "app/com.google.Chrome": {
 *     shortcut: { modifiers: ["meta", "shift"], key: "KeyC" }
 *   }
 * }
 * ```
 *
 * EXAMPLE 3: Configure user scripts
 * ```typescript
 * commands: {
 *   "script/daily-standup": {
 *     shortcut: { modifiers: ["meta", "shift"], key: "KeyD" }
 *   },
 *   "script/deprecated-helper": {
 *     hidden: true  // Keep the script but hide from menu
 *   }
 * }
 * ```
 *
 * EXAMPLE 4: Full configuration with multiple command types
 * ```typescript
 * import type { Config } from "@scriptkit/kit";
 *
 * export default {
 *   hotkey: { modifiers: ["meta"], key: "Semicolon" },
 *   commands: {
 *     // Built-in features with shortcuts
 *     "builtin/clipboard-history": {
 *       shortcut: { modifiers: ["meta", "shift"], key: "KeyV" }
 *     },
 *     "builtin/app-launcher": {
 *       shortcut: { modifiers: ["meta"], key: "Space" },
 *       hidden: false
 *     },
 *     // Applications
 *     "app/com.apple.Safari": {
 *       shortcut: { modifiers: ["meta", "shift"], key: "KeyS" }
 *     },
 *     // User scripts
 *     "script/my-workflow": {
 *       shortcut: { modifiers: ["meta", "shift"], key: "KeyW" }
 *     }
 *   }
 * } satisfies Config;
 * ```
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * AI AGENT PATTERNS FOR COMMANDS
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * PATTERN: Add a shortcut to a built-in feature
 * ```typescript
 * // Add Cmd+Shift+V for clipboard history
 * commands: {
 *   ...existingCommands,
 *   "builtin/clipboard-history": {
 *     shortcut: { modifiers: ["meta", "shift"], key: "KeyV" }
 *   }
 * }
 * ```
 *
 * PATTERN: Hide a command but keep its shortcut
 * ```typescript
 * // Hide from menu but still accessible via Cmd+Shift+X
 * commands: {
 *   "script/secret-tool": {
 *     shortcut: { modifiers: ["meta", "shift"], key: "KeyX" },
 *     hidden: true
 *   }
 * }
 * ```
 *
 * PATTERN: Remove a shortcut (set to undefined or delete key)
 * ```typescript
 * // Option 1: Explicitly set to undefined
 * commands: {
 *   "builtin/clipboard-history": {
 *     shortcut: undefined
 *   }
 * }
 *
 * // Option 2: Omit the command entry entirely (use default behavior)
 * commands: {
 *   // no entry for builtin/clipboard-history
 * }
 * ```
 *
 * PATTERN: Determine command ID from user request
 * ```
 * User says: "Add shortcut for Safari" → "app/com.apple.Safari"
 * User says: "Add shortcut for clipboard" → "builtin/clipboard-history"
 * User says: "Add shortcut for my-script" → "script/my-script"
 * User says: "Hide the app launcher" → "builtin/app-launcher" with hidden: true
 * ```
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * VALIDATION RULES FOR COMMANDS
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * 1. Command IDs MUST start with a valid category prefix:
 *    - "builtin/" | "app/" | "script/" | "scriptlet/"
 *
 * 2. Shortcuts follow the same rules as the main hotkey:
 *    - Modifiers: ["meta", "ctrl", "alt", "shift"]
 *    - Keys: "KeyA"-"KeyZ", "Digit0"-"Digit9", "Space", etc.
 *
 * 3. The commands field is optional - omit it entirely if not needed
 *
 * 4. Individual command configs are optional - only include what you want to customize
 *
 * 5. Unknown command IDs are ignored (safe for forward compatibility)
 *
 * ┌───────────────────────────────────────────────────────────────────────────┐
 * │                           CONFIGURATION FILE                               │
 * └───────────────────────────────────────────────────────────────────────────┘
 *
 * LOCATION: ~/.sk/kit/config.ts
 *
 * PURPOSE: Controls Script Kit's behavior, appearance, and built-in features.
 * The config file is a TypeScript module that exports a default Config object.
 *
 * FILE STRUCTURE:
 * ```typescript
 * import type { Config } from "@scriptkit/kit";
 *
 * export default {
 *   // ... configuration options ...
 * } satisfies Config;
 * ```
 *
 * ┌───────────────────────────────────────────────────────────────────────────┐
 * │                           CONFIGURATION OPTIONS                            │
 * └───────────────────────────────────────────────────────────────────────────┘
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * CATEGORY 1: HOTKEY CONFIGURATION (REQUIRED)
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * FIELD: hotkey
 * TYPE: HotkeyConfig (REQUIRED)
 * PURPOSE: Global keyboard shortcut to open Script Kit
 *
 * STRUCTURE:
 * ```typescript
 * hotkey: {
 *   modifiers: KeyModifier[],  // Array of modifier keys
 *   key: KeyCode               // Main key code
 * }
 * ```
 *
 * VALID MODIFIERS (KeyModifier):
 * - "meta"   → Cmd on macOS, Win on Windows
 * - "ctrl"   → Control key
 * - "alt"    → Option on macOS, Alt on Windows
 * - "shift"  → Shift key
 *
 * VALID KEY CODES (KeyCode):
 * - Letters: "KeyA" through "KeyZ"
 * - Numbers: "Digit0" through "Digit9"
 * - Special: "Space", "Enter", "Semicolon"
 * - Function: "F1" through "F12"
 *
 * EXAMPLES:
 * - Cmd+; (macOS default): { modifiers: ["meta"], key: "Semicolon" }
 * - Ctrl+Space:           { modifiers: ["ctrl"], key: "Space" }
 * - Cmd+Shift+K:          { modifiers: ["meta", "shift"], key: "KeyK" }
 * - Alt+0:                { modifiers: ["alt"], key: "Digit0" }
 *
 * CONSTRAINTS:
 * - At least one modifier is recommended (avoid conflicts with system shortcuts)
 * - Key must be a valid KeyCode value (see list above)
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * CATEGORY 2: UI SETTINGS (OPTIONAL)
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * FIELD: padding
 * TYPE: ContentPadding (optional)
 * PURPOSE: Controls spacing around content in prompts
 * DEFAULT: { top: 8, left: 12, right: 12 }
 *
 * STRUCTURE:
 * ```typescript
 * padding: {
 *   top?: number,    // Top padding in pixels (default: 8)
 *   left?: number,   // Left padding in pixels (default: 12)
 *   right?: number   // Right padding in pixels (default: 12)
 * }
 * ```
 *
 * EXAMPLES:
 * - More spacious: { top: 16, left: 20, right: 20 }
 * - More compact:  { top: 4, left: 8, right: 8 }
 * - Just top:      { top: 16 }  // left/right use defaults
 *
 * ───────────────────────────────────────────────────────────────────────────
 *
 * FIELD: editorFontSize
 * TYPE: number (optional)
 * PURPOSE: Font size for the Monaco-style code editor in pixels
 * DEFAULT: 14
 * VALID RANGE: 8-32 (recommended); any positive number works
 *
 * EXAMPLES:
 * - Smaller for more code:   12
 * - Default:                 14
 * - Larger for readability:  16
 * - Accessibility:           18-24
 *
 * ───────────────────────────────────────────────────────────────────────────
 *
 * FIELD: terminalFontSize
 * TYPE: number (optional)
 * PURPOSE: Font size for the integrated terminal in pixels
 * DEFAULT: 14
 * VALID RANGE: 8-32 (recommended); any positive number works
 *
 * EXAMPLES:
 * - Compact:                 12
 * - Default:                 14
 * - Larger:                  16
 *
 * ───────────────────────────────────────────────────────────────────────────
 *
 * FIELD: uiScale
 * TYPE: number (optional)
 * PURPOSE: Scale factor for the entire UI (1.0 = 100%)
 * DEFAULT: 1.0
 * VALID RANGE: 0.5-2.0 (recommended)
 *
 * EXAMPLES:
 * - Slightly smaller: 0.9
 * - Default:          1.0
 * - 125% scale:       1.25
 * - 150% for HiDPI:   1.5
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * CATEGORY 3: BUILT-IN FEATURES (OPTIONAL)
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * FIELD: builtIns
 * TYPE: BuiltInConfig (optional)
 * PURPOSE: Enable/disable built-in features
 * DEFAULT: { clipboardHistory: true, appLauncher: true, windowSwitcher: true }
 *
 * STRUCTURE:
 * ```typescript
 * builtIns: {
 *   clipboardHistory?: boolean,  // Clipboard history tracking (default: true)
 *   appLauncher?: boolean,       // Application launcher (default: true)
 *   windowSwitcher?: boolean     // Window switcher (default: true)
 * }
 * ```
 *
 * EXAMPLES:
 * - Disable clipboard only:  { clipboardHistory: false }
 * - Disable all but launcher: { clipboardHistory: false, windowSwitcher: false }
 * - Enable all (explicit):   { clipboardHistory: true, appLauncher: true, windowSwitcher: true }
 *
 * FIELD: clipboardHistoryMaxTextLength
 * TYPE: number (optional)
 * PURPOSE: Limit text size stored for clipboard history entries (bytes)
 * DEFAULT: 100000
 * NOTES: Set to 0 to disable the limit
 *
 * EXAMPLES:
 * - Default limit: 100000
 * - Larger limit:  200000
 * - No limit:      0
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * CATEGORY 4: PROCESS LIMITS (OPTIONAL)
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * FIELD: processLimits
 * TYPE: ProcessLimits (optional)
 * PURPOSE: Control script execution resources and monitoring
 * DEFAULT: { healthCheckIntervalMs: 5000 } (no memory/runtime limits)
 *
 * STRUCTURE:
 * ```typescript
 * processLimits: {
 *   maxMemoryMb?: number,           // Max memory in MB (default: unlimited)
 *   maxRuntimeSeconds?: number,     // Max runtime in seconds (default: unlimited)
 *   healthCheckIntervalMs?: number  // Health check interval in ms (default: 5000)
 * }
 * ```
 *
 * EXAMPLES:
 * - Memory limit only:     { maxMemoryMb: 512 }
 * - Runtime limit only:    { maxRuntimeSeconds: 60 }
 * - Both limits:           { maxMemoryMb: 256, maxRuntimeSeconds: 30 }
 * - Faster health checks:  { healthCheckIntervalMs: 1000 }
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * CATEGORY 5: EXTERNAL TOOLS (OPTIONAL)
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * FIELD: bun_path
 * TYPE: string (optional)
 * PURPOSE: Custom path to bun executable
 * DEFAULT: Auto-detected from PATH
 *
 * EXAMPLES:
 * - Homebrew:   "/opt/homebrew/bin/bun"
 * - Linux:      "/usr/local/bin/bun"
 * - Custom:     "/Users/me/tools/bun"
 *
 * ───────────────────────────────────────────────────────────────────────────
 *
 * FIELD: editor
 * TYPE: string (optional)
 * PURPOSE: Command for "Open in Editor" actions
 * DEFAULT: Uses $EDITOR env var, or "code" (VS Code)
 *
 * EXAMPLES:
 * - VS Code:       "code"
 * - Vim:           "vim"
 * - Neovim:        "nvim"
 * - Sublime Text:  "subl"
 * - Zed:           "zed"
 *
 * ┌───────────────────────────────────────────────────────────────────────────┐
 * │                    COMMON MODIFICATION PATTERNS                           │
 * └───────────────────────────────────────────────────────────────────────────┘
 *
 * PATTERN: Change the global hotkey
 * ```typescript
 * // Before: Cmd+;
 * hotkey: { modifiers: ["meta"], key: "Semicolon" }
 *
 * // After: Ctrl+Space
 * hotkey: { modifiers: ["ctrl"], key: "Space" }
 * ```
 *
 * PATTERN: Increase font size for accessibility
 * ```typescript
 * // Add these fields to increase readability
 * editorFontSize: 18,
 * terminalFontSize: 18,
 * uiScale: 1.25
 * ```
 *
 * PATTERN: Disable a built-in feature
 * ```typescript
 * // Disable clipboard history (privacy concern)
 * builtIns: {
 *   clipboardHistory: false
 * }
 * ```
 *
 * PATTERN: Add script resource limits
 * ```typescript
 * // Prevent runaway scripts
 * processLimits: {
 *   maxMemoryMb: 512,
 *   maxRuntimeSeconds: 300  // 5 minutes
 * }
 * ```
 *
 * PATTERN: Configure for Vim user
 * ```typescript
 * editor: "nvim"
 * ```
 *
 * ┌───────────────────────────────────────────────────────────────────────────┐
 * │                         EXAMPLE CONFIGURATIONS                             │
 * └───────────────────────────────────────────────────────────────────────────┘
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * EXAMPLE 1: MINIMAL CONFIG (Just the essentials)
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * ```typescript
 * import type { Config } from "@scriptkit/kit";
 *
 * export default {
 *   hotkey: {
 *     modifiers: ["meta"],
 *     key: "Semicolon"
 *   }
 * } satisfies Config;
 * ```
 *
 * USE CASE: New users, default behavior desired
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * EXAMPLE 2: POWER USER CONFIG (All features, optimized)
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * ```typescript
 * import type { Config } from "@scriptkit/kit";
 *
 * export default {
 *   hotkey: {
 *     modifiers: ["meta"],
 *     key: "Semicolon"
 *   },
 *   editor: "zed",
 *   padding: { top: 8, left: 12, right: 12 },
 *   editorFontSize: 14,
 *   terminalFontSize: 14,
 *   uiScale: 1.0,
 *   builtIns: {
 *     clipboardHistory: true,
 *     appLauncher: true,
 *     windowSwitcher: true
 *   },
 *   processLimits: {
 *     maxMemoryMb: 1024,
 *     maxRuntimeSeconds: 600,
 *     healthCheckIntervalMs: 5000
 *   }
 * } satisfies Config;
 * ```
 *
 * USE CASE: Users who want explicit control over all settings
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * EXAMPLE 3: ACCESSIBILITY-FOCUSED CONFIG
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * ```typescript
 * import type { Config } from "@scriptkit/kit";
 *
 * export default {
 *   hotkey: {
 *     modifiers: ["meta"],
 *     key: "Semicolon"
 *   },
 *   // Large fonts for better readability
 *   editorFontSize: 20,
 *   terminalFontSize: 20,
 *   // Scale up the entire UI
 *   uiScale: 1.5,
 *   // More padding for easier targeting
 *   padding: { top: 16, left: 20, right: 20 }
 * } satisfies Config;
 * ```
 *
 * USE CASE: Users with visual impairments, large monitors
 * NOTE: Theme colors are controlled separately in ~/.sk/kit/theme.json
 *       High contrast themes should be configured there, not here.
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * EXAMPLE 4: DEVELOPER-FOCUSED CONFIG
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * ```typescript
 * import type { Config } from "@scriptkit/kit";
 *
 * export default {
 *   hotkey: {
 *     modifiers: ["meta", "shift"],
 *     key: "KeyK"
 *   },
 *   // Use Neovim as editor
 *   editor: "nvim",
 *   // Smaller fonts for more code visibility
 *   editorFontSize: 12,
 *   terminalFontSize: 12,
 *   // Compact padding
 *   padding: { top: 4, left: 8, right: 8 },
 *   // Disable features not needed
 *   builtIns: {
 *     clipboardHistory: false,  // Use external clipboard manager
 *     appLauncher: true,
 *     windowSwitcher: false     // Use external window manager
 *   },
 *   // Strict resource limits for CI/automation
 *   processLimits: {
 *     maxMemoryMb: 256,
 *     maxRuntimeSeconds: 30,
 *     healthCheckIntervalMs: 1000
 *   }
 * } satisfies Config;
 * ```
 *
 * USE CASE: Developers with custom tooling, CI environments
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * EXAMPLE 5: PRIVACY-FOCUSED CONFIG
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * ```typescript
 * import type { Config } from "@scriptkit/kit";
 *
 * export default {
 *   hotkey: {
 *     modifiers: ["meta"],
 *     key: "Semicolon"
 *   },
 *   // Disable all features that track data
 *   builtIns: {
 *     clipboardHistory: false,  // Don't track clipboard
 *     appLauncher: true,        // Safe - just launches apps
 *     windowSwitcher: true      // Safe - just switches windows
 *   }
 * } satisfies Config;
 * ```
 *
 * USE CASE: Privacy-conscious users, shared computers
 *
 * ┌───────────────────────────────────────────────────────────────────────────┐
 * │                           FIELD QUICK REFERENCE                           │
 * └───────────────────────────────────────────────────────────────────────────┘
 *
 * | Field             | Type           | Default                     | Required |
 * |-------------------|----------------|-----------------------------|----------|
 * | hotkey            | HotkeyConfig   | -                           | YES      |
 * | hotkey.modifiers  | KeyModifier[]  | -                           | YES      |
 * | hotkey.key        | KeyCode        | -                           | YES      |
 * | padding           | ContentPadding | {top:8,left:12,right:12}    | no       |
 * | padding.top       | number         | 8                           | no       |
 * | padding.left      | number         | 12                          | no       |
 * | padding.right     | number         | 12                          | no       |
 * | editorFontSize    | number         | 14                          | no       |
 * | terminalFontSize  | number         | 14                          | no       |
 * | uiScale           | number         | 1.0                         | no       |
 * | builtIns          | BuiltInConfig  | {all: true}                 | no       |
 * | builtIns.clipboardHistory | boolean | true                       | no       |
 * | builtIns.appLauncher      | boolean | true                       | no       |
 * | builtIns.windowSwitcher   | boolean | true                       | no       |
 * | clipboardHistoryMaxTextLength | number | 100000                  | no       |
 * | processLimits     | ProcessLimits  | {healthCheck:5000}          | no       |
 * | processLimits.maxMemoryMb        | number | unlimited            | no       |
 * | processLimits.maxRuntimeSeconds  | number | unlimited            | no       |
 * | processLimits.healthCheckIntervalMs | number | 5000              | no       |
 * | bun_path          | string         | auto-detected               | no       |
 * | editor            | string         | $EDITOR or "code"           | no       |
 *
 * ┌───────────────────────────────────────────────────────────────────────────┐
 * │                         AI AGENT INSTRUCTIONS                             │
 * └───────────────────────────────────────────────────────────────────────────┘
 *
 * WHEN MODIFYING CONFIG:
 * 1. Always import Config type from "@scriptkit/kit"
 * 2. Use `satisfies Config` at the end for type checking
 * 3. Only include fields that differ from defaults (for minimal configs)
 * 4. Hotkey is the ONLY required field
 *
 * WHEN READING CONFIG:
 * 1. Check if field exists (may be undefined = use default)
 * 2. Use nullish coalescing for defaults: `config.editorFontSize ?? 14`
 *
 * VALIDATION:
 * - Modifiers must be from: "meta", "ctrl", "alt", "shift"
 * - Key must be a valid KeyCode (see list in Category 1)
 * - Font sizes should be positive numbers (8-32 recommended)
 * - UI scale should be 0.5-2.0 for reasonable display
 *
 * RELATED FILES:
 * - ~/.sk/kit/theme.json - Color themes and visual appearance
 * - ~/.sk/kit/scripts/   - User scripts
 * - ~/.sk/kit/sdk/       - SDK runtime files
 */

// Re-export base types from kit-sdk.ts
// These are the foundation types that CommandConfig builds upon

export type {
  // Core config types (base interface - extended below with commands)
  HotkeyConfig,
  ContentPadding,
  BuiltInConfig,
  ProcessLimits,
  
  // Key types for hotkey configuration
  KeyModifier,
  KeyCode,
} from './kit-sdk';

// Import base Config for extension
import type { 
  Config as BaseConfig,
  HotkeyConfig 
} from './kit-sdk';

// =============================================================================
// COMMAND ID TYPES
// =============================================================================

/**
 * Built-in Script Kit feature command ID.
 * 
 * Format: `builtin/{feature-name}`
 * 
 * Built-in features are the core functionality provided by Script Kit:
 * - clipboard-history: Clipboard history manager
 * - app-launcher: Application launcher  
 * - window-switcher: Window switcher
 * - file-search: File search
 * - emoji-picker: Emoji picker
 * 
 * @example "builtin/clipboard-history"
 * @example "builtin/app-launcher"
 * @example "builtin/window-switcher"
 * 
 * @see BuiltInConfig for enabling/disabling built-in features
 */
export type BuiltinCommandId = `builtin/${string}`;

/**
 * Application command ID (macOS bundle identifier).
 * 
 * Format: `app/{bundle-id}`
 * 
 * Applications are identified by their macOS bundle identifier.
 * You can find an app's bundle ID using:
 * - Terminal: `osascript -e 'id of app "App Name"'`
 * - Finder: Right-click .app → Show Package Contents → Info.plist → CFBundleIdentifier
 * 
 * @example "app/com.apple.Safari"
 * @example "app/com.google.Chrome"
 * @example "app/com.microsoft.VSCode"
 * @example "app/com.apple.finder"
 * @example "app/com.spotify.client"
 */
export type AppCommandId = `app/${string}`;

/**
 * User script command ID.
 * 
 * Format: `script/{script-name}`
 * 
 * The script name is the filename without the .ts extension.
 * Scripts are located in ~/.sk/kit/scripts/
 * 
 * @example "script/my-custom-script" → ~/.sk/kit/scripts/my-custom-script.ts
 * @example "script/daily-standup" → ~/.sk/kit/scripts/daily-standup.ts
 * @example "script/git-commit-helper" → ~/.sk/kit/scripts/git-commit-helper.ts
 */
export type ScriptCommandId = `script/${string}`;

/**
 * Scriptlet command ID (inline script).
 * 
 * Format: `scriptlet/{uuid-or-name}`
 * 
 * Scriptlets are small inline scripts that can be created and edited
 * directly in Script Kit without a separate file. They're identified
 * by a UUID or a user-provided name.
 * 
 * @example "scriptlet/a1b2c3d4-e5f6-7890-abcd-ef1234567890"
 * @example "scriptlet/clipboard-to-uppercase"
 */
export type ScriptletCommandId = `scriptlet/${string}`;

/**
 * Union of all valid command ID formats.
 * 
 * This is the type to use when you need to accept any command ID.
 * Each category has a specific prefix that identifies the command type.
 * 
 * Categories:
 * - `builtin/` - Built-in Script Kit features
 * - `app/` - macOS applications (by bundle identifier)
 * - `script/` - User scripts (by filename)
 * - `scriptlet/` - Inline scriptlets (by UUID or name)
 * 
 * @example
 * ```typescript
 * function getDeeplink(commandId: CommandId): string {
 *   return `kit://commands/${commandId}`;
 * }
 * 
 * getDeeplink("builtin/clipboard-history");  // "kit://commands/builtin/clipboard-history"
 * getDeeplink("app/com.apple.Safari");       // "kit://commands/app/com.apple.Safari"
 * ```
 */
export type CommandId = 
  | BuiltinCommandId 
  | AppCommandId 
  | ScriptCommandId 
  | ScriptletCommandId;

// =============================================================================
// COMMAND CONFIGURATION TYPES
// =============================================================================

/**
 * Configuration options for a single command.
 * 
 * Commands can have:
 * 1. A global keyboard shortcut to invoke them directly
 * 2. A hidden flag to remove them from the main menu
 * 
 * All fields are optional - only include what you want to customize.
 * 
 * @example Add a shortcut to clipboard history
 * ```typescript
 * "builtin/clipboard-history": {
 *   shortcut: { modifiers: ["meta", "shift"], key: "KeyV" }
 * }
 * ```
 * 
 * @example Hide a command but keep its shortcut
 * ```typescript
 * "script/secret-tool": {
 *   shortcut: { modifiers: ["meta", "shift"], key: "KeyX" },
 *   hidden: true
 * }
 * ```
 * 
 * @example Just hide a command (no shortcut)
 * ```typescript
 * "builtin/app-launcher": {
 *   hidden: true
 * }
 * ```
 */
export interface CommandConfig {
  /**
   * Global keyboard shortcut to invoke this command directly.
   * 
   * When set, pressing this shortcut will immediately run the command
   * without going through the main Script Kit menu.
   * 
   * Uses the same format as the main hotkey configuration:
   * - modifiers: Array of ["meta", "ctrl", "alt", "shift"]
   * - key: Key code like "KeyV", "Space", "Semicolon"
   * 
   * @default undefined (no shortcut)
   * @example { modifiers: ["meta", "shift"], key: "KeyV" } // Cmd+Shift+V
   * @example { modifiers: ["meta", "ctrl"], key: "Space" } // Cmd+Ctrl+Space
   */
  shortcut?: HotkeyConfig;

  /**
   * Whether to hide this command from the main menu.
   * 
   * When true, the command won't appear in the Script Kit main menu
   * or search results. However, it can still be invoked via:
   * - Its keyboard shortcut (if configured)
   * - Deeplink URL (kit://commands/{id})
   * - Programmatic invocation
   * 
   * Use this for commands you want accessible but not cluttering the menu.
   * 
   * @default false (visible in menu)
   * @example true // Hide from menu
   * @example false // Show in menu (default)
   */
  hidden?: boolean;
}

/**
 * Map of command IDs to their configurations.
 * 
 * This is a partial record - you only need to include commands you want
 * to customize. Commands not listed will use default behavior.
 * 
 * @example
 * ```typescript
 * commands: {
 *   "builtin/clipboard-history": { 
 *     shortcut: { modifiers: ["meta", "shift"], key: "KeyV" } 
 *   },
 *   "app/com.apple.Safari": { 
 *     shortcut: { modifiers: ["meta", "shift"], key: "KeyS" } 
 *   },
 *   "script/my-workflow": { 
 *     hidden: true 
 *   }
 * }
 * ```
 */
export type CommandsConfig = Partial<Record<CommandId, CommandConfig>>;

// =============================================================================
// EXTENDED CONFIG TYPE
// =============================================================================

/**
 * Script Kit configuration schema with command configuration support.
 * 
 * This extends the base Config interface to add the `commands` field
 * for configuring individual command shortcuts and visibility.
 * 
 * @example Minimal configuration (only hotkey required)
 * ```typescript
 * import type { Config } from "@scriptkit/kit";
 * 
 * export default {
 *   hotkey: { modifiers: ["meta"], key: "Semicolon" }
 * } satisfies Config;
 * ```
 * 
 * @example Configuration with command shortcuts
 * ```typescript
 * import type { Config } from "@scriptkit/kit";
 * 
 * export default {
 *   hotkey: { modifiers: ["meta"], key: "Semicolon" },
 *   commands: {
 *     "builtin/clipboard-history": {
 *       shortcut: { modifiers: ["meta", "shift"], key: "KeyV" }
 *     },
 *     "app/com.apple.Safari": {
 *       shortcut: { modifiers: ["meta", "shift"], key: "KeyS" }
 *     }
 *   }
 * } satisfies Config;
 * ```
 */
export interface Config extends BaseConfig {
  /**
   * Command-specific configuration for shortcuts and visibility.
   * 
   * Each key is a command ID in the format `{category}/{identifier}`:
   * - `builtin/{name}` - Built-in features (clipboard-history, app-launcher, etc.)
   * - `app/{bundle-id}` - macOS applications (com.apple.Safari, etc.)
   * - `script/{name}` - User scripts (my-script, etc.)
   * - `scriptlet/{uuid}` - Inline scriptlets
   * 
   * Each value is a CommandConfig object with optional shortcut and hidden fields.
   * 
   * Command IDs map 1:1 to deeplinks: `kit://commands/{id}`
   * 
   * @default undefined (no command customizations)
   * @example
   * ```typescript
   * commands: {
   *   "builtin/clipboard-history": {
   *     shortcut: { modifiers: ["meta", "shift"], key: "KeyV" }
   *   },
   *   "app/com.apple.Safari": {
   *     shortcut: { modifiers: ["meta", "shift"], key: "KeyS" }
   *   },
   *   "script/deprecated-tool": {
   *     hidden: true
   *   }
   * }
   * ```
   */
  commands?: CommandsConfig;
}

// =============================================================================
// UTILITY TYPES FOR AI AGENTS
// =============================================================================

/**
 * Extract the category from a command ID.
 * 
 * @example
 * ```typescript
 * type Category = CommandCategory<"builtin/clipboard-history">;  // "builtin"
 * type Category = CommandCategory<"app/com.apple.Safari">;       // "app"
 * ```
 */
export type CommandCategory<T extends CommandId> = 
  T extends `${infer Category}/${string}` ? Category : never;

/**
 * Extract the identifier from a command ID.
 * 
 * @example
 * ```typescript
 * type Id = CommandIdentifier<"builtin/clipboard-history">;  // "clipboard-history"
 * type Id = CommandIdentifier<"app/com.apple.Safari">;       // "com.apple.Safari"
 * ```
 */
export type CommandIdentifier<T extends CommandId> = 
  T extends `${string}/${infer Identifier}` ? Identifier : never;

/**
 * Construct a deeplink URL from a command ID.
 * 
 * The deeplink format is: `kit://commands/{commandId}`
 * 
 * @param commandId - The command ID (e.g., "builtin/clipboard-history")
 * @returns The full deeplink URL
 * 
 * @example
 * ```typescript
 * const link = toDeeplink("builtin/clipboard-history");
 * // Returns: "kit://commands/builtin/clipboard-history"
 * ```
 */
export function toDeeplink(commandId: CommandId): string {
  return `kit://commands/${commandId}`;
}

/**
 * Parse a deeplink URL to extract the command ID.
 * 
 * @param deeplink - The deeplink URL (e.g., "kit://commands/builtin/clipboard-history")
 * @returns The command ID, or null if the URL is not a valid command deeplink
 * 
 * @example
 * ```typescript
 * const id = fromDeeplink("kit://commands/builtin/clipboard-history");
 * // Returns: "builtin/clipboard-history"
 * 
 * const invalid = fromDeeplink("kit://other/path");
 * // Returns: null
 * ```
 */
export function fromDeeplink(deeplink: string): CommandId | null {
  const prefix = "kit://commands/";
  if (!deeplink.startsWith(prefix)) {
    return null;
  }
  const id = deeplink.slice(prefix.length);
  // Validate it matches one of our categories
  if (id.startsWith("builtin/") || 
      id.startsWith("app/") || 
      id.startsWith("script/") || 
      id.startsWith("scriptlet/")) {
    return id as CommandId;
  }
  return null;
}

/**
 * Check if a string is a valid command ID.
 * 
 * @param value - The string to check
 * @returns True if the string is a valid command ID format
 * 
 * @example
 * ```typescript
 * isValidCommandId("builtin/clipboard-history");  // true
 * isValidCommandId("app/com.apple.Safari");       // true
 * isValidCommandId("invalid-format");              // false
 * isValidCommandId("unknown/category");            // false
 * ```
 */
export function isValidCommandId(value: string): value is CommandId {
  return (
    value.startsWith("builtin/") ||
    value.startsWith("app/") ||
    value.startsWith("script/") ||
    value.startsWith("scriptlet/")
  );
}
