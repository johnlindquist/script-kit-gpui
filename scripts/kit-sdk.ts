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

// =============================================================================
// TIER 5A: Utility Types
// =============================================================================

export interface ExecOptions {
  cwd?: string;
  shell?: string;
  all?: boolean;
}

export interface ExecResult {
  stdout: string;
  stderr: string;
  all?: string;
  exitCode: number;
}

export interface HttpResponse {
  data: unknown;
}

// =============================================================================
// TIER 5B: Storage/Path Types
// =============================================================================

export interface DbInstance {
  data: unknown;
  items?: unknown[];
  write(): Promise<void>;
}

export interface StoreAPI {
  get(key: string): Promise<unknown>;
  set(key: string, value: unknown): Promise<void>;
}

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
}

interface DivMessage {
  type: 'div';
  id: string;
  html: string;
  tailwind?: string;
}

interface EditorMessage {
  type: 'editor';
  id: string;
  content: string;
  language: string;
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
}

interface FormMessage {
  type: 'form';
  id: string;
  html: string;
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
// TIER 5A: Utility Message Types
// =============================================================================

interface ExecMessage {
  type: 'exec';
  id: string;
  command: string;
  options?: ExecOptions;
}

interface DownloadMessage {
  type: 'download';
  id: string;
  url: string;
  destination: string;
}

interface TrashMessage {
  type: 'trash';
  id: string;
  paths: string[];
}

interface ShowMessage {
  type: 'show';
}

interface HideMessage {
  type: 'hide';
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
// TIER 5B: Storage/Path Message Types
// =============================================================================

interface DbMessage {
  type: 'db';
  id: string;
  scriptName: string;
  initialData?: unknown;
}

interface StoreMessage {
  type: 'store';
  id: string;
  action: 'get' | 'set';
  key: string;
  value?: unknown;
}

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

const pending = new Map<string, (msg: SubmitMessage) => void>();

function send(msg: object): void {
  process.stdout.write(`${JSON.stringify(msg)}\n`);
}

const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: false,
});

rl.on('line', (line: string) => {
  try {
    const msg = JSON.parse(line) as SubmitMessage;
    if (msg.type === 'submit' && pending.has(msg.id)) {
      const resolver = pending.get(msg.id);
      if (resolver) {
        pending.delete(msg.id);
        resolver(msg);
      }
    }
  } catch {
    // Silently ignore non-JSON lines
  }
});

// =============================================================================
// Global API Functions (Script Kit v1 pattern - no imports needed)
// =============================================================================

declare global {
  /**
   * Prompt user for input with optional choices
   */
  function arg(placeholder: string, choices: (string | Choice)[]): Promise<string>;
  
  /**
   * Display HTML content to user
   */
  function div(html: string, tailwind?: string): Promise<void>;
  
  /**
   * Convert Markdown to HTML
   */
  function md(markdown: string): string;
  
  /**
   * Opens a Monaco-style code editor
   * @param content - Initial content to display in the editor
   * @param language - Language for syntax highlighting (e.g., 'typescript', 'javascript', 'json')
   * @returns The edited content when user submits
   */
  function editor(content?: string, language?: string): Promise<string>;
  
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
   * @param template - Template string with variables ($1, $2, ${1:default})
   * @returns The filled-in template string
   */
  function template(template: string): Promise<string>;
  
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
  // TIER 5A: Utility Functions
  // =============================================================================
  
  /**
   * Execute a shell command
   * @param command - Command to execute
   * @param options - Execution options (cwd, shell, all)
   * @returns ExecResult with stdout, stderr, and exitCode
   */
  function exec(command: string, options?: ExecOptions): Promise<ExecResult>;
  
  /**
   * HTTP GET request
   * @param url - URL to fetch
   * @returns Response with data property
   */
  function get(url: string): Promise<HttpResponse>;
  
  /**
   * HTTP POST request
   * @param url - URL to post to
   * @param data - Data to send
   * @returns Response with data property
   */
  function post(url: string, data?: unknown): Promise<HttpResponse>;
  
  /**
   * HTTP PUT request
   * @param url - URL to put to
   * @param data - Data to send
   * @returns Response with data property
   */
  function put(url: string, data?: unknown): Promise<HttpResponse>;
  
  /**
   * HTTP PATCH request
   * @param url - URL to patch
   * @param data - Data to send
   * @returns Response with data property
   */
  function patch(url: string, data?: unknown): Promise<HttpResponse>;
  
  /**
   * HTTP DELETE request
   * @param url - URL to delete
   * @returns Response with data property
   */
  function del(url: string): Promise<HttpResponse>;
  
  /**
   * Download a file from URL to destination
   * @param url - URL to download from
   * @param destination - Local file path
   */
  function download(url: string, destination: string): Promise<void>;
  
  /**
   * Move files to trash
   * @param paths - File path(s) to trash
   */
  function trash(paths: string | string[]): Promise<void>;
  
  /**
   * Show the main window
   */
  function show(): Promise<void>;
  
  /**
   * Hide the main window
   */
  function hide(): Promise<void>;
  
  /**
   * Blur - return focus to previous app
   */
  function blur(): Promise<void>;
  
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
   * Returns path relative to ~/.kit
   * @param segments - Path segments to join
   * @returns Full path from kit directory
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
  // TIER 5B: Database/Store
  // =============================================================================
  
  /**
   * Simple JSON file database
   * @param initialData - Initial data if database doesn't exist
   * @returns Database instance with data and write method
   */
  function db(initialData?: unknown): Promise<DbInstance>;
  
  /**
   * Key-value store for persistent data
   */
  const store: StoreAPI;
  
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
}

globalThis.arg = async function arg(
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
      resolve(msg.value ?? '');
    });
    
    const message: ArgMessage = {
      type: 'arg',
      id,
      placeholder,
      choices: normalizedChoices,
    };
    
    send(message);
  });
};

globalThis.div = async function div(html: string, tailwind?: string): Promise<void> {
  const id = nextId();
  
  return new Promise((resolve) => {
    pending.set(id, () => {
      resolve();
    });
    
    const message: DivMessage = {
      type: 'div',
      id,
      html,
      tailwind,
    };
    
    send(message);
  });
};

globalThis.md = function md(markdown: string): string {
  let html = markdown;

  // Headings
  html = html.replace(/^### (.+)$/gm, '<h3>$1</h3>');
  html = html.replace(/^## (.+)$/gm, '<h2>$1</h2>');
  html = html.replace(/^# (.+)$/gm, '<h1>$1</h1>');

  // Bold
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');

  // Italic
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');

  // Lists
  html = html.replace(/^- (.+)$/gm, '<li>$1</li>');
  html = html.replace(/(<li>.*<\/li>\n?)+/g, '<ul>$&</ul>');

  return html;
};

globalThis.editor = async function editor(
  content: string = '',
  language: string = 'text'
): Promise<string> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      resolve(msg.value ?? '');
    });

    const message: EditorMessage = {
      type: 'editor',
      id,
      content,
      language,
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
  fieldDefs: (string | FieldDef)[]
): Promise<string[]> {
  const id = nextId();

  const normalizedFields: FieldDef[] = fieldDefs.map((f) => {
    if (typeof f === 'string') {
      return { name: f, label: f };
    }
    return f;
  });

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
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
    };

    send(message);
  });
};

globalThis.form = async function form(
  html: string
): Promise<Record<string, string>> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
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

globalThis.template = async function template(
  templateStr: string
): Promise<string> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      resolve(msg.value ?? '');
    });

    const message: TemplateMessage = {
      type: 'template',
      id,
      template: templateStr,
    };

    send(message);
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

// Extend the message handler to also handle widget events
const originalLine = rl.listeners('line')[0] as ((line: string) => void) | undefined;
if (originalLine) {
  rl.removeListener('line', originalLine);
}

rl.on('line', (line: string) => {
  try {
    const msg = JSON.parse(line);
    
    // Handle submit messages (existing functionality)
    if (msg.type === 'submit' && pending.has(msg.id)) {
      const resolver = pending.get(msg.id);
      if (resolver) {
        pending.delete(msg.id);
        resolver(msg);
      }
      return;
    }
    
    // Handle widget events
    if (msg.type === 'widgetEvent' && widgetHandlers.has(msg.id)) {
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
          case 'moved':
            handlers.onMoved?.(msg.data as { x: number; y: number });
            break;
          case 'resized':
            handlers.onResized?.(msg.data as { width: number; height: number });
            break;
        }
      }
    }
  } catch {
    // Silently ignore non-JSON lines
  }
});

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

globalThis.term = async function term(command?: string): Promise<string> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      resolve(msg.value ?? '');
    });

    const message: TermMessage = {
      type: 'term',
      id,
      command,
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
// TIER 5A: Utility Functions
// =============================================================================

// Shell Execution - sends command to GPUI for execution
globalThis.exec = async function exec(
  command: string,
  options?: ExecOptions
): Promise<ExecResult> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      const value = msg.value ?? '{}';
      try {
        const parsed = JSON.parse(value);
        resolve({
          stdout: parsed.stdout ?? '',
          stderr: parsed.stderr ?? '',
          all: options?.all ? parsed.all : undefined,
          exitCode: parsed.exitCode ?? 0,
        });
      } catch {
        resolve({
          stdout: '',
          stderr: 'Failed to parse exec result',
          exitCode: 1,
        });
      }
    });

    const message: ExecMessage = {
      type: 'exec',
      id,
      command,
      options,
    };

    send(message);
  });
};

// HTTP Methods - use fetch directly (Bun supports it natively)
globalThis.get = async function get(url: string): Promise<HttpResponse> {
  const response = await fetch(url);
  const data = await response.json();
  return { data };
};

globalThis.post = async function post(url: string, data?: unknown): Promise<HttpResponse> {
  const response = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: data ? JSON.stringify(data) : undefined,
  });
  const responseData = await response.json();
  return { data: responseData };
};

globalThis.put = async function put(url: string, data?: unknown): Promise<HttpResponse> {
  const response = await fetch(url, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: data ? JSON.stringify(data) : undefined,
  });
  const responseData = await response.json();
  return { data: responseData };
};

globalThis.patch = async function patch(url: string, data?: unknown): Promise<HttpResponse> {
  const response = await fetch(url, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: data ? JSON.stringify(data) : undefined,
  });
  const responseData = await response.json();
  return { data: responseData };
};

globalThis.del = async function del(url: string): Promise<HttpResponse> {
  const response = await fetch(url, {
    method: 'DELETE',
  });
  const data = await response.json();
  return { data };
};

// File Operations
globalThis.download = async function download(
  url: string,
  destination: string
): Promise<void> {
  const id = nextId();

  return new Promise((resolve) => {
    pending.set(id, () => {
      resolve();
    });

    const message: DownloadMessage = {
      type: 'download',
      id,
      url,
      destination,
    };

    send(message);
  });
};

globalThis.trash = async function trash(paths: string | string[]): Promise<void> {
  const id = nextId();
  const pathArray = Array.isArray(paths) ? paths : [paths];

  return new Promise((resolve) => {
    pending.set(id, () => {
      resolve();
    });

    const message: TrashMessage = {
      type: 'trash',
      id,
      paths: pathArray,
    };

    send(message);
  });
};

// Window Control (fire-and-forget)
globalThis.show = async function show(): Promise<void> {
  const message: ShowMessage = { type: 'show' };
  send(message);
};

globalThis.hide = async function hide(): Promise<void> {
  const message: HideMessage = { type: 'hide' };
  send(message);
};

globalThis.blur = async function blur(): Promise<void> {
  const message: BlurMessage = { type: 'blur' };
  send(message);
};

// Prompt Control
globalThis.submit = function submit(value: unknown): void {
  const message: ForceSubmitMessage = { type: 'forceSubmit', value };
  send(message);
};

globalThis.exit = function exit(code?: number): void {
  const message: ExitMessage = { type: 'exit', code };
  send(message);
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
  return nodePath.join(os.homedir(), '.kit', ...segments);
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
// TIER 5B: Database (JSON file storage)
// =============================================================================

// Get the script name from the call stack or default to 'default'
function getScriptName(): string {
  // Try to get the script name from process.argv
  const scriptPath = process.argv[1] || '';
  const basename = nodePath.basename(scriptPath, nodePath.extname(scriptPath));
  return basename || 'default';
}

globalThis.db = async function db(initialData?: unknown): Promise<DbInstance> {
  const scriptName = getScriptName();
  const id = nextId();
  
  return new Promise((resolve) => {
    pending.set(id, (msg: SubmitMessage) => {
      const value = msg.value ?? '{}';
      let parsedData: unknown;
      try {
        parsedData = JSON.parse(value);
      } catch {
        parsedData = initialData ?? {};
      }
      
      // Create the DbInstance
      const instance: DbInstance = {
        data: parsedData,
        items: Array.isArray(parsedData) ? parsedData : undefined,
        write: async () => {
          const writeId = nextId();
          return new Promise<void>((writeResolve) => {
            pending.set(writeId, () => {
              writeResolve();
            });
            
            const writeMessage: DbMessage = {
              type: 'db',
              id: writeId,
              scriptName,
              initialData: instance.data,
            };
            send(writeMessage);
          });
        },
      };
      
      resolve(instance);
    });
    
    const message: DbMessage = {
      type: 'db',
      id,
      scriptName,
      initialData,
    };
    
    send(message);
  });
};

// =============================================================================
// TIER 5B: Key-Value Store
// =============================================================================

globalThis.store = {
  async get(key: string): Promise<unknown> {
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
      
      const message: StoreMessage = {
        type: 'store',
        id,
        action: 'get',
        key,
      };
      send(message);
    });
  },
  
  async set(key: string, value: unknown): Promise<void> {
    const id = nextId();
    
    return new Promise((resolve) => {
      pending.set(id, () => {
        resolve();
      });
      
      const message: StoreMessage = {
        type: 'store',
        id,
        action: 'set',
        key,
        value,
      };
      send(message);
    });
  },
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
