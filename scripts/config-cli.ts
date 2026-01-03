#!/usr/bin/env bun
/**
 * Script Kit Config CLI
 * 
 * A CLI tool for AI agents to read and modify ~/.sk/kit/config.ts
 * 
 * Usage:
 *   bun scripts/config-cli.ts get [key]        - Read value(s)
 *   bun scripts/config-cli.ts set <key> <value> - Modify a value
 *   bun scripts/config-cli.ts list             - Show all options with values
 *   bun scripts/config-cli.ts validate         - Check if config is valid
 *   bun scripts/config-cli.ts reset [key]      - Restore default(s)
 *   bun scripts/config-cli.ts --help           - Show this help
 * 
 * Output is JSON by default for AI parsing.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

// =============================================================================
// Types (matching kit-sdk.ts and src/config.rs)
// =============================================================================

type KeyModifier = "meta" | "ctrl" | "alt" | "shift";
type KeyCode =
  | "KeyA" | "KeyB" | "KeyC" | "KeyD" | "KeyE" | "KeyF" | "KeyG"
  | "KeyH" | "KeyI" | "KeyJ" | "KeyK" | "KeyL" | "KeyM" | "KeyN"
  | "KeyO" | "KeyP" | "KeyQ" | "KeyR" | "KeyS" | "KeyT" | "KeyU"
  | "KeyV" | "KeyW" | "KeyX" | "KeyY" | "KeyZ"
  | "Digit0" | "Digit1" | "Digit2" | "Digit3" | "Digit4"
  | "Digit5" | "Digit6" | "Digit7" | "Digit8" | "Digit9"
  | "Space" | "Enter" | "Semicolon"
  | "F1" | "F2" | "F3" | "F4" | "F5" | "F6"
  | "F7" | "F8" | "F9" | "F10" | "F11" | "F12";

interface HotkeyConfig {
  modifiers: KeyModifier[];
  key: KeyCode;
}

interface ContentPadding {
  top?: number;
  left?: number;
  right?: number;
}

interface BuiltInConfig {
  clipboardHistory?: boolean;
  appLauncher?: boolean;
  windowSwitcher?: boolean;
}

interface ProcessLimits {
  maxMemoryMb?: number;
  maxRuntimeSeconds?: number;
  healthCheckIntervalMs?: number;
}

interface Config {
  hotkey: HotkeyConfig;
  bun_path?: string;
  editor?: string;
  padding?: ContentPadding;
  editorFontSize?: number;
  terminalFontSize?: number;
  uiScale?: number;
  builtIns?: BuiltInConfig;
  clipboardHistoryMaxTextLength?: number;
  processLimits?: ProcessLimits;
}

// =============================================================================
// Default Values (matching src/config.rs)
// =============================================================================

const DEFAULTS: Required<Config> = {
  hotkey: {
    modifiers: ["meta"],
    key: "Semicolon"
  },
  bun_path: "",  // Empty means auto-detect
  editor: "code",
  padding: {
    top: 8,
    left: 12,
    right: 12
  },
  editorFontSize: 14,
  terminalFontSize: 14,
  uiScale: 1.0,
  builtIns: {
    clipboardHistory: true,
    appLauncher: true,
    windowSwitcher: true
  },
  clipboardHistoryMaxTextLength: 100000,
  processLimits: {
    maxMemoryMb: undefined,
    maxRuntimeSeconds: undefined,
    healthCheckIntervalMs: 5000
  }
};

// =============================================================================
// Config Schema for Documentation
// =============================================================================

interface ConfigOption {
  key: string;
  type: string;
  default: unknown;
  description: string;
  example?: string;
}

const CONFIG_SCHEMA: ConfigOption[] = [
  {
    key: "hotkey.modifiers",
    type: "KeyModifier[]",
    default: ["meta"],
    description: "Modifier keys for global hotkey (meta, ctrl, alt, shift)",
    example: '["meta", "shift"]'
  },
  {
    key: "hotkey.key",
    type: "KeyCode",
    default: "Semicolon",
    description: "Main key for global hotkey (KeyA-KeyZ, Digit0-Digit9, Space, Enter, Semicolon, F1-F12)",
    example: "KeyK"
  },
  {
    key: "bun_path",
    type: "string",
    default: "",
    description: "Custom path to bun executable (empty = auto-detect)",
    example: "/opt/homebrew/bin/bun"
  },
  {
    key: "editor",
    type: "string",
    default: "code",
    description: "Editor command for 'Open in Editor' actions",
    example: "vim"
  },
  {
    key: "padding.top",
    type: "number",
    default: 8,
    description: "Top padding in pixels for content areas"
  },
  {
    key: "padding.left",
    type: "number",
    default: 12,
    description: "Left padding in pixels for content areas"
  },
  {
    key: "padding.right",
    type: "number",
    default: 12,
    description: "Right padding in pixels for content areas"
  },
  {
    key: "editorFontSize",
    type: "number",
    default: 14,
    description: "Font size for editor prompt in pixels"
  },
  {
    key: "terminalFontSize",
    type: "number",
    default: 14,
    description: "Font size for terminal prompt in pixels"
  },
  {
    key: "uiScale",
    type: "number",
    default: 1.0,
    description: "UI scale factor (1.0 = 100%)"
  },
  {
    key: "builtIns.clipboardHistory",
    type: "boolean",
    default: true,
    description: "Enable clipboard history built-in feature"
  },
  {
    key: "builtIns.appLauncher",
    type: "boolean",
    default: true,
    description: "Enable app launcher built-in feature"
  },
  {
    key: "builtIns.windowSwitcher",
    type: "boolean",
    default: true,
    description: "Enable window switcher built-in feature"
  },
  {
    key: "clipboardHistoryMaxTextLength",
    type: "number",
    default: 100000,
    description: "Maximum text length (bytes) to store for clipboard history entries (0 = no limit)"
  },
  {
    key: "processLimits.maxMemoryMb",
    type: "number | undefined",
    default: undefined,
    description: "Maximum memory usage in MB (undefined = no limit)"
  },
  {
    key: "processLimits.maxRuntimeSeconds",
    type: "number | undefined",
    default: undefined,
    description: "Maximum runtime in seconds (undefined = no limit)"
  },
  {
    key: "processLimits.healthCheckIntervalMs",
    type: "number",
    default: 5000,
    description: "Health check interval in milliseconds"
  }
];

// =============================================================================
// Utilities
// =============================================================================

const CONFIG_PATH = path.join(os.homedir(), '.sk/kit', 'config.ts');

interface Result<T> {
  success: boolean;
  data?: T;
  error?: string;
}

function output(result: Result<unknown>): void {
  console.log(JSON.stringify(result, null, 2));
}

function success<T>(data: T): void {
  output({ success: true, data });
}

function error(message: string): void {
  output({ success: false, error: message });
  process.exit(1);
}

/**
 * Get a nested value from an object using dot notation
 */
function getNestedValue(obj: Record<string, unknown>, key: string): unknown {
  const parts = key.split('.');
  let current: unknown = obj;
  
  for (const part of parts) {
    if (current === null || current === undefined) {
      return undefined;
    }
    if (typeof current !== 'object') {
      return undefined;
    }
    current = (current as Record<string, unknown>)[part];
  }
  
  return current;
}

/**
 * Set a nested value in an object using dot notation
 */
function setNestedValue(obj: Record<string, unknown>, key: string, value: unknown): void {
  const parts = key.split('.');
  let current = obj;
  
  for (let i = 0; i < parts.length - 1; i++) {
    const part = parts[i];
    if (!(part in current) || typeof current[part] !== 'object' || current[part] === null) {
      current[part] = {};
    }
    current = current[part] as Record<string, unknown>;
  }
  
  const lastPart = parts[parts.length - 1];
  current[lastPart] = value;
}

/**
 * Delete a nested value from an object using dot notation
 */
function deleteNestedValue(obj: Record<string, unknown>, key: string): boolean {
  const parts = key.split('.');
  let current = obj;
  
  for (let i = 0; i < parts.length - 1; i++) {
    const part = parts[i];
    if (!(part in current) || typeof current[part] !== 'object' || current[part] === null) {
      return false;
    }
    current = current[part] as Record<string, unknown>;
  }
  
  const lastPart = parts[parts.length - 1];
  if (lastPart in current) {
    delete current[lastPart];
    return true;
  }
  return false;
}

/**
 * Load and parse the current config
 */
async function loadConfig(): Promise<Config | null> {
  if (!fs.existsSync(CONFIG_PATH)) {
    return null;
  }
  
  // Use bun to transpile and evaluate the config
  const tmpJsPath = '/tmp/kit-config-cli.js';
  
  try {
    // Transpile TypeScript to JavaScript
    const buildResult = Bun.spawnSync(['bun', 'build', '--target=bun', CONFIG_PATH, `--outfile=${tmpJsPath}`]);
    if (buildResult.exitCode !== 0) {
      throw new Error(`Failed to transpile config: ${buildResult.stderr.toString()}`);
    }
    
    // Execute and extract default export
    const jsonResult = Bun.spawnSync(['bun', '-e', `console.log(JSON.stringify(require('${tmpJsPath}').default))`]);
    if (jsonResult.exitCode !== 0) {
      throw new Error(`Failed to evaluate config: ${jsonResult.stderr.toString()}`);
    }
    
    const jsonStr = jsonResult.stdout.toString().trim();
    return JSON.parse(jsonStr) as Config;
  } catch (e) {
    throw new Error(`Failed to load config: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/**
 * Read the raw config.ts file content
 */
function readConfigFile(): string | null {
  if (!fs.existsSync(CONFIG_PATH)) {
    return null;
  }
  return fs.readFileSync(CONFIG_PATH, 'utf-8');
}

/**
 * Write the config.ts file content
 */
function writeConfigFile(content: string): void {
  const dir = path.dirname(CONFIG_PATH);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  fs.writeFileSync(CONFIG_PATH, content, 'utf-8');
}

/**
 * Create a default config file
 */
function createDefaultConfig(): string {
  return `import type { Config } from "@johnlindquist/kit";

export default {
  hotkey: {
    modifiers: ["meta"],
    key: "Semicolon"
  }
} satisfies Config;
`;
}

/**
 * Parse a string value to the appropriate type
 */
function parseValue(value: string, key: string): unknown {
  // Find the schema entry for this key to determine type
  const schema = CONFIG_SCHEMA.find(s => s.key === key);
  if (!schema) {
    // Try to infer type from value
    if (value === 'true') return true;
    if (value === 'false') return false;
    if (value === 'undefined' || value === 'null') return undefined;
    const num = Number(value);
    if (!isNaN(num)) return num;
    // Try to parse as JSON (for arrays)
    try {
      return JSON.parse(value);
    } catch {
      return value;
    }
  }
  
  const type = schema.type;
  
  if (type.includes('boolean')) {
    if (value === 'true') return true;
    if (value === 'false') return false;
    throw new Error(`Invalid boolean value: ${value}. Use 'true' or 'false'.`);
  }
  
  if (type.includes('number')) {
    if (value === 'undefined' || value === 'null') return undefined;
    const num = Number(value);
    if (isNaN(num)) {
      throw new Error(`Invalid number value: ${value}`);
    }
    return num;
  }
  
  if (type.includes('[]')) {
    // Array type - parse as JSON
    try {
      const parsed = JSON.parse(value);
      if (!Array.isArray(parsed)) {
        throw new Error(`Expected array, got: ${typeof parsed}`);
      }
      return parsed;
    } catch (e) {
      throw new Error(`Invalid array value: ${value}. Use JSON format like '["meta", "shift"]'.`);
    }
  }
  
  // String or other - return as-is
  return value;
}

/**
 * Validate a config value against constraints
 */
function validateValue(key: string, value: unknown): { valid: boolean; error?: string } {
  const schema = CONFIG_SCHEMA.find(s => s.key === key);
  if (!schema) {
    return { valid: true }; // Unknown key - allow but warn
  }
  
  // Type validation
  const type = schema.type;
  
  if (key === 'hotkey.modifiers') {
    if (!Array.isArray(value)) {
      return { valid: false, error: 'hotkey.modifiers must be an array' };
    }
    const validMods: KeyModifier[] = ['meta', 'ctrl', 'alt', 'shift'];
    for (const mod of value) {
      if (!validMods.includes(mod as KeyModifier)) {
        return { valid: false, error: `Invalid modifier: ${mod}. Valid modifiers: ${validMods.join(', ')}` };
      }
    }
  }
  
  if (key === 'hotkey.key') {
    const validKeys = [
      'KeyA', 'KeyB', 'KeyC', 'KeyD', 'KeyE', 'KeyF', 'KeyG',
      'KeyH', 'KeyI', 'KeyJ', 'KeyK', 'KeyL', 'KeyM', 'KeyN',
      'KeyO', 'KeyP', 'KeyQ', 'KeyR', 'KeyS', 'KeyT', 'KeyU',
      'KeyV', 'KeyW', 'KeyX', 'KeyY', 'KeyZ',
      'Digit0', 'Digit1', 'Digit2', 'Digit3', 'Digit4',
      'Digit5', 'Digit6', 'Digit7', 'Digit8', 'Digit9',
      'Space', 'Enter', 'Semicolon',
      'F1', 'F2', 'F3', 'F4', 'F5', 'F6',
      'F7', 'F8', 'F9', 'F10', 'F11', 'F12'
    ];
    if (!validKeys.includes(value as string)) {
      return { valid: false, error: `Invalid key: ${value}. Valid keys: ${validKeys.join(', ')}` };
    }
  }
  
  if (type.includes('number') && value !== undefined && value !== null) {
    if (typeof value !== 'number' || isNaN(value)) {
      return { valid: false, error: `${key} must be a number` };
    }
    // Range validations
    if (key === 'uiScale' && (value < 0.5 || value > 3.0)) {
      return { valid: false, error: 'uiScale must be between 0.5 and 3.0' };
    }
    if (key.includes('FontSize') && (value < 8 || value > 72)) {
      return { valid: false, error: 'Font size must be between 8 and 72' };
    }
    if (key.includes('padding') && value < 0) {
      return { valid: false, error: 'Padding cannot be negative' };
    }
  }
  
  if (type === 'boolean' && typeof value !== 'boolean') {
    return { valid: false, error: `${key} must be a boolean` };
  }
  
  return { valid: true };
}

/**
 * Update a value in config.ts while preserving formatting
 * Uses regex-based replacement for simple cases
 */
function updateConfigValue(key: string, value: unknown): void {
  let content = readConfigFile();
  
  if (!content) {
    // Create new config file
    content = createDefaultConfig();
  }
  
  const parts = key.split('.');
  const valueStr = JSON.stringify(value);
  
  // Strategy: For nested keys, we need to find and update the specific property
  // This is a simplified approach - for complex cases, consider ts-morph
  
  if (parts.length === 1) {
    // Top-level key
    const keyName = parts[0];
    // Try to find existing key and replace - use a more flexible pattern
    // Match the key name followed by colon and value, stopping at comma, newline, or closing brace
    const existingKeyRegex = new RegExp(`(\\s*["']?${keyName}["']?\\s*:\\s*)([^,}\\n]+(?:\\{[^}]*\\})?)`, 'g');
    
    if (existingKeyRegex.test(content)) {
      // Reset lastIndex since test() advances it
      existingKeyRegex.lastIndex = 0;
      content = content.replace(existingKeyRegex, `$1${valueStr}`);
    } else {
      // Key doesn't exist - need to add it
      // Find the content before the closing } satisfies/as Config
      // We need to ensure there's a comma after the last property
      const insertRegex = /(\s*)(})\s*(satisfies|as)\s+Config/;
      const match = content.match(insertRegex);
      
      if (match) {
        // Find position to insert
        const beforeClose = content.slice(0, content.indexOf(match[0]));
        
        // Check if we need to add a comma after the last property
        // Look for the last non-whitespace character before the closing brace
        const trimmedBefore = beforeClose.trimEnd();
        const needsComma = !trimmedBefore.endsWith(',') && !trimmedBefore.endsWith('{');
        
        const commaIfNeeded = needsComma ? ',' : '';
        content = content.replace(insertRegex, `${commaIfNeeded}\n  ${keyName}: ${valueStr}\n$2 $3 Config`);
      }
    }
  } else if (parts.length === 2) {
    // Nested key (e.g., hotkey.key, padding.top)
    const [parent, child] = parts;
    
    // Check if parent object exists - use a pattern that captures nested braces properly
    const parentRegex = new RegExp(`(["']?${parent}["']?\\s*:\\s*)\\{([^}]*)\\}`, 's');
    const parentMatch = content.match(parentRegex);
    
    if (parentMatch) {
      // Parent exists - update or add the child property
      const parentContent = parentMatch[2];
      const childRegex = new RegExp(`(["']?${child}["']?\\s*:\\s*)([^,}\\n]+)`);
      
      if (childRegex.test(parentContent)) {
        // Child exists - update it
        const newParentContent = parentContent.replace(childRegex, `$1${valueStr}`);
        content = content.replace(parentRegex, `$1{${newParentContent}}`);
      } else {
        // Child doesn't exist - add it at the end of the parent object
        const trimmedContent = parentContent.trimEnd();
        const needsComma = !trimmedContent.endsWith(',') && trimmedContent.length > 0;
        const commaIfNeeded = needsComma ? ',' : '';
        const newParentContent = parentContent.trimEnd() + commaIfNeeded + `\n    ${child}: ${valueStr}`;
        content = content.replace(parentRegex, `$1{${newParentContent}\n  }`);
      }
    } else {
      // Parent doesn't exist - create it with the child
      // Same logic as top-level insertion but with nested object
      const insertRegex = /(\s*)(})\s*(satisfies|as)\s+Config/;
      const match = content.match(insertRegex);
      
      if (match) {
        const beforeClose = content.slice(0, content.indexOf(match[0]));
        const trimmedBefore = beforeClose.trimEnd();
        const needsComma = !trimmedBefore.endsWith(',') && !trimmedBefore.endsWith('{');
        const commaIfNeeded = needsComma ? ',' : '';
        
        content = content.replace(insertRegex, `${commaIfNeeded}\n  ${parent}: {\n    ${child}: ${valueStr}\n  }\n$2 $3 Config`);
      }
    }
  } else {
    throw new Error(`Deep nesting (${parts.length} levels) not supported. Max 2 levels.`);
  }
  
  writeConfigFile(content);
}

/**
 * Reset a value to default in config.ts
 */
function resetConfigValue(key: string): void {
  const defaultValue = getNestedValue(DEFAULTS as unknown as Record<string, unknown>, key);
  
  if (defaultValue === undefined && !CONFIG_SCHEMA.some(s => s.key === key)) {
    throw new Error(`Unknown config key: ${key}`);
  }
  
  // For optional fields with undefined default, we remove the key
  if (defaultValue === undefined || defaultValue === '') {
    // Remove the key from config
    let content = readConfigFile();
    if (!content) {
      return; // Nothing to reset
    }
    
    const parts = key.split('.');
    if (parts.length === 1) {
      // Remove top-level key
      const regex = new RegExp(`\\s*["']?${parts[0]}["']?\\s*:\\s*[^,}\\n]+,?\\n?`, 'g');
      content = content.replace(regex, '');
    } else if (parts.length === 2) {
      // Remove nested key
      const regex = new RegExp(`\\s*["']?${parts[1]}["']?\\s*:\\s*[^,}\\n]+,?`, 'g');
      content = content.replace(regex, '');
    }
    
    writeConfigFile(content);
  } else {
    // Set to default value
    updateConfigValue(key, defaultValue);
  }
}

// =============================================================================
// Commands
// =============================================================================

async function cmdGet(key?: string): Promise<void> {
  try {
    const config = await loadConfig();
    
    if (!config) {
      success({
        exists: false,
        path: CONFIG_PATH,
        message: "Config file does not exist. Using defaults.",
        config: DEFAULTS
      });
      return;
    }
    
    if (key) {
      const value = getNestedValue(config as unknown as Record<string, unknown>, key);
      const defaultValue = getNestedValue(DEFAULTS as unknown as Record<string, unknown>, key);
      
      success({
        key,
        value: value ?? defaultValue,
        isDefault: value === undefined,
        default: defaultValue
      });
    } else {
      // Return full config merged with defaults
      const merged = { ...DEFAULTS, ...config };
      success({
        path: CONFIG_PATH,
        config: merged
      });
    }
  } catch (e) {
    error(e instanceof Error ? e.message : String(e));
  }
}

async function cmdSet(key: string, value: string): Promise<void> {
  if (!key || value === undefined) {
    error("Usage: bun scripts/config-cli.ts set <key> <value>");
  }
  
  try {
    // Parse the value
    const parsedValue = parseValue(value, key);
    
    // Validate the value
    const validation = validateValue(key, parsedValue);
    if (!validation.valid) {
      error(validation.error!);
    }
    
    // Update the config file
    updateConfigValue(key, parsedValue);
    
    // Read back to verify
    const config = await loadConfig();
    const newValue = config ? getNestedValue(config as unknown as Record<string, unknown>, key) : parsedValue;
    
    success({
      key,
      value: newValue,
      message: `Successfully set ${key} to ${JSON.stringify(parsedValue)}`
    });
  } catch (e) {
    error(e instanceof Error ? e.message : String(e));
  }
}

async function cmdList(): Promise<void> {
  try {
    const config = await loadConfig();
    
    const options = CONFIG_SCHEMA.map(schema => {
      const currentValue = config 
        ? getNestedValue(config as unknown as Record<string, unknown>, schema.key)
        : undefined;
      const effectiveValue = currentValue ?? schema.default;
      
      return {
        key: schema.key,
        type: schema.type,
        current: effectiveValue,
        default: schema.default,
        isCustom: currentValue !== undefined && currentValue !== schema.default,
        description: schema.description,
        example: schema.example
      };
    });
    
    success({
      path: CONFIG_PATH,
      exists: config !== null,
      options
    });
  } catch (e) {
    error(e instanceof Error ? e.message : String(e));
  }
}

async function cmdValidate(): Promise<void> {
  try {
    // Check if file exists
    if (!fs.existsSync(CONFIG_PATH)) {
      success({
        valid: true,
        exists: false,
        message: "Config file does not exist. Default config will be used."
      });
      return;
    }
    
    // Try to load and parse the config
    const config = await loadConfig();
    
    if (!config) {
      error("Failed to parse config file");
    }
    
    // Validate required fields
    const errors: string[] = [];
    const warnings: string[] = [];
    
    if (!config.hotkey) {
      errors.push("Missing required field: hotkey");
    } else {
      if (!config.hotkey.modifiers) {
        errors.push("Missing required field: hotkey.modifiers");
      }
      if (!config.hotkey.key) {
        errors.push("Missing required field: hotkey.key");
      }
    }
    
    // Validate all present values
    const configRecord = config as unknown as Record<string, unknown>;
    for (const schema of CONFIG_SCHEMA) {
      const value = getNestedValue(configRecord, schema.key);
      if (value !== undefined) {
        const validation = validateValue(schema.key, value);
        if (!validation.valid) {
          errors.push(validation.error!);
        }
      }
    }
    
    // Check for unknown keys
    const knownTopLevel = ['hotkey', 'bun_path', 'editor', 'padding', 'editorFontSize', 'terminalFontSize', 'uiScale', 'builtIns', 'processLimits'];
    for (const key of Object.keys(config)) {
      if (!knownTopLevel.includes(key)) {
        warnings.push(`Unknown config key: ${key}`);
      }
    }
    
    if (errors.length > 0) {
      output({
        success: false,
        valid: false,
        errors,
        warnings: warnings.length > 0 ? warnings : undefined
      });
      process.exit(1);
    }
    
    success({
      valid: true,
      message: "Config is valid",
      warnings: warnings.length > 0 ? warnings : undefined
    });
  } catch (e) {
    output({
      success: false,
      valid: false,
      errors: [e instanceof Error ? e.message : String(e)]
    });
    process.exit(1);
  }
}

async function cmdReset(key?: string): Promise<void> {
  try {
    if (key) {
      // Reset specific key
      resetConfigValue(key);
      const defaultValue = getNestedValue(DEFAULTS as unknown as Record<string, unknown>, key);
      
      success({
        key,
        value: defaultValue,
        message: `Reset ${key} to default value`
      });
    } else {
      // Reset entire config
      const content = createDefaultConfig();
      writeConfigFile(content);
      
      success({
        message: "Reset config to defaults",
        config: {
          hotkey: DEFAULTS.hotkey
        }
      });
    }
  } catch (e) {
    error(e instanceof Error ? e.message : String(e));
  }
}

function showHelp(): void {
  const help = `
Script Kit Config CLI

USAGE:
  bun scripts/config-cli.ts <command> [args]

COMMANDS:
  get [key]           Read a config value (or all values if no key specified)
  set <key> <value>   Set a config value
  list                List all available config options with current values
  validate            Validate the current config file
  reset [key]         Reset a config value to default (or all values if no key)
  --help, -h          Show this help message

EXAMPLES:
  # Get the current hotkey
  bun scripts/config-cli.ts get hotkey.key

  # Get all config values
  bun scripts/config-cli.ts get

  # Set editor font size
  bun scripts/config-cli.ts set editorFontSize 16

  # Set hotkey to Cmd+K
  bun scripts/config-cli.ts set hotkey.key KeyK

  # Set hotkey modifiers
  bun scripts/config-cli.ts set hotkey.modifiers '["meta", "shift"]'

  # Disable clipboard history
  bun scripts/config-cli.ts set builtIns.clipboardHistory false

  # List all available options
  bun scripts/config-cli.ts list

  # Check if config is valid
  bun scripts/config-cli.ts validate

  # Reset editor font size to default
  bun scripts/config-cli.ts reset editorFontSize

  # Reset entire config
  bun scripts/config-cli.ts reset

AVAILABLE CONFIG KEYS:
${CONFIG_SCHEMA.map(s => `  ${s.key.padEnd(35)} ${s.type.padEnd(25)} (default: ${JSON.stringify(s.default)})`).join('\n')}

OUTPUT:
  All output is JSON for easy parsing by AI agents.
  Check the "success" field to determine if the operation succeeded.

CONFIG FILE:
  Location: ${CONFIG_PATH}
`;
  
  console.log(help);
}

// =============================================================================
// Main
// =============================================================================

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  
  if (args.length === 0 || args[0] === '--help' || args[0] === '-h') {
    showHelp();
    process.exit(0);
  }
  
  const command = args[0];
  
  switch (command) {
    case 'get':
      await cmdGet(args[1]);
      break;
    case 'set':
      await cmdSet(args[1], args[2]);
      break;
    case 'list':
      await cmdList();
      break;
    case 'validate':
      await cmdValidate();
      break;
    case 'reset':
      await cmdReset(args[1]);
      break;
    default:
      error(`Unknown command: ${command}. Use --help for usage.`);
  }
}

main().catch(e => {
  error(e instanceof Error ? e.message : String(e));
});
