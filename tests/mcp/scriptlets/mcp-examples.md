# MCP Example Scriptlets

These scriptlets demonstrate various patterns for MCP integration.

## Quick Actions

### Current Time
<!-- 
group: MCP Examples
tool: js
-->
```js
new Date().toISOString()
```

### Random UUID  
<!--
group: MCP Examples
tool: js
-->
```js
crypto.randomUUID()
```

### System Info
<!--
group: MCP Examples
tool: js
-->
```js
JSON.stringify({
  platform: process.platform,
  arch: process.arch,
  nodeVersion: process.version,
  cwd: process.cwd(),
  user: process.env.USER || process.env.USERNAME,
}, null, 2)
```

## Templates

### Greeting Template
<!--
group: MCP Examples
tool: template
inputs:
  - name: string
  - time: enum[morning,afternoon,evening]
-->
Good {{time}}, {{name}}! How can I help you today?

### Meeting Notes Template
<!--
group: MCP Examples
tool: template
inputs:
  - title: string
  - attendees: string
  - date: string
-->
# Meeting: {{title}}
**Date:** {{date}}
**Attendees:** {{attendees}}

## Agenda
- [ ] Item 1
- [ ] Item 2

## Notes


## Action Items
- [ ] 

### Email Template
<!--
group: MCP Examples
tool: template
inputs:
  - recipient: string
  - subject: string
-->
To: {{recipient}}
Subject: {{subject}}

Dear {{recipient}},



Best regards,
${process.env.USER}

## Shell Commands

### List Downloads
<!--
group: MCP Examples
tool: bash
-->
```bash
ls -la ~/Downloads | head -20
```

### Git Status
<!--
group: MCP Examples  
tool: bash
-->
```bash
git status --short 2>/dev/null || echo "Not a git repository"
```

### Disk Usage
<!--
group: MCP Examples
tool: bash
-->
```bash
df -h | head -5
```

## Paste Snippets

### JSON Object Template
<!--
group: MCP Examples
tool: paste
-->
```json
{
  "name": "",
  "version": "1.0.0",
  "description": ""
}
```

### TypeScript Function Template
<!--
group: MCP Examples
tool: paste
-->
```typescript
export function {{name}}({{params}}): {{returnType}} {
  // TODO: Implement
}
```

### Console Debug
<!--
group: MCP Examples
tool: paste
expand: ,debug
-->
console.log('[DEBUG]', JSON.stringify({}, null, 2));

## TypeScript Scriptlets

### Calculate Days Until
<!--
group: MCP Examples
tool: ts
-->
```ts
const targetDate = new Date('2025-12-31');
const today = new Date();
const diff = targetDate.getTime() - today.getTime();
const days = Math.ceil(diff / (1000 * 60 * 60 * 24));
await div(`<div class="p-4 text-2xl">Days until Dec 31: ${days}</div>`);
```

### Clipboard Word Count
<!--
group: MCP Examples
tool: ts
-->
```ts
const text = await clipboard.readText();
const words = text.trim().split(/\s+/).filter(w => w.length > 0).length;
const chars = text.length;
await div(`
  <div class="p-4">
    <div class="text-xl">Words: ${words}</div>
    <div class="text-xl">Characters: ${chars}</div>
  </div>
`);
```

### URL Shortener Preview
<!--
group: MCP Examples
tool: ts
-->
```ts
const url = await arg("Enter URL to preview");
try {
  const parsed = new URL(url);
  await div(`
    <div class="p-4 space-y-2">
      <div><strong>Protocol:</strong> ${parsed.protocol}</div>
      <div><strong>Host:</strong> ${parsed.host}</div>
      <div><strong>Path:</strong> ${parsed.pathname}</div>
      <div><strong>Query:</strong> ${parsed.search || '(none)'}</div>
    </div>
  `);
} catch (e) {
  await div(`<div class="p-4 text-red-500">Invalid URL</div>`);
}
```
