// Name: SDK Test - Inline Chat Controller Capability Boundaries
// Description: Proves session-only chat mutations and local readers without opening chat or contacting AI.

type Outcome = 'running' | 'pass' | 'fail';

function report(test: string, status: Outcome, details: Record<string, unknown> = {}): void {
  console.log(JSON.stringify({ test, status, timestamp: new Date().toISOString(), ...details }));
}

async function verify(name: string, operation: () => unknown): Promise<void> {
  report(name, 'running');
  const started = Date.now();
  try {
    report(name, 'pass', { result: await operation(), duration_ms: Date.now() - started });
  } catch (error) {
    report(name, 'fail', { error: String(error), duration_ms: Date.now() - started });
  }
}

await verify('inline-chat-local-readers-never-open-or-contact-a-provider', () => {
  const writes: string[] = [];
  const originalWrite = process.stdout.write;
  process.stdout.write = ((chunk: unknown) => {
    writes.push(String(chunk));
    return true;
  }) as typeof process.stdout.write;

  try {
    const messages = chat.getMessages();
    const result = chat.getResult();
    if (!Array.isArray(messages) || messages.length !== 0) {
      throw new Error(`Expected an empty local message array: ${JSON.stringify(messages)}`);
    }
    if (!Array.isArray(result.messages) || result.messages.length !== 0) {
      throw new Error(`Expected an empty local result snapshot: ${JSON.stringify(result)}`);
    }
    if (writes.length !== 0) {
      throw new Error(`Read-only chat helpers unexpectedly dispatched: ${writes.join('')}`);
    }
    return { readers: ['chat.getMessages', 'chat.getResult'], protocolMessages: 0 };
  } finally {
    process.stdout.write = originalWrite;
  }
});

await verify('inline-chat-mutations-require-an-existing-session-before-dispatch', () => {
  const writes: string[] = [];
  const originalWrite = process.stdout.write;
  process.stdout.write = ((chunk: unknown) => {
    writes.push(String(chunk));
    return true;
  }) as typeof process.stdout.write;

  const operations = [
    ['chat.addMessage', () => chat.addMessage({ role: 'user', content: 'synthetic' })],
    ['chat.startStream', () => chat.startStream('left')],
    ['chat.appendChunk', () => chat.appendChunk('synthetic-message', 'chunk')],
    ['chat.completeStream', () => chat.completeStream('synthetic-message')],
    ['chat.clear', () => chat.clear()],
    ['chat.setError', () => chat.setError('synthetic-message', 'synthetic error')],
    ['chat.clearError', () => chat.clearError('synthetic-message')],
  ] as const;

  try {
    for (const [name, operation] of operations) {
      let failure: unknown;
      try {
        operation();
      } catch (error) {
        failure = error;
      }
      if (!(failure instanceof Error) || !failure.message.includes('outside of a chat session')) {
        throw new Error(`${name} must reject before dispatch without an active session.`);
      }
    }
    if (writes.length !== 0) {
      throw new Error(`Sessionless chat mutations unexpectedly dispatched: ${writes.join('')}`);
    }
    return { guardedControllers: operations.length, protocolMessages: 0, providerRequests: 0 };
  } finally {
    process.stdout.write = originalWrite;
  }
});
