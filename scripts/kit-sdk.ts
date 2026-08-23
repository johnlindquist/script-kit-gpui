import * as readline from 'node:readline';
import * as nodePath from 'node:path';
import * as os from 'node:os';
import * as fs from 'node:fs/promises';
import { constants as fsConstants } from 'node:fs';
import { spawn as spawnChildProcess } from 'node:child_process';

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

/** Wire version published by the Rust-owned kit://sdk-reference capability catalog. */
export const SDK_CAPABILITY_CATALOG_SCHEMA_VERSION = 1;

/** Read-only MCP resource identifiers, not callable SDK functions. */
export const COMMAND_DOCTOR_RESOURCE_URI = 'kit://command-doctor';
export const FAILED_SCRIPTS_RESOURCE_URI = 'kit://failed-scripts';
export const COMMAND_DOCTOR_RESOURCE_SCHEMA_VERSION = 1;

export type SdkCapabilitySupport = 'supported' | 'experimental' | 'unsupported';

export type SdkExecutionTopology =
  | 'typescript-script'
  | 'typescript-scriptlet'
  | 'typescript-scriptlet-interactive'
  | 'shell-scriptlet'
  | 'python-scriptlet';

export interface HostSdkCapability {
  name: string;
  support: SdkCapabilitySupport;
  /** First host version guaranteed to expose this reviewed contract, not its historical debut. */
  minimumHostVersion: string;
  requiresInteractivePrompt: boolean;
  requiredPermissions?: string[];
  supportedPlatforms?: string[];
  alternatives?: string[];
  migrationNote?: string;
}

export interface HostSdkCapabilityCatalog {
  schemaVersion: typeof SDK_CAPABILITY_CATALOG_SCHEMA_VERSION;
  hostVersion: string;
  capabilities: HostSdkCapability[];
}

/** Host MCP resource discoverable under kit://sdk-reference.authoringResources. */
export interface HostSdkAuthoringResource {
  uri: string;
  name: string;
  description: string;
}

export type SdkCapabilityDiagnosticCode =
  | 'unknown_capability'
  | 'unsupported_capability'
  | 'missing_sdk_transport'
  | 'interactive_prompt_unavailable'
  | 'unsupported_platform'
  | 'missing_permission'
  | 'permission_inventory_unavailable'
  | 'host_version_too_old'
  | 'invalid_host_version';

/** Explicit host facts supplied by an already-authorized, read-only inventory. */
export interface HostSdkAvailability {
  hostVersion: string;
  platform: string;
  grantedPermissions: string[];
}

export interface SdkCapabilityDiagnostic {
  code: SdkCapabilityDiagnosticCode;
  capability: string;
  message: string;
  alternatives?: string[];
}

export type HostCommandDoctorState =
  | 'ready'
  | 'experimental'
  | 'unsupported'
  | 'blocked'
  | 'permissionPending';

export type HostScriptValidationKind =
  | { kind: 'metadataParse' | 'schemaParse'; detail: string }
  | { kind: 'invalidValue'; value: string; reason: string }
  | { kind: 'duplicateBinding'; binding: 'shortcut' | 'alias' | 'keyword' | 'trigger'; value: string }
  | {
      kind: 'capabilityUnavailable';
      capability: string;
      code: SdkCapabilityDiagnosticCode;
      alternatives?: string[];
    };

export interface HostScriptValidationIssue {
  severity: 'fatal' | 'warning';
  path: string;
  scriptName: string;
  field?: string;
  message: string;
  kind: HostScriptValidationKind;
  related?: { path: string; name: string }[];
}

export interface HostCommandDoctorCapability {
  name: string;
  support: SdkCapabilitySupport;
  requiredPermissions?: string[];
  supportedPlatforms?: string[];
  minimumHostVersion?: string;
  alternatives?: string[];
}

/** The existing launcher's actual primary action; identity is never raw. */
export interface HostCommandDoctorPrimaryAction {
  title: string;
  enabled: boolean;
  reason?: string;
  identityFingerprint: `sha256:${string}`;
}

export interface HostCommandDoctorEntry {
  source: string;
  name: string;
  path: string;
  pluginId: string;
  pluginTitle?: string;
  state: HostCommandDoctorState;
  executable: boolean;
  primaryAction?: HostCommandDoctorPrimaryAction;
  capabilities?: HostCommandDoctorCapability[];
  issues?: HostScriptValidationIssue[];
  alternatives?: string[];
}

/** Receipt returned by reading the host's kit://command-doctor MCP resource. */
export interface HostCommandDoctorReport {
  schemaVersion: typeof COMMAND_DOCTOR_RESOURCE_SCHEMA_VERSION;
  hostVersion: string;
  platform: string;
  permissionInventoryKnown: boolean;
  totalCommands: number;
  readyCount: number;
  experimentalCount: number;
  unsupportedCount: number;
  blockedCount: number;
  permissionPendingCount: number;
  commands: HostCommandDoctorEntry[];
}

/** Captured output from an explicit, shell-free child process. */
export interface ExecResult {
  stdout: string;
  stderr: string;
  exitCode: number;
}

export type SdkExecErrorCode =
  | 'ERR_SDK_EXEC_INVALID_COMMAND'
  | 'ERR_SDK_EXEC_SPAWN_FAILED'
  | 'ERR_SDK_EXEC_NONZERO_EXIT';

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
 * ✅ IMPLEMENTED GLOBALS AND NAMESPACE METHODS:
 *    - Core prompts: arg, div, editor, term, form, fields, select, confirm,
 *      path, drop, template, env, chat, mini, micro, hotkey
 *    - Window control: show, hide, captureScreenshot, getLayoutInfo, hud
 *    - Clipboard: read/write text/images, clipboard history with all operations
 *    - Window management: getWindows, focusWindow, tileWindow, getDisplays (macOS)
 *    - AI integration: aiIsOpen, aiStartChat, aiSendMessage, aiListChats, etc.
 *    - File/menu: fileSearch, getMenuBar, executeMenuAction
 *    - Pure utilities: home, skPath, uuid, compile, memoryMap
 *    - Explicit filesystem/subprocess helpers: readFile, writeFile, exec
 *
 * ⚠️  UNSUPPORTED COMPATIBILITY HELPERS:
 *    Input/prompt/media helpers reject with UnsupportedSdkFeatureError before
 *    dispatch; setStatus()/menu() return an explicit unsupported feedback
 *    receipt. None claims a successful native behavior or delivery contract.
 *
 *    - Status/menu: setStatus(), menu()
 *    - Automation: keyboard.type(), keyboard.tap(), mouse.move(),
 *      mouse.leftClick(), mouse.rightClick(), mouse.setPosition()
 *    - UI updates: setPanel(), setPreview(), setPrompt()
 *    - Advanced: widget(), find()
 *
 * ❌ UNSUPPORTED MEDIA/CAPTURE HELPERS:
 *    Compatibility globals remain discoverable but reject before dispatch:
 *
 *    - webcam(), mic() - Media streaming requires real-time bidirectional
 *      communication beyond the JSONL protocol
 *    - eyeDropper() - Could work with screen capture but not yet implemented
 *
 * EXTENSION SUPPORT:
 *    - TypeScript scripts (.ts, .js): Full SDK access with stdin/stdout
 *    - Launcher TypeScript scriptlets: Interactive SDK transport; prompts work
 *    - Legacy TypeScript scriptlets: SDK loaded without interactive response
 *      transport; prompt capabilities are rejected before execution
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
  type?:
    | 'text'
    | 'password'
    | 'email'
    | 'number'
    | 'date'
    | 'time'
    | 'datetime-local'
    | 'month'
    | 'week'
    | 'search'
    | 'url'
    | 'tel'
    | 'color';
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
  /** Restrict supported fileSearch() requests to a search root. find() rejects before send. */
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
  /** How bounds were obtained; `paint-time` identifies real rendered-frame geometry. */
  measurementProvenance?: string;
  /** Coordinate system for bounds; fidelity measurements use window logical pixels. */
  coordinateSpace?: "window" | "screen" | string;
  /** Portion of bounds that survives the paint-time ancestor clip chain. */
  visibleBounds?: LayoutBounds;
  /** Paint-time ancestor content mask used to derive visibleBounds. */
  clipBounds?: LayoutBounds;
  /** Monotonic identity of the completed rendered frame supplying these bounds. */
  measurementFrameGeneration?: number;
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
// Config Types (for ~/.scriptkit/config.ts)
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

/** Prompt handoff target command ID: `prompt-target/{target-id}` */
export type PromptTargetCommandId = `prompt-target/${string}`;

/** Built prompt action command ID: `prompt-action/{action-id}` */
export type PromptActionCommandId = `prompt-action/${string}`;

/** Union of all valid command ID formats. */
export type CommandId =
  | BuiltinCommandId
  | AppCommandId
  | ScriptCommandId
  | ScriptletCommandId
  | PromptTargetCommandId
  | PromptActionCommandId;

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
 * User-defined prompt handoff target.
 *
 * Prompt targets appear in Actions as `prompt-target/{id}` commands and can be
 * assigned shortcuts through `commands`.
 */
export interface PromptTargetConfig {
  title?: string;
  description?: string;
  command: string;
  args?: string[];
  cwd?: string;
  env?: Record<string, string>;
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

export type Cmd1Behavior = "picker" | "cycle";
export type DesignDensityChoice = "compact" | "comfortable" | "spacious";
export type FontFamilyChoice = "system" | "monospace" | "serif";
export type VibrancyChoice = "none" | "light" | "medium" | "heavy";
export type ChromeOpacityChoice = "low" | "med" | "high";
export type IconStyleChoice = "mono" | "color" | "hidden";
export type SeparatorStyleChoice = "none" | "hairline" | "rule" | "grid";

export interface DesignOverrides {
  /** Hex accent color override, e.g. "#ff8800". */
  accent?: string;
  /** Row density override for this design. */
  density?: DesignDensityChoice;
  /** Font family class override for this design. */
  fontFamily?: FontFamilyChoice;
  /** Small signed font scale nudge. */
  fontScale?: number;
  /** Window/background vibrancy intensity. */
  vibrancy?: VibrancyChoice;
  /** Chrome opacity preset. */
  chromeOpacity?: ChromeOpacityChoice;
  /** Icon rendering style. */
  iconStyle?: IconStyleChoice;
  /** Separator rendering style. */
  separatorStyle?: SeparatorStyleChoice;
  /** Small signed row height nudge. */
  rowHeightNudge?: number;
}

export interface DesignsConfig {
  /** Stable catalog id for the active design. */
  activeId?: string;
  /** Cmd+1 behavior: open the picker or cycle designs. */
  cmd1Behavior?: Cmd1Behavior;
  /** Per-design overrides keyed by stable catalog id. */
  overrides?: Record<string, DesignOverrides>;
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
 * Script Kit-managed MCP server definitions shared by scripts and Agent Chat.
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
  /** Safe public metadata; credential-like environment values are redacted. */
  env?: Record<string, string>;
  cwd?: string;
  endpoint?: string;
  /** Safe public metadata; authorization and credential values are redacted. */
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

export interface ComputerUseBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ComputerUseRunningApp {
  pid: number;
  bundleId?: string | null;
  name: string;
  isActive: boolean;
  isHidden: boolean;
  activationPolicy: string;
}

export interface ComputerUseNativeWindow {
  nativeWindowId: number;
  title?: string | null;
  bounds: ComputerUseBounds;
  isOnScreen: boolean;
  layer: number;
  zOrder: number;
  observation?: Record<string, unknown> | null;
}

export interface ComputerUseNativeWindowsForApp {
  app: ComputerUseRunningApp;
  status: string;
  windows: ComputerUseNativeWindow[];
  warnings: string[];
}

export interface ComputerUseListNativeWindowsOptions {
  includeHidden?: boolean;
  includeBackground?: boolean;
}

export interface ComputerUseListNativeWindowsResult {
  schemaVersion: number;
  source: string;
  scope: string;
  status: 'listed' | 'partial' | string;
  frontmostPid?: number;
  appCount: number;
  windowCount: number;
  apps: ComputerUseNativeWindowsForApp[];
  warnings: string[];
}

export interface ComputerUseCaptureNativeWindowOptions {
  pid: number;
  nativeWindowId: number;
  hiDpi?: boolean;
  includeImage?: boolean;
  expectedBundleId?: string;
}

export type ComputerUseCaptureNativeWindowStatus =
  | 'captured'
  | 'appNotFound'
  | 'windowNotFound'
  | 'ownershipMismatch'
  | 'notCaptureCandidate'
  | 'ambiguousNativeWindowRows'
  | 'ambiguousNativeWindowId'
  | 'permissionDenied'
  | 'blankImageRejected'
  | 'captureFailed';

export interface ComputerUseCapturePixelAudit {
  sampled: number;
  nonBlack: number;
  nonTransparent: number;
  uniqueBucketCount: number;
  meanLuma: number;
  blankLike: boolean;
}

export interface ComputerUseNativeWindowCapture {
  mimeType: 'image/png' | string;
  width: number;
  height: number;
  byteLength: number;
  sha256: string;
  hiDpi: boolean;
  pixelAudit: ComputerUseCapturePixelAudit;
  pngBase64?: string;
}

export interface ComputerUseCaptureNativeWindowError {
  code: string;
  message: string;
  reason?: string;
  pixelAudit?: ComputerUseCapturePixelAudit;
}

export interface ComputerUseCaptureNativeWindowResult {
  schemaVersion: number;
  source: string;
  scope: string;
  status: ComputerUseCaptureNativeWindowStatus | string;
  correlationId: string;
  app?: ComputerUseRunningApp;
  window?: ComputerUseNativeWindow;
  capture?: ComputerUseNativeWindowCapture;
  error?: ComputerUseCaptureNativeWindowError;
  warnings: string[];
}

export interface ComputerUseApi {
  listNativeWindows(
    options?: ComputerUseListNativeWindowsOptions
  ): Promise<ComputerUseListNativeWindowsResult>;
  captureNativeWindow(
    options: ComputerUseCaptureNativeWindowOptions
  ): Promise<ComputerUseCaptureNativeWindowResult>;
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
 * Theme preset selection stored in `~/.scriptkit/config.ts`.
 */
export interface ThemeSelectionPreferences {
  /**
   * Optional preset identifier, for example `"catppuccin-mocha"`.
   */
  presetId?: string;
}

/**
 * Dictation runtime preferences stored in `~/.scriptkit/config.ts`.
 */
export interface DictationPreferences {
  /**
   * Persisted microphone device ID.
   * Omit this field to use the macOS default microphone.
   *
   * @example "usb-mic"
   */
  selectedDeviceId?: string;

  /**
   * Dictation model catalog ID.
   * Omit this field to use the default model.
   *
   * @default undefined (default model)
   * @example "whisper-large-v3-turbo"
   */
  model?: string;

  /**
   * Language hint for engines that support one (Whisper only — Parakeet
   * auto-detects its supported languages).
   *
   * @default undefined (auto-detect)
   * @example "en"
   */
  language?: string;

  /**
   * Whether transcripts are appended to `dictation-history.jsonl`.
   * Disable to keep dictated text out of plaintext history.
   *
   * @default true
   * @example false // Keep dictations out of plaintext history
   */
  saveHistory?: boolean;

  /**
   * RMS energy threshold below which a capture is treated as silence and
   * skipped. Lower this if a quiet microphone causes dictations to be
   * silently dropped. Clamped to 0.0-0.5 at runtime.
   *
   * @default 0.01
   * @example 0.005 // More sensitive (quiet microphone)
   */
  silenceRms?: number;

  /**
   * Auto-stop guard for runaway recordings, in seconds.
   * Set to 0 to disable the guard. Clamped to 30-14400 at runtime.
   *
   * @default 600
   * @example 1800 // Allow 30-minute recordings
   */
  maxDurationSecs?: number;

  /**
   * Live partial transcript preview in the recording overlay.
   *
   * @default true
   * @example false // Hide partial transcripts while recording
   */
  livePreview?: boolean;

  /**
   * Hold-the-dictation-hotkey push-to-talk: release after holding to stop
   * and transcribe; a quick tap keeps toggle behavior.
   *
   * @default true
   * @example false // Hotkey always toggles recording
   */
  pushToTalk?: boolean;
}

/**
 * Agent Chat runtime preferences stored in `~/.scriptkit/config.ts`.
 */
export type AgentChatBackend = "agent_chat" | "pi";

/**
 * Structured Pi tool policy for an Agent Chat profile.
 * `allow` supersedes the legacy `tools` array.
 */
export interface AgentChatToolPolicyConfig {
  /**
   * Tool allow-list. `[]` means no tools; omit to keep Pi defaults.
   *
   * @example ["read", "grep"]
   */
  allow?: string[];
}

/**
 * Runtime filesystem scope forwarded to Pi for read/write-capable tools.
 */
export interface AgentChatPathPolicyConfig {
  /**
   * Paths (or globs) readable by the profile's tools.
   *
   * @example ["~/notes/**"]
   */
  allowRead?: string[];
  /**
   * Paths (or globs) writable by the profile's tools.
   *
   * @example ["~/notes/drafts/**"]
   */
  allowWrite?: string[];
  /**
   * Paths (or globs) denied even when covered by an allow rule.
   *
   * @example ["~/.ssh/**"]
   */
  deny?: string[];
}

export interface AiProfile {
  id?: string;
  name: string;
  iconName?: string;
  backend?: AgentChatBackend;
  piBinary?: string;
  agent?: string;
  provider?: string;
  model?: string;
  systemPrompt?: string;
  appendSystemPrompt?: string;
  cwd?: string;
  tools?: string[];
  toolPolicy?: AgentChatToolPolicyConfig;
  pathPolicy?: AgentChatPathPolicyConfig;
  blockedActionMessage?: string;
  disableExtensions?: boolean;
  disableSkills?: boolean;
  disablePromptTemplates?: boolean;
  disableContextFiles?: boolean;
  hideCwdInPrompt?: boolean;
  thinking?: string;
  extensionPolicy?: string;
  sessionDir?: string;
  noSession?: boolean;
  sessionDurability?: string;
}

export interface AiPreferences {
  /**
   * Last-selected model ID.
   *
   * @example "gpt-5.4"
   */
  selectedModelId?: string;

  /**
   * Legacy last-selected Agent Chat agent ID.
   *
   * @example "codex-agent_chat"
   */
  selectedAgentChatAgentId?: string;

  /**
   * Last-selected Agent Chat profile id.
   */
  selectedProfileId?: string;

  /**
   * Profile used by the launcher's Tab "Quick AI" mode. Omit (or use
   * "quick-ai") for the pinned fast zero-context default.
   */
  quickAiProfileId?: string;

  /**
   * Last-selected Agent Chat backend.
   */
  selectedBackend?: AgentChatBackend;

  /**
   * Named Agent Chat configuration profiles.
   */
  profiles?: AiProfile[];

  /**
   * Legacy active profile name.
   */
  selectedProfileName?: string;
}

export type SnapMode = "off" | "simple" | "expanded" | "precision";

/**
 * Window-management preferences stored in `~/.scriptkit/config.ts`.
 */
export interface WindowManagementPreferences {
  /**
   * Drag-snap density/mode for desktop tiling.
   */
  snapMode?: SnapMode;
}

/**
 * Hard clipboard secret-rejection extensions (see ADR 0004).
 * Conservative built-in rules always apply; these lists only extend them.
 *
 * @example
 * ```typescript
 * clipboardHistorySecretRejection: {
 *   extraBlockedSourceApps: ["com.example.passwordmanager"],
 *   extraSecretPatterns: ["^corp-token-[A-Za-z0-9]{32}$"]
 * }
 * ```
 */
export interface ClipboardHistorySecretRejectionConfig {
  /**
   * Additional bundle IDs (prefix match) whose clipboard copies are never
   * stored. Defaults already cover common password managers.
   *
   * @default []
   * @example ["com.example.passwordmanager"]
   */
  extraBlockedSourceApps?: string[];

  /**
   * Additional regex patterns for secret-shaped clipboard text rejected
   * before storage.
   *
   * @default []
   * @example ["^corp-token-[A-Za-z0-9]{32}$"]
   */
  extraSecretPatterns?: string[];
}

/**
 * Background shader-effect preferences.
 *
 * @example
 * ```typescript
 * effects: {
 *   background: "aurora",
 *   intensity: 0.5
 * }
 * ```
 */
export interface EffectsPreferences {
  /**
   * Background effect slug (for example: "aurora").
   * Omit this field for the install default (Starfield).
   * Set "off" to disable background effects entirely.
   *
   * @default "starfield"
   * @example "aurora"
   * @example "off" // disable effects
   */
  background?: string;

  /**
   * Effect strength in the range 0.0 to 1.0.
   *
   * @default 0.5
   * @example 0.25 // Subtler effect
   */
  intensity?: number;
}

/**
 * A user-defined `.style` for the spine prompt builder.
 * Merged over the built-in catalog; matching ids override built-ins.
 * The `instruction` is the payload delivered to the agent.
 *
 * @example
 * ```typescript
 * spineStyles: [
 *   {
 *     id: "haiku",
 *     title: "Haiku",
 *     description: "Answer as a haiku",
 *     instruction: "Respond only with a single haiku."
 *   }
 * ]
 * ```
 */
export interface SpineStyleConfig {
  /**
   * Stable style id used by the `.style` sigil.
   *
   * @example "haiku"
   */
  id: string;

  /**
   * Display title shown in the style picker.
   *
   * @default undefined (falls back to the id)
   * @example "Haiku"
   */
  title?: string;

  /**
   * Short description shown next to the style.
   *
   * @default undefined
   * @example "Answer as a haiku"
   */
  description?: string;

  /**
   * Icon name shown in the style picker.
   *
   * @default undefined
   * @example "sparkles"
   */
  icon?: string;

  /**
   * Instruction text delivered to the agent when the style is applied.
   *
   * @example "Respond only with a single haiku."
   */
  instruction: string;
}

/**
 * A user-defined `/command` for the spine prompt builder.
 * Merged over the built-in catalog; matching names override built-ins.
 *
 * @example
 * ```typescript
 * spineCommands: [
 *   { name: "standup", description: "Draft my daily standup" }
 * ]
 * ```
 */
export interface SpineCommandConfig {
  /**
   * Command name used by the `/command` sigil.
   *
   * @example "standup"
   */
  name: string;

  /**
   * Short description shown next to the command.
   *
   * @default undefined
   * @example "Draft my daily standup"
   */
  description?: string;

  /**
   * Icon name shown in the command picker.
   *
   * @default undefined
   * @example "calendar"
   */
  icon?: string;
}

/**
 * Opt-in remote access to the brain (local memory) over Telegram.
 *
 * SECURITY: the bot token grants remote capture and queries into your local
 * memory. It is treated as a secret (never logged, redacted from errors).
 * The bridge only runs when ALL THREE hold: `enabled` is true, a bot token
 * is configured, and `telegramAllowedUserIds` is non-empty. An empty
 * allowlist disables the bot entirely — there is no "open" mode.
 *
 * @example
 * ```typescript
 * brainRemote: {
 *   enabled: true,
 *   telegramBotToken: "123456:ABC-DEF...",
 *   telegramAllowedUserIds: [123456789]
 * }
 * ```
 */
export interface BrainRemoteConfig {
  /**
   * Master switch for the Telegram bridge.
   *
   * @default false
   * @example true
   */
  enabled?: boolean;

  /**
   * Telegram bot token from @BotFather. Treated as a secret: never logged,
   * and redacted from any error output.
   *
   * @default undefined (bridge inactive)
   * @example "123456:ABC-DEF..."
   */
  telegramBotToken?: string;

  /**
   * Numeric Telegram user ids allowed to query the brain. Empty disables
   * the bridge entirely; messages from any unlisted user are rejected with
   * a hint showing their own id.
   *
   * @default []
   * @example [123456789]
   */
  telegramAllowedUserIds?: number[];
}

/**
 * Script Kit configuration schema.
 *
 * This configuration is loaded from `~/.scriptkit/config.ts` and controls
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
   * Hard secret-rejection extensions for clipboard history (ADR 0004).
   * Extends the built-in blocklists; validated before anything is stored.
   *
   * @default { extraBlockedSourceApps: [], extraSecretPatterns: [] }
   * @example { extraBlockedSourceApps: ["com.example.passwordmanager"] }
   * @example { extraSecretPatterns: ["^corp-token-[A-Za-z0-9]{32}$"] }
   */
  clipboardHistorySecretRejection?: ClipboardHistorySecretRejectionConfig;

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
   * Falls back to Cmd+Ctrl+N when enabled and unset.
   *
   * @default { modifiers: ["meta", "ctrl"], key: "KeyN" }
   * @example { modifiers: ["meta", "shift"], key: "KeyN" } // Cmd+Shift+N
   */
  notesHotkey?: HotkeyConfig;

  /**
   * Whether the Notes hotkey should be registered.
   *
   * @default true
   * @example false // Disable the Notes global shortcut
   */
  notesHotkeyEnabled?: boolean;

  /**
   * Hotkey for opening Agent Chat.
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
   * Falls back to Cmd+Shift+; when enabled and not explicitly set.
   *
   * @default { modifiers: ["meta", "shift"], key: "Semicolon" }
   */
  dictationHotkey?: HotkeyConfig;

  /**
   * Whether dictation hotkey registration is enabled.
   *
   * @default true
   */
  dictationHotkeyEnabled?: boolean;

  /**
   * Hotkey for inline AI focused-text editing.
   * Captures the focused text field in the frontmost macOS app, then shows
   * the inline agent overlay. Falls back to Cmd+Ctrl+I when enabled and unset.
   *
   * @default { modifiers: ["meta", "ctrl"], key: "KeyI" }
   */
  inlineAiHotkey?: HotkeyConfig;

  /**
   * Whether the inline AI focused-text hotkey should be registered.
   *
   * @default true
   */
  inlineAiHotkeyEnabled?: boolean;

  /**
   * Hotkey for the instant rewrite flow.
   * Captures the focused text and immediately streams three rewrite
   * variations in the mini UI. Falls back to Cmd+Ctrl+R when enabled and
   * unset; change it if you need Xcode's Cmd+Ctrl+R.
   *
   * @default { modifiers: ["meta", "ctrl"], key: "KeyR" }
   * @example { modifiers: ["meta", "shift"], key: "KeyR" } // Cmd+Shift+R
   */
  rewriteHotkey?: HotkeyConfig;

  /**
   * Whether the instant rewrite hotkey should be registered.
   *
   * @default true
   */
  rewriteHotkeyEnabled?: boolean;

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
   * Theme colors still live in `~/.scriptkit/theme.json`.
   *
   * @default undefined
   */
  theme?: ThemeSelectionPreferences;

  /**
   * Design picker preferences and per-design token overrides.
   *
   * @default undefined
   * @example
   * ```typescript
   * designs: {
   *   activeId: "script-kit-classic",
   *   cmd1Behavior: "picker",
   *   overrides: {
   *     "script-kit-classic": {
   *       density: "comfortable",
   *       iconStyle: "mono"
   *     }
   *   }
   * }
   * ```
   */
  designs?: DesignsConfig;

  /**
   * Dictation runtime preferences, including the selected microphone device.
   *
   * @default undefined
   */
  dictation?: DictationPreferences;

  /**
   * Agent Chat runtime preferences, including the selected profile and model.
   *
   * @default undefined
   */
  ai?: AiPreferences;

  /**
   * User-defined spine prompt-builder styles (`.style` sigil). Merged over
   * the built-in catalog; matching ids override built-ins.
   *
   * @default undefined (built-in styles only)
   * @example
   * ```typescript
   * spineStyles: [
   *   {
   *     id: "haiku",
   *     title: "Haiku",
   *     description: "Answer as a haiku",
   *     instruction: "Respond only with a single haiku."
   *   }
   * ]
   * ```
   */
  spineStyles?: SpineStyleConfig[];

  /**
   * User-defined spine slash commands (`/command` sigil). Merged over the
   * built-in catalog; matching names override built-ins.
   *
   * @default undefined (built-in commands only)
   * @example
   * ```typescript
   * spineCommands: [
   *   { name: "standup", description: "Draft my daily standup" }
   * ]
   * ```
   */
  spineCommands?: SpineCommandConfig[];

  /**
   * Window-management preferences such as drag-snap mode.
   *
   * @default undefined
   */
  windowManagement?: WindowManagementPreferences;

  /**
   * Background shader-effect preferences for the launcher window.
   *
   * @default undefined (no effect; intensity 0.5 once an effect is set)
   * @example
   * ```typescript
   * effects: {
   *   background: "aurora",
   *   intensity: 0.5
   * }
   * ```
   */
  effects?: EffectsPreferences;

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
   * User-defined prompt handoff targets shown in the Actions menu.
   *
   * @example
   * ```typescript
   * promptTargets: {
   *   "my-app": {
   *     title: "My App",
   *     command: "/usr/local/bin/my-app",
   *     args: ["--prompt", "{prompt}"]
   *   }
   * },
   * commands: {
   *   "prompt-target/my-app": {
   *     shortcut: { modifiers: ["meta", "shift"], key: "KeyM" }
   *   },
   *   "prompt-action/export-file": {
   *     shortcut: { modifiers: ["meta", "shift"], key: "KeyE" }
   *   },
   *   "prompt-action/copy-prompt": {
   *     shortcut: { modifiers: ["meta", "shift"], key: "KeyC" }
   *   },
   *   "prompt-action/export-gist": {
   *     shortcut: { modifiers: ["meta", "shift"], key: "KeyG" }
   *   }
   * }
   * ```
   */
  promptTargets?: Record<string, PromptTargetConfig>;

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
   * External MCP servers that scripts and Agent Chat can use.
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

  /**
   * Canonical command IDs to hide from the launcher main menu.
   *
   * Hidden commands remain resolvable via `triggerBuiltin`, hotkeys, and
   * deeplinks — this only filters them from visible lists.
   *
   * @default undefined (no commands hidden)
   * @example ["builtin/clipboard-history", "script/deprecated-tool"]
   */
  hiddenCommands?: string[];

  /**
   * Opt-in Telegram remote access to the brain (local memory).
   *
   * SECURITY: the bot token grants remote capture and queries into your
   * local memory — treat it as a secret. The bridge stays inactive unless
   * `enabled` is true, a bot token is set, AND `telegramAllowedUserIds` is
   * non-empty.
   *
   * @default undefined (bridge disabled)
   * @example
   * ```typescript
   * brainRemote: {
   *   enabled: true,
   *   telegramBotToken: "123456:ABC-DEF...",
   *   telegramAllowedUserIds: [123456789]
   * }
   * ```
   */
  brainRemote?: BrainRemoteConfig;
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
  /** Text-expansion keyword trigger; `snippet`/`expand` are compatibility aliases. */
  keyword?: string;
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
  /** Offer this command when no other launcher results match the current input. */
  fallback?: boolean;
  /** Fallback row label; use `{input}` to interpolate the current launcher text. */
  fallbackLabel?: string;
  /** Explicit SDK globals/object methods required by this script; validated before indexing. */
  sdkCapabilities?: string[];
  /** Declared transport topology; interactive prompts require `typescript-script`. */
  executionTopology?: SdkExecutionTopology;
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
  /** Read the current clipboard image as PNG-encoded bytes. */
  readImage(): Promise<Buffer>;
  /** Decode PNG or JPEG bytes and write a real image payload to the clipboard. */
  writeImage(buffer: Buffer): Promise<void>;
}

export interface KeyboardAPI {
  type(text: string): Promise<never>;
  tap(...keys: string[]): Promise<never>;
}

export interface MouseAPI {
  move(positions: Position[]): Promise<never>;
  leftClick(): Promise<never>;
  rightClick(): Promise<never>;
  setPosition(position: Position): Promise<never>;
}

export type SystemFeedbackApi = 'beep' | 'say' | 'notify' | 'setStatus' | 'menu';
export type SystemFeedbackStatus = 'dispatched' | 'unsupported' | 'invalid' | 'failed' | 'timeout';
export type SystemFeedbackCode =
  | 'ERR_UNSUPPORTED_SDK_FEATURE'
  | 'ERR_SYSTEM_FEEDBACK_EMPTY'
  | 'ERR_SYSTEM_FEEDBACK_DISPATCH_FAILED'
  | 'ERR_SYSTEM_FEEDBACK_TIMEOUT';

export interface SystemFeedbackResult {
  ok: boolean;
  api: SystemFeedbackApi;
  status: SystemFeedbackStatus;
  requestId?: string;
  backend?: string;
  platform?: string;
  sideEffect?: string;
  delivery?: 'not_verified';
  code?: SystemFeedbackCode;
  message?: string;
  alternative?: 'hud' | 'setActions';
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

interface EnvConfig {
  hint?: string;
  placeholder?: string;
  secret?: boolean;
}

interface EnvMessage {
  type: 'env';
  id: string;
  key: string;
  prompt?: string;
  secret?: boolean;
}

// System feedback message types. requestId enables app-originated dispatch receipts.
interface BeepMessage {
  type: 'beep';
  requestId?: string;
}

interface SayMessage {
  type: 'say';
  requestId?: string;
  text: string;
  voice?: string;
}

interface NotifyMessage {
  type: 'notify';
  requestId?: string;
  title?: string;
  body?: string;
}

interface HudMessage {
  type: 'hud';
  text: string;
  duration_ms?: number;
}

interface ClipboardMessage {
  type: 'clipboard';
  id: string;
  action: 'read' | 'write';
  format: 'text' | 'image';
  content?: string;
}

type ClipboardErrorCode =
  | 'ERR_CLIPBOARD_IMAGE_NOT_AVAILABLE'
  | 'ERR_CLIPBOARD_ACCESS_FAILED'
  | 'ERR_CLIPBOARD_IMAGE_DECODE_FAILED'
  | 'ERR_CLIPBOARD_IMAGE_ENCODE_FAILED'
  | 'ERR_CLIPBOARD_IMAGE_WRITE_FAILED'
  | 'ERR_CLIPBOARD_IMAGE_MISSING_CONTENT'
  | 'ERR_CLIPBOARD_NO_RESPONSE';

class ClipboardProtocolError extends Error {
  code: ClipboardErrorCode | string;
  operation: string;

  constructor(operation: string, code: string, message: string) {
    super(message || code);
    this.name = 'ClipboardProtocolError';
    this.code = code;
    this.operation = operation;
  }
}

function clipboardErrorFromSubmitValue(
  operation: string,
  value: string | null | undefined,
): ClipboardProtocolError | null {
  if (typeof value !== 'string' || !value.startsWith('ERROR:')) {
    return null;
  }

  const rest = value.slice('ERROR:'.length);
  const separator = rest.indexOf(':');
  const code = separator === -1 ? rest : rest.slice(0, separator);
  const message = separator === -1 ? rest : rest.slice(separator + 1);
  return new ClipboardProtocolError(operation, code, message);
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
// Agent Chat SDK API Message Types
// =============================================================================

/** Typed context part for Agent Chat — matches Rust AiContextPartInput */
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
  subscriptionId: string;
}

interface AiUnsubscribedMessage {
  type: 'aiUnsubscribed';
  requestId: string;
  subscriptionId: string;
  success: boolean;
  error?: string;
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

export type UnsupportedSdkFeatureCode = 'ERR_UNSUPPORTED_SDK_FEATURE';

export interface UnsupportedSdkFeatureDetails {
  code: UnsupportedSdkFeatureCode;
  supported: false;
  feature: string;
  alternatives: string[];
}

export class UnsupportedSdkFeatureError extends Error {
  readonly code: UnsupportedSdkFeatureCode = 'ERR_UNSUPPORTED_SDK_FEATURE';
  readonly supported = false as const;
  readonly feature: string;
  readonly alternatives: string[];

  constructor(feature: string, alternatives: string[]) {
    super(`${feature} is unsupported in Script Kit GPUI. Use ${alternatives.join(' or ')}.`);
    this.name = 'UnsupportedSdkFeatureError';
    this.feature = feature;
    this.alternatives = alternatives;
  }

  toJSON(): UnsupportedSdkFeatureDetails {
    return {
      code: this.code,
      supported: this.supported,
      feature: this.feature,
      alternatives: this.alternatives,
    };
  }
}

export class SdkExecError extends Error {
  readonly code: SdkExecErrorCode;
  readonly command: string;
  readonly args: string[];
  readonly result?: ExecResult;

  constructor(
    code: SdkExecErrorCode,
    message: string,
    command: string,
    args: string[] = [],
    result?: ExecResult,
  ) {
    super(message);
    this.name = 'SdkExecError';
    this.code = code;
    this.command = command;
    this.args = args;
    this.result = result;
  }
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
  | AiSystemPromptSetMessage
  | AiFocusResultMessage
  | AiStreamingStatusResultMessage
  | AiSubscribedMessage
  | AiUnsubscribedMessage
  | AiStreamChunkMessage
  | AiStreamCompleteMessage
  | AiNewMessageMessage
  | AiErrorMessage
  | MenuBarResultMessage
  | MenuActionResultMessage
  | { type: 'chatSubmit'; id: string; text: string }
  | { type: 'displayListResult'; requestId: string; displays: DisplayInfo[] }
  | { type: 'frontmostWindowResult'; requestId: string; window?: SystemWindowInfo }
  | { type: 'systemFeedbackResult'; requestId: string; result: SystemFeedbackResult }
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
const SDK_TEST_WINDOW_FIXTURES = SDK_TEST_AUTOSUBMIT && process.env.SDK_TEST_WINDOW_FIXTURES === '1';
const SDK_TEST_CLIPBOARD_FIXTURES = SDK_TEST_AUTOSUBMIT && process.env.SDK_TEST_CLIPBOARD_FIXTURES === '1';
const SDK_TEST_FILE_FIXTURES = SDK_TEST_AUTOSUBMIT && process.env.SDK_TEST_FILE_FIXTURES === '1';
const SDK_TEST_MENU_FIXTURES = SDK_TEST_AUTOSUBMIT && process.env.SDK_TEST_MENU_FIXTURES === '1';
// Test fixtures exist only inside the isolated preload child. They never read,
// write, focus, enumerate, or otherwise inspect actual operating-system state.
const autoSubmitClipboardEntries: ClipboardHistoryEntry[] = [];
const autoSubmitWindows: SystemWindowInfo[] = SDK_TEST_WINDOW_FIXTURES
  ? [
      {
        windowId: 42_001,
        title: 'SK Window Fixture — Synthetic',
        appName: 'Window Engine Fixture',
        bounds: { x: 20, y: 30, width: 800, height: 600 },
        isMinimized: false,
        isActive: false,
      },
      {
        windowId: 42_002,
        title: 'SK Window Fixture — Disposable Synthetic',
        appName: 'Window Engine Fixture',
        bounds: { x: 40, y: 50, width: 400, height: 300 },
        isMinimized: false,
        isActive: false,
      },
    ]
  : [];

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
   * @param actions - Optional prompt-scoped actions for the actions panel
   * @returns Array of field values in order
   */
  function fields(fieldDefs: (string | FieldDef)[], actions?: Action[]): Promise<string[]>;
  
  /**
   * Custom HTML form prompt
   * @param html - HTML string containing form fields
   * @param actions - Optional prompt-scoped actions for the actions panel
   * @returns Object with form field names as keys and their values
   */
  function form(html: string, actions?: Action[]): Promise<Record<string, string>>;
  
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
   * @param config - Optional prompt text, v1 env options, or function to prompt for value if not set
   * @returns The environment variable value
   */
  function env(key: string, config?: string | EnvConfig | (() => Promise<string>)): Promise<string>;
  
  // =============================================================================
  // System APIs (TIER 3)
  // =============================================================================

  /**
   * Execute an explicit subprocess without invoking a shell. Prefer the argv
   * form for user-provided values; shell operators/substitution are rejected.
   */
  function exec(command: string, args?: readonly string[]): Promise<ExecResult>;

  /** Read text from an explicitly named filesystem path. */
  function readFile(path: string, encoding?: BufferEncoding): Promise<string>;

  /** Write text to an explicitly named filesystem path. */
  function writeFile(path: string, content: string, encoding?: BufferEncoding): Promise<void>;
  
  /**
   * Play system beep sound
   */
  function beep(): Promise<SystemFeedbackResult>;
  
  /**
   * Text-to-speech
   * @param text - Text to speak
   * @param voice - Optional voice name
   */
  function say(text: string, voice?: string): Promise<SystemFeedbackResult>;
  
  /**
   * Show system notification
   * @param options - Notification options or body string
   */
  function notify(options: string | NotifyOptions): Promise<SystemFeedbackResult>;
  
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
  function setStatus(options: StatusOptions): Promise<SystemFeedbackResult>;
  
  /**
   * Set system menu icon and scripts
   * @param icon - Icon name/path
   * @param scripts - Optional array of script paths
   */
  function menu(icon: string, scripts?: string[]): Promise<SystemFeedbackResult>;
  
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

  /**
   * Typed computer-use helpers backed by Script Kit's own local MCP server.
   */
  var computer: ComputerUseApi;
  
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
  // For Agent Chat with built-in BYOK AI providers,
  // see the ai* functions below (aiStartChat, aiFocus, etc.)
  // =============================================================================

  /**
   * Chat function interface with attached controller methods for streaming
   */
  interface ChatFunction {
    (options?: ChatOptions): Promise<ChatResult>;
    addMessage(msg: ChatMessage | CoreMessage): void;
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
   * For Agent Chat with built-in AI providers (BYOK),
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
   * @deprecated Floating widget windows are unsupported in GPUI. Use div(...).
   * @throws UnsupportedSdkFeatureError before sending a host request.
   */
  function widget(html: string, options?: WidgetOptions): Promise<never>;
  
  /**
   * Opens a terminal window
   * @param command - Optional command to run in the terminal
   * @param actions - Optional prompt-scoped actions for the actions panel
   * @returns Terminal output when command completes
   */
  function term(command?: string, actions?: Action[]): Promise<string>;
  
  /**
   * @deprecated Camera streaming is unsupported in the JSONL SDK transport.
   * @throws UnsupportedSdkFeatureError without accessing a camera.
   */
  function webcam(): Promise<never>;
  
  /**
   * @deprecated Microphone streaming is unsupported in the JSONL SDK transport.
   * @throws UnsupportedSdkFeatureError without accessing a microphone.
   */
  function mic(): Promise<never>;
  
  /**
   * @deprecated Native screen color picking is unsupported in GPUI.
   * @throws UnsupportedSdkFeatureError without capturing the screen.
   */
  function eyeDropper(): Promise<never>;
  
  /**
   * Legacy interactive find prompt.
   *
   * Unsupported in GPUI: there is no implemented Rust find prompt route,
   * renderer, submit contract, or documented onlyin prompt semantics.
   *
   * Use fileSearch(query, { onlyin }) for non-interactive file results, or
   * path({ startPath }) / arg(...) for supported prompt-driven selection.
   *
   * @throws UnsupportedSdkFeatureError
   */
  function find(placeholder: string, options?: FindOptions): Promise<never>;
  
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
  // Agent Chat SDK API
  // =============================================================================
  //
  // These functions control **Agent Chat** — Script Kit's built-in AI chat surface:
  // - BYOK (Bring Your Own Key) AI providers (Anthropic, OpenAI)
  // - Manages its own chat history in SQLite
  // - Provides streaming responses automatically
  // - Supports typed context parts (resource URIs, file paths)
  //
  // For an **inline chat UI** in the main window where YOU control AI generation,
  // use the chat() function instead. See TIER 4A above.
  // =============================================================================

  /** Check if Agent Chat is currently open */
  function aiIsOpen(): Promise<{ isOpen: boolean; activeChatId?: string }>;

  /** Get information about the currently active Agent Chat conversation */
  function aiGetActiveChat(): Promise<AiChatInfo | null>;

  /** List all chats from Agent Chat storage */
  function aiListChats(limit?: number, includeDeleted?: boolean): Promise<AiChatInfo[]>;

  /** Get messages from a specific chat or the active chat */
  function aiGetConversation(chatId?: string, limit?: number): Promise<AiMessageInfo[]>;

  /** Start a new Agent Chat conversation with an initial message */
  function aiStartChat(message: string, options?: AiChatOptions): Promise<AiStartChatResult>;

  /** Append a message to an existing chat without triggering a response */
  function aiAppendMessage(chatId: string, content: string, role: 'user' | 'assistant' | 'system'): Promise<string>;

  /** Send a message to an existing chat and trigger an AI response */
  function aiSendMessage(chatId: string, content: string, imagePath?: string, parts?: AiContextPartInput[]): Promise<{ userMessageId: string; streamingStarted: boolean }>;

  /** Set or update the system prompt for a chat */
  function aiSetSystemPrompt(chatId: string, prompt: string): Promise<void>;

  /** Focus Agent Chat, opening it if necessary */
  function aiFocus(): Promise<{ wasOpen: boolean }>;

  /** Get the current streaming status for a chat */
  function aiGetStreamingStatus(chatId?: string): Promise<{ isStreaming: boolean; chatId?: string; partialContent?: string }>;

  /** Delete a chat from Agent Chat storage */
  function aiDeleteChat(chatId: string, permanent?: boolean): Promise<void>;

  /** Subscribe to Agent Chat events for real-time streaming updates */
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
   * @deprecated Legacy panel mutation is unsupported. Use div(...) instead.
   * @throws UnsupportedSdkFeatureError before sending a host request.
   */
  function setPanel(html: string): never;
  
  /**
   * @deprecated Legacy preview mutation is unsupported. Use arg(...) or div(...).
   * @throws UnsupportedSdkFeatureError before sending a host request.
   */
  function setPreview(html: string): never;
  
  /**
   * @deprecated Legacy prompt mutation is unsupported. Open a supported prompt.
   * @throws UnsupportedSdkFeatureError before sending a host request.
   */
  function setPrompt(html: string): never;
  
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

globalThis.mini = async function mini(
  placeholder: string,
  choices: (string | Choice)[]
): Promise<string> {
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

globalThis.micro = async function micro(
  placeholder: string,
  choices: (string | Choice)[]
): Promise<string> {
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

globalThis.hotkey = async function hotkey(
  placeholder?: string
): Promise<HotkeyInfo> {
  const id = nextId();

  // Auto-submit value used only when SDK_TEST_AUTOSUBMIT is enabled.
  const autoSubmitValue = {
    value: JSON.stringify({
      key: 'K',
      command: false,
      shift: true,
      option: false,
      control: false,
      shortcut: '⇧K',
      keyCode: 'K',
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
  config?: string | EnvConfig | (() => Promise<string>)
): Promise<string> {
  // First check if the env var is already set
  const existingValue = process.env[key];
  if (existingValue !== undefined && existingValue !== '') {
    return existingValue;
  }

  // If a prompt function is provided, use it to get the value
  if (typeof config === 'function') {
    const value = await config();
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

    const inferredSecret = key.toLowerCase().includes('secret') ||
      key.toLowerCase().includes('password') ||
      key.toLowerCase().includes('token') ||
      key.toLowerCase().includes('key');
    const prompt = typeof config === 'string'
      ? config
      : config && typeof config === 'object'
        ? config.hint ?? config.placeholder
        : undefined;
    const secret = config && typeof config === 'object'
      ? config.secret ?? inferredSecret
      : inferredSecret;

    const message: EnvMessage = {
      type: 'env',
      id,
      key,
      secret,
    };
    if (prompt !== undefined) {
      message.prompt = prompt;
    }

    send(message);
  });
};

function unsupportedSystemFeedbackResult(
  api: SystemFeedbackApi,
  message: string,
  alternative: 'hud' | 'setActions'
): Promise<SystemFeedbackResult> {
  return Promise.resolve({
    ok: false,
    api,
    status: 'unsupported',
    code: 'ERR_UNSUPPORTED_SDK_FEATURE',
    message,
    platform: process.platform,
    alternative,
  });
}

function invalidSystemFeedbackResult(
  api: SystemFeedbackApi,
  message: string
): Promise<SystemFeedbackResult> {
  return Promise.resolve({
    ok: false,
    api,
    status: 'invalid',
    code: 'ERR_SYSTEM_FEEDBACK_EMPTY',
    message,
    platform: process.platform,
  });
}

function requestSystemFeedback(
  api: Extract<SystemFeedbackApi, 'beep' | 'say' | 'notify'>,
  message: BeepMessage | SayMessage | NotifyMessage
): Promise<SystemFeedbackResult> {
  const requestId = nextId();
  const fallback: SystemFeedbackResult = {
    ok: false,
    api,
    status: 'timeout',
    code: 'ERR_SYSTEM_FEEDBACK_TIMEOUT',
    requestId,
    platform: process.platform,
    message: `${api}() did not receive a runtime dispatch receipt from the GPUI app.`,
  };

  return new Promise((resolve) => {
    const timeout = setTimeout(() => {
      removePending(requestId);
      resolve(fallback);
    }, 2000);

    addPending(requestId, (msg: ResponseMessage) => {
      clearTimeout(timeout);
      if (msg.type === 'systemFeedbackResult') {
        resolve(msg.result);
      } else {
        resolve(fallback);
      }
    });
    send({ ...message, requestId });
  });
}

// =============================================================================
// TIER 3: System APIs (alerts, clipboard, keyboard, mouse)
// =============================================================================

// Feedback messages resolve from app-originated dispatch receipts. Delivery
// remains platform-dependent and is not verified by the receipt.
globalThis.beep = function beep(): Promise<SystemFeedbackResult> {
  return requestSystemFeedback('beep', { type: 'beep' });
};

globalThis.say = function say(text: string, voice?: string): Promise<SystemFeedbackResult> {
  if (!text?.trim()) {
    return invalidSystemFeedbackResult('say', 'say() requires non-empty text.');
  }
  return requestSystemFeedback('say', { type: 'say', text, voice });
};

globalThis.notify = function notify(options: string | NotifyOptions): Promise<SystemFeedbackResult> {
  const message: NotifyMessage = typeof options === 'string'
    ? { type: 'notify', body: options }
    : { type: 'notify', title: options.title, body: options.body };
  if (!message.title?.trim() && !message.body?.trim()) {
    return invalidSystemFeedbackResult(
      'notify',
      'notify() requires a non-empty title or body.'
    );
  }
  return requestSystemFeedback('notify', message);
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

globalThis.setStatus = function setStatus(_options: StatusOptions): Promise<SystemFeedbackResult> {
  return unsupportedSystemFeedbackResult(
    'setStatus',
    'setStatus() has no visible GPUI status surface or receipt contract.',
    'hud'
  );
};

globalThis.menu = function menu(_icon: string, _scripts?: string[]): Promise<SystemFeedbackResult> {
  return unsupportedSystemFeedbackResult(
    'menu',
    'menu() has no GPUI tray/menu mutation handler.',
    'setActions'
  );
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
      }, { value: SDK_TEST_CLIPBOARD_FIXTURES ? autoSubmitClipboardEntries[0]?.content ?? '' : '' });

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
      if (SDK_TEST_CLIPBOARD_FIXTURES) {
        autoSubmitClipboardEntries.unshift({
          entryId: `synthetic-clipboard-${id}`,
          content: text,
          contentType: 'text',
          timestamp: new Date().toISOString(),
          pinned: false,
        });
      }

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

    return new Promise((resolve, reject) => {
      addPending(id, (msg: SubmitMessage) => {
        const value = msg.value ?? '';
        const error = clipboardErrorFromSubmitValue('clipboard.readImage', value);
        if (error) {
          reject(error);
          return;
        }
        if (!value) {
          reject(new ClipboardProtocolError(
            'clipboard.readImage',
            'ERR_CLIPBOARD_IMAGE_NOT_AVAILABLE',
            'Clipboard does not contain a supported image.',
          ));
          return;
        }

        // Value comes back as a base64-encoded PNG.
        resolve(Buffer.from(value, 'base64'));
      }, {
        value: 'ERROR:ERR_CLIPBOARD_NO_RESPONSE:clipboard.readImage did not receive a runtime response.',
      });

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

    return new Promise((resolve, reject) => {
      addPending(id, (msg: SubmitMessage) => {
        const error = clipboardErrorFromSubmitValue('clipboard.writeImage', msg.value);
        if (error) {
          reject(error);
          return;
        }
        resolve();
      }, {
        value: 'ERROR:ERR_CLIPBOARD_NO_RESPONSE:clipboard.writeImage did not receive a runtime response.',
      });

      const message: ClipboardMessage = {
        type: 'clipboard',
        id,
        action: 'write',
        format: 'image',
        content: Buffer.from(buffer).toString('base64'),
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

function rejectUnsupportedSdkFeature(feature: string, alternatives: string[]): Promise<never> {
  const error = new UnsupportedSdkFeatureError(feature, alternatives);
  console.warn(`[SDK] ${error.message}`);
  return Promise.reject(error);
}

// Keyboard API object
// NOTE: Native keyboard injection is intentionally unsupported until the app has
// explicit focus, permission, target, and receipt contracts.
globalThis.keyboard = {
  type(_text: string): Promise<never> {
    return rejectUnsupportedSdkFeature('keyboard.type', [
      'batch.setInput plus getState/getElements/waitFor for prompt text',
      'simulateKey plus state receipts when testing key routing',
    ]);
  },

  tap(..._keys: string[]): Promise<never> {
    return rejectUnsupportedSdkFeature('keyboard.tap', [
      'batch.forceSubmit for direct submit behavior',
      'simulateKey plus getState/getElements/waitFor when testing key routing',
    ]);
  },
};

// Mouse API object
// NOTE: Native mouse injection is intentionally unsupported until coordinate,
// focus, permission, target, and receipt contracts exist.
globalThis.mouse = {
  move(_positions: Position[]): Promise<never> {
    return rejectUnsupportedSdkFeature('mouse.move', [
      'state-first automation through batch/getState/getElements/waitFor',
      'semantic action APIs instead of coordinate movement',
    ]);
  },

  leftClick(): Promise<never> {
    return rejectUnsupportedSdkFeature('mouse.leftClick', [
      'semantic action APIs instead of coordinate clicks',
      'state-first automation through batch/getState/getElements/waitFor',
    ]);
  },

  rightClick(): Promise<never> {
    return rejectUnsupportedSdkFeature('mouse.rightClick', [
      'semantic action APIs instead of coordinate clicks',
      'state-first automation through batch/getState/getElements/waitFor',
    ]);
  },

  setPosition(_position: Position): Promise<never> {
    return rejectUnsupportedSdkFeature('mouse.setPosition', [
      'state-first automation through batch/getState/getElements/waitFor',
      'semantic action APIs instead of coordinate positioning',
    ]);
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

  // Real hosts must keep stdin alive through onInit. The isolated auto-submit
  // harness has no host pipe, so referencing stdin there leaks the process.
  if (!SDK_TEST_AUTOSUBMIT) {
    (process.stdin as any).ref?.();
  }

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

globalThis.widget = async function widget(
  _html: string,
  _options?: WidgetOptions
): Promise<never> {
  return rejectUnsupportedSdkFeature('widget', [
    'div(html) for an in-launcher surface',
    'arg(placeholder, choices) for a supported native prompt',
  ]);
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

// Unsupported compatibility global: camera streaming has no JSONL transport.
globalThis.webcam = async function webcam(): Promise<never> {
  return rejectUnsupportedSdkFeature('webcam', [
    'an explicitly configured external camera tool',
    'processing an existing image file',
  ]);
};

// Unsupported compatibility global: microphone streaming has no JSONL transport.
globalThis.mic = async function mic(): Promise<never> {
  return rejectUnsupportedSdkFeature('mic', [
    'an explicitly configured external audio tool',
    'processing an existing audio file',
  ]);
};

// Unsupported compatibility global: native screen color picking is unavailable.
globalThis.eyeDropper = async function eyeDropper(): Promise<never> {
  return rejectUnsupportedSdkFeature('eyeDropper', [
    "arg('Enter a color value')",
    'an explicitly configured external color picker',
  ]);
};

globalThis.find = function find(
  _placeholder: string,
  _options?: FindOptions
): Promise<never> {
  return rejectUnsupportedSdkFeature('find', [
    'fileSearch(query, { onlyin }) for non-interactive Spotlight/mdfind results',
    'path({ startPath }) or arg(...) for supported prompt-driven selection',
  ]);
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
// Agent Chat SDK API
// =============================================================================

/**
 * Check if Agent Chat is currently open.
 *
 * @returns Object indicating if Agent Chat is open and the active chat ID if any
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
 * Get information about the currently active chat in Agent Chat.
 * Works directly from SQLite storage — Agent Chat doesn't need to be open.
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
 * List all chats from Agent Chat storage.
 * Works directly from SQLite storage — Agent Chat doesn't need to be open.
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
 * Works directly from SQLite storage — Agent Chat doesn't need to be open.
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
 * Start a new Agent Chat conversation with an initial message.
 * Opens Agent Chat if it is not already open.
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

  return new Promise((resolve, reject) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiMessageAppended') {
        const resultMsg = msg as AiMessageAppendedMessage;
        resolve(resultMsg.messageId);
        return;
      }
      if (msg.type === 'aiError') {
        const error = msg as AiErrorMessage;
        reject(new Error(`${error.code}: ${error.message}`));
        return;
      }
      reject(new Error(`Unexpected aiAppendMessage response: ${msg.type}`));
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

  return new Promise((resolve, reject) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiMessageSent') {
        const resultMsg = msg as AiMessageSentMessage;
        resolve({
          userMessageId: resultMsg.userMessageId,
          streamingStarted: resultMsg.streamingStarted,
        });
        return;
      }
      if (msg.type === 'aiError') {
        const error = msg as AiErrorMessage;
        reject(new Error(`${error.code}: ${error.message}`));
        return;
      }
      reject(new Error(`Unexpected aiSendMessage response: ${msg.type}`));
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

  return new Promise((resolve, reject) => {
    addPending(requestId, (msg: ResponseMessage) => {
      if (msg.type === 'aiSystemPromptSet') {
        const resultMsg = msg as AiSystemPromptSetMessage;
        if (resultMsg.success) {
          resolve();
        } else {
          reject(new Error(resultMsg.error ?? 'AI_SYSTEM_PROMPT_SET_FAILED'));
        }
        return;
      }
      if (msg.type === 'aiError') {
        const error = msg as AiErrorMessage;
        reject(new Error(`${error.code}: ${error.message}`));
        return;
      }
      reject(new Error(`Unexpected aiSetSystemPrompt response: ${msg.type}`));
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
 * Focus Agent Chat, opening it if necessary.
 *
 * @returns Whether Agent Chat was already open
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
 * Delete a chat from Agent Chat storage.
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
const aiSubscriptions = new Map<
  string,
  { eventTypes: AiEventType[]; chatId?: string; handler: AiEventHandler }
>();

/**
 * Subscribe to Agent Chat events for real-time streaming updates.
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
          chatId,
          handler,
        });

        // Return unsubscribe function
        resolve(async () => {
          aiSubscriptions.delete(subscriptionId);
          const unsubRequestId = nextId();
          await new Promise<void>((resolveUnsubscribe) => {
            addPending(unsubRequestId, (msg: ResponseMessage) => {
              if (msg.type === 'aiUnsubscribed') {
                resolveUnsubscribe();
                return;
              }
              resolveUnsubscribe();
            }, {
              type: 'aiUnsubscribed',
              requestId: unsubRequestId,
              subscriptionId,
              success: true,
            } as AiUnsubscribedMessage);
            send({
              type: 'aiUnsubscribe',
              requestId: unsubRequestId,
              subscriptionId,
            } as AiUnsubscribeMessage);
          });
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
      const sub = aiSubscriptions.get(event.subscriptionId);
      if (sub?.eventTypes.includes('streamChunk') && (!sub.chatId || sub.chatId === event.chatId)) {
        sub.handler({
          chatId: event.chatId,
          chunk: event.chunk,
          accumulatedContent: event.accumulatedContent,
        });
      }
      return true;
    }
    case 'aiStreamComplete': {
      const event = msg as AiStreamCompleteMessage;
      const sub = aiSubscriptions.get(event.subscriptionId);
      if (sub?.eventTypes.includes('streamComplete') && (!sub.chatId || sub.chatId === event.chatId)) {
        sub.handler({
          chatId: event.chatId,
          messageId: event.messageId,
          fullContent: event.fullContent,
          tokensUsed: event.tokensUsed,
        });
      }
      return true;
    }
    case 'aiNewMessage': {
      const event = msg as AiNewMessageMessage;
      const sub = aiSubscriptions.get(event.subscriptionId);
      if (sub?.eventTypes.includes('message') && (!sub.chatId || sub.chatId === event.chatId)) {
        sub.handler({
          chatId: event.chatId,
          message: event.message,
        });
      }
      return true;
    }
    case 'aiError': {
      const event = msg as AiErrorMessage;
      if (event.subscriptionId) {
        const sub = aiSubscriptions.get(event.subscriptionId);
        if (sub?.eventTypes.includes('error')) {
          sub.handler({ code: event.code, message: event.message });
        }
      } else {
        for (const [, sub] of aiSubscriptions) {
          if (sub.eventTypes.includes('error')) {
            sub.handler({ code: event.code, message: event.message });
          }
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

// Content setters have no GPUI handler. Refuse before emitting an unhandled
// fire-and-forget message so authors cannot mistake a no-op for success.
globalThis.setPanel = function setPanel(_html: string): never {
  throw new UnsupportedSdkFeatureError('setPanel', [
    'div(html)',
    'arg(placeholder, choices)',
  ]);
};

globalThis.setPreview = function setPreview(_html: string): never {
  throw new UnsupportedSdkFeatureError('setPreview', [
    'arg(placeholder, choices)',
    'div(html)',
  ]);
};

globalThis.setPrompt = function setPrompt(_html: string): never {
  throw new UnsupportedSdkFeatureError('setPrompt', [
    'arg(placeholder, choices)',
    'fields(definitions)',
  ]);
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

function activeScriptKitRoot(): string {
  const configured = process.env.SK_PATH;
  if (!configured) return nodePath.join(os.homedir(), '.scriptkit');
  const homeExpanded = configured === '~' || configured.startsWith('~/')
    ? nodePath.join(os.homedir(), configured.slice(1))
    : configured;
  return homeExpanded.replace(/\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)/g,
    (match, braced: string | undefined, unbraced: string | undefined) =>
      process.env[braced ?? unbraced ?? ''] ?? match);
}

globalThis.skPath = function skPath(...segments: string[]): string {
  return nodePath.join(activeScriptKitRoot(), ...segments);
};

globalThis.kitPath = function kitPath(...segments: string[]): string {
  // Use the same owner-selected workspace as Rust setup; ~/.kit is deprecated.
  return nodePath.join(activeScriptKitRoot(), ...segments);
};

globalThis.tmpPath = function tmpPath(...segments: string[]): string {
  return nodePath.join(os.tmpdir(), 'kit', ...segments);
};

// =============================================================================
// TIER 5B: File Utilities (pure JS using Node fs)
// =============================================================================

function parseShellFreeCommand(command: string): string[] {
  if (typeof command !== 'string' || command.trim().length === 0) {
    throw new SdkExecError(
      'ERR_SDK_EXEC_INVALID_COMMAND',
      'exec() requires a non-empty executable name.',
      String(command),
    );
  }

  const tokens: string[] = [];
  let token = '';
  let quote: "'" | '"' | null = null;
  let escaped = false;
  let tokenStarted = false;

  for (let index = 0; index < command.length; index += 1) {
    const char = command[index];

    if (escaped) {
      token += char;
      escaped = false;
      tokenStarted = true;
      continue;
    }

    if (char === '\\' && quote !== "'") {
      escaped = true;
      tokenStarted = true;
      continue;
    }

    if (quote) {
      if (char === quote) {
        quote = null;
      } else {
        token += char;
      }
      tokenStarted = true;
      continue;
    }

    if (char === "'" || char === '"') {
      quote = char;
      tokenStarted = true;
      continue;
    }

    if (
      '|&;<>`\n\r'.includes(char) ||
      (char === '$' && (command[index + 1] === '(' || command[index + 1] === '{'))
    ) {
      throw new SdkExecError(
        'ERR_SDK_EXEC_INVALID_COMMAND',
        'exec() never invokes a shell; shell operators and command substitution are not allowed. Pass an executable plus an explicit args array.',
        command,
      );
    }

    if (/\s/.test(char)) {
      if (tokenStarted) {
        tokens.push(token);
        token = '';
        tokenStarted = false;
      }
      continue;
    }

    token += char;
    tokenStarted = true;
  }

  if (escaped || quote) {
    throw new SdkExecError(
      'ERR_SDK_EXEC_INVALID_COMMAND',
      'exec() received an unfinished quote or escape sequence.',
      command,
    );
  }
  if (tokenStarted) {
    tokens.push(token);
  }
  if (tokens.length === 0 || tokens[0].length === 0) {
    throw new SdkExecError(
      'ERR_SDK_EXEC_INVALID_COMMAND',
      'exec() requires a non-empty executable name.',
      command,
    );
  }
  return tokens;
}

globalThis.exec = async function exec(
  command: string,
  args?: readonly string[],
): Promise<ExecResult> {
  if (typeof command !== 'string' || command.trim().length === 0) {
    throw new SdkExecError(
      'ERR_SDK_EXEC_INVALID_COMMAND',
      'exec() requires a non-empty executable name.',
      String(command),
    );
  }

  if (args !== undefined && !Array.isArray(args)) {
    throw new SdkExecError(
      'ERR_SDK_EXEC_INVALID_COMMAND',
      'exec() arguments must be an explicit string array.',
      command,
    );
  }

  const argv = args === undefined ? parseShellFreeCommand(command) : [command, ...args];
  if (argv.some((value) => typeof value !== 'string' || value.includes('\0'))) {
    throw new SdkExecError(
      'ERR_SDK_EXEC_INVALID_COMMAND',
      'exec() arguments must be strings without NUL characters.',
      command,
      argv.slice(1),
    );
  }

  const [executable, ...childArgs] = argv;

  return new Promise((resolve, reject) => {
    const child = spawnChildProcess(executable, childArgs, {
      shell: false,
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];

    child.stdout.on('data', (chunk: Buffer | string) => {
      stdout.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    });
    child.stderr.on('data', (chunk: Buffer | string) => {
      stderr.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    });

    child.once('error', (error) => {
      reject(new SdkExecError(
        'ERR_SDK_EXEC_SPAWN_FAILED',
        `exec() could not start ${JSON.stringify(executable)}: ${error.message}`,
        executable,
        childArgs,
      ));
    });

    child.once('close', (exitCode, signal) => {
      const result: ExecResult = {
        stdout: Buffer.concat(stdout).toString('utf8'),
        stderr: Buffer.concat(stderr).toString('utf8'),
        exitCode: exitCode ?? 1,
      };

      if (exitCode === 0) {
        resolve(result);
        return;
      }

      const termination = signal
        ? `signal ${signal}`
        : `exit code ${result.exitCode}`;
      reject(new SdkExecError(
        'ERR_SDK_EXEC_NONZERO_EXIT',
        `exec() failed with ${termination}${result.stderr ? `: ${result.stderr.trim()}` : ''}`,
        executable,
        childArgs,
        result,
      ));
    });
  });
};

globalThis.readFile = async function readFile(
  path: string,
  encoding: BufferEncoding = 'utf8',
): Promise<string> {
  return fs.readFile(path, { encoding });
};

globalThis.writeFile = async function writeFile(
  path: string,
  content: string,
  encoding: BufferEncoding = 'utf8',
): Promise<void> {
  await fs.writeFile(path, content, { encoding });
};

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

function autoSubmitClipboardAction(
  requestId: string,
  action: 'pin' | 'unpin' | 'remove' | 'clear' | 'trimOversize',
  entryId?: string,
): ClipboardHistoryResultMessage {
  if (!SDK_TEST_CLIPBOARD_FIXTURES) {
    return { type: 'clipboardHistoryResult', requestId, success: true };
  }

  if (action === 'clear') {
    const pinned = autoSubmitClipboardEntries.filter((entry) => entry.pinned);
    autoSubmitClipboardEntries.splice(0, autoSubmitClipboardEntries.length, ...pinned);
    return { type: 'clipboardHistoryResult', requestId, success: true };
  }

  if (action === 'trimOversize') {
    const withinLimit = autoSubmitClipboardEntries.filter((entry) => entry.content.length <= 100_000);
    autoSubmitClipboardEntries.splice(0, autoSubmitClipboardEntries.length, ...withinLimit);
    return { type: 'clipboardHistoryResult', requestId, success: true };
  }

  const index = autoSubmitClipboardEntries.findIndex((entry) => entry.entryId === entryId);
  if (index < 0) {
    return {
      type: 'clipboardHistoryResult',
      requestId,
      success: false,
      error: `Unknown synthetic clipboard entry id ${JSON.stringify(entryId)}.`,
    };
  }

  if (action === 'remove') {
    autoSubmitClipboardEntries.splice(index, 1);
  } else {
    autoSubmitClipboardEntries[index].pinned = action === 'pin';
  }

  return { type: 'clipboardHistoryResult', requestId, success: true };
}

globalThis.clipboardHistory = async function clipboardHistory(): Promise<ClipboardHistoryEntry[]> {
  const id = nextId();

  const autoSubmitValue: ClipboardHistoryListMessage = {
    type: 'clipboardHistoryList',
    requestId: id,
    entries: SDK_TEST_CLIPBOARD_FIXTURES
      ? autoSubmitClipboardEntries.map((entry) => ({ ...entry }))
      : [],
  };

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

  const autoSubmitValue = autoSubmitClipboardAction(id, 'pin', entryId);

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

  const autoSubmitValue = autoSubmitClipboardAction(id, 'unpin', entryId);

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

  const autoSubmitValue = autoSubmitClipboardAction(id, 'remove', entryId);

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

  const autoSubmitValue = autoSubmitClipboardAction(id, 'clear');

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

  const autoSubmitValue = autoSubmitClipboardAction(id, 'trimOversize');

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

  const autoSubmitValue: WindowListResultMessage = {
    type: 'windowListResult',
    requestId: id,
    windows: autoSubmitWindows.map((window) => ({
      ...window,
      bounds: window.bounds ? { ...window.bounds } : undefined,
    })),
  };

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

function autoSubmitWindowActionResult(message: WindowActionMessage): WindowActionResultMessage {
  const windowIndex = autoSubmitWindows.findIndex(
    (window) => window.windowId === message.windowId,
  );

  if (windowIndex === -1) {
    return {
      type: 'windowActionResult',
      requestId: message.requestId,
      success: false,
      error: `stale or unknown legacy window id ${message.windowId}`,
    };
  }

  const fixture = autoSubmitWindows[windowIndex];

  switch (message.action) {
    case 'close':
      autoSubmitWindows.splice(windowIndex, 1);
      break;
    case 'focus':
      for (const window of autoSubmitWindows) {
        window.isActive = window.windowId === message.windowId;
      }
      break;
    case 'minimize':
      fixture.isMinimized = true;
      break;
    case 'maximize':
      fixture.isMinimized = false;
      break;
    case 'move':
      if (fixture.bounds && message.bounds) {
        fixture.bounds = { ...fixture.bounds, x: message.bounds.x, y: message.bounds.y };
      }
      break;
    case 'resize':
      if (fixture.bounds && message.bounds) {
        fixture.bounds = {
          ...fixture.bounds,
          width: message.bounds.width,
          height: message.bounds.height,
        };
      }
      break;
    case 'tile':
    case 'moveToNextDisplay':
    case 'moveToPreviousDisplay':
      break;
  }

  return {
    type: 'windowActionResult',
    requestId: message.requestId,
    success: true,
  };
}

function performWindowAction(
  action: WindowActionMessage['action'],
  windowId: number,
  options?: Pick<WindowActionMessage, 'bounds' | 'tilePosition'>,
): Promise<void> {
  const requestId = nextId();
  const message: WindowActionMessage = {
    type: 'windowAction',
    requestId,
    action,
    windowId,
    ...options,
  };
  const autoSubmitValue = SDK_TEST_AUTOSUBMIT
    ? autoSubmitWindowActionResult(message)
    : undefined;

  return new Promise((resolve, reject) => {
    addPending(requestId, (response: ResponseMessage) => {
      if (response.type === 'windowActionResult') {
        if (!response.success) {
          reject(new Error(response.error ?? `Window ${action} failed`));
          return;
        }
        resolve();
        return;
      }

      const submitResponse = response as SubmitMessage;
      if (submitResponse.value?.startsWith('ERROR:')) {
        reject(new Error(submitResponse.value.substring(6).trim()));
        return;
      }

      resolve();
    }, autoSubmitValue);

    send(message);
  });
}

globalThis.focusWindow = async function focusWindow(windowId: number): Promise<void> {
  return performWindowAction('focus', windowId);
};

globalThis.closeWindow = async function closeWindow(windowId: number): Promise<void> {
  return performWindowAction('close', windowId);
};

globalThis.minimizeWindow = async function minimizeWindow(windowId: number): Promise<void> {
  return performWindowAction('minimize', windowId);
};

globalThis.maximizeWindow = async function maximizeWindow(windowId: number): Promise<void> {
  return performWindowAction('maximize', windowId);
};

globalThis.moveWindow = async function moveWindow(windowId: number, x: number, y: number): Promise<void> {
  return performWindowAction('move', windowId, {
    bounds: { x, y, width: 0, height: 0 },
  });
};

globalThis.resizeWindow = async function resizeWindow(windowId: number, width: number, height: number): Promise<void> {
  return performWindowAction('resize', windowId, {
    bounds: { x: 0, y: 0, width, height },
  });
};

/**
 * Tile a window to a specific screen position.
 * Uses the native tiling implementation which calculates bounds based on actual screen dimensions.
 * @param windowId - The ID of the window to tile
 * @param position - The tile position
 */
globalThis.tileWindow = async function tileWindow(windowId: number, position: TilePosition): Promise<void> {
  return performWindowAction('tile', windowId, { tilePosition: position });
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
  return performWindowAction('moveToNextDisplay', windowId);
};

/**
 * Move a window to the previous display/monitor.
 * @param windowId - The ID of the window to move
 */
globalThis.moveToPreviousDisplay = async function moveToPreviousDisplay(windowId: number): Promise<void> {
  return performWindowAction('moveToPreviousDisplay', windowId);
};

// =============================================================================
// File Search Functions
// =============================================================================

globalThis.fileSearch = async function fileSearch(query: string, options?: FindOptions): Promise<FileSearchResult[]> {
  const id = nextId();

  const fixtureRoot = process.cwd();
  const fixtureFiles: FileSearchResult[] = SDK_TEST_FILE_FIXTURES
    ? [
        {
          path: nodePath.join(fixtureRoot, 'package.json'),
          name: 'package.json',
          isDirectory: false,
          size: 1024,
          modifiedAt: '2026-01-01T00:00:00.000Z',
        },
        {
          path: nodePath.join(fixtureRoot, 'README.md'),
          name: 'README.md',
          isDirectory: false,
          size: 2048,
          modifiedAt: '2026-01-01T00:00:00.000Z',
        },
        {
          path: nodePath.join(fixtureRoot, 'Cargo.toml'),
          name: 'Cargo.toml',
          isDirectory: false,
          size: 4096,
          modifiedAt: '2026-01-01T00:00:00.000Z',
        },
        {
          path: nodePath.join(os.tmpdir(), 'sdk-synthetic-search', 'Cargo.external'),
          name: 'Cargo.external',
          isDirectory: false,
        },
      ]
    : [];
  const normalizedQuery = query.trim().toLowerCase();
  const onlyin = options?.onlyin ? nodePath.resolve(options.onlyin) : undefined;
  const autoSubmitValue: FileSearchResultMessage = {
    type: 'fileSearchResult',
    requestId: id,
    files: normalizedQuery.length === 0
      ? []
      : fixtureFiles.filter((file) => (
          file.name.toLowerCase().includes(normalizedQuery)
          && (!onlyin || file.path === onlyin || file.path.startsWith(`${onlyin}${nodePath.sep}`))
        )),
  };

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
  const fixtureItems: MenuBarItem[] = SDK_TEST_MENU_FIXTURES
    && (bundleId === undefined || bundleId === 'com.apple.finder')
    ? [
        {
          title: 'File',
          enabled: true,
          menuPath: ['File'],
          children: [
            {
              title: 'New Window',
              enabled: true,
              shortcut: '⌘N',
              menuPath: ['File', 'New Window'],
              children: [],
            },
          ],
        },
      ]
    : [];

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

    addPending(id, resolver, {
      type: 'menuBarResult',
      requestId: id,
      items: fixtureItems,
    } satisfies MenuBarResultMessage);

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
  const autoSubmitResult: MenuActionResultMessage = {
    type: 'menuActionResult',
    requestId: id,
    success: false,
    error: menuPath.length === 0
      ? 'Invalid menu path: at least one menu item is required.'
      : `Application ${bundleId} is not running in the isolated SDK harness.`,
  };

  return new Promise((resolve, reject) => {
    const resolver = (msg: ResponseMessage | undefined) => {
      if (msg === undefined) {
        reject(new Error('Menu action did not receive a host response.'));
        return;
      }

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

      // Fallback to submit message handling (backwards compatibility)
      const submitMsg = msg as SubmitMessage;
      if (submitMsg.value && submitMsg.value.startsWith('ERROR:')) {
        reject(new Error(submitMsg.value.substring(6).trim()));
      } else {
        resolve();
      }
    };

    addPending(id, resolver, autoSubmitResult);

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
  surfaceContract?: LauncherSurfaceContractSnapshot;
  activePopupContract?: LauncherSurfaceContractSnapshot;
  activeFooter?: ActiveFooterSnapshot;
  drop?: DropPromptState;
  path?: PathPromptState;
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

export interface DropPromptState {
  fileCount: number;
  files: Array<{
    index: number;
    name: string;
    size: number;
  }>;
}

export interface PathPromptState {
  currentPath: string;
  currentDirectory: string;
  requestedStartPath?: string;
  startPathKind:
    | 'defaultHome'
    | 'defaultRoot'
    | 'directory'
    | 'file'
    | 'symlinkDirectory'
    | 'symlinkFile'
    | 'brokenSymlink'
    | 'missing'
    | 'permissionDenied'
    | 'readError';
  filter: string;
  loadStatus: 'loaded' | 'empty' | 'error';
  loadErrorKind?: 'missing' | 'permissionDenied' | 'notDirectory' | 'readError' | null;
  emptyKind?:
    | 'emptyDirectory'
    | 'hiddenOnly'
    | 'noMatches'
    | 'missing'
    | 'permissionDenied'
    | 'notDirectory'
    | 'readError'
    | null;
  statusMessage: string;
  entryCount: number;
  visibleEntryCount: number;
  selectedIndex: number;
  selectedPath?: string | null;
  hiddenPolicy: 'omitDotfiles' | string;
  hiddenEntriesOmitted: number;
  entryErrorsOmitted: number;
  symlinkPolicy: 'followDirectoryTargets' | string;
  selected?: {
    name: string;
    path: string;
    isDir: boolean;
    isSymlink: boolean;
  };
  status: {
    kind:
      | 'ready'
      | 'empty'
      | 'filtered_empty'
      | 'missing'
      | 'not_directory'
      | 'permission_denied'
      | 'read_error';
    message: string;
    isError: boolean;
    hiddenPolicy: 'omitDotfiles' | string;
    hiddenCount: number;
    failedEntryCount: number;
  };
}

export interface LauncherSurfaceContractSnapshot {
  schemaVersion: number;
  surfaceKind: string;
  family: string;
  inputOwnership: string;
  previewRole: string;
  focusPolicy: string;
  keyboardPolicy: string;
  actionsPolicy: string;
  proofPolicy: string;
  visualPolicy: string;
  automationSemanticSurface: string;
  nativeFooterSurface?: string | null;
}

export interface ActiveFooterButtonSnapshot {
  action: string;
  key: string;
  label: string;
  enabled: boolean;
  selected: boolean;
  actionDisabled?: string | null;
}

export interface ActiveFooterSnapshot {
  schemaVersion: number;
  owner: 'native' | 'prompt' | 'popup' | 'content' | 'none' | string;
  expectedSurface?: string | null;
  requestedSurface?: string | null;
  activeSurface?: string | null;
  nativeFooterHostInstalled: boolean;
  gpuiFallbackVisible: boolean;
  leftInfo?: {
    dotStatus: 'hidden' | 'streaming' | 'waitingForPermission' | 'idle' | 'error' | string;
    modelName: string;
  } | null;
  buttonCount: number;
  buttons: ActiveFooterButtonSnapshot[];
  mismatch?: string | null;
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
  role?: string;
  kind?: string;
  source?: string;
  sourceName?: string;
  selectable?: boolean;
  statusKind?: string;
  actionDisabled?: string;
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
  surfaceContract?: LauncherSurfaceContractSnapshot;
  activePopupContract?: LauncherSurfaceContractSnapshot;
  activeFooter?: ActiveFooterSnapshot;
  drop?: DropPromptState;
  path?: PathPromptState;
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
          surfaceContract: state.surfaceContract,
          activePopupContract: state.activePopupContract,
          activeFooter: state.activeFooter,
          drop: state.drop,
          path: state.path,
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
interface ScriptKitConfigRuntime {
  Transpiler: new (options: { loader: 'ts' }) => {
    scanImports(source: string): unknown[];
    transformSync(source: string): string;
  };
  build(options: {
    entrypoints: string[];
    target: 'bun';
    format: 'esm';
    splitting: boolean;
    write: boolean;
  }): Promise<{
    success: boolean;
    outputs: Array<{ text(): Promise<string> }>;
  }>;
}
let cachedScriptKitConfig: {
  path: string;
  ownerFingerprint: string;
  result: Promise<Config | null>;
} | null = null;

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
  const configPath = nodePath.join(activeScriptKitRoot(), 'config.ts');
  let ownerFingerprint: string;
  try {
    const owner = await fs.stat(configPath, { bigint: true });
    ownerFingerprint = [owner.dev, owner.ino, owner.size, owner.mtimeNs, owner.ctimeNs].join(':');
  } catch {
    if (cachedScriptKitConfig?.path === configPath) cachedScriptKitConfig = null;
    return null;
  }
  if (
    !cachedScriptKitConfig ||
    cachedScriptKitConfig.path !== configPath ||
    cachedScriptKitConfig.ownerFingerprint !== ownerFingerprint
  ) {
    const result = (async () => {
      const source = await fs.readFile(configPath, 'utf8');
      const runtime = (globalThis as typeof globalThis & {
        Bun?: ScriptKitConfigRuntime;
      }).Bun;
      if (!runtime) {
        throw new Error('Script Kit configuration requires the Bun TypeScript runtime.');
      }
      const transpiler = new runtime.Transpiler({ loader: 'ts' });
      let executableSource: string;
      if (transpiler.scanImports(source).length === 0) {
        executableSource = transpiler.transformSync(source);
      } else {
        const built = await runtime.build({
          entrypoints: [configPath],
          target: 'bun',
          format: 'esm',
          splitting: false,
          write: false,
        });
        if (!built.success || built.outputs.length !== 1) {
          throw new Error(`Script Kit configuration could not resolve its imports: ${configPath}`);
        }
        executableSource = await built.outputs[0]!.text();
      }
      const moduleUrl = URL.createObjectURL(new Blob([executableSource], {
        type: 'application/javascript',
      }));
      let imported: Record<string, unknown>;
      try {
        imported = await import(moduleUrl) as Record<string, unknown>;
      } finally {
        URL.revokeObjectURL(moduleUrl);
      }
      const config = (imported.default ?? imported) as Config;
      return config ?? null;
    })();
    cachedScriptKitConfig = { path: configPath, ownerFingerprint, result };
  }

  return cachedScriptKitConfig.result;
}

async function loadScriptKitMcpConfig(): Promise<McpConfig> {
  const config = await loadScriptKitConfig();
  return normalizeMcpConfig(config?.mcp);
}

interface ScriptKitMcpDiscovery {
  url: string;
  token: string;
  version?: string;
  capabilities?: Record<string, unknown>;
}

async function loadScriptKitSelfMcpServerConfig(): Promise<McpHttpServerConfig> {
  const discoveryPath = process.env.SCRIPT_KIT_MCP_SERVER_JSON?.trim()
    || nodePath.join(activeScriptKitRoot(), 'server.json');
  let discovery: ScriptKitMcpDiscovery;
  let handle: Awaited<ReturnType<typeof fs.open>> | undefined;

  try {
    const owner = await fs.lstat(discoveryPath);
    if (!owner.isFile() || owner.isSymbolicLink()) {
      throw new Error('Script Kit MCP requires a private regular discovery file');
    }
    handle = await fs.open(discoveryPath, fsConstants.O_RDONLY | (fsConstants.O_NOFOLLOW ?? 0));
    const opened = await handle.stat();
    if (!opened.isFile() || owner.dev !== opened.dev || owner.ino !== opened.ino) {
      throw new Error('Script Kit MCP requires a private regular discovery file');
    }
    if (process.platform !== 'win32' && (opened.mode & 0o077) !== 0) {
      throw new Error('Script Kit MCP discovery requires owner-only permissions');
    }
    if (typeof process.getuid === 'function' && opened.uid !== process.getuid()) {
      throw new Error('Script Kit MCP discovery must belong to the current user');
    }
    const text = await handle.readFile('utf8');
    discovery = JSON.parse(text) as ScriptKitMcpDiscovery;
  } catch (error) {
    throw new Error(
      `[computer] Script Kit MCP discovery is unavailable at ${discoveryPath}. Start Script Kit before using computer.* helpers. ${getMcpErrorMessage(error)}`
    );
  } finally {
    await handle?.close();
  }

  if (
    typeof discovery?.url !== 'string' ||
    !discovery.url.trim() ||
    typeof discovery.token !== 'string' ||
    !discovery.token.trim() ||
    /[\r\n]/.test(discovery.token)
  ) {
    throw new Error(`[computer] Script Kit MCP discovery at ${discoveryPath} is missing url or token`);
  }

  let endpoint: URL;
  try {
    endpoint = new URL(discovery.url);
  } catch {
    throw new Error('[computer] A local Script Kit MCP endpoint must be a valid loopback HTTP /rpc URL.');
  }
  const loopback = endpoint.hostname === 'localhost'
    || endpoint.hostname === '127.0.0.1'
    || endpoint.hostname === '[::1]';
  if (
    !loopback ||
    !['http:', 'https:'].includes(endpoint.protocol) ||
    endpoint.username ||
    endpoint.password ||
    endpoint.search ||
    endpoint.hash ||
    !['/', '/rpc'].includes(endpoint.pathname)
  ) {
    throw new Error('[computer] A local Script Kit MCP endpoint must be a loopback HTTP /rpc URL without credentials, query, or fragment.');
  }
  endpoint.pathname = '/rpc';
  return {
    transport: 'http',
    endpoint: endpoint.toString(),
    headers: {
      authorization: `Bearer ${discovery.token}`,
    },
  };
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

function isMcpCredential(name: string, value: string): boolean {
  return /authorization|token|api[-_]?key|password|passwd|secret|cookie|credential|private[-_]?key|access[-_]?key/i.test(name)
    || /\b(?:Bearer|Basic)\s+\S/i.test(value);
}

function publicMcpServerMetadata(values: Record<string, string> = {}): Record<string, string> {
  return Object.fromEntries(Object.entries(values).map(([name, value]) => [
    name,
    isMcpCredential(name, value) ? '[REDACTED]' : value,
  ]));
}

function publicMcpServerArguments(args: readonly string[] = []): string[] {
  let redactFollowingValue = false;
  return args.map((argument) => {
    if (redactFollowingValue) {
      redactFollowingValue = false;
      return '[REDACTED]';
    }
    const delimiter = argument.indexOf('=');
    const name = delimiter < 0 ? argument : argument.slice(0, delimiter);
    if (isMcpCredential(name, argument)) {
      if (delimiter >= 0) return `${name}=[REDACTED]`;
      if (name.startsWith('-') && !/\b(?:Bearer|Basic)\s+\S/i.test(argument)) {
        redactFollowingValue = true;
        return argument;
      }
      return '[REDACTED]';
    }
    return argument;
  });
}

function publicMcpServerEndpoint(endpoint: string): string {
  let parsed: URL;
  try {
    parsed = new URL(endpoint);
  } catch {
    return isMcpCredential('', endpoint) ? '[REDACTED]' : endpoint;
  }

  let redacted = false;
  if (parsed.username || parsed.password) {
    parsed.username = '';
    parsed.password = '';
    redacted = true;
  }
  for (const [name, value] of parsed.searchParams) {
    if (isMcpCredential(name, value)) {
      parsed.searchParams.set(name, '[REDACTED]');
      redacted = true;
    }
  }
  if (parsed.hash) {
    parsed.hash = '';
    redacted = true;
  }
  return redacted ? parsed.toString() : endpoint;
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
      args: publicMcpServerArguments(server.args),
      env: publicMcpServerMetadata(server.env),
      cwd: server.cwd,
    };
  }

  return {
    id,
    transport: 'http',
    enabled: isServerEnabled(server),
    name: server.name,
    description: server.description,
    endpoint: publicMcpServerEndpoint(server.endpoint),
    headers: publicMcpServerMetadata(server.headers),
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

function safeMcpErrorMessage(message: string, secrets: readonly string[] = []): string {
  let safe = message;
  for (const secret of secrets) {
    if (secret) safe = safe.split(secret).join('[REDACTED]');
  }
  return safe.replace(/\bBearer\s+[^\s,;"'}]+/gi, 'Bearer [REDACTED]').slice(0, 512);
}

function mcpRequestTimeoutMs(): number {
  const configured = process.env.SCRIPT_KIT_MCP_TIMEOUT_MS;
  if (configured === undefined || configured === '') return 30_000;
  const timeout = Number(configured);
  if (!/^[1-9][0-9]*$/.test(configured) || !Number.isSafeInteger(timeout) || timeout > 120_000) {
    throw new Error('SCRIPT_KIT_MCP_TIMEOUT_MS must be a positive whole duration no greater than 120000ms.');
  }
  return timeout;
}

function mcpMaxResponseBytes(): number {
  const configured = process.env.SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES;
  if (configured === undefined || configured === '') return 16 * 1024 * 1024;
  const budget = Number(configured);
  if (!/^[1-9][0-9]*$/.test(configured) || !Number.isSafeInteger(budget) || budget > 64 * 1024 * 1024) {
    throw new Error('SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES must be a positive whole byte budget no greater than 67108864.');
  }
  return budget;
}

function mcpDiscoveryConcurrency(): number {
  const noninteractive = process.env.SCRIPT_KIT_NONINTERACTIVE === '1';
  const maximum = noninteractive ? 2 : 8;
  const configured = process.env.SCRIPT_KIT_MCP_CONCURRENCY;
  if (configured === undefined || configured === '') return noninteractive ? 2 : 4;
  const concurrency = Number(configured);
  if (
    !/^[1-9][0-9]*$/.test(configured) ||
    !Number.isSafeInteger(concurrency) ||
    concurrency > maximum
  ) {
    throw new Error(
      `SCRIPT_KIT_MCP_CONCURRENCY must be a positive whole worker count no greater than ${maximum}.`
    );
  }
  return concurrency;
}

async function readBoundedMcpResponse(
  response: Response,
  maximumBytes: number,
  serverId: string,
  method: string,
): Promise<string> {
  const oversized = () => new Error(`[mcp:${serverId}] ${method} exceeded its ${maximumBytes}-byte response safety budget`);
  const declared = response.headers.get('content-length');
  if (declared !== null && /^\d+$/.test(declared) && Number(declared) > maximumBytes) {
    await response.body?.cancel();
    throw oversized();
  }

  const reader = response.body?.getReader();
  if (!reader) return '';
  const chunks: Uint8Array[] = [];
  let received = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      received += value.byteLength;
      if (received > maximumBytes) {
        await reader.cancel();
        throw oversized();
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  return new TextDecoder().decode(Buffer.concat(chunks, received));
}

function assertMcpJsonRpcSuccess(
  response: unknown,
  serverId: string,
  method: string,
  expectedId: McpJsonRpcId,
  secrets: readonly string[] = [],
): Record<string, unknown> {
  if (
    !isRecord(response) ||
    response.jsonrpc !== '2.0' ||
    response.id !== expectedId ||
    Object.hasOwn(response, 'result') === Object.hasOwn(response, 'error')
  ) {
    throw new Error(`[mcp:${serverId}] ${method} returned an invalid JSON-RPC response`);
  }
  if ('error' in response) {
    if (
      !isRecord(response.error) ||
      !Number.isSafeInteger(response.error.code) ||
      typeof response.error.message !== 'string' ||
      !response.error.message
    ) {
      throw new Error(`[mcp:${serverId}] ${method} returned an invalid JSON-RPC response`);
    }
    throw new Error(
      `[mcp:${serverId}] ${method} failed: ${safeMcpErrorMessage(response.error.message, secrets)} (code ${response.error.code})`
    );
  }
  if (!isRecord(response.result)) {
    throw new Error(`[mcp:${serverId}] ${method} returned an invalid JSON-RPC response`);
  }
  return response.result;
}

async function createStdioMcpSession(serverId: string, server: McpStdioServerConfig) {
  if (!server.command?.trim()) {
    throw new Error(`[mcp:${serverId}] stdio server is missing a command`);
  }

  const requestTimeoutMs = mcpRequestTimeoutMs();
  const maximumResponseBytes = mcpMaxResponseBytes();
  const serverEnvironment = {
    ...process.env,
    ...(server.env ?? {}),
  };
  if (process.env.SCRIPT_KIT_NONINTERACTIVE === '1') {
    const protectedAuthority: Record<string, string> = {
      SCRIPT_KIT_NONINTERACTIVE: '1',
      SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: '0',
      SCRIPT_KIT_ALLOW_VISIBLE_PROBES: '0',
      SCRIPT_KIT_ALLOW_NATIVE_INPUT: '0',
      SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: '0',
      SCRIPT_KIT_ALLOW_LIVE_AI: '0',
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: '0',
      INCLUDE_SYSTEM_INPUT: '0',
      SCRIPT_KIT_TEST_STATUS: '0',
    };
    for (const [permission, required] of Object.entries(protectedAuthority)) {
      if (server.env?.[permission] !== undefined && server.env[permission] !== required) {
        throw new Error(`[mcp:${serverId}] noninteractive MCP server cannot override ${permission}`);
      }
      serverEnvironment[permission] = required;
    }
  }
  const ownsProcessGroup = process.platform !== 'win32';
  const child = spawnChildProcess(server.command, server.args ?? [], {
    cwd: server.cwd,
    env: serverEnvironment,
    detached: ownsProcessGroup,
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  let closed = false;
  const processClosed = new Promise<void>((resolve) => {
    const finish = () => {
      closed = true;
      resolve();
    };
    child.once('close', finish);
    child.once('error', finish);
  });

  let nextRequestId = 1;
  const stderrChunks: string[] = [];
  let stderrBytes = 0;
  let stdoutBytes = 0;
  const secretValues = Object.entries(server.env ?? {})
    .filter(([name]) => /authorization|token|api[-_]?key|password|secret/i.test(name))
    .map(([, value]) => value);
  const pending = new Map<
    number,
    {
      resolve: (response: McpJsonRpcResponse) => void;
      reject: (error: Error) => void;
    }
  >();

  const rejectPending = (error: Error) => {
    for (const { reject } of pending.values()) {
      reject(error);
    }
    pending.clear();
  };

  child.stdout.on('data', (chunk) => {
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    stdoutBytes += bytes.byteLength;
    if (stdoutBytes > maximumResponseBytes) {
      child.stdout.pause();
      rejectPending(new Error(`[mcp:${serverId}] stdio exceeded its ${maximumResponseBytes}-byte response safety budget`));
    }
  });

  child.stderr.on('data', (chunk) => {
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    const remaining = 1024 * 1024 - stderrBytes;
    if (bytes.byteLength > remaining) {
      if (remaining > 0) stderrChunks.push(bytes.subarray(0, remaining).toString());
      stderrBytes = 1024 * 1024;
      rejectPending(new Error(`[mcp:${serverId}] stdio stderr exceeded its 1048576-byte safety budget`));
      return;
    }
    stderrBytes += bytes.byteLength;
    stderrChunks.push(bytes.toString());
  });

  child.once('error', (error) => {
    rejectPending(new Error(`[mcp:${serverId}] failed to start: ${safeMcpErrorMessage(error.message, secretValues)}`));
  });

  child.once('exit', (code, signal) => {
    if (pending.size === 0) {
      return;
    }
    const stderr = safeMcpErrorMessage(stderrChunks.join('').trim(), secretValues);
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

    let message: unknown;
    try {
      message = JSON.parse(trimmed);
    } catch {
      // Ignore non-JSON stdout noise from the child process.
      return;
    }
    if (!isRecord(message) || !Object.hasOwn(message, 'id')) {
      return;
    }
    if (typeof message.id !== 'number' || !pending.has(message.id)) {
      rejectPending(new Error(`[mcp:${serverId}] returned an invalid JSON-RPC response for the pending request`));
      return;
    }
    const request = pending.get(message.id);
    pending.delete(message.id);
    stdoutBytes = 0;
    request?.resolve(message as McpJsonRpcResponse);
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
        const timeout = setTimeout(() => {
          pending.delete(requestId);
          reject(new Error(`[mcp:${serverId}] ${method} timed out after ${requestTimeoutMs}ms`));
        }, requestTimeoutMs);
        pending.set(requestId, {
          resolve: (result) => {
            clearTimeout(timeout);
            resolve(result);
          },
          reject: (error) => {
            clearTimeout(timeout);
            reject(error);
          },
        });
        writeMessage({
          jsonrpc: '2.0',
          id: requestId,
          method,
          params,
        }).catch((error) => {
          pending.delete(requestId);
          clearTimeout(timeout);
          reject(
            new Error(`[mcp:${serverId}] failed to write ${method}: ${safeMcpErrorMessage(getMcpErrorMessage(error), secretValues)}`)
          );
        });
      });
      return assertMcpJsonRpcSuccess(response, serverId, method, requestId, secretValues);
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
      if (closed) return;
      const ownedPid = child.pid;
      if (typeof ownedPid !== 'number' || ownedPid <= 0) {
        throw new Error(`[mcp:${serverId}] cannot identify its owned stdio server process`);
      }

      const signalOwnedServer = (signal: NodeJS.Signals) => {
        try {
          if (ownsProcessGroup) {
            process.kill(-ownedPid, signal);
          } else {
            child.kill(signal);
          }
        } catch {
          if (!closed) {
            try { child.kill(signal); } catch {}
          }
        }
      };

      signalOwnedServer('SIGTERM');
      let graceTimer: ReturnType<typeof setTimeout> | undefined;
      const graceful = await Promise.race([
        processClosed.then(() => true),
        new Promise<boolean>((resolve) => {
          graceTimer = setTimeout(() => resolve(false), 150);
        }),
      ]);
      if (graceTimer) clearTimeout(graceTimer);
      if (!graceful) {
        signalOwnedServer('SIGKILL');
        let killTimer: ReturnType<typeof setTimeout> | undefined;
        const terminated = await Promise.race([
          processClosed.then(() => true),
          new Promise<boolean>((resolve) => {
            killTimer = setTimeout(() => resolve(false), 1_000);
          }),
        ]);
        if (killTimer) clearTimeout(killTimer);
        if (!terminated) {
          throw new Error(`[mcp:${serverId}] owned stdio server did not terminate after escalation`);
        }
      }
    },
  };
}

async function createHttpMcpSession(serverId: string, server: McpHttpServerConfig) {
  if (!server.endpoint?.trim()) {
    throw new Error(`[mcp:${serverId}] http server is missing an endpoint`);
  }

  let sessionId: string | null = null;
  const requestTimeoutMs = mcpRequestTimeoutMs();
  const maximumResponseBytes = mcpMaxResponseBytes();
  const secrets = Object.entries(server.headers ?? {})
    .filter(([name]) => /authorization|token|api[-_]?key/i.test(name))
    .flatMap(([, value]) => [value, value.replace(/^Bearer\s+/i, '')]);

  const sendRequest = async (
    method: string,
    params?: Record<string, unknown>,
    includeId = true
  ): Promise<Record<string, unknown>> => {
    const payload = includeId
      ? {
          jsonrpc: '2.0',
          id: crypto.randomUUID(),
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

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), requestTimeoutMs);
    try {
      const response = await fetch(server.endpoint, {
        method: 'POST',
        redirect: 'error',
        signal: controller.signal,
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

      const json = JSON.parse(await readBoundedMcpResponse(response, maximumResponseBytes, serverId, method));
      return assertMcpJsonRpcSuccess(json, serverId, method, payload.id!, secrets);
    } catch (error) {
      if (controller.signal.aborted) {
        throw new Error(`[mcp:${serverId}] ${method} timed out after ${requestTimeoutMs}ms`);
      }
      const detail = safeMcpErrorMessage(getMcpErrorMessage(error), secrets);
      throw new Error(detail);
    } finally {
      clearTimeout(timer);
    }
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

function parseComputerToolTextResult<T>(result: McpToolCallResult): T {
  if (result.isError) {
    const text = result.content.find((entry) => typeof entry.text === 'string')?.text;
    throw new Error(`[computer] ${result.toolName} failed${text ? `: ${text}` : ''}`);
  }

  const text = result.content.find((entry) => typeof entry.text === 'string')?.text;
  if (!text) {
    throw new Error(`[computer] ${result.toolName} returned no JSON text content`);
  }

  try {
    return JSON.parse(text) as T;
  } catch (error) {
    throw new Error(
      `[computer] ${result.toolName} returned invalid JSON text content: ${getMcpErrorMessage(error)}`
    );
  }
}

async function callScriptKitComputerTool<T>(
  toolName: string,
  args: Record<string, unknown> = {}
): Promise<T> {
  const server = await loadScriptKitSelfMcpServerConfig();
  const result = await withMcpSession('__scriptkit_self__', server, async (session) => {
    const response = await session.request('tools/call', {
      name: toolName,
      arguments: args,
    });
    return normalizeMcpToolCallResult('__scriptkit_self__', toolName, response);
  });
  return parseComputerToolTextResult<T>(result);
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
    const maximumConcurrency = mcpDiscoveryConcurrency();
    const config = await loadScriptKitMcpConfig();
    const entries = serverId
      ? (() => {
          const server = getEnabledMcpServerEntries(config).find(([id]) => id === serverId);
          return server ? [server] : [];
        })()
      : getEnabledMcpServerEntries(config);

    if (entries.length === 0) return [];
    const toolGroups: McpToolInfo[][] = new Array(entries.length);
    let nextIndex = 0;
    let stopped = false;
    let firstFailure: unknown;
    await Promise.all(Array.from({ length: Math.min(maximumConcurrency, entries.length) }, async () => {
      while (!stopped && nextIndex < entries.length) {
        const index = nextIndex++;
        const [id, server] = entries[index]!;
        try {
          toolGroups[index] = await withMcpSession(id, server, async (session) => {
            const response = await session.request('tools/list', {});
            return Array.isArray(response.tools)
              ? response.tools.map((tool) => normalizeMcpToolInfo(id, tool as Record<string, unknown>))
              : [];
          });
        } catch (error) {
          if (!stopped) {
            stopped = true;
            firstFailure = error;
          }
        }
      }
    }));
    if (stopped) throw firstFailure;

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

globalThis.computer = {
  async listNativeWindows(
    options: ComputerUseListNativeWindowsOptions = {}
  ): Promise<ComputerUseListNativeWindowsResult> {
    return callScriptKitComputerTool<ComputerUseListNativeWindowsResult>(
      'computer/list_native_windows',
      {
        includeHidden: options.includeHidden ?? false,
        includeBackground: options.includeBackground ?? false,
      }
    );
  },

  async captureNativeWindow(
    options: ComputerUseCaptureNativeWindowOptions
  ): Promise<ComputerUseCaptureNativeWindowResult> {
    return callScriptKitComputerTool<ComputerUseCaptureNativeWindowResult>(
      'computer/capture_native_window',
      {
        pid: options.pid,
        nativeWindowId: options.nativeWindowId,
        hiDpi: options.hiDpi ?? false,
        includeImage: options.includeImage ?? false,
        ...(options.expectedBundleId
          ? { expectedBundleId: options.expectedBundleId }
          : {}),
      }
    );
  },
} satisfies ComputerUseApi;

export {};
