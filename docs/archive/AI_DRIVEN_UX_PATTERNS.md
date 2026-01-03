# AI-Driven UX Patterns

This document defines patterns and best practices for AI agents interacting with Script Kit GPUI. It covers token-efficient logging, interaction protocols, error handling, and verification strategies.

---

## Table of Contents

1. [AI Compact Log Format](#1-ai-compact-log-format)
2. [Token-Efficient Interaction Patterns](#2-token-efficient-interaction-patterns)
3. [Error Handling Strategies](#3-error-handling-strategies)
4. [Screenshot-Based Feedback Loops](#4-screenshot-based-feedback-loops)
5. [State Verification Patterns](#5-state-verification-patterns)
6. [Common Pitfalls and Solutions](#6-common-pitfalls-and-solutions)
7. [Agent Workflow Decision Trees](#7-agent-workflow-decision-trees)

---

## 1. AI Compact Log Format

### Overview

When `SCRIPT_KIT_AI_LOG=1` is set, the application outputs a token-efficient log format to stderr, reducing log verbosity by ~67% while preserving essential information.

### Format Specification

```
SS.mmm|L|C|message
```

| Field | Description | Example |
|-------|-------------|---------|
| `SS.mmm` | Seconds.milliseconds within current minute | `13.150` |
| `L` | Log level (single character) | `i` |
| `C` | Category code (single character) | `P` |
| `message` | Actual log message | `Selected display origin=(0,0)` |

### Log Levels

| Code | Level | Use Case |
|------|-------|----------|
| `i` | INFO | Normal operations, business events |
| `w` | WARN | Recoverable issues, fallbacks used |
| `e` | ERROR | Failures requiring attention |
| `d` | DEBUG | Development troubleshooting |
| `t` | TRACE | Verbose internal state dumps |

### Category Codes (Complete Reference)

| Code | Category | Description |
|------|----------|-------------|
| `A` | APP | Application lifecycle (startup, shutdown) |
| `C` | CACHE | Caching operations |
| `D` | DESIGN | Design system, visual theming |
| `E` | EXEC | Script/process execution |
| `F` | FOCUS | Focus management, window activation |
| `G` | SCRIPT | Script loading, parsing |
| `H` | HOTKEY | Global hotkey registration/handling |
| `K` | KEY | Keyboard event processing |
| `L` | SCROLL_STATE | Scroll position state changes |
| `M` | MOUSE_HOVER | Mouse hover interactions |
| `N` | CONFIG | Configuration loading/changes |
| `P` | POSITION | Window/element positioning |
| `Q` | SCROLL_PERF | Scroll performance metrics |
| `R` | PERF | General performance timing |
| `S` | STDIN | Stdin message parsing |
| `T` | THEME | Theme loading/application |
| `U` | UI | UI rendering, layout |
| `V` | VISIBILITY | Window show/hide state |
| `W` | WINDOW_MGR | Window manager operations |
| `X` | ERROR | Error conditions (cross-cutting) |
| `Z` | RESIZE | Window resize operations |

### Token Savings Example

```
# Standard format (85 characters):
2025-12-27T15:22:13.150Z INFO script_kit_gpui::logging: Selected display origin=(0,0)

# Compact format (28 characters, 67% reduction):
13.150|i|P|Selected display origin=(0,0)
```

### Enabling AI Log Mode

```bash
# Basic usage
SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1

# With stdin protocol for testing
echo '{"type": "run", "path": "/path/to/test.ts"}' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1

# Filter specific categories
echo '...' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1 | grep '|Z|'  # Resize only
echo '...' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1 | grep '|e|'  # Errors only
```

### Parsing Compact Logs

```bash
# Extract all errors
grep '|e|' output.log

# Extract resize events with timing
grep '|Z|' output.log | awk -F'|' '{print $1, $4}'

# Find slow operations (look for duration in message)
grep -E 'duration.*[0-9]{3,}ms' output.log

# Group by category
awk -F'|' '{count[$3]++} END {for(c in count) print c, count[c]}' output.log
```

---

## 2. Token-Efficient Interaction Patterns

### The Stdin JSON Protocol

**CRITICAL: Never use command-line arguments to run scripts.** The app uses stdin JSON messages:

```bash
# CORRECT - stdin JSON protocol
echo '{"type": "run", "path": "'$(pwd)'/tests/smoke/test.ts"}' | ./target/debug/script-kit-gpui 2>&1

# WRONG - command line args do nothing!
./target/debug/script-kit-gpui tests/smoke/test.ts
```

### Available Commands

| Command | JSON Structure | Purpose |
|---------|---------------|---------|
| Run script | `{"type": "run", "path": "/absolute/path/to/script.ts"}` | Execute a script |
| Show window | `{"type": "show"}` | Make window visible |
| Hide window | `{"type": "hide"}` | Hide window |
| Set filter | `{"type": "setFilter", "text": "search term"}` | Filter list items |

### Minimizing Token Usage

#### 1. Use AI Log Mode (Mandatory)

```bash
# Always set SCRIPT_KIT_AI_LOG=1 when testing
SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1
```

This reduces log tokens by ~70%.

#### 2. Filter Logs to Relevant Categories

```bash
# Focus on specific behavior
echo '...' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1 | grep -E '\|Z\||\|U\|'  # Resize + UI

# Exclude verbose categories
echo '...' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1 | grep -v '|t|'  # No trace
```

#### 3. Use Head/Tail for Large Outputs

```bash
# First 50 lines (startup)
echo '...' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1 | head -50

# Last 50 lines (final state)
echo '...' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1 | tail -50
```

#### 4. Structured Log Queries (JSONL)

Full structured logs are in `~/.sk/kit/logs/script-kit-gpui.jsonl`:

```bash
# Find specific correlation ID
grep '"correlation_id":"abc-123"' ~/.sk/kit/logs/script-kit-gpui.jsonl

# Find slow operations
grep '"duration_ms":' ~/.sk/kit/logs/script-kit-gpui.jsonl | jq 'select(.fields.duration_ms > 100)'

# Recent errors only
tail -100 ~/.sk/kit/logs/script-kit-gpui.jsonl | grep '"level":"ERROR"'
```

### Build-Test-Iterate Loop (Mandatory)

```
┌─────────────────────────────────────────────────────────────┐
│                 BUILD-TEST-ITERATE LOOP                     │
├─────────────────────────────────────────────────────────────┤
│  1. cargo build                                             │
│  2. echo '{"type":"run",...}' | SCRIPT_KIT_AI_LOG=1 ...    │
│  3. Parse logs for expected behavior                        │
│  4. If broken: fix code, goto step 1                        │
│  5. If passing: continue to next change                     │
└─────────────────────────────────────────────────────────────┘
```

**This loop is NON-NEGOTIABLE.** Agents must:
- Never ask users to test
- Never skip testing
- Run tests themselves, read logs, fix issues, repeat

---

## 3. Error Handling Strategies

### Error Categories for Agents

| Category | Examples | Strategy |
|----------|----------|----------|
| **Compilation** | Type errors, borrow checker | Fix code, rebuild |
| **Runtime** | Panics, assertion failures | Check logs for stack trace |
| **Protocol** | Invalid JSON, wrong message type | Validate stdin format |
| **UI State** | Wrong size, missing content | Use visual testing |
| **Integration** | Script execution failure | Check `E` category logs |

### Error Detection in Compact Logs

```bash
# Find all errors
grep '|e|' output.log

# Find errors with context (3 lines before/after)
grep -B3 -A3 '|e|' output.log

# Find error category breakdown
grep '|e|' output.log | awk -F'|' '{print $3}' | sort | uniq -c
```

### Common Error Patterns

#### 1. Script Not Found

```
# Log pattern
|e|G|Script not found: /path/to/script.ts

# Solution
- Verify path is absolute
- Check file exists before running
```

#### 2. JSON Parse Error

```
# Log pattern  
|e|S|Failed to parse stdin: expected `,` or `}`

# Solution
- Validate JSON structure
- Use single quotes around JSON
- Escape special characters
```

#### 3. Window Positioning Failed

```
# Log pattern
|w|P|No display found for mouse position

# Solution
- Check display availability
- Use fallback to primary display
```

### Error Recovery Protocol

```
┌──────────────────────────────────────────────────────────┐
│                   ERROR RECOVERY FLOW                     │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  1. Detect error in logs (|e| pattern)                   │
│                 │                                        │
│                 ▼                                        │
│  2. Identify error category (X, S, E, etc.)              │
│                 │                                        │
│                 ▼                                        │
│  3. Check 5 lines before error for context               │
│                 │                                        │
│                 ▼                                        │
│  4. Apply category-specific fix:                         │
│     • X (ERROR): Check error message                     │
│     • S (STDIN): Validate JSON format                    │
│     • E (EXEC): Check script path/content                │
│     • G (SCRIPT): Verify script exists                   │
│                 │                                        │
│                 ▼                                        │
│  5. Rebuild and retest                                   │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

---

## 4. Screenshot-Based Feedback Loops

### When to Use Visual Testing

| Scenario | Use Visual Testing? |
|----------|---------------------|
| Layout issues (wrong sizes) | Yes |
| Styling problems (colors, borders) | Yes |
| Content visibility | Yes |
| Component positioning | Yes |
| State changes (selection, focus) | Logs first, then visual if unclear |
| Performance issues | No (use logs) |
| Data flow | No (use logs) |

### Visual Test Script

```bash
# Run visual test - captures screenshot after N seconds
./scripts/visual-test.sh tests/smoke/test-editor-height.ts 3

# Output files:
# .test-screenshots/test-editor-height-20251227-012813.png
# .test-screenshots/test-editor-height-20251227-012813.log
```

### Visual Test Workflow

```
┌─────────────────────────────────────────────────────────────┐
│                  VISUAL TEST WORKFLOW                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Run visual test script                                  │
│     ./scripts/visual-test.sh <test.ts> <seconds>            │
│                     │                                       │
│                     ▼                                       │
│  2. Script builds project                                   │
│                     │                                       │
│                     ▼                                       │
│  3. Launches app with stdin JSON                            │
│                     │                                       │
│                     ▼                                       │
│  4. Waits N seconds for render                              │
│                     │                                       │
│                     ▼                                       │
│  5. Captures screenshot (macOS screencapture)               │
│                     │                                       │
│                     ▼                                       │
│  6. Terminates app                                          │
│                     │                                       │
│                     ▼                                       │
│  7. Agent reads screenshot to analyze UI state              │
│                     │                                       │
│                     ▼                                       │
│  8. Compare actual vs expected                              │
│     ├── Match: Continue                                     │
│     └── Mismatch: Fix code, repeat from step 1              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### SDK Screenshot Capture (In-Script)

```typescript
import '../../scripts/kit-sdk';

// Capture screenshot programmatically
const screenshot = await captureScreenshot();
console.error(`Screenshot: ${screenshot.width}x${screenshot.height}`);
// screenshot.data contains base64-encoded PNG
```

### Screenshot Analysis Utilities

```typescript
import { saveScreenshot, analyzeContentFill, generateReport } from './screenshot-utils';

// Save to .test-screenshots/
const path = await saveScreenshot(screenshot.data, 'my-test');

// Analyze content fill
const analysis = await analyzeContentFill(path, expectedHeight);
if (!analysis.pass) {
  console.error(`Visual check failed: ${analysis.message}`);
}
```

### Example: Debugging Editor Height

```bash
# 1. Run visual test
./scripts/visual-test.sh tests/smoke/test-editor-height.ts 3

# 2. Check screenshot for visual correctness
# (Agent reads the PNG file)

# 3. Check log for height values
grep -E 'height|700|resize' .test-screenshots/test-editor-height-*.log

# 4. Expected log patterns:
# |i|Z|height_for_view(EditorPrompt) = 700
# |i|Z|Resize: 501 -> 700

# 5. If editor too small, fix layout code
# 6. Repeat until screenshot shows correct layout
```

---

## 5. State Verification Patterns

### Verification Categories

| Category | What to Check | How to Verify |
|----------|---------------|---------------|
| **Build State** | Compilation | `cargo check` |
| **Code Quality** | Lints, patterns | `cargo clippy --all-targets -- -D warnings` |
| **Test State** | Unit/integration | `cargo test` |
| **UI State** | Layout, styling | Visual testing + logs |
| **Runtime State** | Focus, selection | Compact logs (F, K categories) |

### Pre-Commit Verification Gate

```bash
# MANDATORY before every commit
cargo check && cargo clippy --all-targets -- -D warnings && cargo test
```

| Check | Purpose | On Failure |
|-------|---------|------------|
| `cargo check` | Type errors, borrow checker | Fix compilation errors |
| `cargo clippy` | Lints, anti-patterns | Address warnings |
| `cargo test` | Unit + integration tests | Fix failing tests |

### Focus State Verification

Look for these patterns in compact logs:

```bash
# Focus gained
grep '|F|.*focus' output.log
# Example: |i|F|focus_handle.is_focused=true

# Focus lost
grep '|F|.*unfocus\|blur' output.log
```

### Window State Verification

```bash
# Window visibility
grep '|V|' output.log
# Examples:
# |i|V|Window shown
# |i|V|Window hidden

# Window positioning
grep '|P|' output.log
# Examples:
# |i|P|Selected display origin=(0,0)
# |i|P|Window centered at (500, 300)

# Window resize
grep '|Z|' output.log
# Examples:
# |i|Z|Resize: 501 -> 700
# |i|Z|height_for_view(EditorPrompt) = 700
```

### Selection State Verification

```bash
# Selection changes
grep '|U|.*select' output.log
# Example: |d|U|Selection changed: 0 -> 1

# Keyboard navigation
grep '|K|' output.log
# Examples:
# |d|K|Key: down (ArrowDown)
# |d|K|Key: enter
```

### State Verification Decision Tree

```
┌─────────────────────────────────────────────────────────────┐
│              STATE VERIFICATION DECISION TREE               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Q: What state am I verifying?                              │
│                     │                                       │
│     ┌───────────────┼───────────────┐                       │
│     ▼               ▼               ▼                       │
│  [Build]        [Runtime]       [Visual]                    │
│     │               │               │                       │
│     ▼               ▼               ▼                       │
│  cargo check    Read logs      Screenshot                   │
│  cargo clippy   (|F|,|K|,|V|)  visual-test.sh              │
│  cargo test         │               │                       │
│     │               │               │                       │
│     ▼               ▼               ▼                       │
│  Fix errors     Verify          Compare to                  │
│  if found       expected        expected                    │
│                 patterns        layout                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. Common Pitfalls and Solutions

### Critical Pitfalls Table

| Pitfall | Why It Happens | Solution |
|---------|----------------|----------|
| **Command-line args for scripts** | Habit from other CLIs | Use stdin JSON: `echo '{"type":"run",...}' \| ...` |
| **Missing SCRIPT_KIT_AI_LOG=1** | Forgot env var | Always include in test commands |
| **Relative paths in run command** | Using `./` or `../` | Use absolute paths with `$(pwd)` |
| **Wrong arrow key names** | Platform differences | Match BOTH: `"up" \| "arrowup"` |
| **Skip verification gate** | "Small change" mindset | ALWAYS run check/clippy/test |
| **Guessing at UI issues** | Can't see the window | Use visual testing |
| **Hardcoded colors** | Copy-paste from examples | Use `theme.colors.*` |
| **Missing cx.notify()** | Forgot state update | Always call after state changes |
| **Ask user to test** | Seems polite | YOU must test via stdin protocol |
| **Using hive_close()** | Seems logical | Use swarm_complete() instead |

### Arrow Key Handling (Platform Gotcha)

```rust
// CORRECT - handles both key name variants
match key.as_str() {
    "up" | "arrowup" => self.move_up(),
    "down" | "arrowdown" => self.move_down(),
    "left" | "arrowleft" => self.move_left(),
    "right" | "arrowright" => self.move_right(),
    _ => {}
}

// WRONG - only handles one variant (will break on some platforms)
match key.as_str() {
    "arrowup" => self.move_up(),    // BROKEN on macOS
    "arrowdown" => self.move_down(), // BROKEN on macOS
    _ => {}
}
```

### Theme Color Usage (Anti-Pattern Prevention)

```rust
// CORRECT - uses theme system
div()
    .bg(rgb(colors.background.main))
    .border_color(rgb(colors.ui.border))
    .text_color(rgb(colors.text.primary))

// WRONG - hardcoded colors (breaks theming)
div()
    .bg(rgb(0x2d2d2d))           // Hardcoded!
    .border_color(rgb(0x3d3d3d)) // Hardcoded!
    .text_color(rgb(0x888888))   // Hardcoded!
```

### State Update Pattern

```rust
// CORRECT - always notify after state change
fn set_selection(&mut self, index: usize, cx: &mut Context<Self>) {
    self.selected_index = index;
    cx.notify();  // REQUIRED - triggers re-render
}

// WRONG - missing notify (UI won't update)
fn set_selection(&mut self, index: usize, _cx: &mut Context<Self>) {
    self.selected_index = index;
    // Missing cx.notify() - UI is now stale!
}
```

### Testing Anti-Patterns

| Wrong | Right |
|-------|-------|
| `./target/debug/script-kit-gpui test.ts` | `echo '{"type":"run",...}' \| ./target/debug/script-kit-gpui` |
| "I can't test this without manual interaction" | Use stdin protocol, add logging, verify in output |
| "The user should test this" | YOU must test it using the stdin protocol |
| Committing without running tests | Run `cargo build && echo '...' \| ./target/debug/...` |
| "I can't see what the UI looks like" | Use `./scripts/visual-test.sh` to capture screenshots |
| Guessing at layout issues | Capture screenshot, read it, analyze actual vs expected |
| Running without `SCRIPT_KIT_AI_LOG=1` | ALWAYS use AI log mode to save tokens |

---

## 7. Agent Workflow Decision Trees

### Master Decision Tree

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      AGENT WORKFLOW DECISION TREE                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  START: Received Task                                                   │
│           │                                                             │
│           ▼                                                             │
│  ┌────────────────┐                                                     │
│  │ 1. swarmmail   │                                                     │
│  │    _init()     │◄──── MANDATORY FIRST STEP                           │
│  └───────┬────────┘                                                     │
│          │                                                              │
│          ▼                                                              │
│  ┌────────────────┐                                                     │
│  │ 2. Query       │                                                     │
│  │ semantic memory│◄──── Check for past learnings                       │
│  └───────┬────────┘                                                     │
│          │                                                              │
│          ▼                                                              │
│  ┌────────────────┐                                                     │
│  │ 3. Load skills │                                                     │
│  │ skills_list()  │◄──── Get relevant skill knowledge                   │
│  └───────┬────────┘                                                     │
│          │                                                              │
│          ▼                                                              │
│  ┌────────────────┐                                                     │
│  │ 4. Reserve     │                                                     │
│  │    files       │◄──── swarmmail_reserve() BEFORE editing             │
│  └───────┬────────┘                                                     │
│          │                                                              │
│          ▼                                                              │
│  ┌────────────────┐     ┌────────────────┐                              │
│  │ 5. Read code   │────►│ 6. Write test  │                              │
│  │    first       │     │    (TDD)       │                              │
│  └────────────────┘     └───────┬────────┘                              │
│                                 │                                       │
│                                 ▼                                       │
│                         ┌────────────────┐                              │
│                         │ 7. Implement   │                              │
│                         │    feature     │                              │
│                         └───────┬────────┘                              │
│                                 │                                       │
│                                 ▼                                       │
│                         ┌────────────────┐     No                       │
│                         │ 8. Verify:     │────────┐                     │
│                         │ check/clippy/  │        │                     │
│                         │ test           │        │                     │
│                         └───────┬────────┘        │                     │
│                                 │ Yes             │                     │
│                                 ▼                 │                     │
│                         ┌────────────────┐        │                     │
│                         │ 9. UI change?  │────────┼──No──►┌────────┐    │
│                         └───────┬────────┘        │       │ Skip   │    │
│                                 │ Yes             │       │ visual │    │
│                                 ▼                 │       └───┬────┘    │
│                         ┌────────────────┐        │           │         │
│                         │ 10. Visual     │        │           │         │
│                         │     test       │        │           │         │
│                         └───────┬────────┘        │           │         │
│                                 │                 │           │         │
│                                 ▼                 │           │         │
│                         ┌────────────────┐        │           │         │
│                         │ 11. Progress   │◄───────┘◄──────────┘         │
│                         │  (25/50/75%)   │                              │
│                         └───────┬────────┘                              │
│                                 │                                       │
│                                 ▼                                       │
│                   ┌─────────────┴─────────────┐                         │
│                   │       More work?          │                         │
│                   └─────────────┬─────────────┘                         │
│                        Yes │         │ No                               │
│                            ▼         ▼                                  │
│                    ┌───────────┐ ┌────────────────┐                     │
│                    │ Loop to   │ │ 12. swarm_     │                     │
│                    │ step 5    │ │     complete() │◄── NOT hive_close() │
│                    └───────────┘ └────────────────┘                     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Test Type Decision Tree

```
┌─────────────────────────────────────────────────────────────┐
│                 TEST TYPE DECISION TREE                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Q: What kind of change did I make?                         │
│                     │                                       │
│     ┌───────────────┼───────────────┬───────────────┐       │
│     ▼               ▼               ▼               ▼       │
│  [Rust code]    [UI layout]    [SDK method]    [Protocol]   │
│     │               │               │               │       │
│     ▼               ▼               ▼               ▼       │
│  cargo test     visual-test     stdin JSON      stdin JSON  │
│     +           + logs (|Z|)    protocol        parse test  │
│  clippy             │               │               │       │
│     │               │               │               │       │
│     ▼               ▼               ▼               ▼       │
│  [Pass?]──No──►Fix [Match?]──No──►Fix [Works?]──No──►Fix    │
│     │Yes            │Yes            │Yes                    │
│     ▼               ▼               ▼                       │
│  Continue       Continue        Continue                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Error Recovery Decision Tree

```
┌─────────────────────────────────────────────────────────────┐
│               ERROR RECOVERY DECISION TREE                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Error detected in logs (|e| pattern)                       │
│                     │                                       │
│                     ▼                                       │
│         What category code?                                 │
│                     │                                       │
│     ┌───────┬───────┼───────┬───────┐                       │
│     ▼       ▼       ▼       ▼       ▼                       │
│    [S]     [E]     [G]     [X]    [other]                   │
│  STDIN    EXEC   SCRIPT  ERROR                              │
│     │       │       │       │       │                       │
│     ▼       ▼       ▼       ▼       ▼                       │
│  Validate  Check   Verify  Read    Check                    │
│  JSON      path    file    full    context                  │
│  format    exists  exists  message (5 lines)                │
│     │       │       │       │       │                       │
│     ▼       ▼       ▼       ▼       ▼                       │
│  Fix JSON  Fix     Fix     Apply   Apply                    │
│  structure path    path    fix     generic                  │
│            or      or              fix                      │
│            perms   content                                  │
│     │       │       │       │       │                       │
│     └───────┴───────┴───────┴───────┘                       │
│                     │                                       │
│                     ▼                                       │
│              Rebuild & Retest                               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Blocked State Decision Tree

```
┌─────────────────────────────────────────────────────────────┐
│                BLOCKED STATE DECISION TREE                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  I am blocked on something                                  │
│                     │                                       │
│                     ▼                                       │
│  Can I resolve it myself?                                   │
│                     │                                       │
│         ┌──────────┴──────────┐                             │
│         │ Yes                 │ No                          │
│         ▼                     ▼                             │
│  ┌──────────────┐    ┌──────────────────┐                   │
│  │ Resolve it   │    │ Report to        │                   │
│  │ Continue     │    │ coordinator:     │                   │
│  │ working      │    │                  │                   │
│  └──────────────┘    │ swarmmail_send(  │                   │
│                      │   to:["coord"],  │                   │
│                      │   subject:       │                   │
│                      │    "BLOCKED:...",│                   │
│                      │   importance:    │                   │
│                      │    "high"        │                   │
│                      │ )                │                   │
│                      │                  │                   │
│                      │ hive_update(     │                   │
│                      │   status:        │                   │
│                      │    "blocked"     │                   │
│                      │ )                │                   │
│                      └────────┬─────────┘                   │
│                               │                             │
│                               ▼                             │
│                      Wait for coordinator                   │
│                      response before                        │
│                      continuing                             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Scope Change Decision Tree

```
┌─────────────────────────────────────────────────────────────┐
│                SCOPE CHANGE DECISION TREE                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  I discovered additional work is needed                     │
│                     │                                       │
│                     ▼                                       │
│  Is it within my reserved files?                            │
│                     │                                       │
│         ┌──────────┴──────────┐                             │
│         │ Yes                 │ No                          │
│         ▼                     ▼                             │
│  ┌──────────────┐    ┌──────────────────┐                   │
│  │ Proceed with │    │ Request scope    │                   │
│  │ additional   │    │ change:          │                   │
│  │ work         │    │                  │                   │
│  └──────────────┘    │ swarmmail_send(  │                   │
│                      │   to:["coord"],  │                   │
│                      │   subject:       │                   │
│                      │    "Scope change │                   │
│                      │     request:..." │                   │
│                      │   importance:    │                   │
│                      │    "high"        │                   │
│                      │ )                │                   │
│                      └────────┬─────────┘                   │
│                               │                             │
│                               ▼                             │
│                      Wait for approval                      │
│                      before expanding                       │
│                      beyond files_owned                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Quick Reference Card

### Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `SCRIPT_KIT_AI_LOG=1` | Enable compact log format | `SCRIPT_KIT_AI_LOG=1 ./target/debug/...` |
| `RUST_LOG` | Filter log levels/modules | `RUST_LOG=script_kit::ui=debug` |

### Essential Commands

```bash
# Build
cargo build

# Verify (pre-commit)
cargo check && cargo clippy --all-targets -- -D warnings && cargo test

# Run test script
echo '{"type": "run", "path": "'$(pwd)'/tests/smoke/test.ts"}' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1

# Visual test
./scripts/visual-test.sh tests/smoke/test.ts 3

# Filter logs by category
grep '|Z|' output.log  # Resize
grep '|e|' output.log  # Errors
grep '|F|' output.log  # Focus
```

### Log File Locations

| File | Format | Purpose |
|------|--------|---------|
| stderr | Compact (`SS.mmm\|L\|C\|msg`) | Real-time debugging |
| `~/.sk/kit/logs/script-kit-gpui.jsonl` | JSONL | Structured log analysis |

### Mandatory Workflow

1. `swarmmail_init()` - Initialize session
2. `swarmmail_reserve()` - Reserve files
3. Build-Test-Iterate loop
4. Report progress at 25/50/75%
5. `swarm_complete()` - Close task (NOT `hive_close()`)
