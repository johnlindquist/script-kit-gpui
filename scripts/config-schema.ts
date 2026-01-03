/**
 * ╔═══════════════════════════════════════════════════════════════════════════╗
 * ║                    SCRIPT KIT CONFIGURATION SCHEMA                         ║
 * ║                                                                             ║
 * ║  This is the AUTHORITATIVE REFERENCE for AI agents modifying config.ts     ║
 * ║  READ THIS FILE FIRST before making any configuration changes.             ║
 * ╚═══════════════════════════════════════════════════════════════════════════╝
 *
 * @fileoverview AI Agent Configuration Reference for Script Kit
 * @version 1.0.0
 * @license MIT
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
 * import type { Config } from "@johnlindquist/kit";
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
 * import type { Config } from "@johnlindquist/kit";
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
 * import type { Config } from "@johnlindquist/kit";
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
 * import type { Config } from "@johnlindquist/kit";
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
 * import type { Config } from "@johnlindquist/kit";
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
 * import type { Config } from "@johnlindquist/kit";
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
 * 1. Always import Config type from "@johnlindquist/kit"
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

// Re-export all Config types from kit-sdk.ts
// This file serves as the single import point for config types

export type {
  // Core config types
  Config,
  HotkeyConfig,
  ContentPadding,
  BuiltInConfig,
  ProcessLimits,
  
  // Key types for hotkey configuration
  KeyModifier,
  KeyCode,
} from './kit-sdk';
