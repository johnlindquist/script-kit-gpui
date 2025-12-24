import * as readline from 'node:readline';

// =============================================================================
// Types
// =============================================================================

export interface Choice {
  name: string;
  value: string;
  description?: string;
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

interface SubmitMessage {
  type: 'submit';
  id: string;
  value: string | null;
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
