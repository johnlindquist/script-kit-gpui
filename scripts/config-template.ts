import type { Config } from "@johnlindquist/kit";

/**
 * Script Kit Configuration
 * ========================
 *
 * This file controls Script Kit's behavior, appearance, and built-in features.
 * It's loaded on startup from ~/.sk/kit/config.ts.
 *
 * HOW TO CUSTOMIZE:
 * 1. Uncomment the options you want to change
 * 2. Modify the values to your preference
 * 3. Save the file - changes take effect on next Script Kit restart
 *
 * DOCUMENTATION:
 * - Full schema with all options: See Config interface in kit-sdk.ts
 * - Type definitions provide inline documentation via your editor's hover
 *
 * TYPE SAFETY:
 * This file uses `satisfies Config` for compile-time type checking.
 * Your editor will warn you about invalid options or values.
 */
export default {
  // ===========================================================================
  // REQUIRED: Global Hotkey
  // ===========================================================================
  // The keyboard shortcut to activate Script Kit from any application.
  // This is the only required setting.

  hotkey: {
    // Modifier keys: "meta" (Cmd/Win), "ctrl", "alt" (Option), "shift"
    modifiers: ["meta"],

    // Main key (W3C key codes): "KeyA"-"KeyZ", "Digit0"-"Digit9",
    // "Space", "Enter", "Semicolon", "F1"-"F12"
    key: "Semicolon", // Cmd+; on Mac, Win+; on Windows
  },

  // ===========================================================================
  // UI Settings
  // ===========================================================================
  // Customize the appearance of Script Kit's interface.

  // Font size for the Monaco-style code editor (in pixels)
  // editorFontSize: 14,

  // Font size for the integrated terminal (in pixels)
  // terminalFontSize: 14,

  // UI scale factor (1.0 = 100%, 1.5 = 150%, etc.)
  // Useful for HiDPI displays or accessibility
  // uiScale: 1.0,

  // Content padding for prompts (terminal, editor, etc.)
  // All values in pixels
  // padding: {
  //   top: 8,
  //   left: 12,
  //   right: 12,
  // },

  // ===========================================================================
  // Editor Settings
  // ===========================================================================
  // Configure the external editor used for "Open in Editor" actions.

  // Editor command (falls back to $EDITOR env var, then "code")
  // Examples: "code", "vim", "nvim", "subl", "zed", "cursor"
  // editor: "code",

  // ===========================================================================
  // Built-in Features
  // ===========================================================================
  // Enable or disable Script Kit's built-in productivity features.

  // builtIns: {
  //   // Clipboard history - tracks clipboard changes with searchable history
  //   clipboardHistory: true,
  //
  //   // App launcher - search and launch applications
  //   appLauncher: true,
  //
  //   // Window switcher - manage open windows across applications
  //   windowSwitcher: true,
  // },
  //
  // Max text size (bytes) stored per clipboard history entry
  // Set to 0 to disable the limit
  // clipboardHistoryMaxTextLength: 100000,

  // ===========================================================================
  // Process Limits
  // ===========================================================================
  // Control resource usage for running scripts.
  // Leave undefined for no limits.

  // processLimits: {
  //   // Maximum memory usage in MB (scripts exceeding this may be terminated)
  //   maxMemoryMb: 512,
  //
  //   // Maximum runtime in seconds (scripts running longer will be terminated)
  //   maxRuntimeSeconds: 300,  // 5 minutes
  //
  //   // How often to check script health (in milliseconds)
  //   healthCheckIntervalMs: 5000,  // 5 seconds
  // },

  // ===========================================================================
  // Advanced Settings
  // ===========================================================================
  // These settings are rarely needed but available for special cases.

  // Custom path to the bun executable (auto-detected by default)
  // bun_path: "/opt/homebrew/bin/bun",
} satisfies Config;
