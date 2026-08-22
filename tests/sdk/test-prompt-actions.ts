// Name: SDK Test - Prompt Action Type and Protocol Parity
// Description: Verifies fields/form/term prompt-scoped actions through isolated auto-submit only.

import type { Action } from '../../scripts/kit-sdk';

if (process.env.SDK_TEST_AUTOSUBMIT !== '1') {
  throw new Error('Prompt action protocol tests require isolated SDK_TEST_AUTOSUBMIT=1.');
}

type Outcome = 'running' | 'pass' | 'fail';

function report(test: string, status: Outcome, details: Record<string, unknown> = {}): void {
  console.log(JSON.stringify({ test, status, timestamp: new Date().toISOString(), ...details }));
}

async function captureSdkRequest<T>(operation: () => Promise<T>): Promise<{
  result: T;
  message: Record<string, unknown>;
}> {
  const originalWrite = process.stdout.write.bind(process.stdout);
  const messages: Array<Record<string, unknown>> = [];
  process.stdout.write = ((chunk: unknown, ...args: unknown[]) => {
    for (const line of String(chunk).split('\n').filter(Boolean)) {
      try {
        messages.push(JSON.parse(line) as Record<string, unknown>);
      } catch {
        // Only SDK JSONL protocol messages participate in this assertion.
      }
    }
    return originalWrite(chunk as string, ...(args as []));
  }) as typeof process.stdout.write;

  try {
    const result = await operation();
    const message = messages.find((candidate) => (
      candidate.type === 'fields' || candidate.type === 'form' || candidate.type === 'term'
    ));
    if (!message) {
      throw new Error(`No typed prompt request was emitted: ${JSON.stringify(messages)}`);
    }
    return { result, message };
  } finally {
    process.stdout.write = originalWrite;
  }
}

const actions: Action[] = [
  { name: 'Run', value: 'run', shortcut: 'cmd+r' },
  { name: 'Run', value: 'duplicate' },
  { name: 'Hidden', value: 'hidden', visible: false },
  { name: 'Without action' },
  { name: 'Callback', onAction: () => undefined },
];

for (const [name, expectedType, operation] of [
  [
    'fields-prompt-actions-typed-and-deduplicated',
    'fields',
    () => fields([{ name: 'name', label: 'Name', value: 'Ada' }], actions),
  ],
  [
    'form-prompt-actions-typed-and-deduplicated',
    'form',
    () => form('<input name="name" />', actions),
  ],
  [
    'terminal-prompt-actions-typed-and-deduplicated',
    'term',
    () => term(undefined, actions),
  ],
] as const) {
  report(name, 'running');
  const started = Date.now();

  try {
    const { message } = await captureSdkRequest(operation);
    if (message.type !== expectedType) {
      throw new Error(`Expected ${expectedType} protocol, got ${String(message.type)}.`);
    }

    const serialized = message.actions;
    if (!Array.isArray(serialized) || serialized.length !== 2) {
      throw new Error(`Expected two visible unique actionable entries: ${JSON.stringify(serialized)}`);
    }
    if (
      serialized[0]?.name !== 'Run'
      || serialized[0]?.value !== 'run'
      || serialized[1]?.name !== 'Callback'
      || serialized[1]?.hasAction !== true
      || 'onAction' in serialized[1]
    ) {
      throw new Error(`Prompt actions did not serialize safely: ${JSON.stringify(serialized)}`);
    }

    report(name, 'pass', {
      result: { prompt: expectedType, actions: serialized.length, hostWindowShown: false },
      duration_ms: Date.now() - started,
    });
  } catch (error) {
    report(name, 'fail', { error: String(error), duration_ms: Date.now() - started });
  }
}
