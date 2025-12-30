// Name: No Schema Tool
// Description: A script without schema (should NOT appear as MCP tool)

import "@johnlindquist/kit"

// This script has no schema, so it should NOT be exposed as an MCP tool
// Only scripts with `schema = {...}` or `defineSchema({...})` become tools

const name = await arg("What's your name?")
await div(`<div class="p-8 text-2xl">Hello, ${name}!</div>`)
