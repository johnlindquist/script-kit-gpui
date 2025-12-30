// Name: Greeting Tool
// Description: Simple greeting example for MCP

import "@johnlindquist/kit"

// Typed metadata takes priority over comments
metadata = {
  name: "Greeting Generator",
  description: "Generate personalized greetings with customizable format",
  author: "Script Kit",
  version: "1.0.0",
}

// Define schema with full type inference
const { input, output } = defineSchema({
  input: {
    name: {
      type: "string",
      description: "Name of the person to greet",
      required: true,
    },
    style: {
      type: "string",
      description: "Greeting style",
      enum: ["formal", "casual", "enthusiastic"],
      default: "casual",
    },
  },
  output: {
    greeting: {
      type: "string",
      description: "The generated greeting message",
    },
    style_used: {
      type: "string",
      description: "The style that was applied",
    },
  },
} as const)

// Get typed input
const { name, style } = await input()

// Generate greeting based on style
let greeting: string
switch (style) {
  case "formal":
    greeting = `Good day, ${name}. It is a pleasure to make your acquaintance.`
    break
  case "enthusiastic":
    greeting = `HEY ${name.toUpperCase()}! SO AWESOME TO SEE YOU! 🎉`
    break
  case "casual":
  default:
    greeting = `Hey ${name}! What's up?`
}

// Send typed output
output({ greeting, style_used: style || "casual" })

// For interactive use, show the greeting
if (!metadata.mcp) {
  await div(`<div class="p-8 text-2xl">${greeting}</div>`)
}
