// Name: File Info Tool  
// Description: Get information about files

import "@johnlindquist/kit"
import { stat } from "node:fs/promises"
import { basename, extname, dirname } from "node:path"

metadata = {
  name: "File Information",
  description: "Get detailed information about a file path",
  version: "1.0.0",
}

const { input, output } = defineSchema({
  input: {
    path: {
      type: "string",
      description: "Absolute path to the file",
      required: true,
    },
  },
  output: {
    exists: {
      type: "boolean",
      description: "Whether the file exists",
    },
    name: {
      type: "string",
      description: "File name without directory",
    },
    extension: {
      type: "string",
      description: "File extension",
    },
    directory: {
      type: "string",
      description: "Parent directory path",
    },
    size_bytes: {
      type: "number",
      description: "File size in bytes",
    },
    is_directory: {
      type: "boolean",
      description: "Whether path is a directory",
    },
    modified: {
      type: "string",
      description: "Last modified timestamp (ISO)",
    },
    error: {
      type: "string",
      description: "Error message if file access failed",
    },
  },
} as const)

const { path } = await input()

try {
  const stats = await stat(path)
  
  output({
    exists: true,
    name: basename(path),
    extension: extname(path),
    directory: dirname(path),
    size_bytes: stats.size,
    is_directory: stats.isDirectory(),
    modified: stats.mtime.toISOString(),
  })
} catch (err) {
  output({
    exists: false,
    name: basename(path),
    extension: extname(path),
    directory: dirname(path),
    error: err instanceof Error ? err.message : String(err),
  })
}

if (!metadata.mcp) {
  const info = _getScriptOutput()
  await div(`<pre class="p-4 text-sm">${JSON.stringify(info, null, 2)}</pre>`)
}
