// Name: Text Transform Tool
// Description: Transform text in various ways

import "@johnlindquist/kit"

metadata = {
  name: "Text Transformer",
  description: "Apply various transformations to text input",
  version: "1.0.0",
}

const { input, output } = defineSchema({
  input: {
    text: {
      type: "string",
      description: "The text to transform",
      required: true,
    },
    transforms: {
      type: "array",
      description: "List of transformations to apply in order",
      items: {
        type: "string",
        enum: ["uppercase", "lowercase", "reverse", "trim", "slug", "base64"],
      },
      default: ["trim"],
    },
  },
  output: {
    original: {
      type: "string",
      description: "Original input text",
    },
    result: {
      type: "string",
      description: "Transformed text",
    },
    transforms_applied: {
      type: "array",
      description: "List of transformations that were applied",
    },
  },
} as const)

const { text, transforms } = await input()

let result = text
const applied: string[] = []

for (const transform of transforms || ["trim"]) {
  switch (transform) {
    case "uppercase":
      result = result.toUpperCase()
      applied.push("uppercase")
      break
    case "lowercase":
      result = result.toLowerCase()
      applied.push("lowercase")
      break
    case "reverse":
      result = result.split("").reverse().join("")
      applied.push("reverse")
      break
    case "trim":
      result = result.trim()
      applied.push("trim")
      break
    case "slug":
      result = result
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, "")
      applied.push("slug")
      break
    case "base64":
      result = Buffer.from(result).toString("base64")
      applied.push("base64")
      break
  }
}

output({
  original: text,
  result,
  transforms_applied: applied,
})

if (!metadata.mcp) {
  await div(`
    <div class="p-4 space-y-4">
      <div class="text-gray-400">Original: ${text}</div>
      <div class="text-2xl font-mono">${result}</div>
      <div class="text-sm text-gray-500">Applied: ${applied.join(" -> ")}</div>
    </div>
  `)
}
