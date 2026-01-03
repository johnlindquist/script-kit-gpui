import * as readline from 'node:readline';
import * as nodePath from 'node:path';
import * as os from 'node:os';
import * as fs from 'node:fs/promises';
import { constants as fsConstants } from 'node:fs';

// =============================================================================
// SDK Version - Used to verify correct version is loaded
// =============================================================================
export const SDK_VERSION = '0.2.0';

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
// Chat Types (TIER 4A)
// =============================================================================

export interface ChatMessage {
  text: string;
  position: 'left' | 'right';
}

export interface ChatOptions {
  onInit?: () => Promise<void>;
  onSubmit?: (input: string) => Promise<void>;
}

export interface ChatController {
  addMessage(msg: ChatMessage): void;
  setInput(text: string): void;
  submit(): void;
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
  | 'left'
  | 'right'
  | 'top'
  | 'bottom'
  | 'top-left'
  | 'top-right'
  | 'bottom-left'
  | 'bottom-right'
  | 'center'
  | 'maximize';

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
// Config Types (for ~/.kenv/config.ts)
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
  // Special keys
  | "Space" | "Enter" | "Semicolon"
  // Function keys (if supported)
  | "F1" | "F2" | "F3" | "F4" | "F5" | "F6"
  | "F7" | "F8" | "F9" | "F10" | "F11" | "F12";

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
   * - Special keys: "Space", "Enter", "Semicolon"
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
 * Script Kit configuration schema.
 * 
 * This configuration is loaded from `~/.kenv/config.ts` and controls
 * Script Kit's behavior, appearance, and built-in features.
 * 
 * @example Minimal configuration (only hotkey required)
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
 * @example Full configuration with all options
 * ```typescript
 * import type { Config } from "@johnlindquist/kit";
 * 
 * export default {
 *   hotkey: {
 *     modifiers: ["meta"],
 *     key: "Semicolon"
 *   },
 *   editor: "code",
 *   padding: { top: 8, left: 12, right: 12 },
 *   editorFontSize: 14,
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
   * @default 14
   * @example 12 // Smaller for more code visibility
   * @example 16 // Larger for better readability
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
}

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
  | ActionTriggeredMessage;

interface ChatMessageType {
  type: 'chat';
  id: string;
}

interface ChatActionMessage {
  type: 'chatAction';
  id: string;
  action: 'addMessage' | 'setInput' | 'submit';
  data?: ChatMessage | string;
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
  action: 'focus' | 'close' | 'minimize' | 'maximize' | 'resize' | 'move';
  windowId?: number;
  bounds?: TargetWindowBounds;
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

// Use raw stdin reading instead of readline interface
// This works better with bun's --preload mode
let stdinBuffer = '';

console.error('[SDK] Setting up stdin handler...');

// Set up raw stdin handling
process.stdin.setEncoding('utf8');
// Resume stdin to start receiving data - it may be paused by default  
process.stdin.resume();
// Unref stdin so it doesn't keep the process alive when script completes
// This allows the process to exit naturally when all async work is done
(process.stdin as any).unref?.();
console.error('[SDK] stdin resumed, readable:', process.stdin.readable);

// Helper to manage stdin ref counting for pending operations
// When there are pending operations waiting for stdin responses, we need to
// keep stdin referenced so the process doesn't exit prematurely
function addPending(id: string, resolver: (msg: any) => void): void {
  const wasEmpty = pending.size === 0;
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
  let newlineIndex;
  while ((newlineIndex = stdinBuffer.indexOf('\n')) !== -1) {
    const line = stdinBuffer.substring(0, newlineIndex);
    stdinBuffer = stdinBuffer.substring(newlineIndex + 1);
    
    if (line.trim()) {
      try {
        const msg = JSON.parse(line) as ResponseMessage;
        
        // Get the ID based on message type
        let id: string | undefined;
        if (msg.type === 'submit') {
          id = (msg as SubmitMessage).id;
        } else if ('requestId' in msg) {
          id = (msg as { requestId: string }).requestId;
        }
        
        if (id && pending.has(id)) {
          const resolver = removePending(id);
          if (resolver) {
            // For submit messages, pass the value directly
            if (msg.type === 'submit') {
              resolver((msg as SubmitMessage).value ?? undefined);
            } else {
              resolver(msg);
            }
          }
        }
        
        // Handle actionTriggered messages
        if (msg.type === 'actionTriggered') {
          (globalThis as any).__handleActionTriggered(msg as ActionTriggeredMessage);
        }
        
        // Also emit a custom event for widget handlers
        if ((msg as any).type === 'widgetEvent') {
          process.emit('widgetEvent' as any, msg);
        }
      } catch (e) {
        // Ignore parse errors - they're usually test output
      }
    }
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
  const clipboard: ClipboardAPI;
  
  /**
   * Keyboard API object
   */
  const keyboard: KeyboardAPI;
  
  /**
   * Mouse API object
   */
  const mouse: MouseAPI;
  
  // =============================================================================
  // TIER 4A: Chat Prompt
  // =============================================================================
  
  /**
   * Chat function interface with attached controller methods
   */
  interface ChatFunction {
    (options?: ChatOptions): Promise<string>;
    addMessage(msg: ChatMessage): void;
    setInput(text: string): void;
    submit(): void;
  }
  
  /**
   * Conversational chat UI where messages can be added programmatically
   * @param options - Optional chat options with onInit and onSubmit callbacks
   * @returns The final user input when submitted
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
   * Promise-based delay
   * @param ms - Milliseconds to wait
   */
  function wait(ms: number): Promise<void>;
  
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
   * Returns path relative to ~/.kenv
   * @param segments - Path segments to join
   * @returns Full path from kenv directory
   */
  function kenvPath(...segments: string[]): string;
  
  /**
   * Returns path relative to ~/.kenv (unified Script Kit directory)
   * @param segments - Path segments to join
   * @returns Full path from kenv directory
   * @deprecated Use kenvPath() instead - kitPath() now returns ~/.kenv paths for backwards compatibility
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
  const memoryMap: MemoryMapAPI;
  
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
    });
    
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
    addPending(id, (value?: any) => {
      resolve(value);
    });
    
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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    });

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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    });

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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    });

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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
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
    });

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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
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
    });

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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
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
    });

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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    });

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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
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
    });

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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
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
    });

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
  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      if (msg.value === null) {
        process.exit(0);  // Escape pressed
      }
      resolve(msg.value ?? '');
    });
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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      const value = msg.value ?? '';
      process.env[key] = value;
      resolve(value);
    });

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
globalThis.beep = async function beep(): Promise<void> {
  const message: BeepMessage = { type: 'beep' };
  send(message);
};

globalThis.say = async function say(text: string, voice?: string): Promise<void> {
  const message: SayMessage = { type: 'say', text, voice };
  send(message);
};

globalThis.notify = async function notify(options: string | NotifyOptions): Promise<void> {
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
  const message: SetStatusMessage = {
    type: 'setStatus',
    status: options.status,
    message: options.message,
  };
  send(message);
};

globalThis.menu = async function menu(icon: string, scripts?: string[]): Promise<void> {
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
  const id = nextId();
  
  return new Promise((resolve, reject) => {
    pending.set(id, (msg: SubmitMessage) => {
      // Check if there was an error
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    });
    
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
  // Auto-hide the Script Kit window so the previous app regains focus
  // and its text selection becomes accessible
  await globalThis.hide();
  
  // Small delay to ensure focus has transferred to the previous app
  await new Promise(resolve => setTimeout(resolve, 50));
  
  const id = nextId();
  
  return new Promise((resolve, reject) => {
    pending.set(id, (msg: SubmitMessage) => {
      // Check if there was an error
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve(msg.value ?? '');
      }
    });
    
    const message: GetSelectedTextMessage = { type: 'getSelectedText', requestId: id };
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
    pending.set(id, (msg: SubmitMessage) => {
      resolve(msg.value === 'true');
    });
    
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
    pending.set(id, (msg: SubmitMessage) => {
      resolve(msg.value === 'true');
    });
    
    const message: RequestAccessibilityMessage = { type: 'requestAccessibility', requestId: id };
    send(message);
  });
};

// Clipboard API object
globalThis.clipboard = {
  async readText(): Promise<string> {
    const id = nextId();
    
    return new Promise((resolve) => {
      pending.set(id, (msg: SubmitMessage) => {
        resolve(msg.value ?? '');
      });
      
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
      pending.set(id, () => {
        resolve();
      });
      
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
      pending.set(id, (msg: SubmitMessage) => {
        // Value comes back as base64-encoded string
        const base64 = msg.value ?? '';
        resolve(Buffer.from(base64, 'base64'));
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
    
    return new Promise((resolve) => {
      pending.set(id, () => {
        resolve();
      });
      
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
globalThis.keyboard = {
  async type(text: string): Promise<void> {
    const message: KeyboardMessage = {
      type: 'keyboard',
      action: 'type',
      text,
    };
    send(message);
  },
  
  async tap(...keys: string[]): Promise<void> {
    const message: KeyboardMessage = {
      type: 'keyboard',
      action: 'tap',
      keys,
    };
    send(message);
  },
};

// Mouse API object
globalThis.mouse = {
  async move(positions: Position[]): Promise<void> {
    const message: MouseMessage = {
      type: 'mouse',
      action: 'move',
      positions,
    };
    send(message);
  },
  
  async leftClick(): Promise<void> {
    const message: MouseMessage = {
      type: 'mouse',
      action: 'click',
      button: 'left',
    };
    send(message);
  },
  
  async rightClick(): Promise<void> {
    const message: MouseMessage = {
      type: 'mouse',
      action: 'click',
      button: 'right',
    };
    send(message);
  },
  
  async setPosition(position: Position): Promise<void> {
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

// The chat function with attached controller methods
const chatFn = async function chat(options?: ChatOptions): Promise<string> {
  const id = nextId();
  currentChatId = id;

  // Send the initial chat message to open the UI
  const message: ChatMessageType = {
    type: 'chat',
    id,
  };
  send(message);

  // Call onInit if provided (allows script to add initial messages)
  if (options?.onInit) {
    await options.onInit();
  }

  // Wait for user submission
  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      const value = msg.value ?? '';
      
      // Call onSubmit if provided
      if (options?.onSubmit) {
        options.onSubmit(value).then(() => {
          resolve(value);
        });
      } else {
        resolve(value);
      }
      
      currentChatId = null;
    });
  });
};

// Attach controller methods to the chat function
chatFn.addMessage = function addMessage(msg: ChatMessage): void {
  if (currentChatId === null) {
    throw new Error('chat.addMessage() called outside of a chat session');
  }
  
  const message: ChatActionMessage = {
    type: 'chatAction',
    id: currentChatId,
    action: 'addMessage',
    data: msg,
  };
  send(message);
};

chatFn.setInput = function setInput(text: string): void {
  if (currentChatId === null) {
    throw new Error('chat.setInput() called outside of a chat session');
  }
  
  const message: ChatActionMessage = {
    type: 'chatAction',
    id: currentChatId,
    action: 'setInput',
    data: text,
  };
  send(message);
};

chatFn.submit = function submit(): void {
  if (currentChatId === null) {
    throw new Error('chat.submit() called outside of a chat session');
  }
  
  const message: ChatActionMessage = {
    type: 'chatAction',
    id: currentChatId,
    action: 'submit',
  };
  send(message);
};

// Expose as global
(globalThis as unknown as { chat: typeof chatFn }).chat = chatFn;

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
  html: string,
  options?: WidgetOptions
): Promise<WidgetController> {
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

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    });

    const message: TermMessage = {
      type: 'term',
      id,
      command,
      actions: serializedActions,
    };

    send(message);
  });
};

globalThis.webcam = async function webcam(): Promise<Buffer> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // Value comes back as base64-encoded string
      const base64 = msg.value ?? '';
      resolve(Buffer.from(base64, 'base64'));
    });

    const message: WebcamMessage = {
      type: 'webcam',
      id,
    };

    send(message);
  });
};

globalThis.mic = async function mic(): Promise<Buffer> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // Value comes back as base64-encoded string
      const base64 = msg.value ?? '';
      resolve(Buffer.from(base64, 'base64'));
    });

    const message: MicMessage = {
      type: 'mic',
      id,
    };

    send(message);
  });
};

globalThis.eyeDropper = async function eyeDropper(): Promise<ColorInfo> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // Value comes back as JSON with color info
      const value = msg.value ?? '{}';
      try {
        const parsed = JSON.parse(value);
        resolve({
          sRGBHex: parsed.sRGBHex ?? '#000000',
          rgb: parsed.rgb ?? 'rgb(0, 0, 0)',
          rgba: parsed.rgba ?? 'rgba(0, 0, 0, 1)',
          hsl: parsed.hsl ?? 'hsl(0, 0%, 0%)',
          hsla: parsed.hsla ?? 'hsla(0, 0%, 0%, 1)',
          cmyk: parsed.cmyk ?? 'cmyk(0%, 0%, 0%, 100%)',
        });
      } catch {
        resolve({
          sRGBHex: '#000000',
          rgb: 'rgb(0, 0, 0)',
          rgba: 'rgba(0, 0, 0, 1)',
          hsl: 'hsl(0, 0%, 0%)',
          hsla: 'hsla(0, 0%, 0%, 1)',
          cmyk: 'cmyk(0%, 0%, 0%, 100%)',
        });
      }
    });

    const message: EyeDropperMessage = {
      type: 'eyeDropper',
      id,
    };

    send(message);
  });
};

globalThis.find = async function find(
  placeholder: string,
  options?: FindOptions
): Promise<string> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      // If user pressed Escape (value is null), exit the script
      if (msg.value === null) {
        process.exit(0);
      }
      resolve(msg.value ?? '');
    });

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
  
  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
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
    });
    
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
  
  return new Promise((resolve) => {
    addPending(requestId, (msg: ResponseMessage) => {
      // Handle screenshotResult message type
      if (msg.type === 'screenshotResult') {
        const resultMsg = msg as ScreenshotResultMessage;
        resolve({
          data: resultMsg.data ?? '',
          width: resultMsg.width ?? 0,
          height: resultMsg.height ?? 0,
        });
        return;
      }
      
      // Fallback for unexpected message type
      resolve({
        data: '',
        width: 0,
        height: 0,
      });
    });
    
    const message: CaptureScreenshotMessage = {
      type: 'captureScreenshot',
      requestId,
      hiDpi: options?.hiDpi ?? false,
    };
    
    send(message);
  });
};

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

globalThis.wait = function wait(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
};

// Content Setters
globalThis.setPanel = function setPanel(html: string): void {
  const message: SetPanelMessage = { type: 'setPanel', html };
  send(message);
};

globalThis.setPreview = function setPreview(html: string): void {
  const message: SetPreviewMessage = { type: 'setPreview', html };
  send(message);
};

globalThis.setPrompt = function setPrompt(html: string): void {
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

globalThis.kenvPath = function kenvPath(...segments: string[]): string {
  return nodePath.join(os.homedir(), '.kenv', ...segments);
};

globalThis.kitPath = function kitPath(...segments: string[]): string {
  // Now returns ~/.kenv paths - ~/.kit is deprecated
  return nodePath.join(os.homedir(), '.kenv', ...segments);
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
    pending.set(id, (msg: SubmitMessage) => {
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
    });
    
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
  
  return new Promise((resolve) => {
    pending.set(id, (msg: ResponseMessage) => {
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
    });
    
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
  
  return new Promise((resolve, reject) => {
    pending.set(id, (msg: ResponseMessage) => {
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
    });
    
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
  
  return new Promise((resolve, reject) => {
    pending.set(id, (msg: ResponseMessage) => {
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
    });
    
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
  
  return new Promise((resolve, reject) => {
    pending.set(id, (msg: ResponseMessage) => {
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
    });
    
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
  
  return new Promise((resolve, reject) => {
    pending.set(id, (msg: ResponseMessage) => {
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
    });
    
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
  
  return new Promise((resolve, reject) => {
    pending.set(id, (msg: ResponseMessage) => {
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
    });
    
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
  
  return new Promise((resolve) => {
    pending.set(id, (msg: ResponseMessage) => {
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
    });
    
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
    pending.set(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    });
    
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
    pending.set(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    });
    
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
    pending.set(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    });
    
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
    pending.set(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    });
    
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
    pending.set(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    });
    
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
    pending.set(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    });
    
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
 * Calculate bounds for tiling a window to a specific screen position
 */
function calculateTileBounds(position: TilePosition, screenWidth: number, screenHeight: number): TargetWindowBounds {
  const halfWidth = Math.floor(screenWidth / 2);
  const halfHeight = Math.floor(screenHeight / 2);
  
  switch (position) {
    case 'left':
      return { x: 0, y: 0, width: halfWidth, height: screenHeight };
    case 'right':
      return { x: halfWidth, y: 0, width: halfWidth, height: screenHeight };
    case 'top':
      return { x: 0, y: 0, width: screenWidth, height: halfHeight };
    case 'bottom':
      return { x: 0, y: halfHeight, width: screenWidth, height: halfHeight };
    case 'top-left':
      return { x: 0, y: 0, width: halfWidth, height: halfHeight };
    case 'top-right':
      return { x: halfWidth, y: 0, width: halfWidth, height: halfHeight };
    case 'bottom-left':
      return { x: 0, y: halfHeight, width: halfWidth, height: halfHeight };
    case 'bottom-right':
      return { x: halfWidth, y: halfHeight, width: halfWidth, height: halfHeight };
    case 'center':
      const centerWidth = Math.floor(screenWidth * 0.6);
      const centerHeight = Math.floor(screenHeight * 0.6);
      return { 
        x: Math.floor((screenWidth - centerWidth) / 2), 
        y: Math.floor((screenHeight - centerHeight) / 2), 
        width: centerWidth, 
        height: centerHeight 
      };
    case 'maximize':
      return { x: 0, y: 0, width: screenWidth, height: screenHeight };
    default:
      return { x: 0, y: 0, width: screenWidth, height: screenHeight };
  }
}

globalThis.tileWindow = async function tileWindow(windowId: number, position: TilePosition): Promise<void> {
  // Get screen dimensions - for now use reasonable defaults
  // In a real implementation, this would query the actual screen size
  const screenWidth = 1920;
  const screenHeight = 1080;
  
  const bounds = calculateTileBounds(position, screenWidth, screenHeight);
  
  const id = nextId();
  
  return new Promise((resolve, reject) => {
    pending.set(id, (msg: SubmitMessage) => {
      if (msg.value && msg.value.startsWith('ERROR:')) {
        reject(new Error(msg.value.substring(6).trim()));
      } else {
        resolve();
      }
    });
    
    // Combine move and resize into a single action
    const message: WindowActionMessage = {
      type: 'windowAction',
      requestId: id,
      action: 'resize',
      windowId,
      bounds,
    };
    
    send(message);
  });
};

// =============================================================================
// File Search Functions
// =============================================================================

globalThis.fileSearch = async function fileSearch(query: string, options?: FindOptions): Promise<FileSearchResult[]> {
  const id = nextId();
  
  return new Promise((resolve) => {
    pending.set(id, (msg: ResponseMessage) => {
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
    });
    
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
// Global Type Declarations
// =============================================================================

declare global {
  // SDK Version
  const SDK_VERSION: string;

  // Metadata and Schema globals (set at top of script, parsed by app)
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
   * //      ^ string   ^ number | undefined
   * 
   * output({ message: `${greeting} x${count}` })
   * ```
   */
  var schema: ScriptSchema;

  // Schema type inference utilities - use with `typeof schema`
  type InferInput<S> = S extends { input: infer I extends Record<string, SchemaFieldDef> }
    ? { [K in keyof I as I[K] extends { required: true } ? K : never]: 
        I[K] extends { enum: readonly (infer E)[] } ? E :
        I[K] extends { type: 'array'; items: infer IT extends SchemaFieldType } ? SchemaTypeMap[IT][] :
        I[K] extends { type: infer T extends SchemaFieldType } ? SchemaTypeMap[T] : unknown 
      } & { [K in keyof I as I[K] extends { required: true } ? never : K]?: 
        I[K] extends { enum: readonly (infer E)[] } ? E :
        I[K] extends { type: 'array'; items: infer IT extends SchemaFieldType } ? SchemaTypeMap[IT][] :
        I[K] extends { type: infer T extends SchemaFieldType } ? SchemaTypeMap[T] : unknown 
      }
    : Record<string, unknown>;
    
  type InferOutput<S> = S extends { output: infer O extends Record<string, SchemaFieldDef> }
    ? { [K in keyof O]?: 
        O[K] extends { enum: readonly (infer E)[] } ? E :
        O[K] extends { type: 'array'; items: infer IT extends SchemaFieldType } ? SchemaTypeMap[IT][] :
        O[K] extends { type: infer T extends SchemaFieldType } ? SchemaTypeMap[T] : unknown 
      }
    : Record<string, unknown>;
    
  type SchemaTypeMap = {
    string: string;
    number: number;
    boolean: boolean;
    array: unknown[];
    object: Record<string, unknown>;
    any: unknown;
  };

  /** Typed API returned by defineSchema */
  interface TypedSchemaAPI<TInput, TOutput> {
    input: () => Promise<TInput>;
    output: (data: Partial<TOutput>) => void;
  }

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

  // AI-First Protocol functions (untyped versions - use defineSchema for typed versions)
  /**
   * Get input data. For typed version, use defineSchema().
   */
  function input<T = Record<string, unknown>>(): Promise<T>;
  
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

  // Core prompt functions
  function arg(placeholderOrConfig?: string | ArgConfig, choices?: ChoicesInput, actions?: Action[]): Promise<string>;
  function div(html?: string | DivConfig, actions?: Action[]): Promise<void>;
  function md(markdown: string): string;
  function editor(content?: string, language?: string, actions?: Action[]): Promise<string>;
  function mini(placeholderOrConfig?: string | ArgConfig, choices?: ChoicesInput): Promise<string>;
  function micro(placeholderOrConfig?: string | ArgConfig, choices?: ChoicesInput): Promise<string>;
  function select(placeholderOrConfig?: string | ArgConfig, choices?: ChoicesInput): Promise<string>;
  function fields(fieldDefs: FieldDef[]): Promise<string[]>;
  function form(htmlOrConfig: string | FormConfig): Promise<Record<string, string>>;
  function path(hint?: string | PathOptions): Promise<string>;
  function hotkey(placeholder?: string): Promise<HotkeyInfo>;
  function drop(): Promise<FileInfo[]>;
  function template(content: string, options?: TemplateOptions): Promise<string>;
  function env(name: string, defaultValue?: string): Promise<string>;

  // Chat (TIER 4A)
  function chat(options?: ChatOptions): Promise<string>;

  // Widget/Term/Media (TIER 4B)
  function widget(html: string, options?: WidgetOptions): Promise<WidgetController>;
  function term(command?: string): Promise<string>;
  function webcam(): Promise<Buffer>;
  function mic(): Promise<Buffer>;
  function eyeDropper(): Promise<ColorInfo>;
  function find(name: string, options?: FindOptions): Promise<string[]>;

  // System
  function beep(): Promise<void>;
  function say(text: string, voice?: string): Promise<void>;
  function notify(options: string | NotifyOptions): Promise<void>;
  function hud(message: string, options?: { duration?: number }): void;
  function setStatus(options: StatusOptions): Promise<void>;
  function menu(icon: string, scripts?: string[]): Promise<void>;
  function setSelectedText(text: string): Promise<void>;
  function getSelectedText(): Promise<string>;
  function hasAccessibilityPermission(): Promise<boolean>;
  function requestAccessibilityPermission(): Promise<boolean>;

  // Clipboard
  const clipboard: ClipboardAPI;
  function copy(text: string): Promise<void>;
  function paste(): Promise<string>;

  // Clipboard History
  function clipboardHistory(): Promise<ClipboardHistoryEntry[]>;
  function clipboardHistoryPin(entryId: string): Promise<void>;
  function clipboardHistoryUnpin(entryId: string): Promise<void>;
  function clipboardHistoryRemove(entryId: string): Promise<void>;
  function clipboardHistoryClear(): Promise<void>;
  function clipboardHistoryTrimOversize(): Promise<void>;

  // Window Management
  function getWindows(): Promise<SystemWindowInfo[]>;
  function focusWindow(windowId: number): Promise<void>;
  function closeWindow(windowId: number): Promise<void>;
  function minimizeWindow(windowId: number): Promise<void>;
  function maximizeWindow(windowId: number): Promise<void>;
  function moveWindow(windowId: number, x: number, y: number): Promise<void>;
  function resizeWindow(windowId: number, width: number, height: number): Promise<void>;
  function tileWindow(windowId: number, position: TilePosition): Promise<void>;

  // File Search
  function fileSearch(query: string, options?: FindOptions): Promise<FileSearchResult[]>;

  // Input
  const keyboard: KeyboardAPI;
  const mouse: MouseAPI;

  // UI Control
  function show(): Promise<void>;
  function hide(): Promise<void>;
  function showGrid(options?: GridOptions): Promise<void>;
  function hideGrid(): Promise<void>;
  function blur(): Promise<void>;
  function getWindowBounds(): Promise<WindowBounds>;
  function setWindowBounds(bounds: Partial<WindowBounds>): Promise<void>;
  function centerWindow(): Promise<void>;
  function submit(value: unknown): void;
  function exit(code?: number): void;
  function wait(ms: number): Promise<void>;
  function setPanel(html: string): void;
  function setPreview(html: string): void;
  function setPrompt(html: string): void;
  function setInput(text: string): void;
  function captureScreenshot(options?: ScreenshotOptions): Promise<ScreenshotData>;

  // Utilities
  function uuid(): string;
  function compile(template: string): (data: Record<string, string>) => string;
  function home(...segments: string[]): string;
  function kenvPath(...segments: string[]): string;
  function kitPath(...segments: string[]): string;
  function tmpPath(...segments: string[]): string;
  function isFile(filePath: string): Promise<boolean>;
  function isDir(dirPath: string): Promise<boolean>;
  function isBin(filePath: string): Promise<boolean>;

  // Memory
  const memoryMap: MemoryMapAPI;

  // File operations
  function browse(url: string): Promise<void>;
  function editFile(filePath: string): Promise<void>;
  function run(scriptName: string, ...args: string[]): Promise<unknown>;
  function inspect(data: unknown): Promise<void>;
  
  // Actions API
  function setActions(actions: Action[]): Promise<void>;
}

export {};
