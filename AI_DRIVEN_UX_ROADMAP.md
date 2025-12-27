# AI-Driven UX Protocol Roadmap

> Protocol extensions for autonomous AI agent interaction with Script Kit GPUI

**Version:** 1.0-draft  
**Status:** Proposal  
**Created:** 2025-12-27

---

## Executive Summary

This roadmap proposes protocol extensions to transform Script Kit GPUI from a human-first launcher into a first-class AI automation platform. The current protocol supports basic command/response patterns but lacks the introspection, batching, and semantic targeting capabilities required for reliable autonomous agent operation.

### Current Gaps

| Gap | Impact | Priority |
|-----|--------|----------|
| No query/introspection | Agents can't ask "what's on screen?" | P0 |
| No element inspection | Can't get list of visible elements | P0 |
| No batch commands | Must send one message at a time | P1 |
| No streaming responses | All-or-nothing responses | P1 |
| No semantic IDs | Elements lack stable identifiers | P2 |
| No accessibility tree | Can't traverse UI hierarchy | P2 |
| No agent handshake | No capability negotiation | P2 |

### Design Principles

1. **Backward Compatible** - All extensions are additive; existing scripts continue to work
2. **Opt-in Complexity** - Simple by default, powerful when needed
3. **Token Efficient** - Minimize response sizes for AI agents
4. **Deterministic** - Same input produces same output

---

## Phase 1: Query/Introspection Protocol

**Priority:** P0 - Critical  
**Estimated Effort:** 2 weeks

Enable agents to query current UI state without modifying it.

### 1.1 getState

Query the current application state.

**Request:**
```json
{
  "type": "getState",
  "requestId": "req-001",
  "include": ["prompt", "choices", "input", "selection"],
  "format": "minimal"
}
```

**Response:**
```json
{
  "type": "stateResult",
  "requestId": "req-001",
  "state": {
    "promptType": "arg",
    "promptId": "prompt-123",
    "placeholder": "Pick a fruit",
    "inputValue": "app",
    "choiceCount": 15,
    "visibleChoiceCount": 5,
    "selectedIndex": 2,
    "selectedValue": "apple",
    "isFocused": true,
    "windowVisible": true
  }
}
```

**JSON Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/getState.schema.json",
  "title": "GetState Request",
  "type": "object",
  "required": ["type", "requestId"],
  "properties": {
    "type": { "const": "getState" },
    "requestId": { 
      "type": "string",
      "description": "Unique identifier for request correlation"
    },
    "include": {
      "type": "array",
      "items": {
        "type": "string",
        "enum": ["prompt", "choices", "input", "selection", "window", "focus", "all"]
      },
      "default": ["all"],
      "description": "State components to include in response"
    },
    "format": {
      "type": "string",
      "enum": ["minimal", "full", "debug"],
      "default": "minimal",
      "description": "Response verbosity level"
    }
  }
}
```

**Response Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/stateResult.schema.json",
  "title": "StateResult Response",
  "type": "object",
  "required": ["type", "requestId", "state"],
  "properties": {
    "type": { "const": "stateResult" },
    "requestId": { "type": "string" },
    "state": {
      "type": "object",
      "properties": {
        "promptType": { 
          "type": "string",
          "enum": ["arg", "div", "editor", "select", "fields", "path", "hotkey", "term", "chat", "none"]
        },
        "promptId": { "type": "string" },
        "placeholder": { "type": "string" },
        "inputValue": { "type": "string" },
        "choiceCount": { "type": "integer", "minimum": 0 },
        "visibleChoiceCount": { "type": "integer", "minimum": 0 },
        "selectedIndex": { "type": "integer", "minimum": -1 },
        "selectedValue": { "type": ["string", "null"] },
        "isFocused": { "type": "boolean" },
        "windowVisible": { "type": "boolean" }
      }
    }
  }
}
```

### 1.2 getElements

Query visible UI elements with optional filtering.

**Request:**
```json
{
  "type": "getElements",
  "requestId": "req-002",
  "filter": {
    "type": ["choice", "button", "input"],
    "visible": true,
    "interactive": true
  },
  "include": ["bounds", "text", "semanticId"],
  "limit": 50
}
```

**Response:**
```json
{
  "type": "elementsResult",
  "requestId": "req-002",
  "elements": [
    {
      "semanticId": "choice:0:apple",
      "type": "choice",
      "index": 0,
      "text": "Apple",
      "description": "A red fruit",
      "value": "apple",
      "bounds": { "x": 10, "y": 52, "width": 480, "height": 52 },
      "selected": true,
      "visible": true,
      "interactive": true
    },
    {
      "semanticId": "input:filter",
      "type": "input",
      "text": "app",
      "bounds": { "x": 10, "y": 0, "width": 480, "height": 44 },
      "focused": true,
      "visible": true,
      "interactive": true
    }
  ],
  "totalCount": 15,
  "truncated": false
}
```

**JSON Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/getElements.schema.json",
  "title": "GetElements Request",
  "type": "object",
  "required": ["type", "requestId"],
  "properties": {
    "type": { "const": "getElements" },
    "requestId": { "type": "string" },
    "filter": {
      "type": "object",
      "properties": {
        "type": {
          "type": "array",
          "items": { 
            "type": "string",
            "enum": ["choice", "button", "input", "text", "panel", "preview", "header", "footer", "action"]
          }
        },
        "visible": { "type": "boolean" },
        "interactive": { "type": "boolean" },
        "semanticIdPattern": { 
          "type": "string",
          "description": "Regex pattern to match semantic IDs"
        }
      }
    },
    "include": {
      "type": "array",
      "items": {
        "type": "string",
        "enum": ["bounds", "text", "semanticId", "value", "state", "children"]
      },
      "default": ["semanticId", "text", "value"]
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "maximum": 1000,
      "default": 100
    }
  }
}
```

### 1.3 getAccessibilityTree

Get hierarchical accessibility tree for navigation and automation.

**Request:**
```json
{
  "type": "getAccessibilityTree",
  "requestId": "req-003",
  "depth": 3,
  "includeHidden": false
}
```

**Response:**
```json
{
  "type": "accessibilityTreeResult",
  "requestId": "req-003",
  "tree": {
    "role": "window",
    "name": "Script Kit",
    "semanticId": "window:main",
    "bounds": { "x": 100, "y": 100, "width": 500, "height": 700 },
    "children": [
      {
        "role": "searchbox",
        "name": "Filter",
        "semanticId": "input:filter",
        "value": "app",
        "focused": true,
        "bounds": { "x": 10, "y": 10, "width": 480, "height": 44 },
        "actions": ["focus", "clear", "type"]
      },
      {
        "role": "listbox",
        "name": "Choices",
        "semanticId": "list:choices",
        "bounds": { "x": 10, "y": 60, "width": 480, "height": 400 },
        "children": [
          {
            "role": "option",
            "name": "Apple",
            "semanticId": "choice:0:apple",
            "selected": true,
            "bounds": { "x": 10, "y": 60, "width": 480, "height": 52 },
            "actions": ["select", "submit"]
          }
        ]
      }
    ]
  },
  "nodeCount": 25
}
```

**JSON Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/getAccessibilityTree.schema.json",
  "title": "GetAccessibilityTree Request",
  "type": "object",
  "required": ["type", "requestId"],
  "properties": {
    "type": { "const": "getAccessibilityTree" },
    "requestId": { "type": "string" },
    "depth": {
      "type": "integer",
      "minimum": 1,
      "maximum": 10,
      "default": 5,
      "description": "Maximum depth of tree traversal"
    },
    "includeHidden": {
      "type": "boolean",
      "default": false,
      "description": "Include hidden/collapsed elements"
    },
    "root": {
      "type": "string",
      "description": "Semantic ID of root element (defaults to window)"
    }
  }
}
```

---

## Phase 2: Batch/Transaction Command Support

**Priority:** P1 - High  
**Estimated Effort:** 2 weeks

Enable multiple operations in a single message with transaction semantics.

### 2.1 Batch Command

Execute multiple commands atomically.

**Request:**
```json
{
  "type": "batch",
  "requestId": "batch-001",
  "commands": [
    { "type": "setFilter", "text": "apple" },
    { "type": "waitFor", "condition": "choicesRendered", "timeout": 1000 },
    { "type": "navigate", "direction": "down", "count": 2 },
    { "type": "submit" }
  ],
  "options": {
    "stopOnError": true,
    "rollbackOnError": false,
    "timeout": 5000
  }
}
```

**Response:**
```json
{
  "type": "batchResult",
  "requestId": "batch-001",
  "success": true,
  "results": [
    { "index": 0, "success": true, "command": "setFilter" },
    { "index": 1, "success": true, "command": "waitFor", "elapsed": 45 },
    { "index": 2, "success": true, "command": "navigate" },
    { "index": 3, "success": true, "command": "submit", "value": "apple" }
  ],
  "totalElapsed": 89
}
```

**Error Case:**
```json
{
  "type": "batchResult",
  "requestId": "batch-001",
  "success": false,
  "results": [
    { "index": 0, "success": true, "command": "setFilter" },
    { "index": 1, "success": false, "command": "waitFor", "error": "Timeout waiting for choicesRendered" }
  ],
  "failedAt": 1,
  "totalElapsed": 1045
}
```

**JSON Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/batch.schema.json",
  "title": "Batch Command Request",
  "type": "object",
  "required": ["type", "requestId", "commands"],
  "properties": {
    "type": { "const": "batch" },
    "requestId": { "type": "string" },
    "commands": {
      "type": "array",
      "minItems": 1,
      "maxItems": 100,
      "items": {
        "type": "object",
        "required": ["type"],
        "description": "Any valid protocol command"
      }
    },
    "options": {
      "type": "object",
      "properties": {
        "stopOnError": {
          "type": "boolean",
          "default": true,
          "description": "Stop executing on first error"
        },
        "rollbackOnError": {
          "type": "boolean", 
          "default": false,
          "description": "Attempt to undo completed commands on error"
        },
        "timeout": {
          "type": "integer",
          "minimum": 100,
          "maximum": 60000,
          "default": 5000,
          "description": "Total timeout for all commands in ms"
        },
        "sequential": {
          "type": "boolean",
          "default": true,
          "description": "Execute commands in order (false = parallel where possible)"
        }
      }
    }
  }
}
```

### 2.2 waitFor Condition

Block until a condition is met (used within batch or standalone).

**Request:**
```json
{
  "type": "waitFor",
  "requestId": "wait-001",
  "condition": {
    "type": "elementExists",
    "semanticId": "choice:0:*"
  },
  "timeout": 2000,
  "pollInterval": 50
}
```

**Condition Types:**
- `choicesRendered` - Choices list has items
- `inputEmpty` - Input field is empty
- `elementExists` - Element with semantic ID exists
- `elementVisible` - Element is visible
- `elementFocused` - Element has focus
- `stateMatch` - State matches criteria
- `custom` - Custom JavaScript expression (sandboxed)

**JSON Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/waitFor.schema.json",
  "title": "WaitFor Request",
  "type": "object",
  "required": ["type", "requestId", "condition"],
  "properties": {
    "type": { "const": "waitFor" },
    "requestId": { "type": "string" },
    "condition": {
      "oneOf": [
        { "type": "string", "enum": ["choicesRendered", "inputEmpty", "windowVisible", "windowFocused"] },
        {
          "type": "object",
          "required": ["type"],
          "properties": {
            "type": { 
              "type": "string",
              "enum": ["elementExists", "elementVisible", "elementFocused", "stateMatch", "custom"]
            },
            "semanticId": { "type": "string" },
            "state": { "type": "object" },
            "expression": { "type": "string" }
          }
        }
      ]
    },
    "timeout": {
      "type": "integer",
      "minimum": 10,
      "maximum": 60000,
      "default": 5000
    },
    "pollInterval": {
      "type": "integer",
      "minimum": 10,
      "maximum": 1000,
      "default": 50
    }
  }
}
```

### 2.3 Atomic Actions

Single-message actions that combine common patterns.

**Request:**
```json
{
  "type": "selectByValue",
  "requestId": "sel-001",
  "value": "apple",
  "submit": true
}
```

**Other Atomic Actions:**
```json
// Select by index
{ "type": "selectByIndex", "requestId": "...", "index": 2, "submit": true }

// Select by text match
{ "type": "selectByText", "requestId": "...", "text": "Apple", "exact": false, "submit": true }

// Filter and select first
{ "type": "filterAndSelect", "requestId": "...", "filter": "app", "selectFirst": true, "submit": true }

// Type and submit
{ "type": "typeAndSubmit", "requestId": "...", "text": "my input" }
```

---

## Phase 3: Streaming/Incremental Responses

**Priority:** P1 - High  
**Estimated Effort:** 3 weeks

Support long-running operations with incremental updates.

### 3.1 Streaming Protocol

Subscribe to state changes with filtered updates.

**Subscribe Request:**
```json
{
  "type": "subscribe",
  "requestId": "sub-001",
  "events": ["choiceSelected", "inputChanged", "promptChanged"],
  "filter": {
    "minInterval": 50,
    "debounce": true
  }
}
```

**Stream Events:**
```json
{
  "type": "stream",
  "subscriptionId": "sub-001",
  "event": "inputChanged",
  "data": {
    "value": "appl",
    "previousValue": "app",
    "timestamp": 1703692800000
  },
  "sequence": 42
}
```

**Unsubscribe:**
```json
{
  "type": "unsubscribe",
  "subscriptionId": "sub-001"
}
```

**JSON Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/subscribe.schema.json",
  "title": "Subscribe Request",
  "type": "object",
  "required": ["type", "requestId", "events"],
  "properties": {
    "type": { "const": "subscribe" },
    "requestId": { "type": "string" },
    "events": {
      "type": "array",
      "items": {
        "type": "string",
        "enum": [
          "inputChanged",
          "choiceSelected", 
          "choicesUpdated",
          "promptChanged",
          "windowFocused",
          "windowBlurred",
          "keyPressed",
          "actionTriggered",
          "errorOccurred",
          "stateChanged"
        ]
      }
    },
    "filter": {
      "type": "object",
      "properties": {
        "minInterval": {
          "type": "integer",
          "minimum": 10,
          "maximum": 5000,
          "default": 50,
          "description": "Minimum ms between events of same type"
        },
        "debounce": {
          "type": "boolean",
          "default": true,
          "description": "Debounce rapid events"
        },
        "includeData": {
          "type": "boolean",
          "default": true,
          "description": "Include event data (false = just event type)"
        }
      }
    }
  }
}
```

### 3.2 Long-Running Operation Support

For operations that take time (e.g., file search, async script execution).

**Request with Progress:**
```json
{
  "type": "fileSearch",
  "requestId": "search-001",
  "query": "*.ts",
  "streaming": true
}
```

**Progress Updates:**
```json
{
  "type": "progress",
  "requestId": "search-001",
  "progress": 45,
  "message": "Searched 450/1000 files",
  "partial": {
    "files": [
      { "path": "/src/main.ts", "name": "main.ts" },
      { "path": "/src/lib.ts", "name": "lib.ts" }
    ]
  }
}
```

**Completion:**
```json
{
  "type": "fileSearchResult",
  "requestId": "search-001",
  "files": [...],
  "complete": true,
  "totalSearched": 1000,
  "elapsed": 2340
}
```

### 3.3 Chunked Large Responses

For responses that exceed token limits.

**Request:**
```json
{
  "type": "getElements",
  "requestId": "elem-001",
  "limit": 1000,
  "chunked": true,
  "chunkSize": 100
}
```

**Chunked Responses:**
```json
{
  "type": "elementsResult",
  "requestId": "elem-001",
  "chunk": 1,
  "totalChunks": 10,
  "elements": [...],
  "hasMore": true
}
```

---

## Phase 4: Semantic Element Identifiers

**Priority:** P2 - Medium  
**Estimated Effort:** 2 weeks

Stable, meaningful identifiers for UI elements that persist across renders.

### 4.1 Semantic ID Format

**Pattern:** `{type}:{qualifier}:{value}`

| Element Type | ID Pattern | Example |
|--------------|------------|---------|
| Choice | `choice:{index}:{value}` | `choice:0:apple` |
| Input | `input:{name}` | `input:filter` |
| Button | `button:{action}` | `button:submit` |
| Action | `action:{shortcut}` | `action:cmd+k` |
| Panel | `panel:{position}` | `panel:preview` |
| Header | `header:{promptId}` | `header:arg-001` |
| List | `list:{name}` | `list:choices` |
| Window | `window:{name}` | `window:main` |

### 4.2 Target by Semantic ID

Use semantic IDs for targeting actions.

**Request:**
```json
{
  "type": "action",
  "requestId": "act-001",
  "target": "choice:2:banana",
  "action": "select"
}
```

**Request:**
```json
{
  "type": "action",
  "requestId": "act-002", 
  "target": "input:filter",
  "action": "setValue",
  "value": "apple"
}
```

**JSON Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/action.schema.json",
  "title": "Action Request",
  "type": "object",
  "required": ["type", "requestId", "target", "action"],
  "properties": {
    "type": { "const": "action" },
    "requestId": { "type": "string" },
    "target": {
      "type": "string",
      "pattern": "^[a-z]+:[^:]+(?::[^:]+)?$",
      "description": "Semantic ID of target element"
    },
    "action": {
      "type": "string",
      "enum": [
        "select", "submit", "focus", "blur", "click",
        "setValue", "appendValue", "clearValue",
        "scrollTo", "expand", "collapse"
      ]
    },
    "value": {
      "description": "Action-specific value"
    }
  }
}
```

### 4.3 ID Stability Guarantees

| Stability Level | Guarantee | Examples |
|----------------|-----------|----------|
| **Stable** | Same across sessions | `window:main`, `input:filter` |
| **Prompt-stable** | Same within prompt lifetime | `choice:0:apple` (value-based) |
| **Render-stable** | Same within render cycle | `choice:5` (index-based) |

---

## Phase 5: Agent Handshake Protocol

**Priority:** P2 - Medium  
**Estimated Effort:** 1 week

Capability negotiation for graceful degradation.

### 5.1 Handshake Flow

**Agent Hello:**
```json
{
  "type": "agentHello",
  "requestId": "hello-001",
  "agent": {
    "name": "claude-assistant",
    "version": "1.0.0",
    "capabilities": [
      "streaming",
      "batch",
      "semanticIds",
      "accessibilityTree"
    ]
  },
  "protocol": {
    "version": "2.0",
    "minVersion": "1.5"
  }
}
```

**App Response:**
```json
{
  "type": "agentWelcome",
  "requestId": "hello-001",
  "app": {
    "name": "script-kit-gpui",
    "version": "0.1.0"
  },
  "protocol": {
    "version": "2.0",
    "supported": ["1.0", "1.5", "2.0"]
  },
  "capabilities": {
    "available": [
      "streaming",
      "batch",
      "semanticIds",
      "getState",
      "getElements"
    ],
    "unavailable": [
      { "name": "accessibilityTree", "reason": "Not implemented yet" }
    ]
  },
  "limits": {
    "maxBatchSize": 100,
    "maxSubscriptions": 10,
    "maxResponseSize": 1048576
  }
}
```

**JSON Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/agentHello.schema.json",
  "title": "Agent Hello Request",
  "type": "object",
  "required": ["type", "requestId", "agent", "protocol"],
  "properties": {
    "type": { "const": "agentHello" },
    "requestId": { "type": "string" },
    "agent": {
      "type": "object",
      "required": ["name"],
      "properties": {
        "name": { "type": "string" },
        "version": { "type": "string" },
        "capabilities": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    },
    "protocol": {
      "type": "object",
      "required": ["version"],
      "properties": {
        "version": { "type": "string" },
        "minVersion": { "type": "string" }
      }
    }
  }
}
```

### 5.2 Capability Discovery

Query available capabilities at runtime.

**Request:**
```json
{
  "type": "getCapabilities",
  "requestId": "cap-001"
}
```

**Response:**
```json
{
  "type": "capabilitiesResult",
  "requestId": "cap-001",
  "capabilities": {
    "core": {
      "arg": true,
      "div": true,
      "editor": true,
      "submit": true
    },
    "extended": {
      "getState": { "available": true, "version": "2.0" },
      "getElements": { "available": true, "version": "2.0" },
      "batch": { "available": true, "maxSize": 100 },
      "streaming": { "available": true, "maxSubscriptions": 10 },
      "accessibilityTree": { "available": false, "eta": "v0.2.0" }
    }
  }
}
```

---

## Phase 6: Headless Mode Improvements

**Priority:** P2 - Medium  
**Estimated Effort:** 2 weeks

Optimize for CI/CD and automated testing scenarios.

### 6.1 Headless Configuration

**Request:**
```json
{
  "type": "configureHeadless",
  "requestId": "hl-001",
  "options": {
    "renderMode": "virtual",
    "windowVisible": false,
    "captureScreenshots": false,
    "mockUserInput": true,
    "acceleratedTime": true
  }
}
```

### 6.2 Virtual DOM Mode

Skip rendering but maintain state for testing.

**Request:**
```json
{
  "type": "getVirtualDOM",
  "requestId": "vdom-001",
  "format": "json"
}
```

**Response:**
```json
{
  "type": "virtualDOMResult",
  "requestId": "vdom-001",
  "dom": {
    "tag": "div",
    "class": "prompt-container",
    "children": [
      {
        "tag": "input",
        "id": "filter-input",
        "value": "apple",
        "focused": true
      },
      {
        "tag": "ul",
        "class": "choice-list",
        "children": [...]
      }
    ]
  }
}
```

### 6.3 Test Harness Integration

Built-in support for test frameworks.

**Request:**
```json
{
  "type": "testMode",
  "requestId": "test-001",
  "enable": true,
  "options": {
    "recordActions": true,
    "assertionMode": true,
    "mockTime": true,
    "seed": 12345
  }
}
```

**Assertion Support:**
```json
{
  "type": "assert",
  "requestId": "assert-001",
  "assertions": [
    { "type": "elementExists", "target": "choice:0:apple" },
    { "type": "stateEquals", "path": "selectedIndex", "value": 0 },
    { "type": "inputValue", "target": "input:filter", "value": "app" }
  ]
}
```

**Response:**
```json
{
  "type": "assertResult",
  "requestId": "assert-001",
  "passed": true,
  "results": [
    { "assertion": "elementExists", "passed": true },
    { "assertion": "stateEquals", "passed": true },
    { "assertion": "inputValue", "passed": true }
  ]
}
```

**JSON Schema:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://scriptkit.com/protocol/assert.schema.json",
  "title": "Assert Request",
  "type": "object",
  "required": ["type", "requestId", "assertions"],
  "properties": {
    "type": { "const": "assert" },
    "requestId": { "type": "string" },
    "assertions": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["type"],
        "properties": {
          "type": {
            "type": "string",
            "enum": [
              "elementExists", "elementNotExists",
              "stateEquals", "stateContains",
              "inputValue", "selectedValue",
              "choiceCount", "windowVisible"
            ]
          },
          "target": { "type": "string" },
          "path": { "type": "string" },
          "value": {},
          "message": { "type": "string" }
        }
      }
    }
  }
}
```

---

## Implementation Priority Matrix

| Phase | Feature | Effort | Impact | Priority | Dependencies |
|-------|---------|--------|--------|----------|--------------|
| 1 | getState | S | High | P0 | None |
| 1 | getElements | M | High | P0 | Semantic IDs |
| 1 | getAccessibilityTree | L | Medium | P1 | getElements |
| 2 | batch | M | High | P1 | None |
| 2 | waitFor | S | High | P1 | getState |
| 2 | atomicActions | S | Medium | P2 | batch |
| 3 | subscribe | M | High | P1 | None |
| 3 | progress | S | Medium | P2 | None |
| 3 | chunked | M | Low | P3 | None |
| 4 | semanticIds | M | High | P0 | None |
| 4 | actionByTarget | S | High | P1 | semanticIds |
| 5 | handshake | S | Medium | P2 | None |
| 5 | capabilities | S | Low | P3 | handshake |
| 6 | headless | M | Medium | P2 | None |
| 6 | virtualDOM | L | Low | P3 | headless |
| 6 | assert | M | Medium | P2 | headless |

**Effort Key:** S = Small (1-3 days), M = Medium (1-2 weeks), L = Large (2-4 weeks)

---

## Migration Guide

### Phase 1 Migration

**Before (Current):**
```javascript
// No way to query state - must track externally
let currentSelection = null;
await arg("Pick fruit", choices);
```

**After:**
```javascript
// Query state any time
const state = await sendMessage({ type: "getState", requestId: "1" });
console.log(`Selected: ${state.selectedValue}`);
```

### Phase 2 Migration

**Before (Current):**
```javascript
// Multiple round-trips
await sendMessage({ type: "setFilter", text: "apple" });
await delay(100); // Hope it's ready
await sendMessage({ type: "navigate", direction: "down" });
await sendMessage({ type: "submit" });
```

**After:**
```javascript
// Single atomic operation
await sendMessage({
  type: "batch",
  requestId: "1",
  commands: [
    { type: "setFilter", text: "apple" },
    { type: "waitFor", condition: "choicesRendered" },
    { type: "navigate", direction: "down" },
    { type: "submit" }
  ]
});
```

### Phase 4 Migration

**Before:**
```javascript
// Fragile index-based targeting
await sendMessage({ type: "selectIndex", index: 2 });
// Breaks if list order changes!
```

**After:**
```javascript
// Stable semantic targeting
await sendMessage({
  type: "action",
  target: "choice:*:apple", // Value-based, order-independent
  action: "select"
});
```

---

## Backward Compatibility

All extensions follow these rules:

1. **New message types** - Unknown types are ignored (per existing `parse_message_graceful`)
2. **New fields** - Extra fields are ignored via `#[serde(skip_unknown)]`
3. **Version negotiation** - Handshake allows capability discovery
4. **Graceful degradation** - Missing capabilities return clear errors

**Example Version Check:**
```json
{
  "type": "getState",
  "requestId": "1"
}
```

**If unsupported:**
```json
{
  "type": "error",
  "requestId": "1",
  "code": "UNSUPPORTED_MESSAGE_TYPE",
  "message": "getState requires protocol version 2.0+",
  "suggestion": "Use agentHello to check supported features"
}
```

---

## Appendix: Complete JSON Schema Index

| Schema | URL |
|--------|-----|
| getState | `https://scriptkit.com/protocol/getState.schema.json` |
| stateResult | `https://scriptkit.com/protocol/stateResult.schema.json` |
| getElements | `https://scriptkit.com/protocol/getElements.schema.json` |
| elementsResult | `https://scriptkit.com/protocol/elementsResult.schema.json` |
| getAccessibilityTree | `https://scriptkit.com/protocol/getAccessibilityTree.schema.json` |
| accessibilityTreeResult | `https://scriptkit.com/protocol/accessibilityTreeResult.schema.json` |
| batch | `https://scriptkit.com/protocol/batch.schema.json` |
| batchResult | `https://scriptkit.com/protocol/batchResult.schema.json` |
| waitFor | `https://scriptkit.com/protocol/waitFor.schema.json` |
| subscribe | `https://scriptkit.com/protocol/subscribe.schema.json` |
| stream | `https://scriptkit.com/protocol/stream.schema.json` |
| action | `https://scriptkit.com/protocol/action.schema.json` |
| agentHello | `https://scriptkit.com/protocol/agentHello.schema.json` |
| agentWelcome | `https://scriptkit.com/protocol/agentWelcome.schema.json` |
| assert | `https://scriptkit.com/protocol/assert.schema.json` |

---

## References

- [Current Protocol Implementation](src/protocol.rs)
- [GPUI Framework](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- [Script Kit Documentation](https://github.com/johnlindquist/kit)
- [W3C ARIA Accessibility Tree](https://www.w3.org/TR/wai-aria-1.2/)
