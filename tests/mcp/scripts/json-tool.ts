// Name: JSON Tool
// Description: Parse and manipulate JSON

import "@johnlindquist/kit"

metadata = {
  name: "JSON Processor",
  description: "Parse, format, and extract data from JSON",
  version: "1.0.0",
}

const { input, output } = defineSchema({
  input: {
    json: {
      type: "string",
      description: "JSON string to process",
      required: true,
    },
    action: {
      type: "string",
      description: "Action to perform on the JSON",
      enum: ["parse", "format", "minify", "extract"],
      default: "format",
    },
    path: {
      type: "string",
      description: "JSON path for extraction (e.g., 'data.users[0].name')",
    },
  },
  output: {
    success: {
      type: "boolean",
      description: "Whether the operation succeeded",
    },
    result: {
      type: "string",
      description: "The processed JSON or extracted value",
    },
    type: {
      type: "string",
      description: "Type of the result value",
    },
    error: {
      type: "string",
      description: "Error message if parsing failed",
    },
  },
} as const)

const { json, action, path } = await input()

try {
  const parsed = JSON.parse(json)
  
  switch (action) {
    case "parse":
      output({
        success: true,
        result: JSON.stringify(parsed),
        type: Array.isArray(parsed) ? "array" : typeof parsed,
      })
      break
      
    case "format":
      output({
        success: true,
        result: JSON.stringify(parsed, null, 2),
        type: Array.isArray(parsed) ? "array" : typeof parsed,
      })
      break
      
    case "minify":
      output({
        success: true,
        result: JSON.stringify(parsed),
        type: Array.isArray(parsed) ? "array" : typeof parsed,
      })
      break
      
    case "extract":
      if (!path) {
        output({
          success: false,
          error: "Path required for extract action",
        })
      } else {
        // Simple path extraction (supports dot notation and array indices)
        const parts = path.replace(/\[(\d+)\]/g, ".$1").split(".")
        let value: unknown = parsed
        for (const part of parts) {
          if (value && typeof value === "object") {
            value = (value as Record<string, unknown>)[part]
          } else {
            value = undefined
            break
          }
        }
        output({
          success: true,
          result: typeof value === "string" ? value : JSON.stringify(value),
          type: Array.isArray(value) ? "array" : typeof value,
        })
      }
      break
  }
} catch (err) {
  output({
    success: false,
    error: err instanceof Error ? err.message : "Invalid JSON",
  })
}

if (!metadata.mcp) {
  const result = _getScriptOutput()
  await div(`<pre class="p-4 text-sm">${JSON.stringify(result, null, 2)}</pre>`)
}
