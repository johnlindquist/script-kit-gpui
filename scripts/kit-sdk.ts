import * as readline from 'node:readline';
import * as nodePath from 'node:path';
import * as os from 'node:os';
import * as fs from 'node:fs/promises';
import { constants as fsConstants } from 'node:fs';
import { spawn as spawnChildProcess } from 'node:child_process';
import { pathToFileURL } from 'node:url';

// =============================================================================
// SDK Benchmarking - for hotkey → chat latency analysis
// =============================================================================
const SDK_BENCH_START = performance.now();
const bench = (step: string) => {
  const elapsed = Math.round(performance.now() - SDK_BENCH_START);
  console.error(`[BENCH] [+${String(elapsed).padStart(4)}ms] SDK: ${step}`);
};
bench('imports_complete');

// =============================================================================
// SDK Version - Used to verify correct version is loaded
// =============================================================================
export const SDK_VERSION = '0.2.0';

// =============================================================================
// SDK Status & Architecture
// =============================================================================
/**
 * Script Kit GPUI SDK - Message-Passing Architecture
 *
 * This SDK communicates with the Rust GPUI application via JSON messages over
 * stdin/stdout. The app handles all UI rendering, system integration, and
 * complex operations. The SDK is a thin message-passing layer.
 *
 * IMPLEMENTATION STATUS:
 *
 * ✅ FULLY IMPLEMENTED (~45 methods):
 *    - Core prompts: arg, div, editor, term, form, fields, select, confirm,
 *      path, drop, template, env, chat
 *    - Window control: show, hide, captureScreenshot, getLayoutInfo, hud
 *    - Clipboard: read/write text/images, clipboard history with all operations
 *    - Window management: getWindows, focusWindow, tileWindow, getDisplays (macOS)
 *    - AI integration: aiIsOpen, aiStartChat, aiSendMessage, aiListChats, etc.
 *    - File/menu: fileSearch, getMenuBar, executeMenuAction
 *    - Pure utilities: home, skPath, uuid, compile, memoryMap
 *
 * ⚠️  NOT YET IMPLEMENTED (~18 methods):
 *    These functions send messages but the GPUI app doesn't handle them yet.
 *    They log warnings and may auto-submit in test mode.
 *
 *    - Notifications: beep(), say(), notify(), setStatus()
 *    - Automation: keyboard.type(), keyboard.tap(), mouse.move(), mouse.click()
 *    - UI updates: setPanel(), setPreview(), setPrompt()
 *    - Compact prompts: mini(), micro()
 *    - Advanced: hotkey(), widget(), menu()
 *
 * ❌ NOT FEASIBLE (removed):
 *    These don't fit the message-passing architecture and throw errors:
 *
 *    - webcam(), mic() - Media streaming requires real-time bidirectional
 *      communication beyond the JSONL protocol
 *    - eyeDropper() - Could work with screen capture but not yet implemented
 *
 * EXTENSION SUPPORT:
 *    - TypeScript scripts (.ts, .js): Full SDK access with stdin/stdout
 *    - TypeScript scriptlets (markdown): SDK loaded but NO interactive prompts
 *      (no stdin pipe - arg(), div(), chat() will hang)
 *    - Non-TypeScript scriptlets (bash, python): NO SDK access
 *
 * See .omegathink/20260120-142315-sdk-audit-methods-payload-scripts-extensions.md
 * for complete audit findings.
 */

// =============================================================================
// Types
// =============================================================================

export interface Choice {
  name: string;
  value: string;
  description?: string;
}

export interface FieldDef {
  name: string;
  label: string;
  type?: 'text' | 'password' | 'email' | 'number' | 'date' | 'time' | 'url' | 'tel' | 'color';
  placeholder?: string;
  value?: string;
}

export interface PathOptions {
  startPath?: string;
  hint?: string;
}

export interface HotkeyInfo {
  key: string;
  command: boolean;
  shift: boolean;
  option: boolean;
  control: boolean;
  shortcut: string;
  keyCode: string;
}

export interface FileInfo {
  path: string;
  name: string;
  size: number;
}

// =============================================================================
// Chat Types (TIER 4A) - AI SDK Compatible
// =============================================================================

/** AI SDK compatible message role */
export type ChatMessageRole = 'system' | 'user' | 'assistant' | 'tool';

/** AI SDK CoreMessage - pass directly to generateText({ messages }) */
export interface CoreMessage {
  role: ChatMessageRole;
  content: string;
}

/** Message displayed in the chat UI - AI SDK compatible with Script Kit extensions */
export interface ChatMessage {
  /** Unique message identifier (auto-generated if not provided) */
  id?: string;

  // === AI SDK Compatible Fields ===
  /** Message role (AI SDK format) - takes precedence over position if set */
  role?: ChatMessageRole;
  /** Message content (AI SDK format) - alias for text */
  content?: string;

  // === Script Kit Fields (backwards compatible) ===
  /** Message text content (supports markdown) - use content for AI SDK compat */
  text?: string;
  /** Position: 'left' (assistant/other) or 'right' (user) - derived from role if not set */
  position?: 'left' | 'right';

  // === Metadata ===
  /** Optional sender name */
  name?: string;
  /** Model that generated this message (assistant only) */
  model?: string;
  /** Whether this message is currently streaming */
  streaming?: boolean;
  /** Error message if generation failed */
  error?: string;
  /** Creation timestamp (ISO 8601) */
  createdAt?: string;
}

/** Result returned from chat() */
export interface ChatResult {
  /** AI SDK compatible messages - pass directly to generateText({ messages }) */
  messages: CoreMessage[];
  /** UI messages with metadata */
  uiMessages: ChatMessage[];
  /** Convenience: last user message */
  lastUserMessage: string;
  /** Convenience: last assistant message */
  lastAssistantMessage: string;
  /** Model used (if any) */
  model?: string;
  /** How the chat ended: 'escape' or 'continue' */
  action: 'escape' | 'continue';
  /** Conversation ID for database persistence */
  conversationId?: string;
}

/** Configuration options for the chat() prompt */
export interface ChatOptions {
  // === Messages (AI SDK compatible) ===
  /** Initial messages to display (supports both ChatMessage and CoreMessage formats) */
  messages?: (ChatMessage | CoreMessage)[];
  /** System prompt shorthand - convenience for adding a system message */
  system?: string;

  // === Model Configuration ===
  /** Default model to use */
  model?: string;
  /** Available models in actions menu */
  models?: string[];

  // === UI Configuration ===
  /** Placeholder text for the input field */
  placeholder?: string;
  /** Hint text (shown in header) */
  hint?: string;
  /** Footer text */
  footer?: string;
  /** Actions for the actions panel (Cmd+K) */
  actions?: Action[];

  // === Behavior ===
  /** Save conversation to database (default: true) */
  saveHistory?: boolean;

  // === Callbacks ===
  /** Called when chat opens (before user interaction) */
  onInit?: () => Promise<void>;
  /** Called when user submits a message */
  onMessage?: (text: string) => Promise<void>;
  /** Called when a chunk is received during streaming */
  onChunk?: (chunk: string) => void;
  /** Called when a message is finished */
  onFinish?: (message: ChatMessage) => void;
  /** Called when an error occurs */
  onError?: (error: Error) => void;
}

/** Controller for interacting with an active chat session */
export interface ChatController {
  /** Add a message to the chat */
  addMessage(msg: ChatMessage | CoreMessage): void;
  /** Start streaming a message (returns message ID for subsequent chunks) */
  startStream(position?: 'left' | 'right'): string;
  /** Append text to a streaming message */
  appendChunk(messageId: string, chunk: string): void;
  /** Complete a streaming message */
  completeStream(messageId: string): void;
  /** Clear all messages */
  clear(): void;
}

// =============================================================================
// Widget/Term/Media Types (TIER 4B)
// =============================================================================

export interface WidgetOptions {
  transparent?: boolean;
  draggable?: boolean;
  hasShadow?: boolean;
  alwaysOnTop?: boolean;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
}

export interface WidgetEvent {
  targetId: string;
  type: string;
  dataset: Record<string, string>;
}

export interface WidgetInputEvent {
  targetId: string;
  value: string;
  dataset: Record<string, string>;
}

export interface WidgetController {
  setState(state: Record<string, unknown>): void;
  onClick(handler: (event: WidgetEvent) => void): void;
  onInput(handler: (event: WidgetInputEvent) => void): void;
  onClose(handler: () => void): void;
  onMoved(handler: (pos: { x: number; y: number }) => void): void;
  onResized(handler: (size: { width: number; height: number }) => void): void;
  close(): void;
}

export interface ColorInfo {
  sRGBHex: string;
  rgb: string;
  rgba: string;
  hsl: string;
  hsla: string;
  cmyk: string;
}

export interface FindOptions {
  onlyin?: string;
}

/**
 * Configuration for form() prompt
 */
export interface FormConfig {
  /** HTML content for the form */
  html: string;
  /** Optional actions for the form */
  actions?: Action[];
}

/**
 * Configuration for template() prompt
 */
export interface TemplateOptions {
  /** Language for syntax highlighting */
  language?: string;
}

export interface WindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

// =============================================================================
// Div Prompt Options
// =============================================================================

/**
 * Configuration for div() prompt (matches original Script Kit API)
 */
export interface DivConfig {
  /** HTML content to display */
  html?: string;
  /** Placeholder text (shown in header) */
  placeholder?: string;
  /** Hint text */
  hint?: string;
  /** Footer text */
  footer?: string;
  /** 
   * Tailwind classes for the container.
   * Use "bg-transparent" for transparent background,
   * or any Tailwind bg-* classes for custom backgrounds.
   */
  containerClasses?: string;
  /**
   * Container background color (alternative to containerClasses).
   * Can be:
   * - "transparent" - fully transparent background
   * - "#RGB" or "#RRGGBB" - hex color (e.g., "#f00", "#ff0000")
   * - "#RRGGBBAA" - hex color with alpha (e.g., "#ff000080" for 50% opacity red)
   * - Tailwind color name (e.g., "blue-500", "gray-900")
   */
  containerBg?: string;
  /**
   * Container padding in pixels, or "none" to disable padding.
   * Default is theme-based padding (~16px).
   */
  containerPadding?: number | "none";
  /**
   * Container opacity (0-100).
   * Applied to the entire container including the background.
   * Default is 100 (fully opaque).
   */
  opacity?: number;
}

// =============================================================================
// Clipboard History Types
// =============================================================================

export interface ClipboardHistoryEntry {
  entryId: string;
  content: string;
  contentType: 'text' | 'image';
  timestamp: string;
  pinned: boolean;
}

// =============================================================================
// Window Management Types (System Windows)
// =============================================================================

export interface SystemWindowInfo {
  windowId: number;
  title: string;
  appName: string;
  bounds?: TargetWindowBounds;
  isMinimized?: boolean;
  isActive?: boolean;
}

export interface TargetWindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type TilePosition =
  // Half positions
  | 'left'
  | 'right'
  | 'top'
  | 'bottom'
  // Quadrant positions
  | 'top-left'
  | 'top-right'
  | 'bottom-left'
  | 'bottom-right'
  // Horizontal thirds
  | 'left-third'
  | 'center-third'
  | 'right-third'
  // Vertical thirds
  | 'top-third'
  | 'middle-third'
  | 'bottom-third'
  // Horizontal two-thirds
  | 'first-two-thirds'
  | 'last-two-thirds'
  // Vertical two-thirds
  | 'top-two-thirds'
  | 'bottom-two-thirds'
  // Centered positions
  | 'center'
  | 'almost-maximize'
  // Full screen
  | 'maximize';

/**
 * Information about a display/monitor
 */
export interface DisplayInfo {
  /** Display ID */
  displayId: number;
  /** Display name (e.g., "Built-in Retina Display") */
  name: string;
  /** Whether this is the primary display */
  isPrimary: boolean;
  /** Full display bounds (total resolution) */
  bounds: TargetWindowBounds;
  /** Visible bounds (excluding menu bar and dock) */
  visibleBounds: TargetWindowBounds;
  /** Scale factor (e.g., 2.0 for Retina) */
  scaleFactor?: number;
}

// =============================================================================
// File Search Types
// =============================================================================

export interface FileSearchResult {
  path: string;
  name: string;
  isDirectory: boolean;
  size?: number;
  modifiedAt?: string;
}

// =============================================================================
// Menu Bar Types
// =============================================================================

/**
 * A menu bar item with its children and metadata.
 * Used for reading application menu bars and executing menu actions.
 */
export interface MenuBarItem {
  /** The display title of the menu item */
  title: string;
  /** Whether the menu item is enabled (clickable) */
  enabled: boolean;
  /** Keyboard shortcut string if any (e.g., "⌘S") */
  shortcut?: string;
  /** Child menu items (for submenus) */
  children: MenuBarItem[];
  /** Path of menu titles to reach this item (e.g., ["File", "New", "Window"]) */
  menuPath: string[];
}

// =============================================================================
// Debug Grid Types
// =============================================================================

/** Custom color scheme for grid overlay */
export interface GridColorScheme {
  gridLines?: string;
  promptBounds?: string;
  inputBounds?: string;
  buttonBounds?: string;
  listBounds?: string;
  paddingFill?: string;
  marginFill?: string;
  alignmentGuide?: string;
}

/** Options for the debug grid overlay */
export interface GridOptions {
  /** Grid line spacing in pixels (8 or 16, default: 8) */
  gridSize?: 8 | 16;
  /** Show component bounding boxes with labels */
  showBounds?: boolean;
  /** Show CSS box model (padding/margin) visualization */
  showBoxModel?: boolean;
  /** Show alignment guides between components */
  showAlignmentGuides?: boolean;
  /** Show component dimensions in labels (e.g., "Header (500x45)") */
  showDimensions?: boolean;
  /** Which components to show bounds for */
  depth?: 'prompts' | 'all' | string[];
  /** Custom color scheme */
  colorScheme?: GridColorScheme;
}

// =============================================================================
// Screenshot Types
// =============================================================================

export interface ScreenshotData {
  /** Base64-encoded PNG data */
  data: string;
  /** Width in pixels */
  width: number;
  /** Height in pixels */
  height: number;
}

export interface ScreenshotOptions {
  /**
   * If true, capture at full retina resolution (2x).
   * If false (default), scale down to 1x resolution for smaller file sizes.
   * @default false
   */
  hiDpi?: boolean;
}

// =============================================================================
// Layout Info Types (AI Agent Debugging)
// =============================================================================

/** Box model sides (top, right, bottom, left) in pixels */
export interface BoxModelSides {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

/** Computed box model for a component */
export interface ComputedBoxModel {
  /** Padding values (inner spacing) */
  padding?: BoxModelSides;
  /** Margin values (outer spacing) */
  margin?: BoxModelSides;
  /** Gap between flex/grid children */
  gap?: number;
}

/** Computed flex properties for a component */
export interface ComputedFlexStyle {
  /** Flex direction: "row" or "column" */
  direction?: string;
  /** Flex grow value */
  grow?: number;
  /** Flex shrink value */
  shrink?: number;
  /** Align items: "start", "center", "end", "stretch" */
  alignItems?: string;
  /** Justify content: "start", "center", "end", "space-between", etc. */
  justifyContent?: string;
}

/** Bounding rectangle in pixels */
export interface LayoutBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Component type for categorization */
export type LayoutComponentType =
  | 'prompt'
  | 'input'
  | 'button'
  | 'list'
  | 'listItem'
  | 'header'
  | 'container'
  | 'panel'
  | 'other';

/**
 * Information about a single component in the layout tree.
 * 
 * This provides everything an AI agent needs to understand "why"
 * a component is positioned/sized the way it is.
 */
export interface LayoutComponentInfo {
  /** Component name/identifier */
  name: string;
  /** Component type for categorization */
  type: LayoutComponentType;
  /** Bounding rectangle (absolute position and size) */
  bounds: LayoutBounds;
  /** Computed box model (padding, margin, gap) */
  boxModel?: ComputedBoxModel;
  /** Computed flex properties */
  flex?: ComputedFlexStyle;
  /** Nesting depth (0 = root, 1 = child of root, etc.) */
  depth: number;
  /** Parent component name (if any) */
  parent?: string;
  /** Child component names */
  children?: string[];
  /**
   * Human-readable explanation of why this component has its current size/position.
   * Example: "Height is 45px = padding(8) + content(28) + padding(8) + divider(1)"
   */
  explanation?: string;
}

/**
 * Full layout information for the current UI state.
 * 
 * Returned by `getLayoutInfo()` SDK function.
 * Contains the component tree and window-level information.
 */
export interface LayoutInfo {
  /** Window width in pixels */
  windowWidth: number;
  /** Window height in pixels */
  windowHeight: number;
  /** Current prompt type (e.g., "arg", "div", "editor", "mainMenu") */
  promptType: string;
  /** All components in the layout tree */
  components: LayoutComponentInfo[];
  /** Timestamp when layout was captured (ISO 8601) */
  timestamp: string;
}

// =============================================================================
// Config Types (for ~/.scriptkit/kit/config.ts)
// =============================================================================

/**
 * Modifier keys for keyboard shortcuts
 */
export type KeyModifier = "meta" | "ctrl" | "alt" | "shift";

/**
 * Supported key codes for global hotkeys
 * Based on the W3C UI Events KeyboardEvent code values
 */
export type KeyCode =
  // Letter keys
  | "KeyA" | "KeyB" | "KeyC" | "KeyD" | "KeyE" | "KeyF" | "KeyG"
  | "KeyH" | "KeyI" | "KeyJ" | "KeyK" | "KeyL" | "KeyM" | "KeyN"
  | "KeyO" | "KeyP" | "KeyQ" | "KeyR" | "KeyS" | "KeyT" | "KeyU"
  | "KeyV" | "KeyW" | "KeyX" | "KeyY" | "KeyZ"
  // Number keys (top row)
  | "Digit0" | "Digit1" | "Digit2" | "Digit3" | "Digit4"
  | "Digit5" | "Digit6" | "Digit7" | "Digit8" | "Digit9"
  // Navigation keys
  | "Space" | "Enter" | "Tab" | "Escape" | "Backspace" | "Delete"
  | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
  | "Home" | "End" | "PageUp" | "PageDown" | "Insert"
  // Punctuation keys
  | "Semicolon" | "Quote" | "Comma" | "Period" | "Slash"
  | "Backslash" | "BracketLeft" | "BracketRight"
  | "Minus" | "Equal" | "Backquote"
  // Function keys
  | "F1" | "F2" | "F3" | "F4" | "F5" | "F6"
  | "F7" | "F8" | "F9" | "F10" | "F11" | "F12"
  | "F13" | "F14" | "F15" | "F16" | "F17" | "F18"
  | "F19" | "F20" | "F21" | "F22" | "F23" | "F24"
  // Lock/system keys
  | "CapsLock" | "NumLock" | "ScrollLock" | "PrintScreen" | "Pause";

/**
 * Hotkey configuration for global keyboard shortcuts.
 * Defines the modifier keys and main key for activating Script Kit.
 * 
 * @example Cmd+; (default on Mac)
 * ```typescript
 * hotkey: {
 *   modifiers: ["meta"],
 *   key: "Semicolon"
 * }
 * ```
 * 
 * @example Ctrl+Shift+Space
 * ```typescript
 * hotkey: {
 *   modifiers: ["ctrl", "shift"],
 *   key: "Space"
 * }
 * ```
 */
export interface HotkeyConfig {
  /**
   * Modifier keys that must be held while pressing the main key.
   * - "meta" = Cmd on Mac, Win on Windows
   * - "ctrl" = Control key
   * - "alt" = Option on Mac, Alt on Windows
   * - "shift" = Shift key
   * 
   * @default ["meta"] (Cmd on Mac)
   * @example ["meta"] // Just Cmd
   * @example ["meta", "shift"] // Cmd+Shift
   * @example ["ctrl", "alt"] // Ctrl+Alt
   */
  modifiers: KeyModifier[];
  
  /**
   * The main key code (W3C UI Events KeyboardEvent code format).
   * Common values:
   * - Letter keys: "KeyA" through "KeyZ"
   * - Number keys: "Digit0" through "Digit9"
   * - Special keys: "Space", "Enter", "Semicolon", "Comma", "Period", "Slash"
   * - Function keys: "F1" through "F12"
   * 
   * @default "Semicolon" (the ; key)
   * @example "Semicolon" // The ; key
   * @example "KeyK" // The K key
   * @example "Digit0" // The 0 key
   * @example "Space" // The spacebar
   */
  key: KeyCode;
}

// =============================================================================
// COMMAND ID TYPES
// =============================================================================

/** Built-in Script Kit feature command ID: `builtin/{feature-name}` */
export type BuiltinCommandId = `builtin/${string}`;

/** Application command ID (macOS bundle identifier): `app/{bundle-id}` */
export type AppCommandId = `app/${string}`;

/** User script command ID: `script/{script-name}` */
export type ScriptCommandId = `script/${string}`;

/** Scriptlet command ID: `scriptlet/{uuid-or-name}` */
export type ScriptletCommandId = `scriptlet/${string}`;

/** Union of all valid command ID formats. */
export type CommandId =
  | BuiltinCommandId
  | AppCommandId
  | ScriptCommandId
  | ScriptletCommandId;

/** Map of command IDs to their configurations. */
export type CommandsConfig = Partial<Record<CommandId, CommandConfig>>;

/**
 * Per-command configuration for shortcuts and visibility.
 * 
 * Commands are identified by category-prefixed IDs:
 * - `builtin/` - Built-in features (clipboard-history, app-launcher, etc.)
 * - `app/` - macOS apps by bundle ID (com.apple.Safari, etc.)
 * - `script/` - User scripts by filename without .ts (my-script, etc.)
 * - `scriptlet/` - Inline scriptlets by UUID or name
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
 *     hidden: true // Hide from menu, still accessible via shortcut/deeplink
 *   }
 * }
 * ```
 */
export interface CommandConfig {
  /**
   * Optional keyboard shortcut to invoke this command directly.
   * When set, the command can be triggered globally without opening Script Kit.
   * 
   * @example { modifiers: ["meta", "shift"], key: "KeyV" } // Cmd+Shift+V
   * @example { modifiers: ["meta", "ctrl"], key: "KeyI" } // Cmd+Ctrl+I
   */
  shortcut?: HotkeyConfig;
  
  /**
   * Whether this command should be hidden from the main menu.
   * Hidden commands are still accessible via keyboard shortcut or deeplink.
   * 
   * @default false
   * @example true // Hide from main menu
   */
  hidden?: boolean;
  
  /**
   * Whether this command requires confirmation before execution.
   * Use this for potentially destructive operations.
   * 
   * @default false
   * @example true // Require confirmation dialog
   */
  confirmationRequired?: boolean;
}

/**
 * Content padding configuration for prompts (terminal, editor, etc.)
 * All values are in pixels.
 * 
 * @example
 * ```typescript
 * padding: {
 *   top: 16,
 *   left: 20,
 *   right: 20
 * }
 * ```
 */
export interface ContentPadding {
  /**
   * Top padding in pixels
   * @default 8
   * @example 16
   */
  top?: number;
  
  /**
   * Left padding in pixels
   * @default 12
   * @example 20
   */
  left?: number;
  
  /**
   * Right padding in pixels
   * @default 12
   * @example 20
   */
  right?: number;
}

/**
 * Configuration for built-in features (clipboard history, app launcher, window switcher).
 * These are optional features that can be enabled or disabled.
 * 
 * @example
 * ```typescript
 * builtIns: {
 *   clipboardHistory: true,
 *   appLauncher: true,
 *   windowSwitcher: false
 * }
 * ```
 */
export interface BuiltInConfig {
  /**
   * Enable the clipboard history built-in feature.
   * When enabled, Script Kit tracks clipboard changes and provides a searchable history.
   * @default true
   * @example false // Disable clipboard history
   */
  clipboardHistory?: boolean;
  
  /**
   * Enable the app launcher built-in feature.
   * When enabled, Script Kit can search and launch applications.
   * @default true
   * @example false // Disable app launcher
   */
  appLauncher?: boolean;
  
  /**
   * Enable the window switcher built-in feature.
   * When enabled, Script Kit provides a window switcher for managing open windows.
   * @default true
   * @example false // Disable window switcher
   */
  windowSwitcher?: boolean;
}

/**
 * Configuration for process resource limits and health monitoring.
 * Use these settings to control script execution resources and monitoring.
 * 
 * @example
 * ```typescript
 * processLimits: {
 *   maxMemoryMb: 512,
 *   maxRuntimeSeconds: 300,
 *   healthCheckIntervalMs: 10000
 * }
 * ```
 */
export interface ProcessLimits {
  /**
   * Maximum memory usage in megabytes (MB).
   * Scripts exceeding this limit may be terminated.
   * Set to undefined/null for no limit.
   * 
   * @default undefined (no limit)
   * @example 512 // Limit scripts to 512 MB
   * @example 1024 // Limit scripts to 1 GB
   */
  maxMemoryMb?: number;
  
  /**
   * Maximum runtime in seconds.
   * Scripts running longer than this will be terminated.
   * Set to undefined/null for no limit.
   * 
   * @default undefined (no limit)
   * @example 60 // 1 minute timeout
   * @example 300 // 5 minute timeout
   * @example 3600 // 1 hour timeout
   */
  maxRuntimeSeconds?: number;
  
  /**
   * Health check interval in milliseconds.
   * How often Script Kit checks on running scripts for resource usage.
   * Lower values = more responsive limits but more overhead.
   * 
   * @default 5000 (5 seconds)
   * @example 1000 // Check every 1 second (more responsive)
   * @example 10000 // Check every 10 seconds (less overhead)
   */
  healthCheckIntervalMs?: number;
}

/**
 * Configuration for the Claude Code CLI provider.
 *
 * This allows Script Kit's AI Chat to use the local `claude` CLI as an AI provider,
 * speaking JSONL over stdin/stdout for streaming responses with tool support.
 *
 * @example Basic enabled configuration
 * ```typescript
 * claudeCode: {
 *   enabled: true
 * }
 * ```
 *
 * @example Full configuration with tool restrictions
 * ```typescript
 * claudeCode: {
 *   enabled: true,
 *   path: "/opt/homebrew/bin/claude",
 *   permissionMode: "plan",
 *   allowedTools: "Read,Edit,Bash(git:*)",
 *   addDirs: ["/home/user/projects", "/tmp/scratch"]
 * }
 * ```
 */
export interface ClaudeCodeConfig {
  /**
   * Enable the Claude Code CLI provider.
   * When enabled, "Claude Code" models will appear in the AI chat model picker.
   *
   * @default false (requires explicit opt-in)
   * @example true // Enable Claude Code as a provider
   */
  enabled?: boolean;

  /**
   * Custom path to the `claude` CLI binary.
   * If not specified, will look for `claude` in your PATH.
   *
   * @default undefined (uses "claude" from PATH)
   * @example "/opt/homebrew/bin/claude"
   * @example "/usr/local/bin/claude"
   */
  path?: string;

  /**
   * Permission mode for Claude Code tool execution.
   * - "plan": Safe default - Claude plans but asks before executing tools
   * - "dontAsk": Agent can execute tools without confirmation (for trusted sandboxes only!)
   *
   * @default "plan"
   * @example "plan" // Safe: Claude asks before running commands
   * @example "dontAsk" // Dangerous: Claude runs tools without asking
   */
  permissionMode?: ClaudeCodePermissionMode;

  /**
   * Comma-separated list of allowed tools.
   * Restricts which tools Claude Code can use.
   * Follows Claude Code CLI's tool permission syntax.
   *
   * @default undefined (uses Claude Code's default tool set)
   * @example "Read,Edit,Bash(git:*)" // Read, Edit, and git commands only
   * @example "Read,Edit" // Read and Edit only, no shell access
   * @example "Read,Edit,Bash,Write" // Common set for development
   */
  allowedTools?: string;

  /**
   * Additional directories to add to Claude Code's workspace.
   * Each path is passed as `--add-dir` to the CLI, giving Claude access to those directories.
   *
   * @default [] (empty array)
   * @example ["/home/user/projects"] // Single project directory
   * @example ["/home/user/projects", "/tmp/scratch"] // Multiple directories
   */
  addDirs?: string[];
}

export type McpTransport = 'stdio' | 'http';

export interface McpBaseServerConfig {
  /**
   * Optional human-readable label for the server.
   */
  name?: string;

  /**
   * Optional description shown in config documentation and tooling.
   */
  description?: string;

  /**
   * Whether Script Kit should use this server.
   *
   * @default true
   */
  enabled?: boolean;
}

/**
 * A local stdio-backed MCP server.
 */
export interface McpStdioServerConfig extends McpBaseServerConfig {
  transport: 'stdio';
  command: string;
  args?: string[];
  env?: Record<string, string>;
  cwd?: string;
}

/**
 * A remote HTTP-backed MCP server.
 */
export interface McpHttpServerConfig extends McpBaseServerConfig {
  transport: 'http';
  endpoint: string;
  headers?: Record<string, string>;
}

export type McpServerConfig = McpStdioServerConfig | McpHttpServerConfig;

/**
 * Script Kit-managed MCP server definitions shared by scripts and ACP Chat.
 */
export interface McpConfig {
  /**
   * Master switch for Script Kit-managed MCP integrations.
   *
   * @default true
   */
  enabled?: boolean;

  /**
   * Named MCP server definitions keyed by server id.
   *
   * @default {}
   */
  servers?: Record<string, McpServerConfig>;
}

export interface McpServerInfo {
  id: string;
  transport: McpTransport;
  enabled: boolean;
  name?: string;
  description?: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  cwd?: string;
  endpoint?: string;
  headers?: Record<string, string>;
}

export interface McpToolInfo {
  serverId: string;
  name: string;
  description?: string;
  inputSchema?: Record<string, unknown>;
}

export interface McpToolCallContent {
  type: string;
  text?: string;
  mimeType?: string;
  data?: unknown;
  [key: string]: unknown;
}

export interface McpToolCallResult {
  serverId: string;
  toolName: string;
  content: McpToolCallContent[];
  structuredContent?: unknown;
  isError?: boolean;
  raw: Record<string, unknown>;
}

export interface McpApi {
  listServers(): Promise<McpServerInfo[]>;
  getServer(id: string): Promise<McpServerInfo | null>;
  listTools(serverId?: string): Promise<McpToolInfo[]>;
  discover(query: string): Promise<McpToolInfo[]>;
  call(
    serverId: string,
    toolName: string,
    args?: Record<string, unknown>
  ): Promise<McpToolCallResult>;
}

/**
 * Suggested-commands configuration.
 * Controls the frecency-based "Suggested" section in the main menu.
 *
 * @example
 * ```typescript
 * suggested: {
 *   enabled: true,
 *   maxItems: 10,
 *   minScore: 0.1,
 *   halfLifeDays: 7,
 *   trackUsage: true,
 * }
 * ```
 */
export interface SuggestedConfig {
  /**
   * Enable the Suggested section.
   * @default true
   */
  enabled?: boolean;

  /**
   * Maximum number of suggested items shown.
   * @default 10
   */
  maxItems?: number;

  /**
   * Minimum frecency score to include an item.
   * @default 0.1
   */
  minScore?: number;

  /**
   * Half-life (in days) for the frecency decay curve.
   * @default 7
   */
  halfLifeDays?: number;

  /**
   * Track command usage for frecency scoring.
   * @default true
   */
  trackUsage?: boolean;

  /**
   * Command IDs to exclude from suggestions.
   * @default ["builtin/quit-script-kit"]
   */
  excludedCommands?: CommandId[];
}

/**
 * File-watcher debounce and back-off configuration.
 *
 * @example
 * ```typescript
 * watcher: {
 *   debounceMs: 500,
 *   stormThreshold: 200,
 *   initialBackoffMs: 100,
 *   maxBackoffMs: 30000,
 *   maxNotifyErrors: 10,
 * }
 * ```
 */
export interface WatcherConfig {
  /**
   * Debounce interval in milliseconds.
   * @default 500
   */
  debounceMs?: number;

  /**
   * Number of events within one debounce window considered a storm.
   * @default 200
   */
  stormThreshold?: number;

  /**
   * Initial back-off delay in milliseconds after a storm.
   * @default 100
   */
  initialBackoffMs?: number;

  /**
   * Maximum back-off delay in milliseconds.
   * @default 30000
   */
  maxBackoffMs?: number;

  /**
   * Maximum consecutive notify errors before the watcher gives up.
   * @default 10
   */
  maxNotifyErrors?: number;
}

/**
 * Window layout sizing configuration.
 *
 * @example
 * ```typescript
 * layout: {
 *   standardHeight: 500,
 *   maxHeight: 700,
 * }
 * ```
 */
export interface LayoutConfig {
  /**
   * Standard window height in pixels.
   * @default 500
   */
  standardHeight?: number;

  /**
   * Maximum window height in pixels.
   * @default 700
   */
  maxHeight?: number;
}

export type ClaudeCodePermissionMode = "plan" | "dontAsk";

/**
 * Theme preset selection stored in `~/.scriptkit/kit/config.ts`.
 */
export interface ThemeSelectionPreferences {
  /**
   * Optional preset identifier, for example `"catppuccin-mocha"`.
   */
  presetId?: string;
}

/**
 * Dictation runtime preferences stored in `~/.scriptkit/kit/config.ts`.
 */
export interface DictationPreferences {
  /**
   * Persisted microphone device ID.
   * Omit this field to use the macOS default microphone.
   *
   * @example "usb-mic"
   */
  selectedDeviceId?: string;
}

/**
 * ACP Chat runtime preferences stored in `~/.scriptkit/kit/config.ts`.
 */
export interface AiPreferences {
  /**
   * Last-selected model ID.
   *
   * @example "gpt-5.4"
   */
  selectedModelId?: string;

  /**
   * Last-selected ACP agent ID.
   *
   * @example "codex-acp"
   */
  selectedAcpAgentId?: string;
}

export type SnapMode = "off" | "simple" | "expanded" | "precision";

/**
 * Window-management preferences stored in `~/.scriptkit/kit/config.ts`.
 */
export interface WindowManagementPreferences {
  /**
   * Drag-snap density/mode for desktop tiling.
   */
  snapMode?: SnapMode;
}

/**
 * Script Kit configuration schema.
 *
 * This configuration is loaded from `~/.scriptkit/kit/config.ts` and controls
 * Script Kit's behavior, appearance, and built-in features.
 * 
 * @example Minimal configuration (only hotkey required)
 * ```typescript
 * import type { Config } from "@scriptkit/sdk";
 * 
 * export default {
 *   hotkey: {
 *     modifiers: ["meta"],
 *     key: "Semicolon"
 *   }
 * } satisfies Config;
 * ```
 * 
 * @example Full configuration with all options
 * ```typescript
 * import type { Config } from "@scriptkit/sdk";
 * 
 * export default {
 *   hotkey: {
 *     modifiers: ["meta"],
 *     key: "Semicolon"
 *   },
 *   editor: "code",
 *   padding: { top: 8, left: 12, right: 12 },
 *   editorFontSize: 16,
 *   terminalFontSize: 14,
 *   uiScale: 1.0,
 *   builtIns: {
 *     clipboardHistory: true,
 *     appLauncher: true,
 *     windowSwitcher: true
 *   },
 *   clipboardHistoryMaxTextLength: 100000,
 *   processLimits: {
 *     maxMemoryMb: 512,
 *     maxRuntimeSeconds: 300,
 *     healthCheckIntervalMs: 5000
 *   }
 * } satisfies Config;
 * ```
 */
export interface Config {
  /**
   * Main keyboard shortcut to open Script Kit.
   * This is the global hotkey that activates Script Kit from any application.
   * 
   * @required This field is required
   * @example { modifiers: ["meta"], key: "Semicolon" } // Cmd+; on Mac
   * @example { modifiers: ["ctrl", "shift"], key: "Space" } // Ctrl+Shift+Space
   */
  hotkey: HotkeyConfig;
  
  /**
   * Custom path to the bun executable.
   * Use this if bun is not in your PATH or you want to use a specific version.
   * 
   * @default undefined (auto-detected from PATH)
   * @example "/opt/homebrew/bin/bun"
   * @example "/usr/local/bin/bun"
   */
  bun_path?: string;
  
  /**
   * Preferred editor command for "Open in Editor" actions.
   * Falls back to $EDITOR environment variable, then to "code" (VS Code).
   * 
   * @default undefined (uses $EDITOR or "code")
   * @example "code" // VS Code
   * @example "vim" // Vim
   * @example "nvim" // Neovim
   * @example "subl" // Sublime Text
   * @example "zed" // Zed
   */
  editor?: string;
  
  /**
   * Content padding for prompt areas (terminal, editor, etc.).
   * Controls the spacing around content in various prompts.
   * 
   * @default { top: 8, left: 12, right: 12 }
   * @example { top: 16, left: 20, right: 20 } // More spacious
   * @example { top: 4, left: 8, right: 8 } // More compact
   */
  padding?: ContentPadding;
  
  /**
   * Font size for the editor prompt in pixels.
   * Affects the Monaco-style code editor.
   *
   * @default 16
   * @example 12 // Smaller for more code visibility
   * @example 16 // Default
   * @example 18 // Extra large
   */
  editorFontSize?: number;
  
  /**
   * Font size for the terminal prompt in pixels.
   * Affects the integrated terminal.
   * 
   * @default 14
   * @example 12 // Smaller terminal font
   * @example 16 // Larger terminal font
   */
  terminalFontSize?: number;
  
  /**
   * UI scale factor for the entire interface.
   * 1.0 = 100% (normal), 1.5 = 150% (larger), 0.8 = 80% (smaller).
   * 
   * @default 1.0
   * @example 1.25 // 125% scale
   * @example 1.5 // 150% scale for HiDPI or accessibility
   * @example 0.9 // Slightly smaller
   */
  uiScale?: number;
  
  /**
   * Configuration for built-in features.
   * Enable or disable clipboard history, app launcher, and window switcher.
   * 
   * @default { clipboardHistory: true, appLauncher: true, windowSwitcher: true }
   * @example { clipboardHistory: false } // Disable only clipboard history
   * @example { appLauncher: false, windowSwitcher: false } // Disable launcher and switcher
   */
  builtIns?: BuiltInConfig;
  
  /**
   * Maximum text length (bytes) to store for clipboard history entries.
   * Set to 0 to disable the limit.
   * 
   * @default 100000
   * @example 200000 // Allow larger text entries
   * @example 0 // No limit
   */
  clipboardHistoryMaxTextLength?: number;
  
  /**
   * Process resource limits and health monitoring configuration.
   * Control memory usage, runtime limits, and monitoring frequency for scripts.
   * 
   * @default { healthCheckIntervalMs: 5000 } (no memory or runtime limits)
   * @example { maxMemoryMb: 512 } // Limit scripts to 512 MB
   * @example { maxRuntimeSeconds: 60 } // 1 minute timeout
   * @example { maxMemoryMb: 256, maxRuntimeSeconds: 30, healthCheckIntervalMs: 1000 }
   */
  processLimits?: ProcessLimits;

  /**
   * Hotkey for opening the Notes window.
   * No default is registered; set this explicitly if you want one.
   *
   * @default undefined
   */
  notesHotkey?: HotkeyConfig;

  /**
   * Hotkey for opening ACP Chat.
   * Falls back to Cmd+Shift+Space when enabled and unset.
   *
   * @default undefined
   */
  aiHotkey?: HotkeyConfig;

  /**
   * Whether the AI hotkey should be registered.
   *
   * @default true
   */
  aiHotkeyEnabled?: boolean;

  /**
   * Hotkey for toggling log capture.
   * Falls back to Cmd+Shift+L when enabled and unset.
   *
   * @default undefined
   */
  logsHotkey?: HotkeyConfig;

  /**
   * Whether the logs hotkey should be registered.
   *
   * @default true
   */
  logsHotkeyEnabled?: boolean;

  /**
   * Hotkey for toggling dictation.
   * No default shortcut is registered; set this explicitly if you want one.
   *
   * @default undefined
   */
  dictationHotkey?: HotkeyConfig;

  /**
   * Whether dictation hotkey registration is enabled.
   * This does not create a shortcut by itself; set `dictationHotkey` too.
   *
   * @default true
   */
  dictationHotkeyEnabled?: boolean;

  /**
   * Suggested-commands (frecency) configuration.
   *
   * @default { enabled: true, maxItems: 10, minScore: 0.1, halfLifeDays: 7, trackUsage: true, excludedCommands: ["builtin/quit-script-kit"] }
   */
  suggested?: SuggestedConfig;

  /**
   * File-watcher debounce and back-off configuration.
   *
   * @default { debounceMs: 500, stormThreshold: 200, initialBackoffMs: 100, maxBackoffMs: 30000, maxNotifyErrors: 10 }
   */
  watcher?: WatcherConfig;

  /**
   * Window layout sizing configuration.
   *
   * @default { standardHeight: 500, maxHeight: 700 }
   */
  layout?: LayoutConfig;

  /**
   * Theme preset selection.
   *
   * Theme colors still live in `~/.scriptkit/kit/theme.json`.
   *
   * @default undefined
   */
  theme?: ThemeSelectionPreferences;

  /**
   * Dictation runtime preferences, including the selected microphone device.
   *
   * @default undefined
   */
  dictation?: DictationPreferences;

  /**
   * ACP Chat runtime preferences, including the selected agent and model.
   *
   * @default undefined
   */
  ai?: AiPreferences;

  /**
   * Window-management preferences such as drag-snap mode.
   *
   * @default undefined
   */
  windowManagement?: WindowManagementPreferences;

  /**
   * Per-command configuration for shortcuts and visibility.
   * Override default shortcuts or hide commands from the main menu.
   * 
   * Commands are identified by category-prefixed IDs:
   * - `builtin/` - Built-in features (clipboard-history, app-launcher, etc.)
   * - `app/` - macOS apps by bundle ID (com.apple.Safari, etc.)
   * - `script/` - User scripts by filename without .ts (my-script, etc.)
   * - `scriptlet/` - Inline scriptlets by UUID or name
   * 
   * Each command also has a deeplink: `scriptkit://commands/{id}`
   * 
   * @default undefined (no overrides)
   * @example
   * ```typescript
   * commands: {
   *   "builtin/clipboard-history": {
   *     shortcut: { modifiers: ["meta", "shift"], key: "KeyV" }
   *   },
   *   "app/com.apple.Safari": {
   *     shortcut: { modifiers: ["meta", "shift"], key: "KeyS" }
   *   }
   * }
   * ```
   */
  commands?: Partial<Record<CommandId, CommandConfig>>;

  /**
   * Claude Code CLI provider configuration.
   * Enable and configure the local `claude` CLI as an AI provider in Script Kit's AI Chat.
   *
   * Requires the Claude Code CLI to be installed: https://claude.ai/code
   *
   * @default undefined (Claude Code provider disabled)
   * @example Basic usage
   * ```typescript
   * claudeCode: {
   *   enabled: true
   * }
   * ```
   * @example Full configuration
   * ```typescript
   * claudeCode: {
   *   enabled: true,
   *   path: "/opt/homebrew/bin/claude",
   *   permissionMode: "plan",
   *   allowedTools: "Read,Edit,Bash(git:*)",
   *   addDirs: ["/home/user/projects"]
   * }
   * ```
   */
  claudeCode?: ClaudeCodeConfig;

  /**
   * External MCP servers that scripts and ACP Chat can use.
   *
   * @default undefined
   * @example
   * ```typescript
   * mcp: {
   *   servers: {
   *     notion: {
   *       transport: "http",
   *       endpoint: "https://mcp.notion.com/mcp"
   *     },
   *     github: {
   *       transport: "stdio",
   *       command: "npx",
   *       args: ["-y", "@modelcontextprotocol/server-github"]
   *     }
   *   }
   * }
   * ```
   */
  mcp?: McpConfig;
}

// =============================================================================
// Script Metadata Types (AI-First Protocol)
// =============================================================================

/**
 * Supported field types for schema definitions.
 * These map to JSON Schema types for MCP tool generation.
 */
export type SchemaFieldType = 'string' | 'number' | 'boolean' | 'array' | 'object' | 'any';

// =============================================================================
// Schema Type Inference Utilities
// =============================================================================

/**
 * Maps a SchemaFieldType string to its TypeScript type.
 * Used internally for type inference from schema definitions.
 */
type SchemaTypeMap = {
  string: string;
  number: number;
  boolean: boolean;
  array: unknown[];
  object: Record<string, unknown>;
  any: unknown;
};

/**
 * Infers the TypeScript type from a SchemaFieldDef.
 * Handles required/optional, enums, arrays with typed items, and nested objects.
 * 
 * @example
 * ```typescript
 * // { type: 'string' } -> string
 * // { type: 'string', enum: ['a', 'b'] as const } -> 'a' | 'b'
 * // { type: 'array', items: 'number' } -> number[]
 * // { type: 'object', properties: { name: { type: 'string' } } } -> { name: string }
 * ```
 */
type InferFieldType<F extends SchemaFieldDef> = 
  // Handle enums: narrow to literal union
  F extends { enum: readonly (infer E)[] } 
    ? E
  // Handle arrays with typed items
  : F extends { type: 'array'; items: infer I extends SchemaFieldType }
    ? SchemaTypeMap[I][]
  // Handle nested objects
  : F extends { type: 'object'; properties: infer P extends Record<string, SchemaFieldDef> }
    ? { [K in keyof P]: InferFieldType<P[K]> }
  // Handle basic types
  : F extends { type: infer T extends SchemaFieldType }
    ? SchemaTypeMap[T]
  : unknown;

/**
 * Extracts keys of required fields from a schema record.
 */
type RequiredKeys<T extends Record<string, SchemaFieldDef>> = {
  [K in keyof T]: T[K] extends { required: true } ? K : never;
}[keyof T];

/**
 * Extracts keys of optional fields from a schema record.
 */
type OptionalKeys<T extends Record<string, SchemaFieldDef>> = {
  [K in keyof T]: T[K] extends { required: true } ? never : K;
}[keyof T];

/**
 * Infers a TypeScript interface from a schema input/output definition.
 * Required fields are non-optional, others are optional.
 * 
 * @example
 * ```typescript
 * type Input = InferSchema<{
 *   title: { type: 'string'; required: true };
 *   tags: { type: 'array'; items: 'string' };
 * }>;
 * // Result: { title: string; tags?: string[] }
 * ```
 */
export type InferSchema<T extends Record<string, SchemaFieldDef>> = 
  { [K in RequiredKeys<T>]: InferFieldType<T[K]> } &
  { [K in OptionalKeys<T>]?: InferFieldType<T[K]> };

/**
 * Infers the input type from a ScriptSchema.
 * Use this with `typeof schema` to get compile-time type safety.
 * 
 * @example
 * ```typescript
 * const schema = {
 *   input: {
 *     greeting: { type: 'string', required: true },
 *     count: { type: 'number' }
 *   }
 * } as const;
 * 
 * type MyInput = InferInput<typeof schema>;
 * // Result: { greeting: string; count?: number }
 * 
 * const data = await input<MyInput>();
 * console.log(data.greeting); // TypeScript knows this is string
 * ```
 */
export type InferInput<S extends { input?: Record<string, SchemaFieldDef> }> = 
  S extends { input: infer I extends Record<string, SchemaFieldDef> }
    ? InferSchema<I>
    : Record<string, unknown>;

/**
 * Infers the output type from a ScriptSchema.
 * Use this with `typeof schema` to get compile-time type safety.
 * 
 * @example
 * ```typescript
 * const schema = {
 *   output: {
 *     path: { type: 'string' },
 *     wordCount: { type: 'number' }
 *   }
 * } as const;
 * 
 * type MyOutput = InferOutput<typeof schema>;
 * // Result: { path?: string; wordCount?: number }
 * 
 * output({ path: '/notes/test.md' } satisfies Partial<MyOutput>);
 * ```
 */
export type InferOutput<S extends { output?: Record<string, SchemaFieldDef> }> = 
  S extends { output: infer O extends Record<string, SchemaFieldDef> }
    ? InferSchema<O>
    : Record<string, unknown>;

/**
 * Interface for the typed API returned by defineSchema.
 */
export interface TypedSchemaAPI<TInput, TOutput> {
  /** Get typed input matching the schema's input definition */
  input: () => Promise<TInput>;
  /** Send typed output matching the schema's output definition */
  output: (data: Partial<TOutput>) => void;
}

/**
 * Define a schema and get typed input/output functions.
 * This is the recommended way to get full type inference.
 * 
 * @example
 * ```typescript
 * const { input, output } = defineSchema({
 *   input: {
 *     greeting: { type: 'string', required: true },
 *     count: { type: 'number' }
 *   },
 *   output: {
 *     message: { type: 'string' }
 *   }
 * } as const)
 * 
 * // Types are fully inferred!
 * const { greeting, count } = await input()
 * //      ^ string   ^ number | undefined
 * 
 * output({ message: `Hello ${greeting}!` })
 * ```
 * 
 * @param schemaDefinition - The schema definition object (use `as const` for best inference)
 * @returns Object with typed `input()` and `output()` functions
 */
export function defineSchema<T extends ScriptSchema>(
  schemaDefinition: T
): TypedSchemaAPI<InferInput<T>, InferOutput<T>> & { schema: T } {
  // Assign to global for runtime parsing by the app
  (globalThis as any).schema = schemaDefinition;
  
  return {
    schema: schemaDefinition,
    input: (globalThis as any).input as () => Promise<InferInput<T>>,
    output: (globalThis as any).output as (data: Partial<InferOutput<T>>) => void,
  };
}

/**
 * Field definition for schema input/output.
 * Defines the type, validation rules, and documentation for a single field.
 * 
 * @example Simple required string field
 * ```typescript
 * { type: 'string', required: true, description: 'The title of the note' }
 * ```
 * 
 * @example Number field with constraints
 * ```typescript
 * { type: 'number', min: 0, max: 100, default: 50 }
 * ```
 * 
 * @example String enum field
 * ```typescript
 * { type: 'string', enum: ['low', 'medium', 'high'] }
 * ```
 */
export interface SchemaFieldDef {
  /** The type of this field */
  type: SchemaFieldType;
  /** Whether this field is required (defaults to false) */
  required?: boolean;
  /** Human-readable description for AI agents and documentation */
  description?: string;
  /** Default value if not provided */
  default?: unknown;
  /** For array types, the type of items */
  items?: SchemaFieldType;
  /** For object types, nested field definitions */
  properties?: Record<string, SchemaFieldDef>;
  /** Enum values (for string fields with limited options) */
  enum?: string[];
  /** Minimum value (for numbers) or length (for strings/arrays) */
  min?: number;
  /** Maximum value (for numbers) or length (for strings/arrays) */
  max?: number;
  /** Regex pattern for validation (strings only) */
  pattern?: string;
  /** Example value for documentation */
  example?: unknown;
}

/**
 * Schema definition for script input/output.
 * Defines the typed interface for the input() and output() functions,
 * enabling MCP tool generation and AI agent integration.
 * 
 * @example Complete schema with input and output
 * ```typescript
 * schema = {
 *   input: {
 *     title: { type: 'string', required: true, description: 'Note title' },
 *     tags: { type: 'array', items: 'string', description: 'Tags for categorization' }
 *   },
 *   output: {
 *     path: { type: 'string', description: 'Path to created file' },
 *     wordCount: { type: 'number' }
 *   }
 * }
 * ```
 */
export interface ScriptSchema {
  /** Input fields - what the script expects to receive */
  input?: Record<string, SchemaFieldDef>;
  /** Output fields - what the script will produce */
  output?: Record<string, SchemaFieldDef>;
}

/**
 * Typed metadata for scripts (replaces comment-based metadata).
 * Provides rich metadata for script discovery, documentation, and AI agents.
 * 
 * @example Basic metadata
 * ```typescript
 * metadata = {
 *   name: 'Create Note',
 *   description: 'Creates a new note in the notes directory',
 *   enter: 'Create'
 * }
 * ```
 * 
 * @example Full metadata with all fields
 * ```typescript
 * metadata = {
 *   name: 'Git Commit',
 *   description: 'Stage and commit changes with a message',
 *   author: 'John Lindquist',
 *   enter: 'Commit',
 *   alias: 'gc',
 *   icon: 'Terminal',
 *   shortcut: 'cmd shift g',
 *   tags: ['git', 'development'],
 *   hidden: false
 * }
 * ```
 * 
 * @example Scheduled script using natural language
 * ```typescript
 * metadata = {
 *   name: 'Daily Backup',
 *   description: 'Backup important files',
 *   schedule: 'every day at 2pm'
 * }
 * ```
 */
export interface ScriptMetadata {
  /** Display name for the script */
  name?: string;
  /** Description shown in the UI and used by AI agents */
  description?: string;
  /** Author of the script */
  author?: string;
  /** Text shown on the Enter/Submit button */
  enter?: string;
  /** Short alias for quick triggering (e.g., 'gc' for 'git-commit') */
  alias?: string;
  /** Icon name (e.g., 'File', 'Terminal', 'Star') */
  icon?: string;
  /** Keyboard shortcut (e.g., 'opt i', 'cmd shift k') */
  shortcut?: string;
  /** Tags for categorization and search */
  tags?: string[];
  /** Whether to hide this script from the main list */
  hidden?: boolean;
  /** Custom placeholder text for the input */
  placeholder?: string;
  /** Cron expression for scheduled execution */
  cron?: string;
  /** Natural language schedule (e.g., 'every tuesday at 2pm') - converted to cron internally */
  schedule?: string;
  /** Watch patterns for file-triggered execution */
  watch?: string[];
  /** Background script (runs without UI) */
  background?: boolean;
  /** System-level script (higher privileges) */
  system?: boolean;
  /** Additional custom metadata fields */
  [key: string]: unknown;
}

// =============================================================================
// Arg Types (for all calling conventions)
// =============================================================================

/**
 * Configuration object for arg() - supports all Script Kit v1 options
 */
export interface ArgConfig {
  placeholder?: string;
  choices?: ChoicesInput;
  hint?: string;
  /** Called when the prompt is first shown */
  onInit?: () => void | Promise<void>;
  /** Called when user submits a value */
  onSubmit?: (value: string) => void | Promise<void>;
  /** Keyboard shortcuts for actions */
  shortcuts?: Array<{
    key: string;
    name: string;
    action: () => void;
  }>;
  /** Actions shown in the actions panel (Cmd+K to open) */
  actions?: Action[];
}

/**
 * Configuration object for confirm() dialog
 */
export interface ConfirmConfig {
  /** The message to display */
  message: string;
  /** Text for the confirm button (default: "OK") */
  confirmText?: string;
  /** Text for the cancel button (default: "Cancel") */
  cancelText?: string;
}

/**
 * Function that generates choices - can be sync or async
 * If it takes an input parameter, it's called on each keystroke for filtering
 */
export type ChoicesFunction = 
  | (() => (string | Choice)[] | Promise<(string | Choice)[]>)
  | ((input: string) => (string | Choice)[] | Promise<(string | Choice)[]>);

/**
 * All valid types for the choices parameter
 */
export type ChoicesInput = (string | Choice)[] | ChoicesFunction;

// =============================================================================
// TIER 5B: In-Memory Types
// =============================================================================

export interface MemoryMapAPI {
  get(key: string): unknown;
  set(key: string, value: unknown): void;
  delete(key: string): boolean;
  clear(): void;
}

// =============================================================================
// System API Types
// =============================================================================

export interface NotifyOptions {
  title?: string;
  body?: string;
}

export interface StatusOptions {
  status: 'busy' | 'idle' | 'error';
  message: string;
}

export interface Position {
  x: number;
  y: number;
}

export interface ClipboardAPI {
  readText(): Promise<string>;
  writeText(text: string): Promise<void>;
  readImage(): Promise<Buffer>;
  writeImage(buffer: Buffer): Promise<void>;
}

export interface KeyboardAPI {
  type(text: string): Promise<void>;
  tap(...keys: string[]): Promise<void>;
}

export interface MouseAPI {
  move(positions: Position[]): Promise<void>;
  leftClick(): Promise<void>;
  rightClick(): Promise<void>;
  setPosition(position: Position): Promise<void>;
}

interface ArgMessage {
  type: 'arg';
  id: string;
  placeholder: string;
  choices: Choice[];
  actions?: SerializableAction[];
}

interface DivMessage {
  type: 'div';
  id: string;
  html: string;
  /** Tailwind classes for the content container */
  containerClasses?: string;
  actions?: SerializableAction[];
  /** Placeholder text for header */
  placeholder?: string;
  /** Hint text */
  hint?: string;
  /** Footer text */
  footer?: string;
  /** Container background color */
  containerBg?: string;
  /** Container padding in pixels, or "none" */
  containerPadding?: number | "none";
  /** Container opacity (0-100) */
  opacity?: number;
}

interface EditorMessage {
  type: 'editor';
  id: string;
  content: string;
  language: string;
  actions?: SerializableAction[];
}

interface MiniMessage {
  type: 'mini';
  id: string;
  placeholder: string;
  choices: Choice[];
}

interface MicroMessage {
  type: 'micro';
  id: string;
  placeholder: string;
  choices: Choice[];
}

interface SelectMessage {
  type: 'select';
  id: string;
  placeholder: string;
  choices: Choice[];
  multiple: boolean;
}

interface ConfirmMessage {
  type: 'confirm';
  id: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
}

interface FieldsMessage {
  type: 'fields';
  id: string;
  fields: FieldDef[];
  actions?: SerializableAction[];
}

interface FormMessage {
  type: 'form';
  id: string;
  html: string;
  actions?: SerializableAction[];
}

interface PathMessage {
  type: 'path';
  id: string;
  startPath?: string;
  hint?: string;
}

interface HotkeyMessage {
  type: 'hotkey';
  id: string;
  placeholder?: string;
}

interface DropMessage {
  type: 'drop';
  id: string;
}

interface TemplateMessage {
  type: 'template';
  id: string;
  template: string;
}

interface EnvMessage {
  type: 'env';
  id: string;
  key: string;
  secret?: boolean;
}

// System message types (fire-and-forget, no response needed)
interface BeepMessage {
  type: 'beep';
}

interface SayMessage {
  type: 'say';
  text: string;
  voice?: string;
}

interface NotifyMessage {
  type: 'notify';
  title?: string;
  body?: string;
}

interface HudMessage {
  type: 'hud';
  text: string;
  duration_ms?: number;
}

interface SetStatusMessage {
  type: 'setStatus';
  status: 'busy' | 'idle' | 'error';
  message: string;
}

interface MenuMessage {
  type: 'menu';
  icon: string;
  scripts?: string[];
}

interface ClipboardMessage {
  type: 'clipboard';
  id: string;
  action: 'read' | 'write';
  format: 'text' | 'image';
  content?: string;
}

interface SetSelectedTextMessage {
  type: 'setSelectedText';
  requestId: string;
  text: string;
}

interface GetSelectedTextMessage {
  type: 'getSelectedText';
  requestId: string;
}

interface CheckAccessibilityMessage {
  type: 'checkAccessibility';
  requestId: string;
}

interface RequestAccessibilityMessage {
  type: 'requestAccessibility';
  requestId: string;
}

interface GetWindowBoundsMessage {
  type: 'getWindowBounds';
  requestId: string;
}

interface CaptureScreenshotMessage {
  type: 'captureScreenshot';
  requestId: string;
  hiDpi?: boolean;
}

interface ScreenshotResultMessage {
  type: 'screenshotResult';
  requestId: string;
  data: string;
  width: number;
  height: number;
  error?: string;
}

interface GetLayoutInfoMessage {
  type: 'getLayoutInfo';
  requestId: string;
}

interface LayoutInfoResultMessage {
  type: 'layoutInfoResult';
  requestId: string;
  windowWidth: number;
  windowHeight: number;
  promptType: string;
  components: LayoutComponentInfo[];
  timestamp: string;
}

// =============================================================================
// ACP Chat SDK API Message Types
// =============================================================================

/** Typed context part for ACP Chat — matches Rust AiContextPartInput */
type AiContextPartInput =
  | { kind: 'resourceUri'; uri: string; label: string }
  | { kind: 'filePath'; path: string; label: string };

interface AiIsOpenMessage {
  type: 'aiIsOpen';
  requestId: string;
}

interface AiIsOpenResultMessage {
  type: 'aiIsOpenResult';
  requestId: string;
  isOpen: boolean;
  activeChatId?: string;
}

interface AiGetActiveChatMessage {
  type: 'aiGetActiveChat';
  requestId: string;
}

interface AiActiveChatResultMessage {
  type: 'aiActiveChatResult';
  requestId: string;
  chat?: AiChatInfo;
}

interface AiListChatsMessage {
  type: 'aiListChats';
  requestId: string;
  limit?: number;
  includeDeleted?: boolean;
}

interface AiChatListResultMessage {
  type: 'aiChatListResult';
  requestId: string;
  chats: AiChatInfo[];
  totalCount: number;
}

interface AiGetConversationMessage {
  type: 'aiGetConversation';
  requestId: string;
  chatId?: string;
  limit?: number;
}

interface AiConversationResultMessage {
  type: 'aiConversationResult';
  requestId: string;
  chatId: string;
  messages: AiMessageInfo[];
  hasMore: boolean;
}

interface AiStartChatMessage {
  type: 'aiStartChat';
  requestId: string;
  message: string;
  systemPrompt?: string;
  image?: string;
  modelId?: string;
  noResponse?: boolean;
  parts?: AiContextPartInput[];
}

interface AiChatCreatedMessage {
  type: 'aiChatCreated';
  requestId: string;
  chatId: string;
  title: string;
  modelId: string;
  provider: string;
  streamingStarted: boolean;
}

interface AiAppendMessageMessage {
  type: 'aiAppendMessage';
  requestId: string;
  chatId: string;
  content: string;
  role: 'user' | 'assistant' | 'system';
}

interface AiMessageAppendedMessage {
  type: 'aiMessageAppended';
  requestId: string;
  messageId: string;
  chatId: string;
}

interface AiSendMessageMessage {
  type: 'aiSendMessage';
  requestId: string;
  chatId: string;
  content: string;
  image?: string;
  parts?: AiContextPartInput[];
}

interface AiMessageSentMessage {
  type: 'aiMessageSent';
  requestId: string;
  userMessageId: string;
  chatId: string;
  streamingStarted: boolean;
}

interface AiSetSystemPromptMessage {
  type: 'aiSetSystemPrompt';
  requestId: string;
  chatId: string;
  prompt: string;
}

interface AiSystemPromptSetMessage {
  type: 'aiSystemPromptSet';
  requestId: string;
  success: boolean;
  error?: string;
}

interface AiFocusMessage {
  type: 'aiFocus';
  requestId: string;
}

interface AiFocusResultMessage {
  type: 'aiFocusResult';
  requestId: string;
  success: boolean;
  wasOpen: boolean;
}

interface AiGetStreamingStatusMessage {
  type: 'aiGetStreamingStatus';
  requestId: string;
  chatId?: string;
}

interface AiStreamingStatusResultMessage {
  type: 'aiStreamingStatusResult';
  requestId: string;
  isStreaming: boolean;
  chatId?: string;
  partialContent?: string;
}

interface AiDeleteChatMessage {
  type: 'aiDeleteChat';
  requestId: string;
  chatId: string;
  permanent?: boolean;
}

interface AiChatDeletedMessage {
  type: 'aiChatDeleted';
  requestId: string;
  success: boolean;
  error?: string;
}

interface AiSubscribeMessage {
  type: 'aiSubscribe';
  requestId: string;
  events: string[];
  chatId?: string;
}

interface AiSubscribedMessage {
  type: 'aiSubscribed';
  requestId: string;
  subscriptionId: string;
  events: string[];
}

interface AiUnsubscribeMessage {
  type: 'aiUnsubscribe';
  requestId: string;
}

interface AiUnsubscribedMessage {
  type: 'aiUnsubscribed';
  requestId: string;
}

interface AiStreamChunkMessage {
  type: 'aiStreamChunk';
  subscriptionId: string;
  chatId: string;
  chunk: string;
  accumulatedContent: string;
}

interface AiStreamCompleteMessage {
  type: 'aiStreamComplete';
  subscriptionId: string;
  chatId: string;
  messageId: string;
  fullContent: string;
  tokensUsed?: number;
}

interface AiNewMessageMessage {
  type: 'aiNewMessage';
  subscriptionId: string;
  chatId: string;
  message: AiMessageInfo;
}

interface AiErrorMessage {
  type: 'aiError';
  subscriptionId?: string;
  requestId?: string;
  code: string;
  message: string;
}

// AI Chat SDK API Data Types
interface AiChatInfo {
  id: string;
  title: string;
  modelId: string;
  provider: string;
  createdAt: string;
  updatedAt: string;
  isDeleted: boolean;
  preview?: string;
  messageCount: number;
}

interface AiMessageInfo {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  createdAt: string;
  tokensUsed?: number;
}

interface AiChatOptions {
  /** Optional system prompt */
  systemPrompt?: string;
  /** File path to image - SDK reads and base64 encodes */
  imagePath?: string;
  /** Model ID (e.g., "claude-3-5-sonnet-20241022") */
  modelId?: string;
  /** If true, don't trigger AI response */
  noResponse?: boolean;
  /** Typed context parts to attach (resource URIs or file paths) */
  parts?: AiContextPartInput[];
}

interface AiStartChatResult {
  chatId: string;
  title: string;
  modelId: string;
  provider: string;
  streamingStarted: boolean;
}

interface AiStreamChunkEvent {
  chatId: string;
  chunk: string;
  accumulatedContent: string;
}

interface AiStreamCompleteEvent {
  chatId: string;
  messageId: string;
  fullContent: string;
  tokensUsed?: number;
}

interface AiMessageEvent {
  chatId: string;
  message: AiMessageInfo;
}

interface AiErrorEvent {
  code: string;
  message: string;
}

type AiEventType = 'streamChunk' | 'streamComplete' | 'message' | 'error';
type AiEventHandler = (
  event: AiStreamChunkEvent | AiStreamCompleteEvent | AiMessageEvent | AiErrorEvent
) => void;

interface KeyboardMessage {
  type: 'keyboard';
  action: 'type' | 'tap';
  text?: string;
  keys?: string[];
}

interface MouseMessage {
  type: 'mouse';
  action: 'move' | 'click' | 'setPosition';
  positions?: Position[];
  button?: 'left' | 'right';
  position?: Position;
}

interface SubmitMessage {
  type: 'submit';
  id: string;
  value: string | null;
}

// Response messages from GPUI that need to be handled like submit
interface FileSearchResultMessage {
  type: 'fileSearchResult';
  requestId: string;
  files: Array<{
    path: string;
    name: string;
    isDirectory: boolean;
    is_directory?: boolean;
    size?: number;
    modifiedAt?: string;
    modified_at?: string;
  }>;
}

// clipboardHistoryList is sent for list responses
interface ClipboardHistoryListMessage {
  type: 'clipboardHistoryList';
  requestId: string;
  entries: Array<{
    entryId: string;
    entry_id?: string;
    content: string;
    contentType: string;
    content_type?: string;
    timestamp: string;
    pinned: boolean;
  }>;
}

// clipboardHistoryResult is sent for action success/error
interface ClipboardHistoryResultMessage {
  type: 'clipboardHistoryResult';
  requestId: string;
  success: boolean;
  error?: string;
}

interface WindowListResultMessage {
  type: 'windowListResult';
  requestId: string;
  windows: Array<{
    windowId: number;
    window_id?: number;
    title: string;
    appName: string;
    app_name?: string;
    bounds?: {
      x: number;
      y: number;
      width: number;
      height: number;
    };
    isMinimized?: boolean;
    is_minimized?: boolean;
    isActive?: boolean;
    is_active?: boolean;
  }>;
}

interface WindowActionResultMessage {
  type: 'windowActionResult';
  requestId: string;
  success: boolean;
  error?: string;
}

interface ClipboardHistoryActionResultMessage {
  type: 'clipboardHistoryActionResult';
  requestId: string;
  success: boolean;
  error?: string;
}

// =============================================================================
// Actions Types
// =============================================================================

/**
 * Action definition for the Actions API.
 * Scripts can define actions that appear in the actions panel.
 */
export interface Action {
  /** Unique name/identifier for the action */
  name: string;
  /** Description shown in the UI */
  description?: string;
  /** Keyboard shortcut (e.g., "cmd+u", "ctrl+shift+p") */
  shortcut?: string;
  /** Value to submit if no onAction handler is provided */
  value?: string;
  /** Handler called when action is triggered. Receives current input and state. */
  onAction?: (input: string, state: any) => void | Promise<void>;
  /** Whether to show this action in the action bar (default: true) */
  visible?: boolean;
  /** Whether to close the prompt after action executes (default: true) */
  close?: boolean;
}

/**
 * Serializable action sent to Rust (without function handlers)
 */
interface SerializableAction {
  name: string;
  description?: string;
  shortcut?: string;
  value?: string;
  hasAction: boolean;
  visible?: boolean;
  close?: boolean;
}

interface SetActionsMessage {
  type: 'setActions';
  actions: SerializableAction[];
}

interface SetInputMessage {
  type: 'setInput';
  text: string;
}

interface ActionTriggeredMessage {
  type: 'actionTriggered';
  action: string;
  input: string;
  state: any;
}

// Union type for all response messages
type ResponseMessage = 
  | SubmitMessage 
  | FileSearchResultMessage 
  | ClipboardHistoryListMessage
  | ClipboardHistoryResultMessage
  | WindowListResultMessage
  | WindowActionResultMessage
  | ClipboardHistoryActionResultMessage
  | ScreenshotResultMessage
  | ActionTriggeredMessage
  | LayoutInfoResultMessage
  | AiIsOpenResultMessage
  | AiActiveChatResultMessage
  | AiChatListResultMessage
  | AiConversationResultMessage
  | AiChatCreatedMessage
  | AiMessageAppendedMessage
  | AiMessageSentMessage
  | AiFocusResultMessage
  | AiStreamingStatusResultMessage
  | AiSubscribedMessage
  | AiStreamChunkMessage
  | AiStreamCompleteMessage
  | AiNewMessageMessage
  | AiErrorMessage
  | MenuBarResultMessage
  | MenuActionResultMessage
  | { type: 'chatSubmit'; id: string; text: string }
  | { type: 'displayListResult'; requestId: string; displays: DisplayInfo[] }
  | { type: 'frontmostWindowResult'; requestId: string; window?: SystemWindowInfo }
  | StateResultMessage
  | ElementsResultMessage
  | WaitForResultMessage
  | BatchResultMessage;

/** Initial chat message to show the prompt */
interface ChatMessageType {
  type: 'chat';
  id: string;
  placeholder?: string;
  messages?: ChatMessage[];
  hint?: string;
  footer?: string;
  actions?: Action[];
  model?: string;
  models?: string[];
  saveHistory?: boolean;
  /** When true, the app handles AI calls instead of SDK callbacks */
  useBuiltinAi?: boolean;
}

/** Add a message to an active chat */
interface ChatAddMessageType {
  type: 'chatMessage';
  id: string;
  message: ChatMessage;
}

/** Start streaming a message */
interface ChatStreamStartType {
  type: 'chatStreamStart';
  id: string;
  messageId: string;
  position: 'left' | 'right';
}

/** Append chunk to streaming message */
interface ChatStreamChunkType {
  type: 'chatStreamChunk';
  id: string;
  messageId: string;
  chunk: string;
}

/** Complete streaming for a message */
interface ChatStreamCompleteType {
  type: 'chatStreamComplete';
  id: string;
  messageId: string;
}

/** Set error on a message */
interface ChatSetErrorType {
  type: 'chatSetError';
  id: string;
  messageId: string;
  error: string;
}

/** Clear error from a message */
interface ChatClearErrorType {
  type: 'chatClearError';
  id: string;
  messageId: string;
}

/** Clear all messages */
interface ChatClearType {
  type: 'chatClear';
  id: string;
}

// =============================================================================
// TIER 4B: Widget/Term/Media Message Types
// =============================================================================

interface WidgetMessage {
  type: 'widget';
  id: string;
  html: string;
  options?: WidgetOptions;
}

interface WidgetActionMessage {
  type: 'widgetAction';
  id: string;
  action: 'setState' | 'close';
  state?: Record<string, unknown>;
}

interface TermMessage {
  type: 'term';
  id: string;
  command?: string;
  actions?: SerializableAction[];
}

interface WebcamMessage {
  type: 'webcam';
  id: string;
}

interface MicMessage {
  type: 'mic';
  id: string;
}

interface EyeDropperMessage {
  type: 'eyeDropper';
  id: string;
}

interface FindMessage {
  type: 'find';
  id: string;
  placeholder: string;
  onlyin?: string;
}

// Widget event message (from GPUI to script)
interface WidgetEventMessage {
  type: 'widgetEvent';
  id: string;
  event: 'click' | 'input' | 'close' | 'moved' | 'resized';
  data?: WidgetEvent | WidgetInputEvent | { x: number; y: number } | { width: number; height: number };
}

// =============================================================================
// Clipboard History Message Types
// =============================================================================

interface ClipboardHistoryMessage {
  type: 'clipboardHistory';
  requestId: string;
  action: 'list' | 'pin' | 'unpin' | 'remove' | 'clear' | 'trimOversize';
  entryId?: string;
}

// =============================================================================
// Window Management Message Types
// =============================================================================

interface WindowListMessage {
  type: 'windowList';
  requestId: string;
}

interface WindowActionMessage {
  type: 'windowAction';
  requestId: string;
  action: 'focus' | 'close' | 'minimize' | 'maximize' | 'resize' | 'move' | 'tile' | 'moveToNextDisplay' | 'moveToPreviousDisplay';
  windowId?: number;
  bounds?: TargetWindowBounds;
  tilePosition?: TilePosition;
}

// =============================================================================
// File Search Message Types
// =============================================================================

interface FileSearchMessage {
  type: 'fileSearch';
  requestId: string;
  query: string;
  onlyin?: string;
}

// =============================================================================
// TIER 5A: Window Control Message Types
// =============================================================================

interface ShowMessage {
  type: 'show';
}

interface HideMessage {
  type: 'hide';
}

interface ShowGridMessage {
  type: 'showGrid';
  gridSize?: 8 | 16;
  showBounds?: boolean;
  showBoxModel?: boolean;
  showAlignmentGuides?: boolean;
  showDimensions?: boolean;
  depth?: 'prompts' | 'all' | string[];
  colorScheme?: GridColorScheme;
}

interface HideGridMessage {
  type: 'hideGrid';
}

interface BlurMessage {
  type: 'blur';
}

interface ForceSubmitMessage {
  type: 'forceSubmit';
  value: unknown;
}

interface ExitMessage {
  type: 'exit';
  code?: number;
}

interface SetPanelMessage {
  type: 'setPanel';
  html: string;
}

interface SetPreviewMessage {
  type: 'setPreview';
  html: string;
}

interface SetPromptMessage {
  type: 'setPrompt';
  html: string;
}

// =============================================================================
// TIER 5B: Browser/App Message Types
// =============================================================================

interface BrowseMessage {
  type: 'browse';
  url: string;
}

interface EditFileMessage {
  type: 'edit';
  path: string;
}

interface RunMessage {
  type: 'run';
  id: string;
  scriptName: string;
  args: string[];
}

interface InspectMessage {
  type: 'inspect';
  data: unknown;
}

// =============================================================================
// Menu Bar Message Types
// =============================================================================

interface GetMenuBarMessage {
  type: 'getMenuBar';
  requestId: string;
  bundleId?: string;
}

interface MenuBarResultMessage {
  type: 'menuBarResult';
  requestId: string;
  items: MenuBarItem[];
}

interface ExecuteMenuActionMessage {
  type: 'executeMenuAction';
  requestId: string;
  bundleId: string;
  path: string[];
}

interface MenuActionResultMessage {
  type: 'menuActionResult';
  requestId: string;
  success: boolean;
  error?: string;
}

// =============================================================================
// Core Infrastructure
// =============================================================================

let messageId = 0;

const nextId = (): string => String(++messageId);

// Generic pending map that can handle any response type
const pending = new Map<string, (value?: any) => void>();

// =============================================================================
// Actions API - Global state for action handlers
// =============================================================================

/** Global map to store full action definitions for lookup when ActionTriggered is received */
(globalThis as any).__kitActionsMap = new Map<string, Action>();

/**
 * Handle an actionTriggered message from the Rust app.
 * Looks up the action in the map and either calls onAction or submits the value.
 * @internal - Exposed for testing
 */
(globalThis as any).__handleActionTriggered = async function __handleActionTriggered(
  msg: ActionTriggeredMessage
): Promise<void> {
  const actionsMap = (globalThis as any).__kitActionsMap as Map<string, Action>;
  const action = actionsMap.get(msg.action);

  if (!action) {
    console.error(`[SDK] Action not found: ${msg.action}`);
    return;
  }

  try {
    if (typeof action.onAction === 'function') {
      await Promise.resolve(action.onAction(msg.input, msg.state));
      return;
    }

    if (action.value !== undefined) {
      send({ type: 'forceSubmit', value: action.value });
      return;
    }

    console.warn(
      `[SDK] Action "${action.name}" has no onAction handler and no value. Ignoring.`
    );
  } catch (error) {
    console.error(`[SDK] Action "${action.name}" failed:`, error);
  }
};

function send(msg: object): void {
  process.stdout.write(`${JSON.stringify(msg)}\n`);
}

// =============================================================================
// SDK Test Mode Configuration
// =============================================================================
// When SDK_TEST_AUTOSUBMIT=1 is set, prompts automatically resolve with defaults
// This allows SDK tests to run without GPUI interaction
const SDK_TEST_AUTOSUBMIT = process.env.SDK_TEST_AUTOSUBMIT === '1';
const SDK_TEST_AUTOSUBMIT_DELAY = parseInt(process.env.SDK_TEST_AUTOSUBMIT_DELAY || '10', 10);

if (SDK_TEST_AUTOSUBMIT) {
  console.error('[SDK] Auto-submit mode enabled - prompts will auto-resolve');
}

// Use raw stdin reading instead of readline interface
// This works better with bun's --preload mode
let stdinBuffer = '';

bench('stdin_setup_start');
console.error('[SDK] Setting up stdin handler...');

// Set up raw stdin handling
process.stdin.setEncoding('utf8');
// Resume stdin to start receiving data - it may be paused by default
process.stdin.resume();
// Unref stdin so it doesn't keep the process alive when script completes
// This allows the process to exit naturally when all async work is done
(process.stdin as any).unref?.();
bench('stdin_ready');
console.error('[SDK] stdin resumed, readable:', process.stdin.readable);

// Helper to manage stdin ref counting for pending operations
// When there are pending operations waiting for stdin responses, we need to
// keep stdin referenced so the process doesn't exit prematurely
function addPending(id: string, resolver: (msg: any) => void, autoSubmitValue?: any): void {
  // In auto-submit mode, immediately resolve after a short delay
  if (SDK_TEST_AUTOSUBMIT) {
    setTimeout(() => {
      console.error(`[SDK] Auto-submitting id=${id} with value:`, autoSubmitValue);
      resolver(autoSubmitValue);
    }, SDK_TEST_AUTOSUBMIT_DELAY);
    return;
  }

  const wasEmpty: boolean = pending.size === 0;
  // Note: This is the internal call in addPending, don't change to addPending
  pending.set(id, resolver);
  // If this is the first pending operation, ref stdin to keep process alive
  if (wasEmpty && pending.size === 1) {
    (process.stdin as any).ref?.();
  }
}

function removePending(id: string): ((msg: any) => void) | undefined {
  const resolver = pending.get(id);
  if (resolver) {
    pending.delete(id);
    // If no more pending operations, unref stdin to allow process to exit
    if (pending.size === 0) {
      (process.stdin as any).unref?.();
    }
  }
  return resolver;
}

process.stdin.on('data', (chunk: string) => {
  console.error('[SDK_DEBUG] Received stdin chunk:', chunk.length, 'bytes');
  stdinBuffer += chunk;
  
  // Process complete lines
  let newlineIndex: number = stdinBuffer.indexOf('\n');
  while (newlineIndex !== -1) {
    const line = stdinBuffer.substring(0, newlineIndex);
    stdinBuffer = stdinBuffer.substring(newlineIndex + 1);
    
    if (line.trim()) {
      try {
        const msg = JSON.parse(line) as ResponseMessage;
        
        // Get the ID based on message type
        let id: string | undefined;
        if (msg.type === 'submit') {
          id = (msg as SubmitMessage).id;
        } else if (msg.type === 'chatSubmit') {
          // Chat submit messages have 'id' field, not 'requestId'
          id = (msg as { id: string; text: string }).id;
        } else if ('requestId' in msg) {
          id = (msg as { requestId: string }).requestId;
        }
        
        if (id && pending.has(id)) {
          const resolver = removePending(id);
          if (resolver) {
            // Pass the full message object so resolvers can check msg.value, msg.type, etc.
            resolver(msg);
          }
        }

        // Emit chatSubmit events for the onMessage handler in chat()
        if (msg.type === 'chatSubmit') {
          process.emit('chatSubmit' as any, msg);
        }
        
        // Handle actionTriggered messages
        if (msg.type === 'actionTriggered') {
          (globalThis as any).__handleActionTriggered(msg as ActionTriggeredMessage);
        }

        // Handle AI SDK events (streamChunk, streamComplete, newMessage, error)
        if (msg.type?.startsWith('ai') && (globalThis as any)._handleAiEvent) {
          (globalThis as any)._handleAiEvent(msg);
        }

        // Also emit a custom event for widget handlers
        if ((msg as any).type === 'widgetEvent') {
          process.emit('widgetEvent' as any, msg);
        }
      } catch (e) {
        // Ignore parse errors - they're usually test output
      }
    }
    newlineIndex = stdinBuffer.indexOf('\n');
  }
});

// Keep a reference for backwards compatibility with widget code
// This is a dummy readline interface that just delegates to the raw stdin handler
const rl = {
  listeners: () => [],
  removeListener: () => {},
  on: (event: string, handler: (...args: any[]) => void) => {
    if (event === 'line') {
      // Widget handlers will use this - redirect to our custom event
      process.on('widgetEvent' as any, handler);
    }
  },
};

// =============================================================================
// Global API Functions (Script Kit v1 pattern - no imports needed)
// =============================================================================

declare global {
  /**
   * Prompt user for input with optional choices
   * 
   * Supports multiple calling conventions:
   * - arg() - no arguments, show text input
   * - arg('placeholder') - placeholder text, no choices
   * - arg('placeholder', ['a','b','c']) - with string array choices
   * - arg('placeholder', [{name, value}]) - with structured choices
   * - arg('placeholder', async () => [...]) - with async function returning choices
   * - arg('placeholder', (input) => [...]) - with filter function
   * - arg('placeholder', choices, actions) - with choices and actions
   * - arg({placeholder, choices, actions, ...}) - config object with all options
   */
  function arg(): Promise<string>;
  function arg(placeholder: string): Promise<string>;
  function arg(placeholder: string, choices: ChoicesInput): Promise<string>;
  function arg(placeholder: string, choices: ChoicesInput, actions: Action[]): Promise<string>;
  function arg(config: ArgConfig): Promise<string>;
  
  /**
   * Display HTML content to user
   * 
   * Matches original Script Kit API: div(html?, config?, actions?)
   * 
   * @param htmlOrConfig - HTML string or DivConfig object
   * @param actions - Optional actions for the actions panel (Cmd+K)
   * 
   * @example
   * // Basic usage
   * await div("<h1>Hello</h1>");
   * 
   * @example
   * // With config object
   * await div({
   *   html: "<h1>Hello</h1>",
   *   placeholder: "My Title",
   *   containerClasses: "bg-blue-500 p-4"
   * });
   * 
   * @example
   * // Transparent background with gradient content
   * await div({
   *   html: '<div class="bg-gradient-to-r from-purple-500 to-pink-500 p-8 h-full">Content</div>',
   *   containerBg: "transparent",
   *   containerPadding: "none"
   * });
   * 
   * @example
   * // With actions
   * await div("<h1>Hello</h1>", [
   *   { name: "Copy", shortcut: "cmd+c", onAction: () => clipboard.writeText("Hello") }
   * ]);
   */
  function div(html?: string | DivConfig, actions?: Action[]): Promise<string | void>;

  /**
   * Show a confirmation dialog with Yes/No buttons.
   *
   * Returns true if the user confirms, false if they cancel.
   * Keyboard shortcuts: Enter = confirm, Escape = cancel, Tab/Arrow = switch buttons.
   *
   * @example
   * // Simple usage
   * const confirmed = await confirm("Delete this file?");
   * if (confirmed) { // proceed }
   *
   * @example
   * // With custom button text
   * const proceed = await confirm({
   *   message: "Overwrite existing file?",
   *   confirmText: "Overwrite",
   *   cancelText: "Keep Original"
   * });
   *
   * @example
   * // Shorthand with custom buttons
   * const yes = await confirm("Continue?", "Yes", "No");
   */
  // Note: confirm is assigned via (globalThis as any).confirm to avoid conflict with DOM's confirm()
  // The actual function signature is:
  // confirm(): Promise<boolean>;
  // confirm(message: string): Promise<boolean>;
  // confirm(message: string, confirmText: string, cancelText: string): Promise<boolean>;
  // confirm(config: ConfirmConfig): Promise<boolean>;

  /**
   * Convert Markdown to HTML
   */
  function md(markdown: string): string;
  
  /**
   * Opens a Monaco-style code editor
   * @param content - Initial content to display in the editor
   * @param language - Language for syntax highlighting (e.g., 'typescript', 'javascript', 'json')
   * @param actions - Optional actions to display in the actions panel (Cmd+K)
   * @returns The edited content when user submits
   */
  function editor(content?: string, language?: string, actions?: Action[]): Promise<string>;
  
  /**
   * Compact prompt variant - same API as arg() but with minimal UI
   * @param placeholder - Prompt text shown to user
   * @param choices - Array of string or Choice objects
   * @returns The selected value
   */
  function mini(placeholder: string, choices: (string | Choice)[]): Promise<string>;
  
  /**
   * Tiny prompt variant - same API as arg() but with smallest UI
   * @param placeholder - Prompt text shown to user
   * @param choices - Array of string or Choice objects
   * @returns The selected value
   */
  function micro(placeholder: string, choices: (string | Choice)[]): Promise<string>;
  
  /**
   * Multi-select prompt - allows selecting multiple choices
   * @param placeholder - Prompt text shown to user
   * @param choices - Array of string or Choice objects
   * @returns Array of selected values
   */
  function select(placeholder: string, choices: (string | Choice)[]): Promise<string[]>;
  
  /**
   * Multi-field form prompt
   * @param fieldDefs - Array of field definitions (strings become both name and label)
   * @returns Array of field values in order
   */
  function fields(fieldDefs: (string | FieldDef)[]): Promise<string[]>;
  
  /**
   * Custom HTML form prompt
   * @param html - HTML string containing form fields
   * @returns Object with form field names as keys and their values
   */
  function form(html: string): Promise<Record<string, string>>;
  
  /**
   * File/folder browser prompt
   * @param options - Optional path options (startPath, hint)
   * @returns The selected file/folder path
   */
  function path(options?: PathOptions): Promise<string>;
  
  /**
   * Capture keyboard shortcut
   * @param placeholder - Optional placeholder text
   * @returns HotkeyInfo with key details and modifier states
   */
  function hotkey(placeholder?: string): Promise<HotkeyInfo>;
  
  /**
   * Drag and drop zone
   * @returns Array of FileInfo for dropped files
   */
  function drop(): Promise<FileInfo[]>;
  
  /**
   * Tab-through template editor like VSCode snippets
   * 
   * @param template - Template string with VSCode snippet syntax:
   *   - $1, $2, $3 - Simple tabstops (Tab to navigate)
   *   - ${1:default} - Tabstop with placeholder
   *   - ${1|a,b,c|} - Choice tabstop
   *   - $0 - Final cursor position
   *   - $$ - Escaped dollar sign
   *   - $SELECTION - Currently selected text
   *   - $CLIPBOARD - Clipboard contents
   *   - $HOME - User's home directory
   * @param options - Editor options (language for syntax highlighting)
   * @returns The filled-in template string
   */
  function template(
    template: string,
    options?: { language?: string }
  ): Promise<string>;
  
  /**
   * Get/set environment variable
   * @param key - Environment variable key
   * @param promptFn - Optional function to prompt for value if not set
   * @returns The environment variable value
   */
  function env(key: string, promptFn?: () => Promise<string>): Promise<string>;
  
  // =============================================================================
  // System APIs (TIER 3)
  // =============================================================================
  
  /**
   * Play system beep sound
   */
  function beep(): Promise<void>;
  
  /**
   * Text-to-speech
   * @param text - Text to speak
   * @param voice - Optional voice name
   */
  function say(text: string, voice?: string): Promise<void>;
  
  /**
   * Show system notification
   * @param options - Notification options or body string
   */
  function notify(options: string | NotifyOptions): Promise<void>;
  
  /**
   * Show a brief HUD notification at bottom-center of screen.
   * Fire-and-forget - no response needed.
   * 
   * @param message - Text to display (supports emoji)
   * @param options - Optional duration configuration
   * @param options.duration - Display duration in milliseconds (default: 2000)
   * 
   * @example
   * hud('Copied!')                           // Simple confirmation
   * hud('Saved! 💾')                         // With emoji  
   * hud('Alias blocked: ...', { duration: 4000 })  // Longer duration
   */
  function hud(message: string, options?: { duration?: number }): void;
  
  /**
   * Set application status
   * @param options - Status options with status and message
   */
  function setStatus(options: StatusOptions): Promise<void>;
  
  /**
   * Set system menu icon and scripts
   * @param icon - Icon name/path
   * @param scripts - Optional array of script paths
   */
  function menu(icon: string, scripts?: string[]): Promise<void>;
  
  /**
   * Set the available actions for the current prompt.
   * Actions appear in the actions panel and can have keyboard shortcuts.
   * 
   * @param actions - Array of action definitions
   */
  function setActions(actions: Action[]): Promise<void>;

  /**
   * Set the current prompt's input text.
   * @param text - Input text to apply
   */
  function setInput(text: string): void;
  
  /**
   * Copy text to clipboard (alias for clipboard.writeText)
   * @param text - Text to copy
   */
  function copy(text: string): Promise<void>;
  
  /**
   * Paste text from clipboard (alias for clipboard.readText)
   * @returns Text from clipboard
   */
  function paste(): Promise<string>;
  
  /**
   * Replace the currently selected text in the focused application.
   * Uses macOS Accessibility APIs for reliability (95%+ of apps).
   * Falls back to clipboard simulation for apps that block accessibility.
   * 
   * @param text - The text to insert (replaces selection)
   * @throws If accessibility permission not granted
   * @throws If paste operation fails
   */
  function setSelectedText(text: string): Promise<void>;
  
  /**
   * Get the currently selected text from the focused application.
   * Uses macOS Accessibility APIs for reliability (95%+ of apps).
   * Falls back to clipboard simulation for apps that block accessibility.
   * 
   * @returns The selected text, or empty string if nothing selected
   * @throws If accessibility permission not granted
   */
  function getSelectedText(): Promise<string>;
  
  /**
   * Check if accessibility permission is granted.
   * Required for getSelectedText and setSelectedText to work reliably.
   * 
   * @returns true if permission granted, false otherwise
   */
  function hasAccessibilityPermission(): Promise<boolean>;
  
  /**
   * Request accessibility permission (opens System Preferences).
   * User must manually grant permission in System Preferences > Privacy & Security > Accessibility.
   * 
   * @returns true if permission was granted after request, false otherwise
   */
  function requestAccessibilityPermission(): Promise<boolean>;
  
  /**
   * Clipboard API object
   */
  var clipboard: ClipboardAPI;
  
  /**
   * Keyboard API object
   */
  var keyboard: KeyboardAPI;
  
  /**
   * Mouse API object
   */
  var mouse: MouseAPI;

  /**
   * Direct MCP client for Script Kit-managed external MCP servers.
   */
  var mcp: McpApi;
  
  // =============================================================================
  // TIER 4A: Chat Prompt (Inline UI in Main Window)
  // =============================================================================
  //
  // IMPORTANT: chat() is a UI-ONLY prompt in the main Script Kit window.
  // It does NOT do AI generation - your script handles that.
  // Use chat() when you want to:
  // - Build a custom chat interface with your own AI provider
  // - Stream responses from any API (Anthropic, OpenAI, local models, etc.)
  // - Control the conversation flow programmatically
  //
  // For ACP Chat with built-in BYOK AI providers,
  // see the ai* functions below (aiStartChat, aiFocus, etc.)
  // =============================================================================

  /**
   * Chat function interface with attached controller methods for streaming
   */
  interface ChatFunction {
    (options?: ChatOptions): Promise<ChatResult>;
    addMessage(msg: ChatMessage): void;
    startStream(position?: 'left' | 'right'): string;
    appendChunk(messageId: string, chunk: string): void;
    completeStream(messageId: string): void;
    clear(): void;
    setError(messageId: string, error: string): void;
    clearError(messageId: string): void;
    getMessages(): CoreMessage[];
    getResult(): ChatResult;
  }

  /**
   * Inline chat UI prompt in the main Script Kit window.
   *
   * **IMPORTANT**: This is UI-only - it does NOT call AI APIs.
   * Your script is responsible for:
   * - Calling AI APIs (Anthropic, OpenAI, etc.) directly or via npm packages
   * - Streaming responses using chat.startStream(), chat.appendChunk(), chat.completeStream()
   *
   * This design lets you use ANY AI provider, including local models, custom APIs,
   * or the Vercel AI SDK. The SDK only provides the chat UI; generation is up to you.
   *
   * For ACP Chat with built-in AI providers (BYOK),
   * see aiStartChat(), aiFocus(), and other ai* functions.
   *
   * @param options - Chat configuration with optional callbacks
   * @param options.messages - Initial messages to display (AI SDK compatible)
   * @param options.system - System prompt shorthand
   * @param options.onInit - Called when chat opens (use to stream initial response)
   * @param options.onMessage - Called when user submits a message
   * @returns ChatResult with messages in AI SDK format
   *
   * @example Basic chat with streaming
   * ```typescript
   * await chat({
   *   messages: [{ role: 'user', content: userPrompt }],
   *   system: 'You are a helpful assistant',
   *   async onInit() {
   *     const msgId = chat.startStream('left');
   *     // Stream from your AI provider
   *     for await (const chunk of myAiStream()) {
   *       chat.appendChunk(msgId, chunk);
   *     }
   *     chat.completeStream(msgId);
   *   }
   * });
   * ```
   */
  const chat: ChatFunction;
  
  // =============================================================================
  // TIER 4B: Widget/Term/Media Prompts
  // =============================================================================
  
  /**
   * Creates a floating HTML widget window
   * @param html - HTML content for the widget
   * @param options - Widget positioning and behavior options
   * @returns WidgetController for managing the widget
   */
  function widget(html: string, options?: WidgetOptions): Promise<WidgetController>;
  
  /**
   * Opens a terminal window
   * @param command - Optional command to run in the terminal
   * @returns Terminal output when command completes
   */
  function term(command?: string): Promise<string>;
  
  /**
   * Opens webcam preview, captures on Enter
   * @returns Image buffer of captured photo
   */
  function webcam(): Promise<Buffer>;
  
  /**
   * Records audio from microphone
   * @returns Audio buffer of recording
   */
  function mic(): Promise<Buffer>;
  
  /**
   * Pick a color from the screen using eye dropper
   * @returns Color information in multiple formats
   */
  function eyeDropper(): Promise<ColorInfo>;
  
  /**
   * File search using Spotlight/mdfind
   * @param placeholder - Search prompt text
   * @param options - Search options including directory filter
   * @returns Selected file path
   */
  function find(placeholder: string, options?: FindOptions): Promise<string>;
  
  // =============================================================================
  // TIER 5A: Window Control Functions
  // =============================================================================
  
  /**
   * Show the main window
   */
  function show(): Promise<void>;
  
  /**
   * Hide the main window
   */
  function hide(): Promise<void>;
  
  /**
   * Show the debug grid overlay for visual testing
   * @param options - Grid display options
   */
  function showGrid(options?: GridOptions): Promise<void>;
  
  /**
   * Hide the debug grid overlay
   */
  function hideGrid(): Promise<void>;
  
  /**
   * Blur - return focus to previous app
   */
  function blur(): Promise<void>;
  
  /**
   * Get the current window bounds (position and size).
   * Useful for testing window resize behavior and layout verification.
   * 
   * @returns Window bounds with x, y, width, height in pixels
   */
  function getWindowBounds(): Promise<WindowBounds>;
  
  /**
   * Capture a screenshot of the Script Kit window.
   * Useful for visual testing and debugging layout issues.
   * 
   * @param options - Screenshot options
   * @param options.hiDpi - If true, capture at full retina resolution (2x). Default false for 1x.
   * @returns Promise with base64-encoded PNG data and dimensions
   */
  function captureScreenshot(options?: ScreenshotOptions): Promise<ScreenshotData>;
  
  /**
   * Get detailed layout information for the current UI state.
   * 
   * Returns comprehensive component information including bounds, box model,
   * flex properties, and human-readable explanations of why components are sized.
   * Designed for AI agents to understand "why" components are positioned/sized.
   * 
   * @returns LayoutInfo with component tree and window information
   */
  function getLayoutInfo(): Promise<LayoutInfo>;

  // =============================================================================
  // ACP Chat SDK API
  // =============================================================================
  //
  // These functions control **ACP Chat** — Script Kit's built-in AI chat surface:
  // - BYOK (Bring Your Own Key) AI providers (Anthropic, OpenAI)
  // - Manages its own chat history in SQLite
  // - Provides streaming responses automatically
  // - Supports typed context parts (resource URIs, file paths)
  //
  // For an **inline chat UI** in the main window where YOU control AI generation,
  // use the chat() function instead. See TIER 4A above.
  // =============================================================================

  /** Check if ACP Chat is currently open */
  function aiIsOpen(): Promise<{ isOpen: boolean; activeChatId?: string }>;

  /** Get information about the currently active chat in ACP Chat */
  function aiGetActiveChat(): Promise<AiChatInfo | null>;

  /** List all chats from ACP Chat storage */
  function aiListChats(limit?: number, includeDeleted?: boolean): Promise<AiChatInfo[]>;

  /** Get messages from a specific chat or the active chat */
  function aiGetConversation(chatId?: string, limit?: number): Promise<AiMessageInfo[]>;

  /** Start a new ACP Chat conversation with an initial message */
  function aiStartChat(message: string, options?: AiChatOptions): Promise<AiStartChatResult>;

  /** Append a message to an existing chat without triggering a response */
  function aiAppendMessage(chatId: string, content: string, role: 'user' | 'assistant' | 'system'): Promise<string>;

  /** Send a message to an existing chat and trigger an AI response */
  function aiSendMessage(chatId: string, content: string, imagePath?: string, parts?: AiContextPartInput[]): Promise<{ userMessageId: string; streamingStarted: boolean }>;

  /** Set or update the system prompt for a chat */
  function aiSetSystemPrompt(chatId: string, prompt: string): Promise<void>;

  /** Focus ACP Chat, opening it if necessary */
  function aiFocus(): Promise<{ wasOpen: boolean }>;

  /** Get the current streaming status for a chat */
  function aiGetStreamingStatus(chatId?: string): Promise<{ isStreaming: boolean; chatId?: string; partialContent?: string }>;

  /** Delete a chat from ACP Chat storage */
  function aiDeleteChat(chatId: string, permanent?: boolean): Promise<void>;

  /** Subscribe to ACP Chat events for real-time streaming updates */
  function aiOn(eventType: AiEventType, handler: AiEventHandler, chatId?: string): Promise<() => void>;

  /**
   * Force submit the current prompt with a value
   * @param value - Value to submit
   */
  function submit(value: unknown): void;
  
  /**
   * Exit the script
   * @param code - Optional exit code
   */
  function exit(code?: number): void;
  
  /**
   * Set the panel HTML content
   * @param html - HTML content
   */
  function setPanel(html: string): void;
  
  /**
   * Set the preview HTML content
   * @param html - HTML content
   */
  function setPreview(html: string): void;
  
  /**
   * Set the prompt HTML content
   * @param html - HTML content
   */
  function setPrompt(html: string): void;
  
  /**
   * Generate a UUID
   * @returns UUID string
   */
  function uuid(): string;
  
  /**
   * Compile a simple template string
   * @param template - Template with {{key}} placeholders
   * @returns Function that takes data and returns filled template
   */
  function compile(template: string): (data: Record<string, unknown>) => string;
  
  // =============================================================================
  // TIER 5B: Path Utilities
  // =============================================================================
  
  /**
   * Returns path relative to user's home directory
   * @param segments - Path segments to join
   * @returns Full path from home directory
   */
  function home(...segments: string[]): string;
  
  /**
   * Returns path relative to ~/.scriptkit
   * @param segments - Path segments to join
   * @returns Full path from sk/kit directory
   */
  function skPath(...segments: string[]): string;
  
  /**
   * Returns path relative to ~/.scriptkit (unified Script Kit directory)
   * @param segments - Path segments to join
   * @returns Full path from sk/kit directory
   * @deprecated Use skPath() instead - kitPath() now returns ~/.scriptkit paths for backwards compatibility
   */
  function kitPath(...segments: string[]): string;
  
  /**
   * Returns path relative to system temp + kit subfolder
   * @param segments - Path segments to join
   * @returns Full path in temp directory
   */
  function tmpPath(...segments: string[]): string;
  
  // =============================================================================
  // TIER 5B: File Utilities
  // =============================================================================
  
  /**
   * Check if path is a file
   * @param filePath - Path to check
   * @returns True if path is a file
   */
  function isFile(filePath: string): Promise<boolean>;
  
  /**
   * Check if path is a directory
   * @param dirPath - Path to check
   * @returns True if path is a directory
   */
  function isDir(dirPath: string): Promise<boolean>;
  
  /**
   * Check if file is executable
   * @param filePath - Path to check
   * @returns True if file is executable
   */
  function isBin(filePath: string): Promise<boolean>;
  
  // =============================================================================
  // TIER 5B: In-Memory Storage
  // =============================================================================
  
  /**
   * In-memory map (not persisted)
   */
  var memoryMap: MemoryMapAPI;
  
  // =============================================================================
  // TIER 5B: Browser/App Utilities
  // =============================================================================
  
  /**
   * Open URL in default browser
   * @param url - URL to open
   */
  function browse(url: string): Promise<void>;
  
  /**
   * Open file in KIT_EDITOR
   * @param filePath - File path to edit
   */
  function editFile(filePath: string): Promise<void>;
  
  /**
   * Run another script
   * @param scriptName - Name of script to run
   * @param args - Arguments to pass to script
   * @returns Result from the script
   */
  function run(scriptName: string, ...args: string[]): Promise<unknown>;
  
  /**
   * Pretty-print data for inspection
   * @param data - Data to inspect
   */
  function inspect(data: unknown): Promise<void>;
  
  // =============================================================================
  // Clipboard History Functions
  // =============================================================================
  
  /**
   * Get the clipboard history list
   * @returns Array of clipboard history entries
   */
  function clipboardHistory(): Promise<ClipboardHistoryEntry[]>;
  
  /**
   * Pin a clipboard history entry to prevent auto-removal
   * @param entryId - ID of the entry to pin
   */
  function clipboardHistoryPin(entryId: string): Promise<void>;
  
  /**
   * Unpin a clipboard history entry
   * @param entryId - ID of the entry to unpin
   */
  function clipboardHistoryUnpin(entryId: string): Promise<void>;
  
  /**
   * Remove a specific entry from clipboard history
   * @param entryId - ID of the entry to remove
   */
  function clipboardHistoryRemove(entryId: string): Promise<void>;
  
  /**
   * Clear all clipboard history entries (except pinned ones)
   */
  function clipboardHistoryClear(): Promise<void>;

  /**
   * Remove text clipboard entries that exceed the max length limit
   */
  function clipboardHistoryTrimOversize(): Promise<void>;
  
  // =============================================================================
  // Window Management Functions (System Windows)
  // =============================================================================
  
  /**
   * Get list of all system windows
   * @returns Array of window information objects
   */
  function getWindows(): Promise<SystemWindowInfo[]>;
  
  /**
   * Focus a specific window by ID
   * @param windowId - ID of the window to focus
   */
  function focusWindow(windowId: number): Promise<void>;
  
  /**
   * Close a specific window by ID
   * @param windowId - ID of the window to close
   */
  function closeWindow(windowId: number): Promise<void>;
  
  /**
   * Minimize a specific window by ID
   * @param windowId - ID of the window to minimize
   */
  function minimizeWindow(windowId: number): Promise<void>;
  
  /**
   * Maximize a specific window by ID
   * @param windowId - ID of the window to maximize
   */
  function maximizeWindow(windowId: number): Promise<void>;
  
  /**
   * Move a window to specific coordinates
   * @param windowId - ID of the window to move
   * @param x - New x coordinate
   * @param y - New y coordinate
   */
  function moveWindow(windowId: number, x: number, y: number): Promise<void>;
  
  /**
   * Resize a window to specific dimensions
   * @param windowId - ID of the window to resize
   * @param width - New width
   * @param height - New height
   */
  function resizeWindow(windowId: number, width: number, height: number): Promise<void>;
  
  /**
   * Tile a window to a specific screen position
   * @param windowId - ID of the window to tile
   * @param position - Tile position (left, right, top-left, etc.)
   */
  function tileWindow(windowId: number, position: TilePosition): Promise<void>;

  /**
   * Get information about all connected displays/monitors
   * @returns Array of display information including bounds and visibility
   */
  function getDisplays(): Promise<DisplayInfo[]>;

  /**
   * Get the frontmost window of the app that was active before Script Kit appeared
   * This is useful for window management commands that operate on the user's previous window
   * @returns The frontmost window info, or null if no window is found
   */
  function getFrontmostWindow(): Promise<SystemWindowInfo | null>;

  /**
   * Move a window to the next display/monitor
   * @param windowId - The ID of the window to move
   */
  function moveToNextDisplay(windowId: number): Promise<void>;

  /**
   * Move a window to the previous display/monitor
   * @param windowId - The ID of the window to move
   */
  function moveToPreviousDisplay(windowId: number): Promise<void>;

  // =============================================================================
  // File Search Functions
  // =============================================================================

  /**
   * Search for files using Spotlight/mdfind
   * @param query - Search query string
   * @param options - Search options including directory filter
   * @returns Array of matching file results
   */
  function fileSearch(query: string, options?: FindOptions): Promise<FileSearchResult[]>;

  // =============================================================================
  // AI-First Protocol (Metadata, Schema, Input/Output)
  // =============================================================================

  /** Typed metadata for scripts (replaces comment-based metadata) */
  var metadata: ScriptMetadata;

  /**
   * Schema definition for typed input/output.
   * Use `as const` for full type inference with input() and output().
   * 
   * @example
   * ```typescript
   * schema = {
   *   input: {
   *     greeting: { type: 'string', required: true },
   *     count: { type: 'number' }
   *   },
   *   output: {
   *     message: { type: 'string' }
   *   }
   * } as const
   * 
   * // Types are automatically inferred!
   * const { greeting, count } = await input()
   * output({ message: `${greeting} x${count}` })
   * ```
   */
  var schema: ScriptSchema;

  /**
   * Define a schema and get typed input/output functions.
   * This is the recommended way to use schema with full type inference.
   * 
   * @example
   * ```typescript
   * const { input, output } = defineSchema({
   *   input: {
   *     greeting: { type: 'string', required: true },
   *     count: { type: 'number' }
   *   },
   *   output: {
   *     message: { type: 'string' }
   *   }
   * } as const)
   * 
   * // Types are fully inferred!
   * const { greeting, count } = await input()
   * output({ message: `Hello ${greeting}!` })
   * ```
   */
  function defineSchema<T extends ScriptSchema>(
    schema: T
  ): TypedSchemaAPI<InferInput<T>, InferOutput<T>> & { schema: T };

  /**
   * Get input data. For typed version, use defineSchema().
   */
  function input<T extends Record<string, unknown> = Record<string, unknown>>(): Promise<T>;

  /**
   * Send output data. For typed version, use defineSchema().
   */
  function output(data: Record<string, unknown>): void;

  /** @internal */
  function _setScriptInput(data: Record<string, unknown>): void;
  /** @internal */
  function _getScriptOutput(): Record<string, unknown>;
  /** @internal */
  function _resetScriptIO(): void;
}

/**
 * Normalize a single choice to {name, value} format
 */
function normalizeChoice(c: string | Choice): Choice {
  if (typeof c === 'string') {
    return { name: c, value: c };
  }
  return c;
}

/**
 * Normalize an array of choices to Choice[] format
 * Handles undefined, empty arrays, and mixed string/object arrays
 */
function normalizeChoices(choices: (string | Choice)[] | undefined | null): Choice[] {
  if (!choices || !Array.isArray(choices)) {
    return [];
  }
  return choices.map(normalizeChoice);
}

/**
 * Check if a value is a function
 */
function isFunction(value: unknown): value is (...args: unknown[]) => unknown {
  return typeof value === 'function';
}

/**
 * Check if a value is an ArgConfig object (not an array, not a function, has object properties)
 */
function isArgConfig(value: unknown): value is ArgConfig {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    !isFunction(value)
  );
}

globalThis.arg = async function arg(
  placeholderOrConfig?: string | ArgConfig,
  choicesInput?: ChoicesInput,
  actionsInput?: Action[]
): Promise<string> {
  const id = nextId();
  
  // Parse arguments to extract placeholder, choices, and actions
  let placeholder = '';
  let choices: ChoicesInput | undefined;
  let actions: Action[] | undefined;
  let config: ArgConfig | undefined;
  
  // Handle different calling conventions:
  // 1. arg() - no arguments
  // 2. arg('placeholder') - string only
  // 3. arg('placeholder', choices) - string + choices
  // 4. arg('placeholder', choices, actions) - string + choices + actions
  // 5. arg({...}) - config object
  
  if (placeholderOrConfig === undefined) {
    // arg() - no arguments, empty prompt
    placeholder = '';
    choices = undefined;
  } else if (typeof placeholderOrConfig === 'string') {
    // arg('placeholder') or arg('placeholder', choices) or arg('placeholder', choices, actions)
    placeholder = placeholderOrConfig;
    choices = choicesInput;
    actions = actionsInput;
  } else if (isArgConfig(placeholderOrConfig)) {
    // arg({placeholder, choices, actions, ...})
    config = placeholderOrConfig;
    placeholder = config.placeholder ?? '';
    choices = config.choices;
    actions = config.actions;
  }
  
  // Resolve choices if it's a function (sync or async)
  let resolvedChoices: (string | Choice)[] = [];
  
  if (choices === undefined || choices === null) {
    // No choices - text input mode
    resolvedChoices = [];
  } else if (Array.isArray(choices)) {
    // Static array of choices
    resolvedChoices = choices;
  } else if (isFunction(choices)) {
    // Function that returns choices
    // Check if the function expects an argument (filter function) or not (generator function)
    // For initial display, call with empty string if it takes an argument
    try {
      // Use type assertion to call the function with appropriate signature
      // If function.length > 0, it expects an input parameter (filter function)
      // Otherwise, it's a simple generator function
      let result: (string | Choice)[] | Promise<(string | Choice)[]>;
      if (choices.length > 0) {
        // Filter function: (input: string) => choices
        result = (choices as (input: string) => (string | Choice)[] | Promise<(string | Choice)[]>)('');
      } else {
        // Generator function: () => choices
        result = (choices as () => (string | Choice)[] | Promise<(string | Choice)[]>)();
      }
      // Handle both sync and async functions
      if (result instanceof Promise) {
        resolvedChoices = await result;
      } else {
        resolvedChoices = result;
      }
    } catch {
      // If the function fails, fall back to empty choices
      resolvedChoices = [];
    }
  }
  
  // Normalize all choices to {name, value} format
  const normalizedChoices = normalizeChoices(resolvedChoices);
  
  // Process actions: store handlers and create serializable actions
  let serializedActions: SerializableAction[] | undefined;
  if (actions && actions.length > 0) {
    const actionsMap = (globalThis as any).__kitActionsMap as Map<string, Action>;
    actionsMap.clear(); // Clear stale actions from previous prompts

    const seen = new Set<string>();
    const normalized: Action[] = [];

    for (const action of actions) {
      // Default visible=true, skip if explicitly hidden
      if (action.visible === false) continue;

      const name = action.name?.trim();
      if (!name) {
        console.warn('[SDK] Skipping action with empty name');
        continue;
      }

      if (seen.has(name)) {
        console.warn(`[SDK] Duplicate action name "${name}". Skipping duplicate.`);
        continue;
      }
      seen.add(name);

      const hasHandler = typeof action.onAction === 'function';
      const hasValue = action.value !== undefined;

      if (!hasHandler && !hasValue) {
        console.warn(`[SDK] Action "${name}" has no onAction and no value. Skipping.`);
        continue;
      }

      // Store the full Action object for __handleActionTriggered
      actionsMap.set(name, action);
      normalized.push(action);
    }

    if (normalized.length > 0) {
      // Convert to serializable format (without function handlers)
      serializedActions = normalized.map(action => ({
        name: action.name,
        description: action.description,
        shortcut: action.shortcut,
        value: action.value,
        hasAction: typeof action.onAction === 'function',
        visible: action.visible,
        close: action.close,
      }));
    }
  }
  
  // Call onInit callback if provided
  if (config?.onInit) {
    await Promise.resolve(config.onInit());
  }

  // Determine auto-submit value: first choice's value, or empty string
  const autoSubmitValue = normalizedChoices.length > 0
    ? { value: normalizedChoices[0].value }
    : { value: '' };

  return new Promise((resolve) => {
    addPending(id, async (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      const value = msg.value ?? '';

      // Call onSubmit callback if provided
      if (config?.onSubmit) {
        await Promise.resolve(config.onSubmit(value));
      }

      resolve(value);
    }, autoSubmitValue);

    const message: ArgMessage = {
      type: 'arg',
      id,
      placeholder,
      choices: normalizedChoices,
      actions: serializedActions,
    };
    
    send(message);
  });
};

/**
 * Display HTML content to user.
 * 
 * Matches original Script Kit API: div(htmlOrConfig?, actions?)
 * 
 * @param htmlOrConfig - HTML string or DivConfig object
 * @param actions - Optional actions for the actions panel (Cmd+K)
 */
globalThis.div = async function div(
  htmlOrConfig?: string | DivConfig,
  actionsInput?: Action[]
): Promise<string | void> {
  const id = nextId();
  
  // Parse arguments - support both string and config object
  let html: string;
  let config: DivConfig | undefined;
  
  if (typeof htmlOrConfig === 'string') {
    html = htmlOrConfig;
  } else if (typeof htmlOrConfig === 'object' && htmlOrConfig !== null) {
    config = htmlOrConfig;
    html = config.html || '';
  } else {
    html = '';
  }
  
  // Process actions: store handlers and create serializable actions
  let serializedActions: SerializableAction[] | undefined;
  if (actionsInput && actionsInput.length > 0) {
    const actionsMap = (globalThis as any).__kitActionsMap as Map<string, Action>;
    actionsMap.clear();

    const seen = new Set<string>();
    const normalized: Action[] = [];

    for (const action of actionsInput) {
      if (action.visible === false) continue;
      const name = action.name?.trim();
      if (!name) continue;
      if (seen.has(name)) continue;
      seen.add(name);

      const hasHandler = typeof action.onAction === 'function';
      const hasValue = action.value !== undefined;
      if (!hasHandler && !hasValue) continue;

      actionsMap.set(name, action);
      normalized.push(action);
    }

    if (normalized.length > 0) {
      serializedActions = normalized.map(action => ({
        name: action.name,
        description: action.description,
        shortcut: action.shortcut,
        value: action.value,
        hasAction: typeof action.onAction === 'function',
        visible: action.visible,
        close: action.close,
      }));
    }
  }
  
  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      resolve(msg?.value);
    }, undefined); // Auto-submit: div just dismisses, no value needed

    const message: DivMessage = {
      type: 'div',
      id,
      html,
      containerClasses: config?.containerClasses,
      actions: serializedActions,
      placeholder: config?.placeholder,
      hint: config?.hint,
      footer: config?.footer,
      containerBg: config?.containerBg,
      containerPadding: config?.containerPadding,
      opacity: config?.opacity,
    };
    
    send(message);
  });
};

/**
 * Show a confirmation dialog with Yes/No buttons.
 *
 * Returns true if the user confirms, false if they cancel.
 * Keyboard shortcuts: Enter = confirm, Escape = cancel, Tab/Arrow keys = switch buttons.
 *
 * @example
 * ```typescript
 * // Simple usage
 * const confirmed = await confirm("Delete this file?");
 * if (confirmed) {
 *   // proceed with deletion
 * }
 *
 * // With custom button text
 * const proceed = await confirm({
 *   message: "Overwrite existing file?",
 *   confirmText: "Overwrite",
 *   cancelText: "Keep Original"
 * });
 *
 * // Shorthand with custom buttons
 * const yes = await confirm("Continue?", "Yes", "No");
 * ```
 *
 * @param messageOrConfig - Message string or ConfirmConfig object
 * @param confirmText - Optional text for the confirm button (default: "OK")
 * @param cancelText - Optional text for the cancel button (default: "Cancel")
 * @returns Promise resolving to true (confirmed) or false (cancelled)
 */
(globalThis as any).confirm = async function confirm(
  messageOrConfig?: string | ConfirmConfig,
  confirmText?: string,
  cancelText?: string
): Promise<boolean> {
  const id = nextId();

  // Parse arguments to extract message and button text
  let message: string;
  let confirmBtn: string | undefined;
  let cancelBtn: string | undefined;

  if (messageOrConfig === undefined) {
    // confirm() - no arguments, show generic confirmation
    message = 'Are you sure?';
  } else if (typeof messageOrConfig === 'string') {
    // confirm('message') or confirm('message', 'OK', 'Cancel')
    message = messageOrConfig;
    confirmBtn = confirmText;
    cancelBtn = cancelText;
  } else {
    // confirm({ message, confirmText, cancelText })
    message = messageOrConfig.message;
    confirmBtn = messageOrConfig.confirmText;
    cancelBtn = messageOrConfig.cancelText;
  }

  // Auto-submit value: false (user didn't explicitly confirm)
  const autoSubmitValue = { value: 'false' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), treat as cancel
      if (msg.value === null) {
        resolve(false);
        return;
      }
      // Parse the boolean value from the string
      const confirmed = msg.value === 'true';
      resolve(confirmed);
    }, autoSubmitValue);

    const confirmMessage: ConfirmMessage = {
      type: 'confirm',
      id,
      message,
      confirmText: confirmBtn,
      cancelText: cancelBtn,
    };

    send(confirmMessage);
  });
};

globalThis.md = function md(markdown: string): string {
  let html = markdown;

  // 1. Fenced code blocks (must be before inline code)
  // Handle ```lang\ncode\n``` -> <pre><code class="lang">code</code></pre>
  html = html.replace(/```(\w*)\n([\s\S]*?)```/g, (_, lang, code) => {
    const langClass = lang ? ` class="${lang}"` : '';
    return `<pre><code${langClass}>${code.trim()}</code></pre>`;
  });

  // 2. Blockquotes (handle nested > as well)
  // Process line by line to handle multiple > for nesting
  html = html.replace(/^((?:>\s?)+)(.*)$/gm, (_, arrows, content) => {
    const depth = (arrows.match(/>/g) || []).length;
    let result = content.trim();
    for (let i = 0; i < depth; i++) {
      result = `<blockquote>${result}</blockquote>`;
    }
    return result;
  });

  // 3. Headings (h1-h6, process larger numbers first to avoid conflicts)
  html = html.replace(/^###### (.+)$/gm, '<h6>$1</h6>');
  html = html.replace(/^##### (.+)$/gm, '<h5>$1</h5>');
  html = html.replace(/^#### (.+)$/gm, '<h4>$1</h4>');
  html = html.replace(/^### (.+)$/gm, '<h3>$1</h3>');
  html = html.replace(/^## (.+)$/gm, '<h2>$1</h2>');
  html = html.replace(/^# (.+)$/gm, '<h1>$1</h1>');

  // 4. Horizontal rules
  html = html.replace(/^---$/gm, '<hr>');
  html = html.replace(/^\*\*\*$/gm, '<hr>');

  // 5. Images (before links, both use [] syntax)
  html = html.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, '<img alt="$1" src="$2">');

  // 6. Links
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');

  // 7. Ordered lists - use ol-li marker to distinguish from unordered
  html = html.replace(/^\d+\. (.+)$/gm, '<ol-li>$1</ol-li>');

  // 8. Unordered lists (existing) - use ul-li marker
  html = html.replace(/^- (.+)$/gm, '<ul-li>$1</ul-li>');

  // Wrap consecutive ol-li in <ol>
  html = html.replace(/(<ol-li>.*?<\/ol-li>\n?)+/g, (match) => {
    const items = match.replace(/<ol-li>/g, '<li>').replace(/<\/ol-li>/g, '</li>');
    return `<ol>${items}</ol>`;
  });

  // Wrap consecutive ul-li in <ul>
  html = html.replace(/(<ul-li>.*?<\/ul-li>\n?)+/g, (match) => {
    const items = match.replace(/<ul-li>/g, '<li>').replace(/<\/ul-li>/g, '</li>');
    return `<ul>${items}</ul>`;
  });

  // 9. Bold (existing)
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');

  // 10. Italic (existing)
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');

  // 11. Strikethrough
  html = html.replace(/~~(.+?)~~/g, '<del>$1</del>');

  // 12. Inline code (after fenced blocks)
  html = html.replace(/`([^`]+)`/g, '<code>$1</code>');

  // 13. Line breaks (double space at end of line)
  html = html.replace(/  $/gm, '<br>');

  return html;
};

globalThis.editor = async function editor(
  content: string = '',
  language: string = 'text',
  actionsInput?: Action[]
): Promise<string> {
  const id = nextId();

  // Process actions: store handlers and create serializable actions
  let serializedActions: SerializableAction[] | undefined;
  if (actionsInput && actionsInput.length > 0) {
    const actionsMap = (globalThis as any).__kitActionsMap as Map<string, Action>;
    actionsMap.clear();

    const seen = new Set<string>();
    const normalized: Action[] = [];

    for (const action of actionsInput) {
      if (action.visible === false) continue;
      const name = action.name?.trim();
      if (!name) continue;
      if (seen.has(name)) continue;
      seen.add(name);

      const hasHandler = typeof action.onAction === 'function';
      const hasValue = action.value !== undefined;
      if (!hasHandler && !hasValue) continue;

      actionsMap.set(name, action);
      normalized.push(action);
    }

    if (normalized.length > 0) {
      serializedActions = normalized.map(action => ({
        name: action.name,
        description: action.description,
        shortcut: action.shortcut,
        value: action.value,
        hasAction: typeof action.onAction === 'function',
        visible: action.visible,
        close: action.close,
      }));
    }
  }

  // Auto-submit value for editor: return the original content
  const autoSubmitValue = { value: content ?? '' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    }, autoSubmitValue);

    const message: EditorMessage = {
      type: 'editor',
      id,
      content,
      language,
      actions: serializedActions,
    };

    send(message);
  });
};

// NOTE: mini() is not yet implemented - use arg() instead
globalThis.mini = async function mini(
  placeholder: string,
  choices: (string | Choice)[]
): Promise<string> {
  console.warn('[SDK] mini() is not yet implemented in the GPUI app - use arg() instead');
  const id = nextId();

  const normalizedChoices: Choice[] = choices.map((c) => {
    if (typeof c === 'string') {
      return { name: c, value: c };
    }
    return c;
  });

  // Auto-submit value: first choice or empty
  const autoSubmitValue = normalizedChoices.length > 0
    ? { value: normalizedChoices[0].value }
    : { value: '' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    }, autoSubmitValue);

    const message: MiniMessage = {
      type: 'mini',
      id,
      placeholder,
      choices: normalizedChoices,
    };

    send(message);
  });
};

// NOTE: micro() is not yet implemented - use arg() instead
globalThis.micro = async function micro(
  placeholder: string,
  choices: (string | Choice)[]
): Promise<string> {
  console.warn('[SDK] micro() is not yet implemented in the GPUI app - use arg() instead');
  const id = nextId();

  const normalizedChoices: Choice[] = choices.map((c) => {
    if (typeof c === 'string') {
      return { name: c, value: c };
    }
    return c;
  });

  // Auto-submit value: first choice or empty
  const autoSubmitValue = normalizedChoices.length > 0
    ? { value: normalizedChoices[0].value }
    : { value: '' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    }, autoSubmitValue);

    const message: MicroMessage = {
      type: 'micro',
      id,
      placeholder,
      choices: normalizedChoices,
    };

    send(message);
  });
};

globalThis.select = async function select(
  placeholder: string,
  choices: (string | Choice)[]
): Promise<string[]> {
  const id = nextId();

  const normalizedChoices: Choice[] = choices.map((c) => {
    if (typeof c === 'string') {
      return { name: c, value: c };
    }
    return c;
  });

  // Auto-submit value: first choice selected as array
  const autoSubmitValue = normalizedChoices.length > 0
    ? { value: JSON.stringify([normalizedChoices[0].value]) }
    : { value: '[]' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      // Value comes back as JSON array or empty
      const value = msg.value ?? '[]';
      try {
        const parsed = JSON.parse(value);
        resolve(Array.isArray(parsed) ? parsed : []);
      } catch {
        resolve([]);
      }
    }, autoSubmitValue);

    const message: SelectMessage = {
      type: 'select',
      id,
      placeholder,
      choices: normalizedChoices,
      multiple: true,
    };

    send(message);
  });
};

globalThis.fields = async function fields(
  fieldDefs: (string | FieldDef)[],
  actionsInput?: Action[]
): Promise<string[]> {
  const id = nextId();

  const normalizedFields: FieldDef[] = fieldDefs.map((f) => {
    if (typeof f === 'string') {
      return { name: f, label: f };
    }
    return f;
  });

  // Process actions: store handlers and create serializable actions
  let serializedActions: SerializableAction[] | undefined;
  if (actionsInput && actionsInput.length > 0) {
    const actionsMap = (globalThis as any).__kitActionsMap as Map<string, Action>;
    actionsMap.clear();
    const seen = new Set<string>();
    const normalized: Action[] = [];
    for (const action of actionsInput) {
      if (action.visible === false) continue;
      const name = action.name?.trim();
      if (!name || seen.has(name)) continue;
      seen.add(name);
      if (typeof action.onAction !== 'function' && action.value === undefined) continue;
      actionsMap.set(name, action);
      normalized.push(action);
    }
    if (normalized.length > 0) {
      serializedActions = normalized.map(action => ({
        name: action.name,
        description: action.description,
        shortcut: action.shortcut,
        value: action.value,
        hasAction: typeof action.onAction === 'function',
        visible: action.visible,
        close: action.close,
      }));
    }
  }

  // Auto-submit value: array of empty strings matching field count
  const autoSubmitValue = {
    value: JSON.stringify(normalizedFields.map(f => f.value ?? '')),
  };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      // Value comes back as JSON array of field values
      const value = msg.value ?? '[]';
      try {
        const parsed = JSON.parse(value);
        resolve(Array.isArray(parsed) ? parsed : []);
      } catch {
        resolve([]);
      }
    }, autoSubmitValue);

    const message: FieldsMessage = {
      type: 'fields',
      id,
      fields: normalizedFields,
      actions: serializedActions,
    };

    send(message);
  });
};

globalThis.form = async function form(
  html: string,
  actionsInput?: Action[]
): Promise<Record<string, string>> {
  const id = nextId();

  // Process actions: store handlers and create serializable actions
  let serializedActions: SerializableAction[] | undefined;
  if (actionsInput && actionsInput.length > 0) {
    const actionsMap = (globalThis as any).__kitActionsMap as Map<string, Action>;
    actionsMap.clear();
    const seen = new Set<string>();
    const normalized: Action[] = [];
    for (const action of actionsInput) {
      if (action.visible === false) continue;
      const name = action.name?.trim();
      if (!name || seen.has(name)) continue;
      seen.add(name);
      if (typeof action.onAction !== 'function' && action.value === undefined) continue;
      actionsMap.set(name, action);
      normalized.push(action);
    }
    if (normalized.length > 0) {
      serializedActions = normalized.map(action => ({
        name: action.name,
        description: action.description,
        shortcut: action.shortcut,
        value: action.value,
        hasAction: typeof action.onAction === 'function',
        visible: action.visible,
        close: action.close,
      }));
    }
  }

  // Auto-submit value: empty object
  const autoSubmitValue = { value: '{}' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      // Value comes back as JSON object with field names as keys
      const value = msg.value ?? '{}';
      try {
        const parsed = JSON.parse(value);
        resolve(typeof parsed === 'object' && parsed !== null ? parsed : {});
      } catch {
        resolve({});
      }
    }, autoSubmitValue);

    const message: FormMessage = {
      type: 'form',
      id,
      html,
      actions: serializedActions,
    };

    send(message);
  });
};

globalThis.path = async function path(
  options?: PathOptions
): Promise<string> {
  const id = nextId();

  // Auto-submit value: mock path for testing
  const autoSubmitValue = { value: options?.startPath || '/tmp/test-selected-file.txt' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    }, autoSubmitValue);

    const message: PathMessage = {
      type: 'path',
      id,
      startPath: options?.startPath,
      hint: options?.hint,
    };

    send(message);
  });
};

// NOTE: hotkey() is not yet implemented
globalThis.hotkey = async function hotkey(
  placeholder?: string
): Promise<HotkeyInfo> {
  console.warn('[SDK] hotkey() is not yet implemented in the GPUI app');
  const id = nextId();

  // Auto-submit value: mock hotkey (Escape key)
  const autoSubmitValue = {
    value: JSON.stringify({
      key: 'Escape',
      command: false,
      shift: false,
      option: false,
      control: false,
      shortcut: 'Escape',
      keyCode: 'Escape',
    }),
  };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      // Value comes back as JSON with hotkey info
      const value = msg.value ?? '{}';
      try {
        const parsed = JSON.parse(value);
        resolve({
          key: parsed.key ?? '',
          command: parsed.command ?? false,
          shift: parsed.shift ?? false,
          option: parsed.option ?? false,
          control: parsed.control ?? false,
          shortcut: parsed.shortcut ?? '',
          keyCode: parsed.keyCode ?? '',
        });
      } catch {
        resolve({
          key: '',
          command: false,
          shift: false,
          option: false,
          control: false,
          shortcut: '',
          keyCode: '',
        });
      }
    }, autoSubmitValue);

    const message: HotkeyMessage = {
      type: 'hotkey',
      id,
      placeholder,
    };

    send(message);
  });
};

globalThis.drop = async function drop(): Promise<FileInfo[]> {
  const id = nextId();

  // Auto-submit value: empty file array
  const autoSubmitValue = { value: '[]' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      // Value comes back as JSON array of file info
      const value = msg.value ?? '[]';
      try {
        const parsed = JSON.parse(value);
        if (Array.isArray(parsed)) {
          resolve(parsed.map((f: { path?: string; name?: string; size?: number }) => ({
            path: f.path ?? '',
            name: f.name ?? '',
            size: f.size ?? 0,
          })));
        } else {
          resolve([]);
        }
      } catch {
        resolve([]);
      }
    }, autoSubmitValue);

    const message: DropMessage = {
      type: 'drop',
      id,
    };

    send(message);
  });
};

/**
 * Template prompt with VSCode snippet tabstops
 * 
 * @param templateStr - Template string with VSCode snippet syntax:
 *   - $1, $2, $3 - Simple tabstops (Tab to navigate)
 *   - ${1:default} - Tabstop with placeholder
 *   - ${1|a,b,c|} - Choice tabstop
 *   - $0 - Final cursor position
 *   - $$ - Escaped dollar sign
 *   - $SELECTION - Currently selected text (calls getSelectedText())
 *   - $CLIPBOARD - Clipboard contents
 *   - $HOME - User's home directory
 * @param options - Editor options (language, etc.)
 * @returns Promise<string> - Final edited content
 */
globalThis.template = async function template(
  templateStr: string,
  options: { language?: string } = {}
): Promise<string> {
  let processed = templateStr;
  
  // Preprocess special variables
  if (processed.includes('$SELECTION')) {
    const selection = await getSelectedText();
    processed = processed.replaceAll('$SELECTION', selection || '');
  }
  if (processed.includes('$CLIPBOARD')) {
    const clip = await globalThis.clipboard.readText();
    processed = processed.replaceAll('$CLIPBOARD', clip || '');
  }
  if (processed.includes('$HOME')) {
    processed = processed.replaceAll('$HOME', process.env.HOME || '');
  }
  
  const id = nextId();

  // Auto-submit value: return the processed template
  const autoSubmitValue = { value: processed };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      if (msg.value === null) {
        process.exit(0);  // Escape pressed
      }
      resolve(msg.value ?? '');
    }, autoSubmitValue);
    send({
      type: 'editor',
      id,
      template: processed,
      language: options.language || 'plaintext',
    });
  });
};

globalThis.env = async function env(
  key: string,
  promptFn?: () => Promise<string>
): Promise<string> {
  // First check if the env var is already set
  const existingValue = process.env[key];
  if (existingValue !== undefined && existingValue !== '') {
    return existingValue;
  }

  // If a prompt function is provided, use it to get the value
  if (promptFn) {
    const value = await promptFn();
    process.env[key] = value;
    return value;
  }

  // Otherwise, send a message to GPUI to prompt for the value
  const id = nextId();

  // Auto-submit value: empty string (user would type something)
  const autoSubmitValue = { value: '' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      const value = msg.value ?? '';
      process.env[key] = value;
      resolve(value);
    }, autoSubmitValue);

    const message: EnvMessage = {
      type: 'env',
      id,
      key,
      secret: key.toLowerCase().includes('secret') ||
              key.toLowerCase().includes('password') ||
              key.toLowerCase().includes('token') ||
              key.toLowerCase().includes('key'),
    };

    send(message);
  });
};

// =============================================================================
// TIER 3: System APIs (alerts, clipboard, keyboard, mouse)
// =============================================================================

// Fire-and-forget messages - send and resolve immediately (no response needed)
// NOTE: These functions send messages to the app but handlers are not yet implemented
globalThis.beep = async function beep(): Promise<void> {
  console.warn('[SDK] beep() is not yet implemented in the GPUI app');
  const message: BeepMessage = { type: 'beep' };
  send(message);
};

globalThis.say = async function say(text: string, voice?: string): Promise<void> {
  console.warn('[SDK] say() is not yet implemented in the GPUI app');
  const message: SayMessage = { type: 'say', text, voice };
  send(message);
};

globalThis.notify = async function notify(options: string | NotifyOptions): Promise<void> {
  console.warn('[SDK] notify() is not yet implemented in the GPUI app (use hud() instead)');
  const message: NotifyMessage = typeof options === 'string'
    ? { type: 'notify', body: options }
    : { type: 'notify', title: options.title, body: options.body };
  send(message);
};

/**
 * Show a brief HUD notification at bottom-center of screen.
 * Fire-and-forget - no response needed.
 */
globalThis.hud = function hud(message: string, options?: { duration?: number }): void {
  const hudMessage: HudMessage = {
    type: 'hud',
    text: message,
    duration_ms: options?.duration,
  };
  send(hudMessage);
};

globalThis.setStatus = async function setStatus(options: StatusOptions): Promise<void> {
  console.warn('[SDK] setStatus() is not yet implemented in the GPUI app');
  const message: SetStatusMessage = {
    type: 'setStatus',
    status: options.status,
    message: options.message,
  };
  send(message);
};

globalThis.menu = async function menu(icon: string, scripts?: string[]): Promise<void> {
  console.warn('[SDK] menu() is not yet implemented in the GPUI app');
  const message: MenuMessage = { type: 'menu', icon, scripts };
  send(message);
};

// =============================================================================
// Actions API
// =============================================================================

/**
 * Set the available actions for the current prompt.
 * Actions appear in the actions panel and can have keyboard shortcuts.
 * 
 * @param actions - Array of action definitions
 * 
 * @example
 * ```typescript
 * await setActions([
 *   {
 *     name: 'copy',
 *     description: 'Copy to clipboard',
 *     shortcut: 'cmd+c',
 *     onAction: async (input, state) => {
 *       await copy(input);
 *       hud('Copied!');
 *     },
 *   },
 *   {
 *     name: 'paste',
 *     shortcut: 'cmd+v',
 *     value: 'paste', // Will be submitted if no onAction
 *   },
 * ]);
 * ```
 */
globalThis.setActions = async function setActions(actions: Action[]): Promise<void> {
  const actionsMap = (globalThis as any).__kitActionsMap as Map<string, Action>;
  
  // Clear previous actions
  actionsMap.clear();
  
  // Store actions with handlers
  for (const action of actions) {
    actionsMap.set(action.name, action);
  }
  
  // Send to Rust (strip onAction function, add hasAction boolean)
  const serializable: SerializableAction[] = actions.map(a => ({
    name: a.name,
    description: a.description,
    shortcut: a.shortcut,
    value: a.value,
    hasAction: typeof a.onAction === 'function',
    visible: a.visible,
    close: a.close,
  }));
  
  const message: SetActionsMessage = {
    type: 'setActions',
    actions: serializable,
  };
  
  send(message);
};

/**
 * Set the current prompt's input text.
 * @param text - Input text to apply
 */
globalThis.setInput = function setInput(text: string): void {
  const message: SetInputMessage = {
    type: 'setInput',
    text,
  };
  send(message);
};

/**
 * Replace the currently selected text in the focused application.
 * Uses macOS Accessibility APIs for reliability (95%+ of apps).
 * Falls back to clipboard simulation for apps that block accessibility.
 * 
 * @param text - The text to insert (replaces selection)
 * @throws If accessibility permission not granted
 * @throws If paste operation fails
 */
globalThis.setSelectedText = async function setSelectedText(text: string): Promise<void> {
  // Hide the window so the previous app regains focus before we simulate Cmd+V.
  await hide();

  // Brief delay to ensure focus has transferred to the previous app.
  await new Promise(r => setTimeout(r, 20));

  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: SubmitMessage) => {
      // Check if there was an error
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' }); // Auto-submit: success

    const message: SetSelectedTextMessage = { type: 'setSelectedText', requestId: id, text };
    send(message);
  });
};

/**
 * Get the currently selected text from the focused application.
 * Uses macOS Accessibility APIs for reliability (95%+ of apps).
 * Falls back to clipboard simulation for apps that block accessibility.
 *
 * @returns The selected text, or empty string if nothing selected
 * @throws If accessibility permission not granted
 */
globalThis.getSelectedText = async function getSelectedText(): Promise<string> {
  bench('getSelectedText_start');
  
  // Hide the window so the previous app regains focus
  // This is required for the AX API to read text from the focused app
  await hide();
  bench('getSelectedText_hide_done');
  
  // Brief delay to ensure focus has transferred to the previous app
  // 20ms is typically sufficient (reduced from original 50ms)
  await new Promise(r => setTimeout(r, 20));
  bench('getSelectedText_delay_done');

  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: SubmitMessage) => {
      bench('getSelectedText_response');
      // Check if there was an error
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve(msg.value ?? '');
      }
    }, { value: '' }); // Auto-submit: empty selection

    const message: GetSelectedTextMessage = { type: 'getSelectedText', requestId: id };
    bench('getSelectedText_request_sent');
    send(message);
  });
};

/**
 * Check if accessibility permission is granted.
 * Required for getSelectedText and setSelectedText to work reliably.
 *
 * @returns true if permission granted, false otherwise
 */
globalThis.hasAccessibilityPermission = async function hasAccessibilityPermission(): Promise<boolean> {
  const id = nextId();

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      resolve(msg.value === 'true');
    }, { value: 'true' }); // Auto-submit: permission granted

    const message: CheckAccessibilityMessage = { type: 'checkAccessibility', requestId: id };
    send(message);
  });
};

/**
 * Request accessibility permission (opens System Preferences).
 * User must manually grant permission in System Preferences > Privacy & Security > Accessibility.
 *
 * @returns true if permission was granted after request, false otherwise
 */
globalThis.requestAccessibilityPermission = async function requestAccessibilityPermission(): Promise<boolean> {
  const id = nextId();

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      resolve(msg.value === 'true');
    }, { value: 'true' }); // Auto-submit: permission granted

    const message: RequestAccessibilityMessage = { type: 'requestAccessibility', requestId: id };
    send(message);
  });
};

// Clipboard API object
globalThis.clipboard = {
  async readText(): Promise<string> {
    const id = nextId();

    return new Promise((resolve) => {
      addPending(id, (msg: SubmitMessage) => {
        resolve(msg.value ?? '');
      }, { value: '' }); // Auto-submit: empty clipboard

      const message: ClipboardMessage = {
        type: 'clipboard',
        id,
        action: 'read',
        format: 'text',
      };
      send(message);
    });
  },

  async writeText(text: string): Promise<void> {
    const id = nextId();

    return new Promise((resolve) => {
      addPending(id, () => {
        resolve();
      }, undefined); // Auto-submit: void return

      const message: ClipboardMessage = {
        type: 'clipboard',
        id,
        action: 'write',
        format: 'text',
        content: text,
      };
      send(message);
    });
  },

  async readImage(): Promise<Buffer> {
    const id = nextId();

    return new Promise((resolve) => {
      addPending(id, (msg: SubmitMessage) => {
        // Value comes back as base64-encoded string
        const base64 = msg.value ?? '';
        resolve(Buffer.from(base64, 'base64'));
      }, { value: '' }); // Auto-submit: empty image

      const message: ClipboardMessage = {
        type: 'clipboard',
        id,
        action: 'read',
        format: 'image',
      };
      send(message);
    });
  },

  async writeImage(buffer: Buffer): Promise<void> {
    const id = nextId();

    return new Promise((resolve) => {
      addPending(id, () => {
        resolve();
      }, undefined); // Auto-submit: void return

      const message: ClipboardMessage = {
        type: 'clipboard',
        id,
        action: 'write',
        format: 'image',
        content: buffer.toString('base64'),
      };
      send(message);
    });
  },
};

// Clipboard aliases
globalThis.copy = async function copy(text: string): Promise<void> {
  return globalThis.clipboard.writeText(text);
};

globalThis.paste = async function paste(): Promise<string> {
  return globalThis.clipboard.readText();
};

// Keyboard API object
// NOTE: These functions send messages to the app but handlers are not yet implemented
globalThis.keyboard = {
  async type(text: string): Promise<void> {
    console.warn('[SDK] keyboard.type() is not yet implemented in the GPUI app');
    const message: KeyboardMessage = {
      type: 'keyboard',
      action: 'type',
      text,
    };
    send(message);
  },

  async tap(...keys: string[]): Promise<void> {
    console.warn('[SDK] keyboard.tap() is not yet implemented in the GPUI app');
    const message: KeyboardMessage = {
      type: 'keyboard',
      action: 'tap',
      keys,
    };
    send(message);
  },
};

// Mouse API object
// NOTE: These functions send messages to the app but handlers are not yet implemented
globalThis.mouse = {
  async move(positions: Position[]): Promise<void> {
    console.warn('[SDK] mouse.move() is not yet implemented in the GPUI app');
    const message: MouseMessage = {
      type: 'mouse',
      action: 'move',
      positions,
    };
    send(message);
  },

  async leftClick(): Promise<void> {
    console.warn('[SDK] mouse.leftClick() is not yet implemented in the GPUI app');
    const message: MouseMessage = {
      type: 'mouse',
      action: 'click',
      button: 'left',
    };
    send(message);
  },

  async rightClick(): Promise<void> {
    console.warn('[SDK] mouse.rightClick() is not yet implemented in the GPUI app');
    const message: MouseMessage = {
      type: 'mouse',
      action: 'click',
      button: 'right',
    };
    send(message);
  },

  async setPosition(position: Position): Promise<void> {
    console.warn('[SDK] mouse.setPosition() is not yet implemented in the GPUI app');
    const message: MouseMessage = {
      type: 'mouse',
      action: 'setPosition',
      position,
    };
    send(message);
  },
};

// =============================================================================
// TIER 4A: Chat Prompt
// =============================================================================

// Current active chat session ID (for controller methods)
let currentChatId: string | null = null;
// Message ID counter for streaming
let chatMessageIdCounter = 0;

// Type for chat function with controller methods
interface ChatFunction {
  (options?: ChatOptions): Promise<ChatResult>;
  addMessage(msg: ChatMessage | CoreMessage): void;
  startStream(position?: 'left' | 'right'): string;
  appendChunk(messageId: string, chunk: string): void;
  completeStream(messageId: string): void;
  clear(): void;
  /** Set an error on a message (typically during streaming failure) */
  setError(messageId: string, error: string): void;
  /** Clear error from a message (before retry) */
  clearError(messageId: string): void;
  /** Get messages in AI SDK CoreMessage format */
  getMessages(): CoreMessage[];
  /** Get full chat result including metadata */
  getResult(): ChatResult;
}

/** Convert CoreMessage or ChatMessage to normalized ChatMessage */
function normalizeMessage(msg: ChatMessage | CoreMessage, index: number): ChatMessage {
  // If it's a CoreMessage (has role and content but no text or position)
  if ('role' in msg && 'content' in msg && !('text' in msg)) {
    const coreMsg = msg as CoreMessage;
    return {
      id: `msg-${index}`,
      role: coreMsg.role,
      content: coreMsg.content,
      text: coreMsg.content,
      position: coreMsg.role === 'user' ? 'right' : 'left',
      createdAt: new Date().toISOString(),
    };
  }
  // It's already a ChatMessage
  const chatMsg = msg as ChatMessage;
  return {
    ...chatMsg,
    id: chatMsg.id || `msg-${index}`,
    // Ensure text and content are in sync
    text: chatMsg.text || chatMsg.content || '',
    content: chatMsg.content || chatMsg.text || '',
    // Derive position from role if not set
    position: chatMsg.position || (chatMsg.role === 'user' ? 'right' : 'left'),
  };
}

/** Convert ChatMessage to CoreMessage for AI SDK compat */
function toCoreMessage(msg: ChatMessage): CoreMessage {
  const role = msg.role || (msg.position === 'right' ? 'user' : 'assistant');
  return {
    role,
    content: msg.content || msg.text || '',
  };
}

// Track messages in the current chat session
let chatMessages: ChatMessage[] = [];

// Store conversation ID and model for result
let currentConversationId: string | undefined;
let currentModel: string | undefined;

// Helper to build ChatResult
function buildChatResult(action: 'escape' | 'continue'): ChatResult {
  const userMsgs = chatMessages.filter((m) => m.role === 'user' || m.position === 'right');
  const assistantMsgs = chatMessages.filter((m) => m.role === 'assistant' || (m.position === 'left' && m.role !== 'system'));

  return {
    messages: chatMessages.map(toCoreMessage),
    uiMessages: chatMessages,
    lastUserMessage: userMsgs[userMsgs.length - 1]?.content || userMsgs[userMsgs.length - 1]?.text || '',
    lastAssistantMessage: assistantMsgs[assistantMsgs.length - 1]?.content || assistantMsgs[assistantMsgs.length - 1]?.text || '',
    model: currentModel,
    action,
    conversationId: currentConversationId,
  };
}

// The chat function with attached controller methods
const chatFn: ChatFunction = async function chat(options?: ChatOptions): Promise<ChatResult> {
  bench('chat_function_start');
  const id = nextId();
  currentChatId = id;
  currentConversationId = `conv-${id}`;
  currentModel = options?.model;
  chatMessages = [];

  // Build initial messages with IDs and normalize format
  const inputMessages = options?.messages || [];

  // If system prompt shorthand is provided, prepend it
  if (options?.system) {
    inputMessages.unshift({ role: 'system' as const, content: options.system });
  }

  const initialMessages = inputMessages.map((msg, i) => normalizeMessage(msg, i));
  chatMessages = [...initialMessages];

  // Determine if we should use built-in AI mode
  // When no onInit/onMessage callbacks are provided, the app handles AI calls
  const useBuiltinAi = !options?.onInit && !options?.onMessage;

  // Send the initial chat message to open the UI
  bench('chat_building_message');
  const message: ChatMessageType = {
    type: 'chat',
    id,
    placeholder: options?.placeholder,
    messages: initialMessages,
    hint: options?.hint,
    footer: options?.footer,
    actions: options?.actions,
    model: options?.model,
    models: options?.models,
    saveHistory: options?.saveHistory ?? true,
    useBuiltinAi,
  };
  bench('chat_sending_message');
  send(message);

  // IMPORTANT: Ref stdin BEFORE onInit to prevent process from exiting
  // while onInit runs async work. addPending() will manage ref counting after.
  (process.stdin as any).ref?.();

  // Call onInit if provided (allows script to add initial messages)
  if (options?.onInit) {
    await options.onInit();
  }

  // If onMessage is provided, set up a loop to handle user messages
  if (options?.onMessage) {
    return new Promise((resolve) => {
      const handleMessage = async (msg: { type: string; id: string; text: string }) => {
        if (msg.type === 'chatSubmit' && msg.id === id) {
          // If user pressed Escape (text is empty or undefined), return with escape action
          if (!msg.text) {
            currentChatId = null;
            resolve(buildChatResult('escape'));
            return;
          }

          // Track the user message
          const userMsg: ChatMessage = {
            id: `user-${chatMessageIdCounter++}`,
            role: 'user',
            content: msg.text,
            text: msg.text,
            position: 'right',
            createdAt: new Date().toISOString(),
          };
          chatMessages.push(userMsg);

          // Call onMessage callback
          await options.onMessage!(msg.text);

          // Continue listening for more messages
        }
      };

      // Add handler for chatSubmit messages
      addPending(id, (msg: SubmitMessage) => {
        currentChatId = null;
        if (msg.value === null) {
          resolve(buildChatResult('escape'));
        } else {
          resolve(buildChatResult('continue'));
        }
      }, { value: '' });

      process.on('chatSubmit' as any, handleMessage);
    });
  }

  // Simple mode: wait for single submission
  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      currentChatId = null;
      if (msg.value === null) {
        resolve(buildChatResult('escape'));
      } else {
        resolve(buildChatResult('continue'));
      }
    }, { value: '' });
  });
};

// Controller method: Add a message to the chat
chatFn.addMessage = function addMessage(msg: ChatMessage | CoreMessage): void {
  if (currentChatId === null) {
    throw new Error('chat.addMessage() called outside of a chat session');
  }

  // Normalize the message
  const normalized = normalizeMessage(msg, chatMessageIdCounter++);
  chatMessages.push(normalized);

  const message: ChatAddMessageType = {
    type: 'chatMessage',
    id: currentChatId,
    message: normalized,
  };
  send(message);
};

// Controller method: Start streaming a message
chatFn.startStream = function startStream(position: 'left' | 'right' = 'left'): string {
  if (currentChatId === null) {
    throw new Error('chat.startStream() called outside of a chat session');
  }

  const messageId = `stream-${chatMessageIdCounter++}`;

  // Track the streaming message
  const streamMsg: ChatMessage = {
    id: messageId,
    role: position === 'right' ? 'user' : 'assistant',
    content: '',
    text: '',
    position,
    streaming: true,
    createdAt: new Date().toISOString(),
  };
  chatMessages.push(streamMsg);

  const message: ChatStreamStartType = {
    type: 'chatStreamStart',
    id: currentChatId,
    messageId,
    position,
  };
  send(message);
  return messageId;
};

// Controller method: Append chunk to streaming message
chatFn.appendChunk = function appendChunk(messageId: string, chunk: string): void {
  if (currentChatId === null) {
    throw new Error('chat.appendChunk() called outside of a chat session');
  }

  // Update the tracked message
  const msg = chatMessages.find((m) => m.id === messageId);
  if (msg) {
    msg.content = (msg.content || '') + chunk;
    msg.text = (msg.text || '') + chunk;
  }

  const message: ChatStreamChunkType = {
    type: 'chatStreamChunk',
    id: currentChatId,
    messageId,
    chunk,
  };
  send(message);
};

// Controller method: Complete streaming for a message
chatFn.completeStream = function completeStream(messageId: string): void {
  if (currentChatId === null) {
    throw new Error('chat.completeStream() called outside of a chat session');
  }

  // Update the tracked message
  const msg = chatMessages.find((m) => m.id === messageId);
  if (msg) {
    msg.streaming = false;
  }

  const message: ChatStreamCompleteType = {
    type: 'chatStreamComplete',
    id: currentChatId,
    messageId,
  };
  send(message);
};

// Controller method: Clear all messages
chatFn.clear = function clear(): void {
  if (currentChatId === null) {
    throw new Error('chat.clear() called outside of a chat session');
  }

  // Clear tracked messages
  chatMessages = [];

  const message: ChatClearType = {
    type: 'chatClear',
    id: currentChatId,
  };
  send(message);
};

// Controller method: Set error on a message (typically during streaming failure)
chatFn.setError = function setError(messageId: string, error: string): void {
  if (currentChatId === null) {
    throw new Error('chat.setError() called outside of a chat session');
  }

  // Update the tracked message
  const msg = chatMessages.find((m) => m.id === messageId);
  if (msg) {
    msg.error = error;
    msg.streaming = false;
  }

  const message: ChatSetErrorType = {
    type: 'chatSetError',
    id: currentChatId,
    messageId,
    error,
  };
  send(message);
};

// Controller method: Clear error from a message (before retry)
chatFn.clearError = function clearError(messageId: string): void {
  if (currentChatId === null) {
    throw new Error('chat.clearError() called outside of a chat session');
  }

  // Update the tracked message
  const msg = chatMessages.find((m) => m.id === messageId);
  if (msg) {
    msg.error = undefined;
  }

  const message: ChatClearErrorType = {
    type: 'chatClearError',
    id: currentChatId,
    messageId,
  };
  send(message);
};

// Helper: Get messages as AI SDK CoreMessages
chatFn.getMessages = function getMessages(): CoreMessage[] {
  return chatMessages.map(toCoreMessage);
};

// Helper: Get full chat result
chatFn.getResult = function getResult(): ChatResult {
  const userMsgs = chatMessages.filter((m) => m.role === 'user' || m.position === 'right');
  const assistantMsgs = chatMessages.filter((m) => m.role === 'assistant' || (m.position === 'left' && m.role !== 'system'));

  return {
    messages: chatMessages.map(toCoreMessage),
    uiMessages: chatMessages,
    lastUserMessage: userMsgs[userMsgs.length - 1]?.content || userMsgs[userMsgs.length - 1]?.text || '',
    lastAssistantMessage: assistantMsgs[assistantMsgs.length - 1]?.content || assistantMsgs[assistantMsgs.length - 1]?.text || '',
    model: assistantMsgs[assistantMsgs.length - 1]?.model,
    action: 'escape',
  };
};

// Expose as global
(globalThis as unknown as { chat: ChatFunction }).chat = chatFn;

// =============================================================================
// TIER 4B: Widget/Term/Media Prompts
// =============================================================================

// Store widget event handlers by widget ID
const widgetHandlers = new Map<string, {
  onClick?: (event: WidgetEvent) => void;
  onInput?: (event: WidgetInputEvent) => void;
  onClose?: () => void;
  onMoved?: (pos: { x: number; y: number }) => void;
  onResized?: (size: { width: number; height: number }) => void;
}>();

// Widget event handler - listens to custom widgetEvent from stdin handler
function handleWidgetEvent(msg: { id: string; event: string; data?: unknown }) {
  if (widgetHandlers.has(msg.id)) {
    const handlers = widgetHandlers.get(msg.id);
    if (handlers) {
      switch (msg.event) {
        case 'click':
          handlers.onClick?.(msg.data as WidgetEvent);
          break;
        case 'input':
          handlers.onInput?.(msg.data as WidgetInputEvent);
          break;
        case 'close':
          handlers.onClose?.();
          widgetHandlers.delete(msg.id);
          break;
        case 'resized':
          handlers.onResized?.(msg.data as { width: number; height: number });
          break;
      }
    }
  }
}

// Register widget event handler with the stdin message handler
process.on('widgetEvent' as any, handleWidgetEvent);

// NOTE: widget() is not yet implemented
globalThis.widget = async function widget(
  html: string,
  options?: WidgetOptions
): Promise<WidgetController> {
  console.warn('[SDK] widget() is not yet implemented in the GPUI app');
  const id = nextId();

  // Initialize handlers for this widget
  widgetHandlers.set(id, {});

  // Send widget creation message
  const message: WidgetMessage = {
    type: 'widget',
    id,
    html,
    options,
  };
  send(message);

  // Return controller object
  const controller: WidgetController = {
    setState(state: Record<string, unknown>): void {
      const actionMessage: WidgetActionMessage = {
        type: 'widgetAction',
        id,
        action: 'setState',
        state,
      };
      send(actionMessage);
    },

    onClick(handler: (event: WidgetEvent) => void): void {
      const handlers = widgetHandlers.get(id);
      if (handlers) {
        handlers.onClick = handler;
      }
    },

    onInput(handler: (event: WidgetInputEvent) => void): void {
      const handlers = widgetHandlers.get(id);
      if (handlers) {
        handlers.onInput = handler;
      }
    },

    onClose(handler: () => void): void {
      const handlers = widgetHandlers.get(id);
      if (handlers) {
        handlers.onClose = handler;
      }
    },

    onMoved(handler: (pos: { x: number; y: number }) => void): void {
      const handlers = widgetHandlers.get(id);
      if (handlers) {
        handlers.onMoved = handler;
      }
    },

    onResized(handler: (size: { width: number; height: number }) => void): void {
      const handlers = widgetHandlers.get(id);
      if (handlers) {
        handlers.onResized = handler;
      }
    },

    close(): void {
      const actionMessage: WidgetActionMessage = {
        type: 'widgetAction',
        id,
        action: 'close',
      };
      send(actionMessage);
      widgetHandlers.delete(id);
    },
  };

  return controller;
};

globalThis.term = async function term(command?: string, actionsInput?: Action[]): Promise<string> {
  const id = nextId();

  // Process actions: store handlers and create serializable actions
  let serializedActions: SerializableAction[] | undefined;
  if (actionsInput && actionsInput.length > 0) {
    const actionsMap = (globalThis as any).__kitActionsMap as Map<string, Action>;
    actionsMap.clear();
    const seen = new Set<string>();
    const normalized: Action[] = [];
    for (const action of actionsInput) {
      if (action.visible === false) continue;
      const name = action.name?.trim();
      if (!name || seen.has(name)) continue;
      seen.add(name);
      if (typeof action.onAction !== 'function' && action.value === undefined) continue;
      actionsMap.set(name, action);
      normalized.push(action);
    }
    if (normalized.length > 0) {
      serializedActions = normalized.map(action => ({
        name: action.name,
        description: action.description,
        shortcut: action.shortcut,
        value: action.value,
        hasAction: typeof action.onAction === 'function',
        visible: action.visible,
        close: action.close,
      }));
    }
  }

  // Auto-submit: empty string
  const autoSubmitValue = { value: '' };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    }, autoSubmitValue);

    const message: TermMessage = {
      type: 'term',
      id,
      command,
      actions: serializedActions,
    };

    send(message);
  });
};

// REMOVED: webcam() is not feasible in the message-passing architecture
// Media streaming requires complex real-time communication beyond the JSONL protocol
globalThis.webcam = async function webcam(): Promise<Buffer> {
  throw new Error(
    'webcam() is not implemented in Script Kit GPUI. ' +
    'Media streaming is not feasible with the message-passing architecture. ' +
    'Consider using external tools or the camera via system APIs.'
  );
};

// REMOVED: mic() is not feasible in the message-passing architecture
// Audio streaming requires complex real-time communication beyond the JSONL protocol
globalThis.mic = async function mic(): Promise<Buffer> {
  throw new Error(
    'mic() is not implemented in Script Kit GPUI. ' +
    'Audio streaming is not feasible with the message-passing architecture. ' +
    'Consider using external tools or audio capture via system APIs.'
  );
};

// REMOVED: eyeDropper() is not yet implemented
// Could potentially be implemented via screen capture + pixel reading
globalThis.eyeDropper = async function eyeDropper(): Promise<ColorInfo> {
  throw new Error(
    'eyeDropper() is not implemented in Script Kit GPUI. ' +
    'Consider using macOS system color picker or external color tools.'
  );
};

globalThis.find = async function find(
  placeholder: string,
  options?: FindOptions
): Promise<string> {
  const id = nextId();

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    }, { value: '' }); // Auto-submit: empty string

    const message: FindMessage = {
      type: 'find',
      id,
      placeholder,
      onlyin: options?.onlyin,
    };

    send(message);
  });
};

// =============================================================================
// TIER 5A: Window Control Functions
// =============================================================================

// Window Control (fire-and-forget)
globalThis.show = async function show(): Promise<void> {
  const message: ShowMessage = { type: 'show' };
  send(message);
};

globalThis.hide = async function hide(): Promise<void> {
  const message: HideMessage = { type: 'hide' };
  send(message);
};

/**
 * Show the debug grid overlay for visual testing
 */
globalThis.showGrid = async function showGrid(options?: GridOptions): Promise<void> {
  const message: ShowGridMessage = { 
    type: 'showGrid',
    ...options
  };
  send(message);
};

/**
 * Hide the debug grid overlay
 */
globalThis.hideGrid = async function hideGrid(): Promise<void> {
  const message: HideGridMessage = { type: 'hideGrid' };
  send(message);
};

globalThis.blur = async function blur(): Promise<void> {
  const message: BlurMessage = { type: 'blur' };
  send(message);
};

/**
 * Get the current window bounds (position and size).
 * Useful for testing window resize behavior and layout verification.
 * 
 * @returns Window bounds with x, y, width, height in pixels
 */
globalThis.getWindowBounds = async function getWindowBounds(): Promise<WindowBounds> {
  const id = nextId();

  // Auto-submit: mock window bounds
  const autoSubmitValue = {
    value: JSON.stringify({ x: 100, y: 100, width: 800, height: 600 }),
  };

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      // Value comes back as JSON with window bounds
      const value = msg.value ?? '{}';
      try {
        const parsed = JSON.parse(value);
        resolve({
          x: parsed.x ?? 0,
          y: parsed.y ?? 0,
          width: parsed.width ?? 0,
          height: parsed.height ?? 0,
        });
      } catch {
        resolve({
          x: 0,
          y: 0,
          width: 0,
          height: 0,
        });
      }
    }, autoSubmitValue);

    const message: GetWindowBoundsMessage = {
      type: 'getWindowBounds',
      requestId: id,
    };

    send(message);
  });
};

/**
 * Capture a screenshot of the Script Kit window.
 * Useful for visual testing and debugging layout issues.
 * 
 * @param options - Screenshot options
 * @param options.hiDpi - If true, capture at full retina resolution (2x). Default false for 1x.
 * @returns Promise with base64-encoded PNG data and dimensions
 */
globalThis.captureScreenshot = async function captureScreenshot(
  options?: ScreenshotOptions
): Promise<ScreenshotData> {
  const requestId = nextId();
  const hiDpi = options?.hiDpi ?? false;

  // Deterministic placeholder screenshot for auto-submit mode so CI can
  // exercise screenshot flows without depending on a live window capture.
  const mockScreenshotResult = {
    type: 'screenshotResult',
    data: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5c8AAAAASUVORK5CYII=',
    width: hiDpi ? 1600 : 800,
    height: hiDpi ? 1200 : 600,
  };

  return new Promise((resolve, reject) => {
    addPending(requestId, (msg: ResponseMessage) => {
      // Handle screenshotResult message type
      if (msg.type === 'screenshotResult') {
        const resultMsg = msg as ScreenshotResultMessage;

        if (resultMsg.error) {
          reject(new Error(resultMsg.error));
          return;
        }

        if (!resultMsg.data || resultMsg.width <= 0 || resultMsg.height <= 0) {
          reject(new Error('Screenshot capture returned an empty PNG payload'));
          return;
        }

        resolve({
          data: resultMsg.data,
          width: resultMsg.width,
          height: resultMsg.height,
        });
        return;
      }

      // Fallback for unexpected message type (or auto-submit)
      resolve({
        data: (msg as any).data ?? '',
        width: (msg as any).width ?? 0,
        height: (msg as any).height ?? 0,
      });
    }, mockScreenshotResult);

    const message: CaptureScreenshotMessage = {
      type: 'captureScreenshot',
      requestId,
      hiDpi,
    };
    
    send(message);
  });
};

/**
 * Get detailed layout information for the current UI state.
 * 
 * This returns comprehensive component information including:
 * - Bounds (position and size)
 * - Box model (padding, margin, gap)
 * - Flex properties (direction, grow, align)
 * - Human-readable explanations of why components are sized as they are
 * 
 * Designed for AI agents to understand "why" components are positioned/sized.
 * 
 * @example
 * ```typescript
 * const layout = await getLayoutInfo();
 * console.log('Window:', layout.windowWidth, 'x', layout.windowHeight);
 * console.log('Prompt type:', layout.promptType);
 * 
 * // Find the header and understand its layout
 * const header = layout.components.find(c => c.name === 'Header');
 * if (header) {
 *   console.log('Header bounds:', header.bounds);
 *   console.log('Why this size:', header.explanation);
 * }
 * ```
 * 
 * @returns LayoutInfo with component tree and window information
 */
globalThis.getLayoutInfo = async function getLayoutInfo(): Promise<LayoutInfo> {
  const requestId = nextId();

  // Mock layout info for auto-submit mode
  const mockLayoutResult = {
    type: 'layoutInfoResult',
    windowWidth: 800,
    windowHeight: 600,
    promptType: 'test',
    components: [],
    timestamp: new Date().toISOString(),
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      // Handle layoutInfoResult message type
      if (msg.type === 'layoutInfoResult') {
        const resultMsg = msg as LayoutInfoResultMessage;
        resolve({
          windowWidth: resultMsg.windowWidth ?? 0,
          windowHeight: resultMsg.windowHeight ?? 0,
          promptType: resultMsg.promptType ?? 'unknown',
          components: resultMsg.components ?? [],
          timestamp: resultMsg.timestamp ?? new Date().toISOString(),
        });
        return;
      }

      // Fallback for unexpected message type (or auto-submit)
      resolve({
        windowWidth: (msg as any).windowWidth ?? 0,
        windowHeight: (msg as any).windowHeight ?? 0,
        promptType: (msg as any).promptType ?? 'unknown',
        components: (msg as any).components ?? [],
        timestamp: (msg as any).timestamp ?? new Date().toISOString(),
      });
    }, mockLayoutResult);

    const message: GetLayoutInfoMessage = {
      type: 'getLayoutInfo',
      requestId,
    };

    send(message);
  });
};

// =============================================================================
// ACP Chat SDK API
// =============================================================================

/**
 * Check if ACP Chat is currently open.
 *
 * @returns Object indicating if ACP Chat is open and the active chat ID if any
 */
globalThis.aiIsOpen = async function aiIsOpen(): Promise<{ isOpen: boolean; activeChatId?: string }> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiIsOpenResult',
    isOpen: false,
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiIsOpenResult') {
        const resultMsg = msg as AiIsOpenResultMessage;
        resolve({
          isOpen: resultMsg.isOpen,
          activeChatId: resultMsg.activeChatId,
        });
        return;
      }
      resolve({ isOpen: false });
    }, mockResult);

    const message: AiIsOpenMessage = {
      type: 'aiIsOpen',
      requestId,
    };
    send(message);
  });
};

/**
 * Get information about the currently active chat in ACP Chat.
 * Works directly from SQLite storage — ACP Chat doesn't need to be open.
 *
 * @returns Chat info if there's an active chat, null otherwise
 */
globalThis.aiGetActiveChat = async function aiGetActiveChat(): Promise<AiChatInfo | null> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiActiveChatResult',
    chat: null,
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiActiveChatResult') {
        const resultMsg = msg as AiActiveChatResultMessage;
        resolve(resultMsg.chat ?? null);
        return;
      }
      resolve(null);
    }, mockResult);

    const message: AiGetActiveChatMessage = {
      type: 'aiGetActiveChat',
      requestId,
    };
    send(message);
  });
};

/**
 * List all chats from ACP Chat storage.
 * Works directly from SQLite storage — ACP Chat doesn't need to be open.
 *
 * @param limit - Maximum number of chats to return (default: 50)
 * @param includeDeleted - If true, include soft-deleted chats (default: false)
 * @returns Array of chat info objects
 */
globalThis.aiListChats = async function aiListChats(
  limit?: number,
  includeDeleted?: boolean
): Promise<AiChatInfo[]> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiChatListResult',
    chats: [],
    totalCount: 0,
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiChatListResult') {
        const resultMsg = msg as AiChatListResultMessage;
        resolve(resultMsg.chats ?? []);
        return;
      }
      resolve([]);
    }, mockResult);

    const message: AiListChatsMessage = {
      type: 'aiListChats',
      requestId,
      limit,
      includeDeleted: includeDeleted ?? false,
    };
    send(message);
  });
};

/**
 * Get messages from a specific chat or the active chat.
 * Works directly from SQLite storage — ACP Chat doesn't need to be open.
 *
 * @param chatId - Specific chat ID, or omit to use active chat
 * @param limit - Maximum number of messages to return (default: 100)
 * @returns Array of message info objects
 */
globalThis.aiGetConversation = async function aiGetConversation(
  chatId?: string,
  limit?: number
): Promise<AiMessageInfo[]> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiConversationResult',
    chatId: chatId ?? '',
    messages: [],
    hasMore: false,
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiConversationResult') {
        const resultMsg = msg as AiConversationResultMessage;
        resolve(resultMsg.messages ?? []);
        return;
      }
      resolve([]);
    }, mockResult);

    const message: AiGetConversationMessage = {
      type: 'aiGetConversation',
      requestId,
      chatId,
      limit,
    };
    send(message);
  });
};

/**
 * Start a new ACP Chat conversation with an initial message.
 * Opens ACP Chat if it is not already open.
 *
 * @param message - The initial message to send
 * @param options - Optional configuration (system prompt, model, context parts)
 * @returns Information about the created chat
 */
globalThis.aiStartChat = async function aiStartChat(
  message: string,
  options?: AiChatOptions
): Promise<AiStartChatResult> {
  const requestId = nextId();

  // Read and encode image if path provided
  let imageBase64: string | undefined;
  if (options?.imagePath) {
    try {
      const fs = await import('fs');
      const imageBuffer = fs.readFileSync(options.imagePath);
      imageBase64 = imageBuffer.toString('base64');
    } catch (err) {
      console.error(`Failed to read image at ${options.imagePath}:`, err);
    }
  }

  const mockResult = {
    type: 'aiChatCreated',
    chatId: 'mock-chat-id',
    title: 'New Chat',
    modelId: options?.modelId ?? 'claude-3-5-sonnet-20241022',
    provider: 'anthropic',
    streamingStarted: false,
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiChatCreated') {
        const resultMsg = msg as AiChatCreatedMessage;
        resolve({
          chatId: resultMsg.chatId,
          title: resultMsg.title,
          modelId: resultMsg.modelId,
          provider: resultMsg.provider,
          streamingStarted: resultMsg.streamingStarted,
        });
        return;
      }
      resolve({
        chatId: 'unknown',
        title: 'New Chat',
        modelId: options?.modelId ?? 'unknown',
        provider: 'unknown',
        streamingStarted: false,
      });
    }, mockResult);

    const sendMessage: AiStartChatMessage = {
      type: 'aiStartChat',
      requestId,
      message,
      systemPrompt: options?.systemPrompt,
      image: imageBase64,
      modelId: options?.modelId,
      noResponse: options?.noResponse ?? false,
      parts: options?.parts,
    };
    send(sendMessage);
  });
};

/**
 * Append a message to an existing chat without triggering an AI response.
 * Useful for programmatically building conversation history.
 *
 * @param chatId - The chat to append to
 * @param content - The message content
 * @param role - Message role: 'user', 'assistant', or 'system'
 * @returns The ID of the appended message
 */
globalThis.aiAppendMessage = async function aiAppendMessage(
  chatId: string,
  content: string,
  role: 'user' | 'assistant' | 'system'
): Promise<string> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiMessageAppended',
    messageId: 'mock-message-id',
    chatId,
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiMessageAppended') {
        const resultMsg = msg as AiMessageAppendedMessage;
        resolve(resultMsg.messageId);
        return;
      }
      resolve('unknown');
    }, mockResult);

    const sendMsg: AiAppendMessageMessage = {
      type: 'aiAppendMessage',
      requestId,
      chatId,
      content,
      role,
    };
    send(sendMsg);
  });
};

/**
 * Send a message to an existing chat and trigger an AI response.
 *
 * @param chatId - The chat to send to
 * @param content - The message content
 * @param imagePath - Optional path to an image to attach
 * @param parts - Optional typed context parts to attach
 * @returns The user message ID and whether streaming started
 */
globalThis.aiSendMessage = async function aiSendMessage(
  chatId: string,
  content: string,
  imagePath?: string,
  parts?: AiContextPartInput[]
): Promise<{ userMessageId: string; streamingStarted: boolean }> {
  const requestId = nextId();

  // Read and encode image if path provided
  let imageBase64: string | undefined;
  if (imagePath) {
    try {
      const fs = await import('fs');
      const imageBuffer = fs.readFileSync(imagePath);
      imageBase64 = imageBuffer.toString('base64');
    } catch (err) {
      console.error(`Failed to read image at ${imagePath}:`, err);
    }
  }

  const mockResult = {
    type: 'aiMessageSent',
    userMessageId: 'mock-user-message-id',
    chatId,
    streamingStarted: false,
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiMessageSent') {
        const resultMsg = msg as AiMessageSentMessage;
        resolve({
          userMessageId: resultMsg.userMessageId,
          streamingStarted: resultMsg.streamingStarted,
        });
        return;
      }
      resolve({ userMessageId: 'unknown', streamingStarted: false });
    }, mockResult);

    const sendMsg: AiSendMessageMessage = {
      type: 'aiSendMessage',
      requestId,
      chatId,
      content,
      image: imageBase64,
      parts,
    };
    send(sendMsg);
  });
};

/**
 * Set or update the system prompt for a chat.
 *
 * @param chatId - The chat to update
 * @param prompt - The system prompt content
 */
globalThis.aiSetSystemPrompt = async function aiSetSystemPrompt(
  chatId: string,
  prompt: string
): Promise<void> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiSystemPromptSet',
    success: true,
  };

  return new Promise((resolve) => {
    addPending(requestId, () => {
      resolve();
    }, mockResult);

    const sendMsg: AiSetSystemPromptMessage = {
      type: 'aiSetSystemPrompt',
      requestId,
      chatId,
      prompt,
    };
    send(sendMsg);
  });
};

/**
 * Focus ACP Chat, opening it if necessary.
 *
 * @returns Whether ACP Chat was already open
 */
globalThis.aiFocus = async function aiFocus(): Promise<{ wasOpen: boolean }> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiFocusResult',
    success: true,
    wasOpen: false,
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiFocusResult') {
        const resultMsg = msg as AiFocusResultMessage;
        resolve({ wasOpen: resultMsg.wasOpen });
        return;
      }
      resolve({ wasOpen: false });
    }, mockResult);

    const sendMsg: AiFocusMessage = {
      type: 'aiFocus',
      requestId,
    };
    send(sendMsg);
  });
};

/**
 * Get the current streaming status for a chat.
 *
 * @param chatId - Specific chat ID, or omit to check active chat
 * @returns Streaming status and partial content if streaming
 */
globalThis.aiGetStreamingStatus = async function aiGetStreamingStatus(
  chatId?: string
): Promise<{ isStreaming: boolean; chatId?: string; partialContent?: string }> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiStreamingStatusResult',
    isStreaming: false,
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiStreamingStatusResult') {
        const resultMsg = msg as AiStreamingStatusResultMessage;
        resolve({
          isStreaming: resultMsg.isStreaming,
          chatId: resultMsg.chatId,
          partialContent: resultMsg.partialContent,
        });
        return;
      }
      resolve({ isStreaming: false });
    }, mockResult);

    const sendMsg: AiGetStreamingStatusMessage = {
      type: 'aiGetStreamingStatus',
      requestId,
      chatId,
    };
    send(sendMsg);
  });
};

/**
 * Delete a chat from ACP Chat storage.
 *
 * @param chatId - The chat to delete
 * @param permanent - If true, permanently delete; otherwise soft-delete (default: false)
 */
globalThis.aiDeleteChat = async function aiDeleteChat(
  chatId: string,
  permanent?: boolean
): Promise<void> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiChatDeleted',
    success: true,
  };

  return new Promise((resolve) => {
    addPending(requestId, () => {
      resolve();
    }, mockResult);

    const sendMsg: AiDeleteChatMessage = {
      type: 'aiDeleteChat',
      requestId,
      chatId,
      permanent: permanent ?? false,
    };
    send(sendMsg);
  });
};

// Subscription tracking for aiOn
const aiSubscriptions = new Map<string, { eventTypes: AiEventType[]; handler: AiEventHandler }>();

/**
 * Subscribe to ACP Chat events for real-time streaming updates.
 *
 * @param eventType - The event type to subscribe to
 * @param handler - Callback function for events
 * @param chatId - Optional specific chat to watch (default: all chats)
 * @returns Unsubscribe function
 */
globalThis.aiOn = async function aiOn(
  eventType: AiEventType,
  handler: AiEventHandler,
  chatId?: string
): Promise<() => void> {
  const requestId = nextId();

  const mockResult = {
    type: 'aiSubscribed',
    subscriptionId: `sub-${requestId}`,
    events: [eventType],
  };

  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiSubscribed') {
        const resultMsg = msg as AiSubscribedMessage;
        const subscriptionId = resultMsg.subscriptionId;

        // Store the subscription handler
        aiSubscriptions.set(subscriptionId, {
          eventTypes: [eventType],
          handler,
        });

        // Return unsubscribe function
        resolve(async () => {
          aiSubscriptions.delete(subscriptionId);
          const unsubRequestId = nextId();
          send({
            type: 'aiUnsubscribe',
            requestId: unsubRequestId,
          } as AiUnsubscribeMessage);
        });
        return;
      }
      resolve(() => {});
    }, mockResult);

    const sendMsg: AiSubscribeMessage = {
      type: 'aiSubscribe',
      requestId,
      events: [eventType],
      chatId,
    };
    send(sendMsg);
  });
};

// Internal handler for AI events (called from stdin message handler)
function handleAiEvent(msg: ResponseMessage): boolean {
  switch (msg.type) {
    case 'aiStreamChunk': {
      const event = msg as AiStreamChunkMessage;
      for (const [, sub] of aiSubscriptions) {
        if (sub.eventTypes.includes('streamChunk')) {
          sub.handler({
            chatId: event.chatId,
            chunk: event.chunk,
            accumulatedContent: event.accumulatedContent,
          });
        }
      }
      return true;
    }
    case 'aiStreamComplete': {
      const event = msg as AiStreamCompleteMessage;
      for (const [, sub] of aiSubscriptions) {
        if (sub.eventTypes.includes('streamComplete')) {
          sub.handler({
            chatId: event.chatId,
            messageId: event.messageId,
            fullContent: event.fullContent,
            tokensUsed: event.tokensUsed,
          });
        }
      }
      return true;
    }
    case 'aiNewMessage': {
      const event = msg as AiNewMessageMessage;
      for (const [, sub] of aiSubscriptions) {
        if (sub.eventTypes.includes('message')) {
          sub.handler({
            chatId: event.chatId,
            message: event.message,
          });
        }
      }
      return true;
    }
    case 'aiError': {
      const event = msg as AiErrorMessage;
      for (const [, sub] of aiSubscriptions) {
        if (sub.eventTypes.includes('error')) {
          sub.handler({
            code: event.code,
            message: event.message,
          });
        }
      }
      return true;
    }
    default:
      return false;
  }
}

// Export for internal use
(globalThis as any)._handleAiEvent = handleAiEvent;

// Prompt Control
globalThis.submit = function submit(value: unknown): void {
  const message: ForceSubmitMessage = { type: 'forceSubmit', value };
  send(message);
};

globalThis.exit = function exit(code?: number): void {
  const message: ExitMessage = { type: 'exit', code };
  send(message);
  // Actually terminate the process so autonomous tests don't hang
  process.exit(code ?? 0);
};

// Content Setters
// NOTE: These functions send messages to the app but handlers are not yet implemented
globalThis.setPanel = function setPanel(html: string): void {
  console.warn('[SDK] setPanel() is not yet implemented in the GPUI app');
  const message: SetPanelMessage = { type: 'setPanel', html };
  send(message);
};

globalThis.setPreview = function setPreview(html: string): void {
  console.warn('[SDK] setPreview() is not yet implemented in the GPUI app');
  const message: SetPreviewMessage = { type: 'setPreview', html };
  send(message);
};

globalThis.setPrompt = function setPrompt(html: string): void {
  console.warn('[SDK] setPrompt() is not yet implemented in the GPUI app');
  const message: SetPromptMessage = { type: 'setPrompt', html };
  send(message);
};

// Misc Utilities
globalThis.uuid = function uuid(): string {
  return crypto.randomUUID();
};

globalThis.compile = function compile(
  template: string
): (data: Record<string, unknown>) => string {
  return (data: Record<string, unknown>) => {
    return template.replace(/\{\{(\w+)\}\}/g, (_, key) => {
      const value = data[key];
      return value !== undefined ? String(value) : '';
    });
  };
};

// =============================================================================
// TIER 5B: Path Utilities (pure functions using node:path and node:os)
// =============================================================================

globalThis.home = function home(...segments: string[]): string {
  return nodePath.join(os.homedir(), ...segments);
};

globalThis.skPath = function skPath(...segments: string[]): string {
  return nodePath.join(os.homedir(), '.scriptkit', ...segments);
};

globalThis.kitPath = function kitPath(...segments: string[]): string {
  // Now returns ~/.scriptkit paths - ~/.kit is deprecated
  return nodePath.join(os.homedir(), '.scriptkit', ...segments);
};

globalThis.tmpPath = function tmpPath(...segments: string[]): string {
  return nodePath.join(os.tmpdir(), 'kit', ...segments);
};

// =============================================================================
// TIER 5B: File Utilities (pure JS using Node fs)
// =============================================================================

globalThis.isFile = async function isFile(filePath: string): Promise<boolean> {
  try {
    const stat = await fs.stat(filePath);
    return stat.isFile();
  } catch {
    return false;
  }
};

globalThis.isDir = async function isDir(dirPath: string): Promise<boolean> {
  try {
    const stat = await fs.stat(dirPath);
    return stat.isDirectory();
  } catch {
    return false;
  }
};

globalThis.isBin = async function isBin(filePath: string): Promise<boolean> {
  try {
    await fs.access(filePath, fsConstants.X_OK);
    return true;
  } catch {
    return false;
  }
};

// =============================================================================
// TIER 5B: Memory Map (in-process only, no messages needed)
// =============================================================================

const internalMemoryMap = new Map<string, unknown>();

globalThis.memoryMap = {
  get(key: string): unknown {
    return internalMemoryMap.get(key);
  },
  
  set(key: string, value: unknown): void {
    internalMemoryMap.set(key, value);
  },
  
  delete(key: string): boolean {
    return internalMemoryMap.delete(key);
  },
  
  clear(): void {
    internalMemoryMap.clear();
  },
};

// =============================================================================
// TIER 5B: Browser/App Utilities
// =============================================================================

globalThis.browse = async function browse(url: string): Promise<void> {
  const message: BrowseMessage = { type: 'browse', url };
  send(message);
};

globalThis.editFile = async function editFile(filePath: string): Promise<void> {
  const message: EditFileMessage = { type: 'edit', path: filePath };
  send(message);
};

globalThis.run = async function run(scriptName: string, ...args: string[]): Promise<unknown> {
  const id = nextId();

  return new Promise((resolve) => {
    addPending(id, (msg: SubmitMessage) => {
      const value = msg.value;
      if (value === undefined || value === null || value === '') {
        resolve(undefined);
      } else {
        try {
          resolve(JSON.parse(value));
        } catch {
          resolve(value);
        }
      }
    }, { value: '' }); // Auto-submit: empty result

    const message: RunMessage = {
      type: 'run',
      id,
      scriptName,
      args,
    };

    send(message);
  });
};

globalThis.inspect = async function inspect(data: unknown): Promise<void> {
  const message: InspectMessage = { type: 'inspect', data };
  send(message);
};

// =============================================================================
// Clipboard History Functions
// =============================================================================

globalThis.clipboardHistory = async function clipboardHistory(): Promise<ClipboardHistoryEntry[]> {
  const id = nextId();

  // Auto-submit: empty clipboard history
  const autoSubmitValue = { type: 'clipboardHistoryList', entries: [] };

  return new Promise((resolve) => {
    addPending(id, (msg: ResponseMessage) => {
      // Handle clipboardHistoryList message type (sent by Rust for list requests)
      if (msg.type === 'clipboardHistoryList') {
        const listMsg = msg as ClipboardHistoryListMessage;
        resolve((listMsg.entries ?? []).map((entry) => ({
          entryId: entry.entryId ?? entry.entry_id ?? '',
          content: entry.content ?? '',
          contentType: (entry.contentType ?? entry.content_type ?? 'text') as 'text' | 'image',
          timestamp: entry.timestamp ?? '',
          pinned: entry.pinned ?? false,
        })));
        return;
      }

      // Fallback to submit message handling (backwards compatibility)
      const submitMsg = msg as SubmitMessage;
      const value = submitMsg.value ?? '[]';
      try {
        const parsed = JSON.parse(value);
        if (Array.isArray(parsed)) {
          resolve(parsed.map((entry: {
            entryId?: string;
            entry_id?: string;
            content?: string;
            contentType?: string;
            content_type?: string;
            timestamp?: string;
            pinned?: boolean;
          }) => ({
            entryId: entry.entryId ?? entry.entry_id ?? '',
            content: entry.content ?? '',
            contentType: (entry.contentType ?? entry.content_type ?? 'text') as 'text' | 'image',
            timestamp: entry.timestamp ?? '',
            pinned: entry.pinned ?? false,
          })));
        } else {
          resolve([]);
        }
      } catch {
        resolve([]);
      }
    }, autoSubmitValue);

    const message: ClipboardHistoryMessage = {
      type: 'clipboardHistory',
      requestId: id,
      action: 'list',
    };

    send(message);
  });
};

globalThis.clipboardHistoryPin = async function clipboardHistoryPin(entryId: string): Promise<void> {
  const id = nextId();

  // Auto-submit: success
  const autoSubmitValue = { type: 'clipboardHistoryResult', success: true };

  return new Promise((resolve, reject) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'clipboardHistoryResult') {
        const resultMsg = msg as ClipboardHistoryResultMessage;
        if (resultMsg.success) {
          resolve();
        } else {
          reject(new Error(resultMsg.error ?? 'Unknown error'));
        }
      } else {
        resolve(); // Fallback
      }
    }, autoSubmitValue);

    const message: ClipboardHistoryMessage = {
      type: 'clipboardHistory',
      requestId: id,
      action: 'pin',
      entryId,
    };

    send(message);
  });
};

globalThis.clipboardHistoryUnpin = async function clipboardHistoryUnpin(entryId: string): Promise<void> {
  const id = nextId();

  // Auto-submit: success
  const autoSubmitValue = { type: 'clipboardHistoryResult', success: true };

  return new Promise((resolve, reject) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'clipboardHistoryResult') {
        const resultMsg = msg as ClipboardHistoryResultMessage;
        if (resultMsg.success) {
          resolve();
        } else {
          reject(new Error(resultMsg.error ?? 'Unknown error'));
        }
      } else {
        resolve(); // Fallback
      }
    }, autoSubmitValue);
    
    const message: ClipboardHistoryMessage = {
      type: 'clipboardHistory',
      requestId: id,
      action: 'unpin',
      entryId,
    };
    
    send(message);
  });
};

globalThis.clipboardHistoryRemove = async function clipboardHistoryRemove(entryId: string): Promise<void> {
  const id = nextId();

  const autoSubmitValue = { type: 'clipboardHistoryResult', success: true };

  return new Promise((resolve, reject) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'clipboardHistoryResult') {
        const resultMsg = msg as ClipboardHistoryResultMessage;
        if (resultMsg.success) {
          resolve();
        } else {
          reject(new Error(resultMsg.error ?? 'Unknown error'));
        }
      } else {
        resolve(); // Fallback
      }
    }, autoSubmitValue);

    const message: ClipboardHistoryMessage = {
      type: 'clipboardHistory',
      requestId: id,
      action: 'remove',
      entryId,
    };

    send(message);
  });
};

globalThis.clipboardHistoryClear = async function clipboardHistoryClear(): Promise<void> {
  const id = nextId();

  const autoSubmitValue = { type: 'clipboardHistoryResult', success: true };

  return new Promise((resolve, reject) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'clipboardHistoryResult') {
        const resultMsg = msg as ClipboardHistoryResultMessage;
        if (resultMsg.success) {
          resolve();
        } else {
          reject(new Error(resultMsg.error ?? 'Unknown error'));
        }
      } else {
        resolve();
      }
    }, autoSubmitValue);

    const message: ClipboardHistoryMessage = {
      type: 'clipboardHistory',
      requestId: id,
      action: 'clear',
    };

    send(message);
  });
};

globalThis.clipboardHistoryTrimOversize = async function clipboardHistoryTrimOversize(): Promise<void> {
  const id = nextId();

  const autoSubmitValue = { type: 'clipboardHistoryResult', success: true };

  return new Promise((resolve, reject) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'clipboardHistoryResult') {
        const resultMsg = msg as ClipboardHistoryResultMessage;
        if (resultMsg.success) {
          resolve();
        } else {
          reject(new Error(resultMsg.error ?? 'Unknown error'));
        }
      } else {
        resolve();
      }
    }, autoSubmitValue);

    const message: ClipboardHistoryMessage = {
      type: 'clipboardHistory',
      requestId: id,
      action: 'trimOversize',
    };

    send(message);
  });
};

// =============================================================================
// Window Management Functions (System Windows)
// =============================================================================

globalThis.getWindows = async function getWindows(): Promise<SystemWindowInfo[]> {
  const id = nextId();

  const autoSubmitValue = { type: 'windowListResult', windows: [] };

  return new Promise((resolve) => {
    addPending(id, (msg: ResponseMessage) => {
      // Handle WindowListResult message type
      if (msg.type === 'windowListResult') {
        const resultMsg = msg as WindowListResultMessage;
        resolve(resultMsg.windows.map((win) => ({
          windowId: win.windowId ?? win.window_id ?? 0,
          title: win.title ?? '',
          appName: win.appName ?? win.app_name ?? '',
          bounds: win.bounds,
          isMinimized: win.isMinimized ?? win.is_minimized,
          isActive: win.isActive ?? win.is_active,
        })));
        return;
      }

      // Fallback to submit message handling (backwards compatibility)
      const submitMsg = msg as SubmitMessage;
      const value = submitMsg.value ?? '[]';
      try {
        const parsed = JSON.parse(value);
        if (Array.isArray(parsed)) {
          resolve(parsed.map((win: {
            windowId?: number;
            window_id?: number;
            title?: string;
            appName?: string;
            app_name?: string;
            bounds?: TargetWindowBounds;
            isMinimized?: boolean;
            is_minimized?: boolean;
            isActive?: boolean;
            is_active?: boolean;
          }) => ({
            windowId: win.windowId ?? win.window_id ?? 0,
            title: win.title ?? '',
            appName: win.appName ?? win.app_name ?? '',
            bounds: win.bounds,
            isMinimized: win.isMinimized ?? win.is_minimized,
            isActive: win.isActive ?? win.is_active,
          })));
        } else {
          resolve([]);
        }
      } catch {
        resolve([]);
      }
    }, autoSubmitValue);

    const message: WindowListMessage = {
      type: 'windowList',
      requestId: id,
    };

    send(message);
  });
};

globalThis.focusWindow = async function focusWindow(windowId: number): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' });

    const message: WindowActionMessage = {
      type: 'windowAction',
      requestId: id,
      action: 'focus',
      windowId,
    };

    send(message);
  });
};

globalThis.closeWindow = async function closeWindow(windowId: number): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' });

    const message: WindowActionMessage = {
      type: 'windowAction',
      requestId: id,
      action: 'close',
      windowId,
    };

    send(message);
  });
};

globalThis.minimizeWindow = async function minimizeWindow(windowId: number): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' });

    const message: WindowActionMessage = {
      type: 'windowAction',
      requestId: id,
      action: 'minimize',
      windowId,
    };

    send(message);
  });
};

globalThis.maximizeWindow = async function maximizeWindow(windowId: number): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' });

    const message: WindowActionMessage = {
      type: 'windowAction',
      requestId: id,
      action: 'maximize',
      windowId,
    };

    send(message);
  });
};

globalThis.moveWindow = async function moveWindow(windowId: number, x: number, y: number): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' });

    const message: WindowActionMessage = {
      type: 'windowAction',
      requestId: id,
      action: 'move',
      windowId,
      bounds: { x, y, width: 0, height: 0 },
    };

    send(message);
  });
};

globalThis.resizeWindow = async function resizeWindow(windowId: number, width: number, height: number): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' });

    const message: WindowActionMessage = {
      type: 'windowAction',
      requestId: id,
      action: 'resize',
      windowId,
      bounds: { x: 0, y: 0, width, height },
    };

    send(message);
  });
};

/**
 * Tile a window to a specific screen position.
 * Uses the native tiling implementation which calculates bounds based on actual screen dimensions.
 * @param windowId - The ID of the window to tile
 * @param position - The tile position
 */
globalThis.tileWindow = async function tileWindow(windowId: number, position: TilePosition): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'windowActionResult') {
        const resultMsg = msg as { success?: boolean; error?: string };
        if (resultMsg.success === false && resultMsg.error) {
          reject(new Error(resultMsg.error));
        } else {
          resolve();
        }
        return;
      }
      // Fallback
      const submitMsg = msg as SubmitMessage;
      if (submitMsg.value && submitMsg.value.startsWith('ERROR:')) {
        reject(new Error(submitMsg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' });

    const message = {
      type: 'windowAction',
      requestId: id,
      action: 'tile',
      windowId,
      tilePosition: position,
    };

    send(message);
  });
};

/**
 * Get information about all connected displays/monitors.
 * @returns Array of display information including bounds and visibility
 */
globalThis.getDisplays = async function getDisplays(): Promise<DisplayInfo[]> {
  const id = nextId();

  return new Promise((resolve) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'displayListResult') {
        const resultMsg = msg as { displays?: DisplayInfo[] };
        resolve(resultMsg.displays ?? []);
        return;
      }
      // Fallback
      resolve([]);
    }, { displays: [] });

    const message = {
      type: 'displayList',
      requestId: id,
    };

    send(message);
  });
};

/**
 * Get the frontmost window of the app that was active before Script Kit appeared.
 * This is useful for window management commands that operate on the user's previous window.
 * @returns The frontmost window info, or null if no window is found
 */
globalThis.getFrontmostWindow = async function getFrontmostWindow(): Promise<SystemWindowInfo | null> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'frontmostWindowResult') {
        const resultMsg = msg as { window?: SystemWindowInfo; error?: string };
        if (resultMsg.error) {
          // Return null instead of rejecting - no frontmost window is not an error
          resolve(null);
        } else {
          resolve(resultMsg.window ?? null);
        }
        return;
      }
      // Fallback
      resolve(null);
    }, { window: null });

    const message = {
      type: 'frontmostWindow',
      requestId: id,
    };

    send(message);
  });
};

/**
 * Move a window to the next display/monitor.
 * @param windowId - The ID of the window to move
 */
globalThis.moveToNextDisplay = async function moveToNextDisplay(windowId: number): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'windowActionResult') {
        const resultMsg = msg as { success?: boolean; error?: string };
        if (resultMsg.success === false && resultMsg.error) {
          reject(new Error(resultMsg.error));
        } else {
          resolve();
        }
        return;
      }
      // Fallback
      const submitMsg = msg as SubmitMessage;
      if (submitMsg.value && submitMsg.value.startsWith('ERROR:')) {
        reject(new Error(submitMsg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' });

    const message: WindowActionMessage = {
      type: 'windowAction',
      requestId: id,
      action: 'moveToNextDisplay',
      windowId,
    };

    send(message);
  });
};

/**
 * Move a window to the previous display/monitor.
 * @param windowId - The ID of the window to move
 */
globalThis.moveToPreviousDisplay = async function moveToPreviousDisplay(windowId: number): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    addPending(id, (msg: ResponseMessage) => {
      if (msg.type === 'windowActionResult') {
        const resultMsg = msg as { success?: boolean; error?: string };
        if (resultMsg.success === false && resultMsg.error) {
          reject(new Error(resultMsg.error));
        } else {
          resolve();
        }
        return;
      }
      // Fallback
      const submitMsg = msg as SubmitMessage;
      if (submitMsg.value && submitMsg.value.startsWith('ERROR:')) {
        reject(new Error(submitMsg.value.substring(6).trim()));
      } else {
        resolve();
      }
    }, { value: '' });

    const message: WindowActionMessage = {
      type: 'windowAction',
      requestId: id,
      action: 'moveToPreviousDisplay',
      windowId,
    };

    send(message);
  });
};

// =============================================================================
// File Search Functions
// =============================================================================

globalThis.fileSearch = async function fileSearch(query: string, options?: FindOptions): Promise<FileSearchResult[]> {
  const id = nextId();

  const autoSubmitValue = { type: 'fileSearchResult', files: [] };

  return new Promise((resolve) => {
    addPending(id, (msg: ResponseMessage) => {
      // Handle FileSearchResult message type
      if (msg.type === 'fileSearchResult') {
        const resultMsg = msg as FileSearchResultMessage;
        resolve(resultMsg.files.map((file) => ({
          path: file.path ?? '',
          name: file.name ?? '',
          isDirectory: file.isDirectory ?? file.is_directory ?? false,
          size: file.size,
          modifiedAt: file.modifiedAt ?? file.modified_at,
        })));
        return;
      }

      // Fallback to submit message handling (backwards compatibility)
      const submitMsg = msg as SubmitMessage;
      const value = submitMsg.value ?? '[]';
      try {
        const parsed = JSON.parse(value);
        if (Array.isArray(parsed)) {
          resolve(parsed.map((file: {
            path?: string;
            name?: string;
            isDirectory?: boolean;
            is_directory?: boolean;
            size?: number;
            modifiedAt?: string;
            modified_at?: string;
          }) => ({
            path: file.path ?? '',
            name: file.name ?? '',
            isDirectory: file.isDirectory ?? file.is_directory ?? false,
            size: file.size,
            modifiedAt: file.modifiedAt ?? file.modified_at,
          })));
        } else {
          resolve([]);
        }
      } catch {
        resolve([]);
      }
    }, autoSubmitValue);

    const message: FileSearchMessage = {
      type: 'fileSearch',
      requestId: id,
      query,
      onlyin: options?.onlyin,
    };

    send(message);
  });
};

// =============================================================================
// Menu Bar Functions
// =============================================================================

/**
 * Get the menu bar items from the frontmost application or a specific app.
 * 
 * Returns a hierarchical tree of menu items with their titles, enabled state,
 * keyboard shortcuts, and menu paths.
 * 
 * @param bundleId - Optional bundle ID to get menu bar from a specific app
 *                   (e.g., "com.apple.finder"). If not provided, uses frontmost app.
 * @returns Promise resolving to array of top-level menu bar items
 * 
 * @example
 * ```typescript
 * // Get menu bar from frontmost app
 * const menus = await getMenuBar();
 * console.log(menus.map(m => m.title)); // ["Apple", "File", "Edit", ...]
 * 
 * // Get menu bar from specific app
 * const finderMenus = await getMenuBar("com.apple.finder");
 * ```
 */
globalThis.getMenuBar = async function getMenuBar(bundleId?: string): Promise<MenuBarItem[]> {
  const id = nextId();

  return new Promise((resolve) => {
    const resolver = (msg: ResponseMessage) => {
      // Handle MenuBarResult message type
      if (msg.type === 'menuBarResult') {
        const resultMsg = msg as MenuBarResultMessage;
        resolve(resultMsg.items);
        return;
      }

      // Handle auto-submit (msg will be the auto-submit value directly)
      if (Array.isArray(msg)) {
        resolve(msg as MenuBarItem[]);
        return;
      }

      // Fallback to submit message handling (backwards compatibility)
      const submitMsg = msg as SubmitMessage;
      const value = submitMsg.value ?? '[]';
      try {
        const parsed = JSON.parse(value);
        if (Array.isArray(parsed)) {
          resolve(parsed as MenuBarItem[]);
        } else {
          resolve([]);
        }
      } catch {
        resolve([]);
      }
    };

    addPending(id, resolver, []);

    const message: GetMenuBarMessage = {
      type: 'getMenuBar',
      requestId: id,
      bundleId,
    };

    send(message);
  });
};

/**
 * Execute a menu action in a specific application.
 * 
 * Clicks a menu item by navigating through the menu hierarchy using the provided path.
 * The path is an array of menu titles from top-level menu to the target item.
 * 
 * @param bundleId - Bundle ID of the application (e.g., "com.apple.finder")
 * @param menuPath - Array of menu titles forming the path to the item
 *                   (e.g., ["File", "New", "Folder"])
 * @returns Promise that resolves when the action is executed
 * @throws Error if the menu item cannot be found or executed
 * 
 * @example
 * ```typescript
 * // Open Finder's New Window menu item
 * await executeMenuAction("com.apple.finder", ["File", "New Finder Window"]);
 * 
 * // Toggle View menu option
 * await executeMenuAction("com.apple.finder", ["View", "Show Path Bar"]);
 * ```
 */
globalThis.executeMenuAction = async function executeMenuAction(
  bundleId: string,
  menuPath: string[]
): Promise<void> {
  const id = nextId();

  return new Promise((resolve, reject) => {
    const resolver = (msg: ResponseMessage) => {
      // Handle MenuActionResult message type
      if (msg.type === 'menuActionResult') {
        const resultMsg = msg as MenuActionResultMessage;
        if (resultMsg.success) {
          resolve();
        } else {
          reject(new Error(resultMsg.error ?? 'Menu action failed'));
        }
        return;
      }

      // Handle auto-submit (msg will be undefined for void)
      if (msg === undefined) {
        resolve();
        return;
      }

      // Fallback to submit message handling (backwards compatibility)
      const submitMsg = msg as SubmitMessage;
      if (submitMsg.value && submitMsg.value.startsWith('ERROR:')) {
        reject(new Error(submitMsg.value.substring(6).trim()));
      } else {
        resolve();
      }
    };

    addPending(id, resolver, undefined);

    const message: ExecuteMenuActionMessage = {
      type: 'executeMenuAction',
      requestId: id,
      bundleId,
      path: menuPath,
    };

    send(message);
  });
};

// =============================================================================
// AI-First Protocol: input() and output() Functions
// =============================================================================

/**
 * Internal state for schema-based input/output
 */
let scriptInputData: Record<string, unknown> | null = null;
let scriptOutputData: Record<string, unknown> = {};

/**
 * Assign defineSchema to globalThis for runtime access.
 * The function itself is defined and exported above with the type utilities.
 */
globalThis.defineSchema = defineSchema;

/**
 * Receive typed input for the script.
 * 
 * When a script has a `schema` with `input` fields defined, this function
 * retrieves the input values passed by the caller (AI agent, MCP client, etc.).
 * 
 * If no schema input is defined or no input was provided, returns an empty object.
 * 
 * @example
 * ```typescript
 * schema = {
 *   input: {
 *     title: { type: 'string', required: true, description: 'Note title' },
 *     tags: { type: 'array', items: 'string' }
 *   }
 * }
 * 
 * const { title, tags } = await input();
 * console.log(`Creating note: ${title} with tags: ${tags?.join(', ')}`);
 * ```
 * 
 * @returns Promise resolving to the input object with typed fields
 */
globalThis.input = async function input<T extends Record<string, unknown> = Record<string, unknown>>(): Promise<T> {
  // If input data was set via protocol message, return it
  if (scriptInputData !== null) {
    return scriptInputData as T;
  }
  
  // Otherwise return empty object (script may be run interactively)
  return {} as T;
};

/**
 * Send typed output from the script.
 * 
 * When a script has a `schema` with `output` fields defined, this function
 * sends structured output back to the caller. Multiple calls accumulate
 * the output object (later calls merge with earlier ones).
 * 
 * Output is streamed via SSE when running through MCP, allowing real-time
 * progress updates.
 * 
 * @param data - The output data to send (will be merged with previous output)
 * 
 * @example
 * ```typescript
 * schema = {
 *   output: {
 *     path: { type: 'string' },
 *     wordCount: { type: 'number' }
 *   }
 * }
 * 
 * // ... create note ...
 * output({ path: notePath });
 * 
 * // ... count words ...
 * output({ wordCount: content.split(' ').length });
 * 
 * // Final output will be { path: '...', wordCount: 42 }
 * ```
 */
globalThis.output = function output(data: Record<string, unknown>): void {
  // Merge with existing output
  scriptOutputData = { ...scriptOutputData, ...data };
  
  // Send output message to app (will be streamed via SSE if MCP)
  send({
    type: 'scriptOutput',
    data: scriptOutputData,
  });
};

/**
 * Set the input data for the script (called by protocol handler).
 * @internal
 */
globalThis._setScriptInput = function _setScriptInput(data: Record<string, unknown>): void {
  scriptInputData = data;
};

/**
 * Get the accumulated output data (called by protocol handler).
 * @internal
 */
globalThis._getScriptOutput = function _getScriptOutput(): Record<string, unknown> {
  return scriptOutputData;
};

/**
 * Reset input/output state (for testing).
 * @internal
 */
globalThis._resetScriptIO = function _resetScriptIO(): void {
  scriptInputData = null;
  scriptOutputData = {};
};

// =============================================================================
// UI Introspection / Deterministic Transactions
// =============================================================================

export type TransactionTraceMode = 'off' | 'on' | 'onFailure';

export interface PromptState {
  promptType: string;
  promptId?: string;
  placeholder?: string;
  inputValue: string;
  choiceCount: number;
  visibleChoiceCount: number;
  selectedIndex: number;
  selectedValue?: string;
  isFocused: boolean;
  windowVisible: boolean;
}

export type ElementType = 'choice' | 'input' | 'button' | 'panel' | 'list' | 'unknown';

export interface ElementInfo {
  semanticId: string;
  type: ElementType;
  text?: string;
  value?: string;
  selected?: boolean;
  focused?: boolean;
  index?: number;
}

export interface ElementsSnapshot {
  elements: ElementInfo[];
  totalCount: number;
  truncated: boolean;
  focusedSemanticId?: string;
  selectedSemanticId?: string;
  warnings: string[];
}

export type WaitNamedCondition =
  | 'choicesRendered'
  | 'inputEmpty'
  | 'windowVisible'
  | 'windowFocused';

export type WaitCondition =
  | WaitNamedCondition
  | { type: 'elementExists'; semanticId: string }
  | { type: 'elementVisible'; semanticId: string }
  | { type: 'elementFocused'; semanticId: string }
  | {
      type: 'stateMatch';
      state: Partial<
        Pick<PromptState, 'promptType' | 'inputValue' | 'selectedValue' | 'windowVisible'>
      >;
    };

export interface TransactionErrorData {
  code?: string;
  message: string;
  suggestion?: string;
}

export interface WaitForOptions {
  timeout?: number;
  pollInterval?: number;
  trace?: TransactionTraceMode;
}

export interface WaitForResult {
  success: boolean;
  elapsed: number;
  error?: TransactionErrorData;
  trace?: unknown;
}

export type BatchCommand =
  | { type: 'setInput'; text: string }
  | { type: 'forceSubmit'; value: unknown }
  | ({ type: 'waitFor'; condition: WaitCondition } & WaitForOptions);

export interface BatchOptions {
  stopOnError?: boolean;
  rollbackOnError?: boolean;
  timeout?: number;
  sequential?: boolean;
}

export interface BatchResultEntry {
  success: boolean;
  elapsed?: number;
  error?: TransactionErrorData;
  value?: unknown;
}

export interface BatchResult {
  success: boolean;
  results: BatchResultEntry[];
  failedAt?: number;
  totalElapsed: number;
  trace?: unknown;
}

// Internal message types for protocol communication

type GetStateMessage = {
  type: 'getState';
  requestId: string;
};

type StateResultMessage = {
  type: 'stateResult';
  requestId: string;
  promptType: string;
  promptId?: string;
  placeholder?: string;
  inputValue: string;
  choiceCount: number;
  visibleChoiceCount: number;
  selectedIndex: number;
  selectedValue?: string;
  isFocused: boolean;
  windowVisible: boolean;
};

type GetElementsMessage = {
  type: 'getElements';
  requestId: string;
  limit?: number;
};

type ElementsResultMessage = {
  type: 'elementsResult';
  requestId: string;
  elements: ElementInfo[];
  totalCount: number;
  truncated: boolean;
  focusedSemanticId?: string;
  selectedSemanticId?: string;
  warnings?: string[];
};

type WaitForMessage = {
  type: 'waitFor';
  requestId: string;
  condition: WaitCondition;
  timeout?: number;
  pollInterval?: number;
  trace?: TransactionTraceMode;
};

type WaitForResultMessage = {
  type: 'waitForResult';
  requestId: string;
  success: boolean;
  elapsed: number;
  error?: TransactionErrorData;
  trace?: unknown;
};

type BatchMessage = {
  type: 'batch';
  requestId: string;
  commands: BatchCommand[];
  options?: BatchOptions;
  trace?: TransactionTraceMode;
};

type BatchResultMessage = {
  type: 'batchResult';
  requestId: string;
  success: boolean;
  results: BatchResultEntry[];
  failedAt?: number;
  totalElapsed: number;
  trace?: unknown;
};

declare global {
  var getState: () => Promise<PromptState>;
  var getElements: (limit?: number) => Promise<ElementsSnapshot>;
  var waitFor: (
    condition: WaitCondition,
    options?: WaitForOptions
  ) => Promise<WaitForResult>;
  var batch: (
    commands: BatchCommand[],
    options?: BatchOptions & { trace?: TransactionTraceMode }
  ) => Promise<BatchResult>;
}

globalThis.getState = async function getState(): Promise<PromptState> {
  const requestId = nextId();
  const fallback: StateResultMessage = {
    type: 'stateResult',
    requestId,
    promptType: 'unknown',
    inputValue: '',
    choiceCount: 0,
    visibleChoiceCount: 0,
    selectedIndex: -1,
    isFocused: false,
    windowVisible: false,
  };

  return new Promise((resolve) => {
    addPending(
      requestId,
      (msg: ResponseMessage) => {
        const state = (msg.type === 'stateResult' ? msg : fallback) as StateResultMessage;
        resolve({
          promptType: state.promptType,
          promptId: state.promptId,
          placeholder: state.placeholder,
          inputValue: state.inputValue,
          choiceCount: state.choiceCount,
          visibleChoiceCount: state.visibleChoiceCount,
          selectedIndex: state.selectedIndex,
          selectedValue: state.selectedValue,
          isFocused: state.isFocused,
          windowVisible: state.windowVisible,
        });
      },
      fallback
    );
    const message: GetStateMessage = { type: 'getState', requestId };
    send(message);
  });
};

globalThis.getElements = async function getElements(limit = 50): Promise<ElementsSnapshot> {
  const requestId = nextId();
  const fallback: ElementsResultMessage = {
    type: 'elementsResult',
    requestId,
    elements: [],
    totalCount: 0,
    truncated: false,
    warnings: [],
  };

  return new Promise((resolve) => {
    addPending(
      requestId,
      (msg: ResponseMessage) => {
        const result = (msg.type === 'elementsResult' ? msg : fallback) as ElementsResultMessage;
        resolve({
          elements: result.elements ?? [],
          totalCount: result.totalCount ?? 0,
          truncated: result.truncated ?? false,
          focusedSemanticId: result.focusedSemanticId,
          selectedSemanticId: result.selectedSemanticId,
          warnings: result.warnings ?? [],
        });
      },
      fallback
    );
    const message: GetElementsMessage = { type: 'getElements', requestId, limit };
    send(message);
  });
};

globalThis.waitFor = async function waitFor(
  condition: WaitCondition,
  options?: WaitForOptions
): Promise<WaitForResult> {
  const requestId = nextId();
  const fallback: WaitForResultMessage = {
    type: 'waitForResult',
    requestId,
    success: false,
    elapsed: 0,
    error: { message: 'waitFor fallback response used' },
  };

  return new Promise((resolve) => {
    addPending(
      requestId,
      (msg: ResponseMessage) => {
        const result = (msg.type === 'waitForResult' ? msg : fallback) as WaitForResultMessage;
        resolve({
          success: result.success,
          elapsed: result.elapsed,
          error: result.error,
          trace: result.trace,
        });
      },
      fallback
    );
    const message: WaitForMessage = {
      type: 'waitFor',
      requestId,
      condition,
      timeout: options?.timeout,
      pollInterval: options?.pollInterval,
      trace: options?.trace,
    };
    send(message);
  });
};

globalThis.batch = async function batch(
  commands: BatchCommand[],
  options?: BatchOptions & { trace?: TransactionTraceMode }
): Promise<BatchResult> {
  const requestId = nextId();
  const fallback: BatchResultMessage = {
    type: 'batchResult',
    requestId,
    success: false,
    results: [],
    totalElapsed: 0,
  };

  return new Promise((resolve) => {
    addPending(
      requestId,
      (msg: ResponseMessage) => {
        const result = (msg.type === 'batchResult' ? msg : fallback) as BatchResultMessage;
        resolve({
          success: result.success,
          results: result.results ?? [],
          failedAt: result.failedAt,
          totalElapsed: result.totalElapsed ?? 0,
          trace: result.trace,
        });
      },
      fallback
    );
    const message: BatchMessage = {
      type: 'batch',
      requestId,
      commands,
      options: options
        ? {
            stopOnError: options.stopOnError,
            rollbackOnError: options.rollbackOnError,
            timeout: options.timeout,
            sequential: options.sequential,
          }
        : undefined,
      trace: options?.trace,
    };
    send(message);
  });
};

const MCP_PROTOCOL_VERSION = '2024-11-05';
let cachedScriptKitConfig: Promise<Config | null> | null = null;

type McpJsonRpcId = number | string;
type McpJsonRpcSuccess = {
  jsonrpc: '2.0';
  id: McpJsonRpcId;
  result?: Record<string, unknown>;
};
type McpJsonRpcFailure = {
  jsonrpc: '2.0';
  id: McpJsonRpcId | null;
  error: {
    code: number;
    message: string;
    data?: unknown;
  };
};
type McpJsonRpcResponse = McpJsonRpcSuccess | McpJsonRpcFailure;

function defaultMcpConfig(): McpConfig {
  return {
    enabled: true,
    servers: {},
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizeMcpConfig(config: unknown): McpConfig {
  if (!isRecord(config)) {
    return defaultMcpConfig();
  }

  const servers = isRecord(config.servers)
    ? (config.servers as Record<string, McpServerConfig>)
    : {};

  return {
    enabled: typeof config.enabled === 'boolean' ? config.enabled : true,
    servers,
  };
}

async function loadScriptKitConfig(): Promise<Config | null> {
  if (!cachedScriptKitConfig) {
    cachedScriptKitConfig = (async () => {
      const configPath = nodePath.join(os.homedir(), '.scriptkit', 'kit', 'config.ts');

      try {
        await fs.access(configPath, fsConstants.F_OK);
      } catch {
        return null;
      }

      const moduleUrl = `${pathToFileURL(configPath).href}?t=${Date.now()}`;
      const imported = await import(moduleUrl);
      const config = (imported.default ?? imported) as Config;
      return config ?? null;
    })();
  }

  return cachedScriptKitConfig;
}

async function loadScriptKitMcpConfig(): Promise<McpConfig> {
  const config = await loadScriptKitConfig();
  return normalizeMcpConfig(config?.mcp);
}

function isServerEnabled(server: McpServerConfig): boolean {
  return server.enabled ?? true;
}

function getEnabledMcpServerEntries(config: McpConfig): Array<[string, McpServerConfig]> {
  if (config.enabled === false) {
    return [];
  }

  return Object.entries(config.servers ?? {}).filter(([, server]) => isServerEnabled(server));
}

function toMcpServerInfo(id: string, server: McpServerConfig): McpServerInfo {
  if (server.transport === 'stdio') {
    return {
      id,
      transport: 'stdio',
      enabled: isServerEnabled(server),
      name: server.name,
      description: server.description,
      command: server.command,
      args: server.args ?? [],
      env: server.env ?? {},
      cwd: server.cwd,
    };
  }

  return {
    id,
    transport: 'http',
    enabled: isServerEnabled(server),
    name: server.name,
    description: server.description,
    endpoint: server.endpoint,
    headers: server.headers ?? {},
  };
}

async function getConfiguredMcpServer(id: string): Promise<McpServerConfig | null> {
  const config = await loadScriptKitMcpConfig();
  const entries = getEnabledMcpServerEntries(config);
  return entries.find(([serverId]) => serverId === id)?.[1] ?? null;
}

function getMcpErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function assertMcpJsonRpcSuccess(
  response: McpJsonRpcResponse,
  serverId: string,
  method: string
): Record<string, unknown> {
  if ('error' in response) {
    throw new Error(
      `[mcp:${serverId}] ${method} failed: ${response.error.message} (code ${response.error.code})`
    );
  }

  return response.result ?? {};
}

async function createStdioMcpSession(serverId: string, server: McpStdioServerConfig) {
  if (!server.command?.trim()) {
    throw new Error(`[mcp:${serverId}] stdio server is missing a command`);
  }

  const child = spawnChildProcess(server.command, server.args ?? [], {
    cwd: server.cwd,
    env: {
      ...process.env,
      ...(server.env ?? {}),
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  let nextRequestId = 1;
  const stderrChunks: string[] = [];
  const pending = new Map<
    number,
    {
      resolve: (response: McpJsonRpcResponse) => void;
      reject: (error: Error) => void;
    }
  >();

  child.stderr.on('data', (chunk) => {
    stderrChunks.push(chunk.toString());
  });

  const rejectPending = (error: Error) => {
    for (const { reject } of pending.values()) {
      reject(error);
    }
    pending.clear();
  };

  child.once('error', (error) => {
    rejectPending(new Error(`[mcp:${serverId}] failed to start: ${error.message}`));
  });

  child.once('exit', (code, signal) => {
    if (pending.size === 0) {
      return;
    }
    const stderr = stderrChunks.join('').trim();
    const details = stderr ? ` stderr: ${stderr}` : '';
    rejectPending(
      new Error(
        `[mcp:${serverId}] server exited before responding (code=${code ?? 'null'}, signal=${signal ?? 'null'})${details}`
      )
    );
  });

  const lineReader = readline.createInterface({
    input: child.stdout,
    crlfDelay: Infinity,
  });

  lineReader.on('line', (line) => {
    const trimmed = line.trim();
    if (!trimmed) {
      return;
    }

    try {
      const message = JSON.parse(trimmed) as Partial<McpJsonRpcResponse>;
      if (typeof message.id === 'number' && pending.has(message.id)) {
        const request = pending.get(message.id);
        pending.delete(message.id);
        request?.resolve(message as McpJsonRpcResponse);
      }
    } catch {
      // Ignore non-JSON stdout noise from the child process.
    }
  });

  const writeMessage = (payload: Record<string, unknown>) =>
    new Promise<void>((resolve, reject) => {
      child.stdin.write(`${JSON.stringify(payload)}\n`, (error) => {
        if (error) {
          reject(error);
          return;
        }
        resolve();
      });
    });

  return {
    async request(method: string, params?: Record<string, unknown>): Promise<Record<string, unknown>> {
      const requestId = nextRequestId++;
      const response = await new Promise<McpJsonRpcResponse>((resolve, reject) => {
        pending.set(requestId, {
          resolve,
          reject,
        });
        writeMessage({
          jsonrpc: '2.0',
          id: requestId,
          method,
          params,
        }).catch((error) => {
          pending.delete(requestId);
          reject(
            new Error(`[mcp:${serverId}] failed to write ${method}: ${getMcpErrorMessage(error)}`)
          );
        });
      });
      return assertMcpJsonRpcSuccess(response, serverId, method);
    },
    async notify(method: string, params?: Record<string, unknown>): Promise<void> {
      await writeMessage({
        jsonrpc: '2.0',
        method,
        params,
      });
    },
    async close(): Promise<void> {
      lineReader.close();
      child.stdin.end();
      if (!child.killed) {
        child.kill();
      }
    },
  };
}

async function createHttpMcpSession(serverId: string, server: McpHttpServerConfig) {
  if (!server.endpoint?.trim()) {
    throw new Error(`[mcp:${serverId}] http server is missing an endpoint`);
  }

  let sessionId: string | null = null;

  const sendRequest = async (
    method: string,
    params?: Record<string, unknown>,
    includeId = true
  ): Promise<Record<string, unknown>> => {
    const payload = includeId
      ? {
          jsonrpc: '2.0',
          id: Date.now() + Math.floor(Math.random() * 1000),
          method,
          params,
        }
      : {
          jsonrpc: '2.0',
          method,
          params,
        };

    const headers: Record<string, string> = {
      'content-type': 'application/json',
      accept: 'application/json',
      'mcp-protocol-version': MCP_PROTOCOL_VERSION,
      ...(server.headers ?? {}),
    };

    if (sessionId) {
      headers['mcp-session-id'] = sessionId;
    }

    const response = await fetch(server.endpoint, {
      method: 'POST',
      headers,
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      throw new Error(
        `[mcp:${serverId}] ${method} failed with HTTP ${response.status} ${response.statusText}`
      );
    }

    const nextSessionId = response.headers.get('mcp-session-id');
    if (nextSessionId) {
      sessionId = nextSessionId;
    }

    if (!includeId) {
      return {};
    }

    const json = (await response.json()) as McpJsonRpcResponse;
    return assertMcpJsonRpcSuccess(json, serverId, method);
  };

  return {
    request(method: string, params?: Record<string, unknown>) {
      return sendRequest(method, params, true);
    },
    async notify(method: string, params?: Record<string, unknown>) {
      try {
        await sendRequest(method, params, false);
      } catch {
        // Ignore initialized notification failures for remote servers.
      }
    },
    async close(): Promise<void> {
      sessionId = null;
    },
  };
}

async function withMcpSession<T>(
  serverId: string,
  server: McpServerConfig,
  handler: (session: {
    request(method: string, params?: Record<string, unknown>): Promise<Record<string, unknown>>;
    notify(method: string, params?: Record<string, unknown>): Promise<void>;
    close(): Promise<void>;
  }) => Promise<T>
): Promise<T> {
  const session =
    server.transport === 'stdio'
      ? await createStdioMcpSession(serverId, server)
      : await createHttpMcpSession(serverId, server);

  try {
    await session.request('initialize', {
      protocolVersion: MCP_PROTOCOL_VERSION,
      capabilities: {},
      clientInfo: {
        name: 'script-kit',
        version: SDK_VERSION,
      },
    });
    await session.notify('notifications/initialized', {});
    return await handler(session);
  } finally {
    await session.close();
  }
}

function normalizeMcpToolInfo(serverId: string, tool: Record<string, unknown>): McpToolInfo {
  return {
    serverId,
    name: typeof tool.name === 'string' ? tool.name : 'unknown',
    description: typeof tool.description === 'string' ? tool.description : undefined,
    inputSchema: isRecord(tool.inputSchema)
      ? tool.inputSchema
      : isRecord(tool.input_schema)
        ? tool.input_schema
        : undefined,
  };
}

function normalizeMcpToolCallResult(
  serverId: string,
  toolName: string,
  result: Record<string, unknown>
): McpToolCallResult {
  const content = Array.isArray(result.content)
    ? (result.content as McpToolCallContent[])
    : [];

  return {
    serverId,
    toolName,
    content,
    structuredContent: result.structuredContent ?? result.structured_content,
    isError: typeof result.isError === 'boolean'
      ? result.isError
      : typeof result.is_error === 'boolean'
        ? result.is_error
        : undefined,
    raw: result,
  };
}

function scoreDiscoveredTool(tool: McpToolInfo, query: string): number {
  const haystack = `${tool.serverId} ${tool.name} ${tool.description ?? ''}`.toLowerCase();
  const terms = query
    .toLowerCase()
    .split(/\s+/)
    .map((term) => term.trim())
    .filter(Boolean);

  let score = 0;
  for (const term of terms) {
    if (tool.name.toLowerCase().includes(term)) {
      score += 3;
    }
    if (tool.serverId.toLowerCase().includes(term)) {
      score += 2;
    }
    if (haystack.includes(term)) {
      score += 1;
    }
  }

  return score;
}

globalThis.mcp = {
  async listServers(): Promise<McpServerInfo[]> {
    const config = await loadScriptKitMcpConfig();
    return getEnabledMcpServerEntries(config).map(([id, server]) => toMcpServerInfo(id, server));
  },

  async getServer(id: string): Promise<McpServerInfo | null> {
    const server = await getConfiguredMcpServer(id);
    return server ? toMcpServerInfo(id, server) : null;
  },

  async listTools(serverId?: string): Promise<McpToolInfo[]> {
    const config = await loadScriptKitMcpConfig();
    const entries = serverId
      ? (() => {
          const server = getEnabledMcpServerEntries(config).find(([id]) => id === serverId);
          return server ? [server] : [];
        })()
      : getEnabledMcpServerEntries(config);

    const toolGroups = await Promise.all(
      entries.map(async ([id, server]) => {
        const result = await withMcpSession(id, server, async (session) => {
          const response = await session.request('tools/list', {});
          return Array.isArray(response.tools)
            ? response.tools.map((tool) => normalizeMcpToolInfo(id, tool as Record<string, unknown>))
            : [];
        });
        return result;
      })
    );

    return toolGroups.flat();
  },

  async discover(query: string): Promise<McpToolInfo[]> {
    const tools = await globalThis.mcp.listTools();
    const scored = tools
      .map((tool) => ({
        tool,
        score: scoreDiscoveredTool(tool, query),
      }))
      .filter((entry) => entry.score > 0)
      .sort((left, right) => right.score - left.score || left.tool.name.localeCompare(right.tool.name));

    return scored.map((entry) => entry.tool);
  },

  async call(
    serverId: string,
    toolName: string,
    args: Record<string, unknown> = {}
  ): Promise<McpToolCallResult> {
    const server = await getConfiguredMcpServer(serverId);
    if (!server) {
      throw new Error(`[mcp:${serverId}] server is not configured or is disabled`);
    }

    return withMcpSession(serverId, server, async (session) => {
      const response = await session.request('tools/call', {
        name: toolName,
        arguments: args,
      });
      return normalizeMcpToolCallResult(serverId, toolName, response);
    });
  },
} satisfies McpApi;

export {};
