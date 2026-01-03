# Bun Alternatives for Removed SDK Functions

Script Kit GPUI focuses on UI prompts and system control. For utility functions that were available in the original Script Kit, use Bun's powerful native capabilities instead.

This guide shows you how to migrate from removed SDK functions to Bun-native alternatives.

## Table of Contents

1. [Shell Execution](#shell-execution-replaces-exec)
2. [HTTP Requests](#http-requests-replaces-get-post-put-patch-del)
3. [Database](#database-replaces-db)
4. [Key-Value Store](#key-value-store-replaces-store)
5. [File Download](#file-download-replaces-download)
6. [Trash Files](#trash-files-replaces-trash)

---

## Shell Execution (replaces `exec()`)

Bun provides two powerful ways to run shell commands.

### Simple Commands with `Bun.$`

The shell template literal is the easiest way to run commands:

```typescript
import '../../scripts/kit-sdk';

// Simple command
const result = await Bun.$`ls -la`.text();
console.log(result);

// With variables (automatically escaped for safety)
const filename = "my file.txt";
await Bun.$`cat ${filename}`;

// Get stdout as text
const gitBranch = await Bun.$`git branch --show-current`.text();
console.log(`Current branch: ${gitBranch.trim()}`);

// Get stdout as JSON
const packageJson = await Bun.$`cat package.json`.json();
console.log(packageJson.name);

// Check exit code
const { exitCode } = await Bun.$`grep "pattern" file.txt`.nothrow();
if (exitCode !== 0) {
  console.log("Pattern not found");
}
```

### More Control with `Bun.spawn()`

For more complex scenarios, use `Bun.spawn()`:

```typescript
import '../../scripts/kit-sdk';

// Run a command with full control
const proc = Bun.spawn(["node", "--version"], {
  cwd: "/path/to/directory",
  env: { ...process.env, MY_VAR: "value" },
  stdout: "pipe",
  stderr: "pipe",
});

// Read output
const stdout = await new Response(proc.stdout).text();
const stderr = await new Response(proc.stderr).text();
const exitCode = await proc.exited;

console.log(`stdout: ${stdout}`);
console.log(`stderr: ${stderr}`);
console.log(`exit code: ${exitCode}`);
```

### Error Handling

```typescript
import '../../scripts/kit-sdk';

// With Bun.$ - use .nothrow() to prevent exceptions on non-zero exit
const { exitCode, stdout, stderr } = await Bun.$`command-that-might-fail`.nothrow();

if (exitCode !== 0) {
  console.error(`Command failed with exit code ${exitCode}`);
  console.error(await new Response(stderr).text());
} else {
  console.log(await new Response(stdout).text());
}

// Or use try/catch
try {
  const result = await Bun.$`risky-command`.text();
  console.log(result);
} catch (error) {
  console.error("Command failed:", error.message);
}
```

---

## HTTP Requests (replaces `get()`, `post()`, `put()`, `patch()`, `del()`)

Use the native `fetch()` API, which Bun supports with excellent performance.

### GET Request

```typescript
import '../../scripts/kit-sdk';

// Simple GET
const response = await fetch("https://api.example.com/data");
const data = await response.json();
console.log(data);

// With headers
const authResponse = await fetch("https://api.example.com/protected", {
  headers: {
    "Authorization": "Bearer your-token",
    "Accept": "application/json",
  },
});
```

### POST Request

```typescript
import '../../scripts/kit-sdk';

// POST with JSON body
const response = await fetch("https://api.example.com/users", {
  method: "POST",
  headers: {
    "Content-Type": "application/json",
  },
  body: JSON.stringify({
    name: "John",
    email: "john@example.com",
  }),
});

const result = await response.json();
console.log(result);
```

### PUT, PATCH, DELETE

```typescript
import '../../scripts/kit-sdk';

// PUT - full update
await fetch("https://api.example.com/users/123", {
  method: "PUT",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ name: "Updated Name", email: "new@example.com" }),
});

// PATCH - partial update  
await fetch("https://api.example.com/users/123", {
  method: "PATCH",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ name: "Only Update Name" }),
});

// DELETE
await fetch("https://api.example.com/users/123", {
  method: "DELETE",
});
```

### Error Handling

```typescript
import '../../scripts/kit-sdk';

async function fetchWithErrorHandling(url: string) {
  try {
    const response = await fetch(url);
    
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
    
    return await response.json();
  } catch (error) {
    if (error instanceof TypeError) {
      console.error("Network error:", error.message);
    } else {
      console.error("Request failed:", error.message);
    }
    throw error;
  }
}

// Usage
const data = await fetchWithErrorHandling("https://api.example.com/data");
```

### Helper Function (if you want the old API style)

```typescript
// Create helper functions if you prefer the old style
const http = {
  async get(url: string, options?: RequestInit) {
    const res = await fetch(url, { ...options, method: "GET" });
    return res.json();
  },
  
  async post(url: string, body: unknown, options?: RequestInit) {
    const res = await fetch(url, {
      ...options,
      method: "POST",
      headers: { "Content-Type": "application/json", ...options?.headers },
      body: JSON.stringify(body),
    });
    return res.json();
  },
  
  async put(url: string, body: unknown, options?: RequestInit) {
    const res = await fetch(url, {
      ...options,
      method: "PUT",
      headers: { "Content-Type": "application/json", ...options?.headers },
      body: JSON.stringify(body),
    });
    return res.json();
  },
  
  async patch(url: string, body: unknown, options?: RequestInit) {
    const res = await fetch(url, {
      ...options,
      method: "PATCH",
      headers: { "Content-Type": "application/json", ...options?.headers },
      body: JSON.stringify(body),
    });
    return res.json();
  },
  
  async del(url: string, options?: RequestInit) {
    const res = await fetch(url, { ...options, method: "DELETE" });
    return res.ok;
  },
};

// Usage
const users = await http.get("https://api.example.com/users");
const newUser = await http.post("https://api.example.com/users", { name: "John" });
```

---

## Database (replaces `db()`)

Bun has built-in SQLite support via `bun:sqlite`.

### Basic Setup

```typescript
import '../../scripts/kit-sdk';
import { Database } from "bun:sqlite";
import { join } from "path";
import { mkdirSync } from "fs";

// Create database in ~/.sk/kit/db/
const dbDir = join(process.env.HOME!, ".kenv", "db");
mkdirSync(dbDir, { recursive: true });

const db = new Database(join(dbDir, "my-script.db"));

// Create a table
db.run(`
  CREATE TABLE IF NOT EXISTS items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    value TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
  )
`);
```

### Insert Data

```typescript
import { Database } from "bun:sqlite";

const db = new Database("~/.sk/kit/db/my-script.db");

// Single insert
db.run("INSERT INTO items (name, value) VALUES (?, ?)", ["item1", "value1"]);

// Prepared statement (better for multiple inserts)
const insert = db.prepare("INSERT INTO items (name, value) VALUES ($name, $value)");

insert.run({ $name: "item2", $value: "value2" });
insert.run({ $name: "item3", $value: "value3" });

// Insert many with transaction (much faster)
const insertMany = db.transaction((items: { name: string; value: string }[]) => {
  for (const item of items) {
    insert.run({ $name: item.name, $value: item.value });
  }
});

insertMany([
  { name: "bulk1", value: "v1" },
  { name: "bulk2", value: "v2" },
  { name: "bulk3", value: "v3" },
]);
```

### Query Data

```typescript
import { Database } from "bun:sqlite";

const db = new Database("~/.sk/kit/db/my-script.db");

// Get all rows
const allItems = db.query("SELECT * FROM items").all();
console.log(allItems);

// Get one row
const item = db.query("SELECT * FROM items WHERE id = ?").get(1);
console.log(item);

// Query with named parameters
const query = db.query("SELECT * FROM items WHERE name LIKE $pattern");
const results = query.all({ $pattern: "%search%" });

// Get specific columns
interface ItemRow {
  id: number;
  name: string;
}
const names = db.query("SELECT id, name FROM items").all() as ItemRow[];
```

### Update and Delete

```typescript
import { Database } from "bun:sqlite";

const db = new Database("~/.sk/kit/db/my-script.db");

// Update
db.run("UPDATE items SET value = ? WHERE id = ?", ["new value", 1]);

// Delete
db.run("DELETE FROM items WHERE id = ?", [1]);

// Delete with condition
db.run("DELETE FROM items WHERE created_at < datetime('now', '-30 days')");
```

---

## Key-Value Store (replaces `store`)

Create a simple SQLite-based key-value store for persistent data.

### Simple Key-Value Store

```typescript
import '../../scripts/kit-sdk';
import { Database } from "bun:sqlite";
import { join } from "path";
import { mkdirSync } from "fs";

// Setup database
const dbDir = join(process.env.HOME!, ".kenv", "db");
mkdirSync(dbDir, { recursive: true });
const db = new Database(join(dbDir, "store.db"));

// Create key-value table
db.run(`
  CREATE TABLE IF NOT EXISTS kv (
    key TEXT PRIMARY KEY,
    value TEXT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
  )
`);

// Store API
const store = {
  get(key: string): unknown {
    const row = db.query("SELECT value FROM kv WHERE key = ?").get(key) as { value: string } | null;
    return row ? JSON.parse(row.value) : undefined;
  },
  
  set(key: string, value: unknown): void {
    db.run(
      `INSERT INTO kv (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)
       ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = CURRENT_TIMESTAMP`,
      [key, JSON.stringify(value), JSON.stringify(value)]
    );
  },
  
  delete(key: string): boolean {
    const result = db.run("DELETE FROM kv WHERE key = ?", [key]);
    return result.changes > 0;
  },
  
  has(key: string): boolean {
    const row = db.query("SELECT 1 FROM kv WHERE key = ?").get(key);
    return row !== null;
  },
  
  keys(): string[] {
    const rows = db.query("SELECT key FROM kv").all() as { key: string }[];
    return rows.map(r => r.key);
  },
  
  clear(): void {
    db.run("DELETE FROM kv");
  },
};

// Usage
store.set("user", { name: "John", age: 30 });
store.set("settings", { theme: "dark", fontSize: 14 });

const user = store.get("user");
console.log(user); // { name: "John", age: 30 }

console.log(store.has("settings")); // true
console.log(store.keys()); // ["user", "settings"]

store.delete("user");
```

### Namespaced Store (for script isolation)

```typescript
import { Database } from "bun:sqlite";
import { join } from "path";
import { mkdirSync } from "fs";

function createStore(namespace: string) {
  const dbDir = join(process.env.HOME!, ".kenv", "db");
  mkdirSync(dbDir, { recursive: true });
  
  // Each namespace gets its own database file
  const db = new Database(join(dbDir, `${namespace}.db`));
  
  db.run(`
    CREATE TABLE IF NOT EXISTS kv (
      key TEXT PRIMARY KEY,
      value TEXT
    )
  `);
  
  return {
    get: (key: string) => {
      const row = db.query("SELECT value FROM kv WHERE key = ?").get(key) as { value: string } | null;
      return row ? JSON.parse(row.value) : undefined;
    },
    set: (key: string, value: unknown) => {
      db.run(
        "INSERT OR REPLACE INTO kv (key, value) VALUES (?, ?)",
        [key, JSON.stringify(value)]
      );
    },
    delete: (key: string) => {
      db.run("DELETE FROM kv WHERE key = ?", [key]);
    },
  };
}

// Usage - each script gets its own store
const myStore = createStore("my-script-name");
myStore.set("lastRun", new Date().toISOString());
console.log(myStore.get("lastRun"));
```

---

## File Download (replaces `download()`)

Use `fetch()` combined with `Bun.write()` for efficient file downloads.

### Basic Download

```typescript
import '../../scripts/kit-sdk';
import { join } from "path";

async function download(url: string, destPath: string): Promise<void> {
  const response = await fetch(url);
  
  if (!response.ok) {
    throw new Error(`Download failed: ${response.status} ${response.statusText}`);
  }
  
  await Bun.write(destPath, response);
  console.log(`Downloaded to ${destPath}`);
}

// Usage
await download(
  "https://example.com/file.zip",
  join(process.env.HOME!, "Downloads", "file.zip")
);
```

### Download with Progress

```typescript
import '../../scripts/kit-sdk';

async function downloadWithProgress(
  url: string, 
  destPath: string,
  onProgress?: (percent: number, downloaded: number, total: number) => void
): Promise<void> {
  const response = await fetch(url);
  
  if (!response.ok) {
    throw new Error(`Download failed: ${response.status}`);
  }
  
  const contentLength = response.headers.get("content-length");
  const total = contentLength ? parseInt(contentLength, 10) : 0;
  
  if (!response.body) {
    throw new Error("Response body is null");
  }
  
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let downloaded = 0;
  
  while (true) {
    const { done, value } = await reader.read();
    
    if (done) break;
    
    chunks.push(value);
    downloaded += value.length;
    
    if (onProgress && total > 0) {
      const percent = Math.round((downloaded / total) * 100);
      onProgress(percent, downloaded, total);
    }
  }
  
  // Combine chunks and write
  const blob = new Blob(chunks);
  await Bun.write(destPath, blob);
}

// Usage with progress
await downloadWithProgress(
  "https://example.com/large-file.zip",
  "./download.zip",
  (percent, downloaded, total) => {
    console.log(`Progress: ${percent}% (${downloaded}/${total} bytes)`);
  }
);
```

### Download JSON

```typescript
import '../../scripts/kit-sdk';

async function downloadJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  
  if (!response.ok) {
    throw new Error(`Download failed: ${response.status}`);
  }
  
  return response.json() as Promise<T>;
}

// Usage
interface ApiData {
  users: { id: number; name: string }[];
}

const data = await downloadJson<ApiData>("https://api.example.com/users");
console.log(data.users);
```

---

## Trash Files (replaces `trash()`)

Move files to trash instead of permanently deleting them.

### macOS Shell Approach

```typescript
import '../../scripts/kit-sdk';
import { join } from "path";

async function trash(filePath: string): Promise<void> {
  // Expand ~ to home directory if needed
  const fullPath = filePath.startsWith("~") 
    ? join(process.env.HOME!, filePath.slice(1))
    : filePath;
  
  // Use macOS 'mv' to trash - simple but works
  const trashPath = join(process.env.HOME!, ".Trash");
  await Bun.$`mv ${fullPath} ${trashPath}/`;
}

// Usage
await trash("~/Desktop/old-file.txt");
await trash("/path/to/delete/file.txt");
```

### Using AppleScript (Better macOS Integration)

```typescript
import '../../scripts/kit-sdk';

async function trashWithFinder(filePath: string): Promise<void> {
  // This uses Finder, so files show up in Trash properly with "Put Back" option
  const absolutePath = filePath.startsWith("/") 
    ? filePath 
    : join(process.cwd(), filePath);
  
  await Bun.$`osascript -e 'tell application "Finder" to delete POSIX file "${absolutePath}"'`;
}

// Usage
await trashWithFinder("/Users/me/Desktop/file.txt");
```

### Cross-Platform with trash-cli

For cross-platform support, use the `trash-cli` npm package:

```bash
# Install globally or as dev dependency
bun add -d trash-cli
```

```typescript
import '../../scripts/kit-sdk';

async function trash(filePath: string): Promise<void> {
  // trash-cli works on macOS, Windows, and Linux
  await Bun.$`bunx trash-cli ${filePath}`;
}

// Or use the programmatic API
import trashModule from "trash";

await trashModule("/path/to/file.txt");
await trashModule(["/path/to/file1.txt", "/path/to/file2.txt"]);
```

### Trash Multiple Files

```typescript
import '../../scripts/kit-sdk';
import { join } from "path";

async function trashMany(filePaths: string[]): Promise<void> {
  const trashPath = join(process.env.HOME!, ".Trash");
  
  for (const filePath of filePaths) {
    try {
      await Bun.$`mv ${filePath} ${trashPath}/`;
      console.log(`Trashed: ${filePath}`);
    } catch (error) {
      console.error(`Failed to trash ${filePath}:`, error);
    }
  }
}

// Usage
await trashMany([
  "~/Desktop/old1.txt",
  "~/Desktop/old2.txt", 
  "~/Downloads/temp-file.zip",
]);
```

---

## Summary

| Old SDK Function | Bun Alternative |
|------------------|-----------------|
| `exec(cmd)` | `Bun.$\`cmd\`` or `Bun.spawn()` |
| `get(url)` | `fetch(url)` |
| `post(url, data)` | `fetch(url, { method: 'POST', body: JSON.stringify(data) })` |
| `put(url, data)` | `fetch(url, { method: 'PUT', body: JSON.stringify(data) })` |
| `patch(url, data)` | `fetch(url, { method: 'PATCH', body: JSON.stringify(data) })` |
| `del(url)` | `fetch(url, { method: 'DELETE' })` |
| `db()` | `import { Database } from 'bun:sqlite'` |
| `store.get/set` | SQLite-based key-value store (see above) |
| `download(url, path)` | `fetch(url)` + `Bun.write(path, response)` |
| `trash(path)` | `Bun.$\`mv \${path} ~/.Trash/\`` or `trash-cli` |

## Benefits of Bun Native

1. **Performance**: Bun's built-in APIs are highly optimized
2. **No Dependencies**: SQLite, fetch, and shell are built into Bun
3. **Type Safety**: Full TypeScript support out of the box
4. **Simpler**: Direct APIs with less abstraction overhead
5. **Standard**: Uses standard Web APIs (fetch) where possible
