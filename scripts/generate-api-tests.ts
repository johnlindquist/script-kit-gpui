#!/usr/bin/env bun
/**
 * API Test Generator
 * 
 * Analyzes gpui-*.ts demo scripts and generates corresponding test cases.
 * 
 * Usage:
 *   bun run scripts/generate-api-tests.ts
 *   bun run scripts/generate-api-tests.ts --dry-run
 *   bun run scripts/generate-api-tests.ts --verbose
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { Glob } from "bun";

// =============================================================================
// Types
// =============================================================================

interface APIInfo {
  name: string;
  tier: "core" | "forms" | "system" | "files" | "media" | "utility" | "storage";
  demoScript: string | null;
  testFile: string;
  autoSubmit: boolean;
  status: "tested" | "untested" | "hardware" | "fire-and-forget";
  parameters?: string[];
  usageExamples?: string[];
}

interface APIManifest {
  generated_at: string;
  total_apis: number;
  tested_apis: number;
  coverage_percent: number;
  apis: APIInfo[];
  tiers: {
    [key: string]: {
      total: number;
      tested: number;
      apis: string[];
    };
  };
}

interface ParsedDemo {
  filename: string;
  apis: string[];
  usagePatterns: Map<string, string[]>;
}

// =============================================================================
// Configuration
// =============================================================================

const KENV_SCRIPTS = path.join(process.env.HOME || "", ".kenv/scripts");
const OUTPUT_DIR = path.join(process.cwd(), "tests/autonomous");
const MANIFEST_FILE = path.join(OUTPUT_DIR, "api-manifest.json");

// API categorization based on AUTONOMOUS_APP_TESTING.md
const API_TIERS: Record<string, { tier: APIInfo["tier"]; autoSubmit: boolean; status: APIInfo["status"] }> = {
  // TIER 1: Core Prompts
  arg: { tier: "core", autoSubmit: true, status: "tested" },
  div: { tier: "core", autoSubmit: true, status: "tested" },
  editor: { tier: "core", autoSubmit: true, status: "untested" },
  mini: { tier: "core", autoSubmit: true, status: "untested" },
  micro: { tier: "core", autoSubmit: true, status: "untested" },
  select: { tier: "core", autoSubmit: true, status: "untested" },

  // TIER 2: Form Prompts
  fields: { tier: "forms", autoSubmit: true, status: "untested" },
  form: { tier: "forms", autoSubmit: true, status: "untested" },
  template: { tier: "forms", autoSubmit: true, status: "untested" },
  env: { tier: "forms", autoSubmit: true, status: "untested" },

  // TIER 3: System APIs
  beep: { tier: "system", autoSubmit: false, status: "fire-and-forget" },
  say: { tier: "system", autoSubmit: false, status: "fire-and-forget" },
  notify: { tier: "system", autoSubmit: false, status: "fire-and-forget" },
  setStatus: { tier: "system", autoSubmit: false, status: "fire-and-forget" },
  menu: { tier: "system", autoSubmit: false, status: "fire-and-forget" },
  copy: { tier: "system", autoSubmit: false, status: "fire-and-forget" },
  paste: { tier: "system", autoSubmit: true, status: "untested" },
  clipboard: { tier: "system", autoSubmit: true, status: "untested" },
  keyboard: { tier: "system", autoSubmit: false, status: "fire-and-forget" },
  mouse: { tier: "system", autoSubmit: false, status: "fire-and-forget" },
  getSelectedText: { tier: "system", autoSubmit: true, status: "untested" },
  setSelectedText: { tier: "system", autoSubmit: false, status: "fire-and-forget" },

  // TIER 4A: Input Capture
  hotkey: { tier: "files", autoSubmit: true, status: "untested" },
  drop: { tier: "files", autoSubmit: true, status: "untested" },
  path: { tier: "files", autoSubmit: true, status: "untested" },

  // TIER 4B: Media Prompts
  chat: { tier: "media", autoSubmit: true, status: "untested" },
  term: { tier: "media", autoSubmit: true, status: "tested" },
  widget: { tier: "media", autoSubmit: true, status: "untested" },
  webcam: { tier: "media", autoSubmit: false, status: "hardware" },
  mic: { tier: "media", autoSubmit: false, status: "hardware" },
  eyeDropper: { tier: "media", autoSubmit: true, status: "untested" },
  find: { tier: "media", autoSubmit: true, status: "untested" },

  // TIER 5A: Utility Functions
  exec: { tier: "utility", autoSubmit: true, status: "untested" },
  get: { tier: "utility", autoSubmit: true, status: "untested" },
  post: { tier: "utility", autoSubmit: true, status: "untested" },
  put: { tier: "utility", autoSubmit: true, status: "untested" },
  patch: { tier: "utility", autoSubmit: true, status: "untested" },
  del: { tier: "utility", autoSubmit: true, status: "untested" },
  download: { tier: "utility", autoSubmit: true, status: "untested" },
  trash: { tier: "utility", autoSubmit: true, status: "untested" },
  show: { tier: "utility", autoSubmit: false, status: "fire-and-forget" },
  hide: { tier: "utility", autoSubmit: false, status: "fire-and-forget" },
  blur: { tier: "utility", autoSubmit: false, status: "fire-and-forget" },
  submit: { tier: "utility", autoSubmit: false, status: "fire-and-forget" },
  exit: { tier: "utility", autoSubmit: false, status: "fire-and-forget" },
  wait: { tier: "utility", autoSubmit: false, status: "fire-and-forget" },

  // TIER 5B: Storage & Path APIs
  uuid: { tier: "storage", autoSubmit: false, status: "untested" },
  compile: { tier: "storage", autoSubmit: false, status: "untested" },
  home: { tier: "storage", autoSubmit: false, status: "untested" },
  kenvPath: { tier: "storage", autoSubmit: false, status: "untested" },
  kitPath: { tier: "storage", autoSubmit: false, status: "untested" },
  tmpPath: { tier: "storage", autoSubmit: false, status: "untested" },
  isFile: { tier: "storage", autoSubmit: false, status: "untested" },
  isDir: { tier: "storage", autoSubmit: false, status: "untested" },
  isBin: { tier: "storage", autoSubmit: false, status: "untested" },
  db: { tier: "storage", autoSubmit: true, status: "untested" },
  store: { tier: "storage", autoSubmit: true, status: "untested" },
  memoryMap: { tier: "storage", autoSubmit: false, status: "untested" },
  browse: { tier: "storage", autoSubmit: false, status: "fire-and-forget" },
  editFile: { tier: "storage", autoSubmit: false, status: "fire-and-forget" },
  run: { tier: "storage", autoSubmit: true, status: "untested" },
  inspect: { tier: "storage", autoSubmit: false, status: "fire-and-forget" },
  md: { tier: "utility", autoSubmit: false, status: "untested" },
};

const TIER_TO_TEST_FILE: Record<APIInfo["tier"], string> = {
  core: "test-core-prompts.ts",
  forms: "test-form-inputs.ts",
  system: "test-system-apis.ts",
  files: "test-file-apis.ts",
  media: "test-media-apis.ts",
  utility: "test-utility-apis.ts",
  storage: "test-storage-apis.ts",
};

// =============================================================================
// Parser
// =============================================================================

function parseDemo(filepath: string): ParsedDemo {
  const content = fs.readFileSync(filepath, "utf-8");
  const filename = path.basename(filepath);
  const apis: string[] = [];
  const usagePatterns = new Map<string, string[]>();

  // Extract API calls from the demo script
  const apiNames = Object.keys(API_TIERS);

  for (const api of apiNames) {
    // Match function calls like: await api(...) or api(...)
    // Also match property access like: store.set, keyboard.type
    const patterns = [
      new RegExp(`await\\s+${api}\\s*\\(`, "g"),
      new RegExp(`(?<!\\.)\\b${api}\\s*\\(`, "g"),
      new RegExp(`${api}\\.\\w+\\s*\\(`, "g"),
    ];

    for (const pattern of patterns) {
      const matches = content.match(pattern);
      if (matches && matches.length > 0) {
        if (!apis.includes(api)) {
          apis.push(api);
        }

        // Extract the full line for usage examples
        const lines = content.split("\n");
        for (const line of lines) {
          if (line.match(pattern) && !line.trim().startsWith("//")) {
            const existing = usagePatterns.get(api) || [];
            existing.push(line.trim());
            usagePatterns.set(api, existing);
          }
        }
      }
    }
  }

  return { filename, apis, usagePatterns };
}

function extractDemoScriptName(api: string): string | null {
  // Map API to demo script name
  const mappings: Record<string, string> = {
    arg: "gpui-arg.ts",
    div: "gpui-div.ts",
    editor: "gpui-editor.ts",
    mini: "gpui-mini.ts",
    micro: "gpui-micro.ts",
    select: "gpui-select.ts",
    fields: "gpui-fields.ts",
    form: "gpui-form.ts",
    template: "gpui-template.ts",
    env: "gpui-env.ts",
    beep: "gpui-beep.ts",
    say: "gpui-say.ts",
    notify: "gpui-notify.ts",
    setStatus: "gpui-set-status.ts",
    menu: "gpui-menu.ts",
    copy: "gpui-clipboard.ts",
    paste: "gpui-clipboard.ts",
    clipboard: "gpui-clipboard.ts",
    keyboard: "gpui-keyboard.ts",
    mouse: "gpui-mouse.ts",
    getSelectedText: "gpui-get-selected-text.ts",
    setSelectedText: "gpui-selected-text.ts",
    hotkey: "gpui-hotkey.ts",
    drop: "gpui-drop.ts",
    path: "gpui-path.ts",
    chat: "gpui-chat.ts",
    term: "gpui-term.ts",
    widget: "gpui-widget.ts",
    webcam: "gpui-webcam.ts",
    mic: "gpui-mic.ts",
    eyeDropper: "gpui-eye-dropper.ts",
    find: "gpui-find.ts",
    exec: "gpui-exec.ts",
    get: "gpui-http.ts",
    post: "gpui-http.ts",
    put: "gpui-http.ts",
    patch: "gpui-http.ts",
    del: "gpui-http.ts",
    download: "gpui-download.ts",
    trash: "gpui-trash.ts",
    show: "gpui-window-control.ts",
    hide: "gpui-window-control.ts",
    blur: "gpui-window-control.ts",
    submit: "gpui-submit-exit.ts",
    exit: "gpui-submit-exit.ts",
    wait: "gpui-wait.ts",
    uuid: "gpui-uuid.ts",
    compile: "gpui-compile.ts",
    home: "gpui-paths.ts",
    kenvPath: "gpui-paths.ts",
    kitPath: "gpui-paths.ts",
    tmpPath: "gpui-paths.ts",
    isFile: "gpui-file-checks.ts",
    isDir: "gpui-file-checks.ts",
    isBin: "gpui-file-checks.ts",
    db: "gpui-db.ts",
    store: "gpui-store.ts",
    memoryMap: "gpui-memory-map.ts",
    browse: "gpui-browse.ts",
    editFile: "gpui-edit.ts",
    run: "gpui-run.ts",
    inspect: "gpui-inspect.ts",
    md: "gpui-div.ts",
  };

  return mappings[api] || null;
}

// =============================================================================
// Test Generator
// =============================================================================

function generateTestHeader(): string {
  return `// Auto-generated by scripts/generate-api-tests.ts
// Do not edit manually - regenerate with: bun run scripts/generate-api-tests.ts

import '../../scripts/kit-sdk';

// =============================================================================
// Test Infrastructure
// =============================================================================

interface TestResult {
  test: string;
  status: 'running' | 'pass' | 'fail' | 'skip';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
}

function logTest(name: string, status: TestResult['status'], extra?: Partial<TestResult>) {
  const result: TestResult = {
    test: name,
    status,
    timestamp: new Date().toISOString(),
    ...extra
  };
  console.log(JSON.stringify(result));
}

function debug(msg: string) {
  console.error(\`[TEST] \${msg}\`);
}

async function runTest(name: string, fn: () => Promise<void>) {
  logTest(name, 'running');
  const start = Date.now();
  try {
    await fn();
    logTest(name, 'pass', { duration_ms: Date.now() - start });
  } catch (err) {
    logTest(name, 'fail', { error: String(err), duration_ms: Date.now() - start });
  }
}

function skipTest(name: string, reason: string) {
  logTest(name, 'skip', { error: reason });
}

// =============================================================================
// Tests
// =============================================================================

`;
}

function generateCorePromptsTests(apis: Map<string, string[]>): string {
  let content = generateTestHeader();
  content += `debug('test-core-prompts.ts starting...');\n\n`;

  // arg tests
  content += `// -----------------------------------------------------------------------------
// arg() tests
// -----------------------------------------------------------------------------

await runTest('arg-string-choices', async () => {
  const result = await arg('Select a fruit', ['Apple', 'Banana', 'Cherry']);
  if (typeof result !== 'string') {
    throw new Error(\`Expected string, got \${typeof result}\`);
  }
  debug(\`arg result: \${result}\`);
});

await runTest('arg-structured-choices', async () => {
  const result = await arg('Select action', [
    { name: 'Run', value: 'run', description: 'Execute script' },
    { name: 'Edit', value: 'edit', description: 'Open editor' },
  ]);
  if (typeof result !== 'string') {
    throw new Error(\`Expected string value, got \${typeof result}\`);
  }
  debug(\`arg structured result: \${result}\`);
});

`;

  // div tests
  content += `// -----------------------------------------------------------------------------
// div() tests
// -----------------------------------------------------------------------------

await runTest('div-html-content', async () => {
  await div('<h1>Test Content</h1><p>This is a test paragraph.</p>');
  // Success = no error thrown
});

await runTest('div-with-markdown', async () => {
  await div(md(\`# Markdown Test
  
  - Item 1
  - Item 2
  
  **Bold** and *italic* text.\`));
});

`;

  // editor tests
  content += `// -----------------------------------------------------------------------------
// editor() tests
// -----------------------------------------------------------------------------

await runTest('editor-empty', async () => {
  const result = await editor();
  if (typeof result !== 'string') {
    throw new Error(\`Expected string, got \${typeof result}\`);
  }
});

await runTest('editor-with-content', async () => {
  const initial = 'console.log("hello")';
  const result = await editor(initial, 'javascript');
  if (typeof result !== 'string') {
    throw new Error(\`Expected string, got \${typeof result}\`);
  }
});

`;

  // mini tests
  content += `// -----------------------------------------------------------------------------
// mini() tests
// -----------------------------------------------------------------------------

await runTest('mini-basic', async () => {
  const result = await mini('Quick input', ['Option A', 'Option B']);
  if (typeof result !== 'string') {
    throw new Error(\`Expected string, got \${typeof result}\`);
  }
});

`;

  // micro tests
  content += `// -----------------------------------------------------------------------------
// micro() tests
// -----------------------------------------------------------------------------

await runTest('micro-basic', async () => {
  const result = await micro('Tiny prompt', ['X', 'Y', 'Z']);
  if (typeof result !== 'string') {
    throw new Error(\`Expected string, got \${typeof result}\`);
  }
});

`;

  // select tests
  content += `// -----------------------------------------------------------------------------
// select() tests
// -----------------------------------------------------------------------------

await runTest('select-multi', async () => {
  const result = await select('Select multiple', ['One', 'Two', 'Three']);
  if (!Array.isArray(result)) {
    throw new Error(\`Expected array, got \${typeof result}\`);
  }
});

`;

  content += `debug('test-core-prompts.ts completed!');\n`;
  return content;
}

function generateFormInputsTests(): string {
  let content = generateTestHeader();
  content += `debug('test-form-inputs.ts starting...');\n\n`;

  // fields tests
  content += `// -----------------------------------------------------------------------------
// fields() tests
// -----------------------------------------------------------------------------

await runTest('fields-string-array', async () => {
  const [firstName, lastName] = await fields(['First Name', 'Last Name']);
  if (typeof firstName !== 'string' || typeof lastName !== 'string') {
    throw new Error('Expected string values from fields()');
  }
  debug(\`fields result: [\${firstName}, \${lastName}]\`);
});

await runTest('fields-structured', async () => {
  const [email, age] = await fields([
    { name: 'email', label: 'Email', type: 'email', placeholder: 'you@example.com' },
    { name: 'age', label: 'Age', type: 'number', placeholder: '25' },
  ]);
  if (typeof email !== 'string' || typeof age !== 'string') {
    throw new Error('Expected string values from fields()');
  }
});

`;

  // form tests
  content += `// -----------------------------------------------------------------------------
// form() tests
// -----------------------------------------------------------------------------

await runTest('form-html', async () => {
  const result = await form(\`
    <input name="username" placeholder="Username" />
    <input name="password" type="password" placeholder="Password" />
  \`);
  if (typeof result !== 'object') {
    throw new Error(\`Expected object, got \${typeof result}\`);
  }
});

`;

  // template tests
  content += `// -----------------------------------------------------------------------------
// template() tests
// -----------------------------------------------------------------------------

await runTest('template-basic', async () => {
  const result = await template('Hello $1, welcome to $2!');
  if (typeof result !== 'string') {
    throw new Error(\`Expected string, got \${typeof result}\`);
  }
});

`;

  // env tests
  content += `// -----------------------------------------------------------------------------
// env() tests
// -----------------------------------------------------------------------------

await runTest('env-basic', async () => {
  const result = await env('TEST_API_KEY');
  if (typeof result !== 'string') {
    throw new Error(\`Expected string, got \${typeof result}\`);
  }
});

`;

  content += `debug('test-form-inputs.ts completed!');\n`;
  return content;
}

function generateSystemApisTests(): string {
  let content = generateTestHeader();
  content += `debug('test-system-apis.ts starting...');\n\n`;

  // beep tests
  content += `// -----------------------------------------------------------------------------
// beep() tests (fire-and-forget)
// -----------------------------------------------------------------------------

await runTest('beep-basic', async () => {
  await beep();
  // Success = no error thrown
});

`;

  // say tests
  content += `// -----------------------------------------------------------------------------
// say() tests (fire-and-forget)
// -----------------------------------------------------------------------------

await runTest('say-basic', async () => {
  await say('Test message');
  // Success = no error thrown
});

`;

  // notify tests
  content += `// -----------------------------------------------------------------------------
// notify() tests (fire-and-forget)
// -----------------------------------------------------------------------------

await runTest('notify-string', async () => {
  await notify('Simple notification');
  // Success = no error thrown
});

await runTest('notify-object', async () => {
  await notify({
    title: 'Test Title',
    body: 'Test notification body',
  });
});

`;

  // setStatus tests
  content += `// -----------------------------------------------------------------------------
// setStatus() tests (fire-and-forget)
// -----------------------------------------------------------------------------

await runTest('setStatus-basic', async () => {
  await setStatus({ status: 'busy', message: 'Testing...' });
  // Success = no error thrown
});

`;

  // clipboard tests
  content += `// -----------------------------------------------------------------------------
// clipboard tests
// -----------------------------------------------------------------------------

await runTest('copy-paste-roundtrip', async () => {
  const testValue = 'clipboard-test-' + Date.now();
  await copy(testValue);
  const pasted = await paste();
  if (pasted !== testValue) {
    throw new Error(\`Clipboard roundtrip failed: expected "\${testValue}", got "\${pasted}"\`);
  }
});

`;

  // keyboard tests
  content += `// -----------------------------------------------------------------------------
// keyboard tests (fire-and-forget)
// -----------------------------------------------------------------------------

skipTest('keyboard-type', 'Requires window focus - skip in autonomous mode');
skipTest('keyboard-tap', 'Requires window focus - skip in autonomous mode');

`;

  // mouse tests
  content += `// -----------------------------------------------------------------------------
// mouse tests (fire-and-forget)
// -----------------------------------------------------------------------------

skipTest('mouse-setPosition', 'Requires window focus - skip in autonomous mode');
skipTest('mouse-click', 'Requires window focus - skip in autonomous mode');

`;

  content += `debug('test-system-apis.ts completed!');\n`;
  return content;
}

function generateFileApisTests(): string {
  let content = generateTestHeader();
  content += `debug('test-file-apis.ts starting...');\n\n`;

  // hotkey tests
  content += `// -----------------------------------------------------------------------------
// hotkey() tests
// -----------------------------------------------------------------------------

await runTest('hotkey-capture', async () => {
  const result = await hotkey('Press a key');
  if (!result || typeof result.key !== 'string') {
    throw new Error('Expected hotkey info with key property');
  }
  debug(\`hotkey captured: \${result.key}\`);
});

`;

  // drop tests
  content += `// -----------------------------------------------------------------------------
// drop() tests
// -----------------------------------------------------------------------------

await runTest('drop-basic', async () => {
  const result = await drop();
  if (!Array.isArray(result)) {
    throw new Error(\`Expected array, got \${typeof result}\`);
  }
});

`;

  // path tests
  content += `// -----------------------------------------------------------------------------
// path() tests
// -----------------------------------------------------------------------------

await runTest('path-basic', async () => {
  const result = await path({ startPath: '/tmp', hint: 'Select a file' });
  if (typeof result !== 'string') {
    throw new Error(\`Expected string path, got \${typeof result}\`);
  }
});

`;

  content += `debug('test-file-apis.ts completed!');\n`;
  return content;
}

function generateMediaApisTests(): string {
  let content = generateTestHeader();
  content += `debug('test-media-apis.ts starting...');\n\n`;

  // term tests
  content += `// -----------------------------------------------------------------------------
// term() tests
// -----------------------------------------------------------------------------

await runTest('term-with-command', async () => {
  const result = await term('echo "hello from term"');
  if (typeof result !== 'string') {
    throw new Error(\`Expected string output, got \${typeof result}\`);
  }
});

`;

  // chat tests
  content += `// -----------------------------------------------------------------------------
// chat() tests
// -----------------------------------------------------------------------------

await runTest('chat-basic', async () => {
  const result = await chat();
  if (typeof result !== 'string') {
    throw new Error(\`Expected string, got \${typeof result}\`);
  }
});

`;

  // widget tests
  content += `// -----------------------------------------------------------------------------
// widget() tests
// -----------------------------------------------------------------------------

await runTest('widget-basic', async () => {
  const w = await widget('<div class="p-4">Widget Test</div>', {
    width: 200,
    height: 100,
  });
  if (!w || typeof w.close !== 'function') {
    throw new Error('Expected widget controller with close method');
  }
  w.close();
});

`;

  // eyeDropper tests
  content += `// -----------------------------------------------------------------------------
// eyeDropper() tests
// -----------------------------------------------------------------------------

await runTest('eyeDropper-basic', async () => {
  const result = await eyeDropper();
  if (!result || typeof result.sRGBHex !== 'string') {
    throw new Error('Expected color info with sRGBHex property');
  }
});

`;

  // find tests
  content += `// -----------------------------------------------------------------------------
// find() tests
// -----------------------------------------------------------------------------

await runTest('find-basic', async () => {
  const result = await find('Search for files');
  if (typeof result !== 'string') {
    throw new Error(\`Expected string path, got \${typeof result}\`);
  }
});

`;

  // Hardware-dependent tests
  content += `// -----------------------------------------------------------------------------
// Hardware-dependent tests (skipped)
// -----------------------------------------------------------------------------

skipTest('webcam-capture', 'Requires camera hardware');
skipTest('mic-capture', 'Requires microphone hardware');

`;

  content += `debug('test-media-apis.ts completed!');\n`;
  return content;
}

function generateUtilityApisTests(): string {
  let content = generateTestHeader();
  content += `debug('test-utility-apis.ts starting...');\n\n`;

  // exec tests
  content += `// -----------------------------------------------------------------------------
// exec() tests
// -----------------------------------------------------------------------------

await runTest('exec-basic', async () => {
  const result = await exec('echo "hello"');
  if (!result || typeof result.stdout !== 'string') {
    throw new Error('Expected exec result with stdout');
  }
  debug(\`exec stdout: \${result.stdout.trim()}\`);
});

`;

  // HTTP tests
  content += `// -----------------------------------------------------------------------------
// HTTP tests (get, post, put, patch, del)
// -----------------------------------------------------------------------------

await runTest('get-request', async () => {
  const result = await get('https://jsonplaceholder.typicode.com/todos/1');
  if (!result || !result.data) {
    throw new Error('Expected response with data');
  }
  debug(\`get result: \${JSON.stringify(result.data).slice(0, 100)}\`);
});

await runTest('post-request', async () => {
  const result = await post('https://jsonplaceholder.typicode.com/posts', {
    title: 'Test',
    body: 'Test body',
    userId: 1,
  });
  if (!result || !result.data) {
    throw new Error('Expected response with data');
  }
});

await runTest('put-request', async () => {
  const result = await put('https://jsonplaceholder.typicode.com/posts/1', {
    id: 1,
    title: 'Updated',
    body: 'Updated body',
    userId: 1,
  });
  if (!result || !result.data) {
    throw new Error('Expected response with data');
  }
});

await runTest('patch-request', async () => {
  const result = await patch('https://jsonplaceholder.typicode.com/posts/1', {
    title: 'Patched',
  });
  if (!result || !result.data) {
    throw new Error('Expected response with data');
  }
});

await runTest('del-request', async () => {
  const result = await del('https://jsonplaceholder.typicode.com/posts/1');
  // DELETE typically returns empty or the deleted resource
  if (typeof result !== 'object') {
    throw new Error('Expected response object');
  }
});

`;

  // wait tests
  content += `// -----------------------------------------------------------------------------
// wait() tests
// -----------------------------------------------------------------------------

await runTest('wait-basic', async () => {
  const start = Date.now();
  await wait(100);
  const elapsed = Date.now() - start;
  if (elapsed < 90) {
    throw new Error(\`wait() returned too quickly: \${elapsed}ms\`);
  }
});

`;

  // Fire-and-forget tests
  content += `// -----------------------------------------------------------------------------
// Window control tests (fire-and-forget)
// -----------------------------------------------------------------------------

skipTest('show-window', 'Fire-and-forget - cannot verify');
skipTest('hide-window', 'Fire-and-forget - cannot verify');
skipTest('blur-window', 'Fire-and-forget - cannot verify');

`;

  content += `debug('test-utility-apis.ts completed!');\n`;
  return content;
}

function generateStorageApisTests(): string {
  let content = generateTestHeader();
  content += `debug('test-storage-apis.ts starting...');\n\n`;

  // Path utilities tests
  content += `// -----------------------------------------------------------------------------
// Path utilities tests (pure functions)
// -----------------------------------------------------------------------------

await runTest('home-path', async () => {
  const result = home();
  if (typeof result !== 'string' || !result.includes('/')) {
    throw new Error(\`Expected valid home path, got: \${result}\`);
  }
  debug(\`home(): \${result}\`);
});

await runTest('home-with-subpath', async () => {
  const result = home('Documents');
  if (!result.includes('Documents')) {
    throw new Error(\`Expected path with Documents, got: \${result}\`);
  }
});

await runTest('kenvPath-basic', async () => {
  const result = kenvPath();
  if (!result.includes('.kenv')) {
    throw new Error(\`Expected .kenv in path, got: \${result}\`);
  }
});

await runTest('kitPath-basic', async () => {
  const result = kitPath();
  if (!result.includes('.kit')) {
    throw new Error(\`Expected .kit in path, got: \${result}\`);
  }
});

await runTest('tmpPath-basic', async () => {
  const result = tmpPath();
  if (typeof result !== 'string') {
    throw new Error(\`Expected string path, got: \${typeof result}\`);
  }
});

`;

  // File check tests
  content += `// -----------------------------------------------------------------------------
// File check tests
// -----------------------------------------------------------------------------

await runTest('isFile-true', async () => {
  const result = await isFile('/etc/hosts');
  if (result !== true) {
    throw new Error(\`Expected true for /etc/hosts, got: \${result}\`);
  }
});

await runTest('isFile-false', async () => {
  const result = await isFile('/nonexistent-file-12345');
  if (result !== false) {
    throw new Error(\`Expected false for nonexistent file, got: \${result}\`);
  }
});

await runTest('isDir-true', async () => {
  const result = await isDir('/tmp');
  if (result !== true) {
    throw new Error(\`Expected true for /tmp, got: \${result}\`);
  }
});

await runTest('isDir-false', async () => {
  const result = await isDir('/nonexistent-dir-12345');
  if (result !== false) {
    throw new Error(\`Expected false for nonexistent dir, got: \${result}\`);
  }
});

`;

  // uuid tests
  content += `// -----------------------------------------------------------------------------
// uuid() tests
// -----------------------------------------------------------------------------

await runTest('uuid-basic', async () => {
  const result = uuid();
  if (typeof result !== 'string' || result.length < 32) {
    throw new Error(\`Expected UUID string, got: \${result}\`);
  }
  // Basic UUID format check
  if (!/^[0-9a-f-]{36}$/i.test(result)) {
    throw new Error(\`Invalid UUID format: \${result}\`);
  }
});

`;

  // store tests
  content += `// -----------------------------------------------------------------------------
// store tests
// -----------------------------------------------------------------------------

await runTest('store-set-get', async () => {
  const testKey = 'test-key-' + Date.now();
  const testValue = 'test-value-' + Date.now();
  
  await store.set(testKey, testValue);
  const result = await store.get(testKey);
  
  if (result !== testValue) {
    throw new Error(\`Store roundtrip failed: expected "\${testValue}", got "\${result}"\`);
  }
});

`;

  // db tests
  content += `// -----------------------------------------------------------------------------
// db() tests
// -----------------------------------------------------------------------------

await runTest('db-basic', async () => {
  const database = await db({ items: [] });
  if (!database || database.data === undefined) {
    throw new Error('Expected database object with data property');
  }
});

`;

  content += `debug('test-storage-apis.ts completed!');\n`;
  return content;
}

// =============================================================================
// Manifest Generator
// =============================================================================

function generateManifest(parsedDemos: ParsedDemo[]): APIManifest {
  const apis: APIInfo[] = [];
  const tiers: APIManifest["tiers"] = {};

  // Build API info from known APIs
  for (const [apiName, tierInfo] of Object.entries(API_TIERS)) {
    const demoScript = extractDemoScriptName(apiName);
    const testFile = TIER_TO_TEST_FILE[tierInfo.tier];

    const apiInfo: APIInfo = {
      name: apiName,
      tier: tierInfo.tier,
      demoScript,
      testFile,
      autoSubmit: tierInfo.autoSubmit,
      status: tierInfo.status,
    };

    // Extract usage examples from parsed demos
    for (const demo of parsedDemos) {
      const usages = demo.usagePatterns.get(apiName);
      if (usages && usages.length > 0) {
        apiInfo.usageExamples = usages.slice(0, 3); // Limit to 3 examples
      }
    }

    apis.push(apiInfo);

    // Build tier summary
    if (!tiers[tierInfo.tier]) {
      tiers[tierInfo.tier] = { total: 0, tested: 0, apis: [] };
    }
    tiers[tierInfo.tier].total++;
    tiers[tierInfo.tier].apis.push(apiName);
    if (tierInfo.status === "tested") {
      tiers[tierInfo.tier].tested++;
    }
  }

  const totalApis = apis.length;
  const testedApis = apis.filter((a) => a.status === "tested").length;

  return {
    generated_at: new Date().toISOString(),
    total_apis: totalApis,
    tested_apis: testedApis,
    coverage_percent: Math.round((testedApis / totalApis) * 100),
    apis: apis.sort((a, b) => a.name.localeCompare(b.name)),
    tiers,
  };
}

// =============================================================================
// Main
// =============================================================================

async function main() {
  const args = process.argv.slice(2);
  const dryRun = args.includes("--dry-run");
  const verbose = args.includes("--verbose");

  console.log("╔═══════════════════════════════════════════════════════════════╗");
  console.log("║            API TEST GENERATOR                                  ║");
  console.log("╚═══════════════════════════════════════════════════════════════╝");
  console.log("");

  // Discover demo scripts
  console.log(`📁 Scanning ${KENV_SCRIPTS}/gpui-*.ts...`);
  const glob = new Glob(`${KENV_SCRIPTS}/gpui-*.ts`);
  const demoFiles: string[] = [];
  for await (const file of glob.scan(".")) {
    demoFiles.push(file);
  }
  console.log(`   Found ${demoFiles.length} demo scripts`);

  // Parse demos
  console.log("\n📖 Parsing demo scripts...");
  const parsedDemos: ParsedDemo[] = [];
  for (const file of demoFiles) {
    const parsed = parseDemo(file);
    parsedDemos.push(parsed);
    if (verbose) {
      console.log(`   ${parsed.filename}: ${parsed.apis.join(", ") || "(no APIs)"}`);
    }
  }

  // Ensure output directory exists
  if (!dryRun) {
    if (!fs.existsSync(OUTPUT_DIR)) {
      fs.mkdirSync(OUTPUT_DIR, { recursive: true });
      console.log(`\n📁 Created ${OUTPUT_DIR}`);
    }
  }

  // Generate test files
  console.log("\n🔧 Generating test files...");

  const testFiles = [
    { name: "test-core-prompts.ts", generator: generateCorePromptsTests },
    { name: "test-form-inputs.ts", generator: generateFormInputsTests },
    { name: "test-system-apis.ts", generator: generateSystemApisTests },
    { name: "test-file-apis.ts", generator: generateFileApisTests },
    { name: "test-media-apis.ts", generator: generateMediaApisTests },
    { name: "test-utility-apis.ts", generator: generateUtilityApisTests },
    { name: "test-storage-apis.ts", generator: generateStorageApisTests },
  ];

  const apiUsages = new Map<string, string[]>();
  for (const demo of parsedDemos) {
    for (const [api, usages] of demo.usagePatterns) {
      const existing = apiUsages.get(api) || [];
      apiUsages.set(api, [...existing, ...usages]);
    }
  }

  for (const { name, generator } of testFiles) {
    const content = generator(apiUsages);
    const filepath = path.join(OUTPUT_DIR, name);

    if (dryRun) {
      console.log(`   [DRY RUN] Would write ${name} (${content.length} bytes)`);
    } else {
      fs.writeFileSync(filepath, content);
      console.log(`   ✅ ${name}`);
    }
  }

  // Generate manifest
  console.log("\n📋 Generating API manifest...");
  const manifest = generateManifest(parsedDemos);

  if (dryRun) {
    console.log(`   [DRY RUN] Would write api-manifest.json`);
    if (verbose) {
      console.log(JSON.stringify(manifest, null, 2));
    }
  } else {
    fs.writeFileSync(MANIFEST_FILE, JSON.stringify(manifest, null, 2));
    console.log(`   ✅ api-manifest.json`);
  }

  // Summary
  console.log("\n════════════════════════════════════════════════════════════════");
  console.log("                         SUMMARY");
  console.log("════════════════════════════════════════════════════════════════");
  console.log(`  Demo scripts analyzed: ${demoFiles.length}`);
  console.log(`  Total APIs tracked: ${manifest.total_apis}`);
  console.log(`  APIs with tests: ${manifest.tested_apis}`);
  console.log(`  Coverage: ${manifest.coverage_percent}%`);
  console.log("");
  console.log("  Tier breakdown:");
  for (const [tier, info] of Object.entries(manifest.tiers)) {
    console.log(`    ${tier.padEnd(10)} ${info.tested}/${info.total} tested`);
  }
  console.log("");
  console.log(`  Output: ${OUTPUT_DIR}`);
  console.log("════════════════════════════════════════════════════════════════");

  if (!dryRun) {
    console.log("\n✨ Done! Run tests with:");
    console.log("   bun run tests/autonomous/test-core-prompts.ts");
    console.log("   # or via the GPUI app:");
    console.log("   AUTO_SUBMIT=true ./target/debug/script-kit-gpui tests/autonomous/test-core-prompts.ts");
  }
}

main().catch((err) => {
  console.error("❌ Error:", err);
  process.exit(1);
});
