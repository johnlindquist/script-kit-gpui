// Name: SDK Test - form() All Input Types
// Description: Proves the isolated form SDK request/response contract without GUI or screen capture.

import '../../scripts/kit-sdk.ts';

interface TestResult {
  test: string;
  status: 'running' | 'pass' | 'fail';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
}

function logTest(name: string, status: TestResult['status'], extra?: Partial<TestResult>): void {
  console.log(JSON.stringify({
    test: name,
    status,
    timestamp: new Date().toISOString(),
    ...extra,
  } satisfies TestResult));
}

const FORM_HTML_ALL_TYPES = `
<div class="p-4">
  <input type="text" name="textInput" placeholder="Text Input" />
  <input type="password" name="passwordInput" placeholder="Password" />
  <input type="email" name="emailInput" placeholder="Email" />
  <input type="number" name="numberInput" placeholder="Number" />
  <input type="date" name="dateInput" placeholder="Date" />
  <input type="time" name="timeInput" placeholder="Time" />
  <input type="datetime-local" name="dateTimeInput" placeholder="Date and Time" />
  <input type="month" name="monthInput" placeholder="Month" />
  <input type="week" name="weekInput" placeholder="Week" />
  <input type="url" name="urlInput" placeholder="URL" />
  <input type="search" name="searchInput" placeholder="Search" />
  <input type="tel" name="telInput" placeholder="Telephone" />
  <input type="color" name="colorInput" placeholder="Color" />
  <textarea name="textareaInput" placeholder="Textarea"></textarea>
</div>
`;

const EXPECTED_FIELD_NAMES = [
  'textInput',
  'passwordInput',
  'emailInput',
  'numberInput',
  'dateInput',
  'timeInput',
  'dateTimeInput',
  'monthInput',
  'weekInput',
  'urlInput',
  'searchInput',
  'telInput',
  'colorInput',
  'textareaInput',
] as const;

async function runTest(name: string, run: () => Promise<unknown> | unknown): Promise<void> {
  logTest(name, 'running');
  const started = Date.now();
  try {
    logTest(name, 'pass', { result: await run(), duration_ms: Date.now() - started });
  } catch (error) {
    logTest(name, 'fail', { error: String(error), duration_ms: Date.now() - started });
  }
}

await runTest('form-function-exists', () => {
  if (typeof form !== 'function') {
    throw new Error(`Expected form() to be a function, got ${typeof form}.`);
  }
  return { type: 'function' };
});

await runTest('form-expected-parsing', () => {
  for (const name of EXPECTED_FIELD_NAMES) {
    if (!FORM_HTML_ALL_TYPES.includes(`name="${name}"`)) {
      throw new Error(`The form fixture omitted expected field ${name}.`);
    }
  }
  return { fields: EXPECTED_FIELD_NAMES, nativeRenderingVerified: false };
});

await runTest('form-all-types-sdk-request-response', async () => {
  if (process.env.SDK_TEST_AUTOSUBMIT !== '1') {
    throw new Error('This headless protocol fixture requires SDK_TEST_AUTOSUBMIT=1.');
  }

  const originalWrite = process.stdout.write.bind(process.stdout);
  const messages: Array<Record<string, unknown>> = [];
  process.stdout.write = ((chunk: unknown, ...args: unknown[]) => {
    const text = typeof chunk === 'string' ? chunk : String(chunk);
    for (const line of text.split('\n').filter(Boolean)) {
      try {
        messages.push(JSON.parse(line) as Record<string, unknown>);
      } catch {
        // SDK benchmark lines go to stderr; non-JSON stdout is irrelevant.
      }
    }
    return originalWrite(chunk as string, ...(args as []));
  }) as typeof process.stdout.write;

  let response: Record<string, string>;
  try {
    response = await form(FORM_HTML_ALL_TYPES);
  } finally {
    process.stdout.write = originalWrite;
  }

  const request = messages.find((message) => message.type === 'form');
  if (!request || typeof request.id !== 'string' || typeof request.html !== 'string') {
    throw new Error(`Missing typed form protocol request: ${JSON.stringify(messages)}`);
  }
  for (const name of EXPECTED_FIELD_NAMES) {
    if (!request.html.includes(`name="${name}"`)) {
      throw new Error(`Form protocol omitted expected field ${name}.`);
    }
  }
  if (response === null || Array.isArray(response) || typeof response !== 'object') {
    throw new Error(`Form auto-submit must resolve to a field-value object: ${JSON.stringify(response)}`);
  }

  return {
    requestType: request.type,
    requestedFields: EXPECTED_FIELD_NAMES.length,
    responseType: 'record',
    nativeRenderingVerified: false,
    screenCapturePerformed: false,
  };
});
