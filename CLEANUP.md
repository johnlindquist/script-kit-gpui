# Script Kit GPUI - Codebase Cleanup Plan

> **Generated**: 2025-12-31
> **Analysis**: Comprehensive AI slop detection across documentation, expert bundles, source code, tests, and infrastructure

---

## Executive Summary

| Category | Current | Estimated After | Savings |
|----------|---------|-----------------|---------|
| **Root Markdown** | 12,847 lines | ~3,500 lines | 73% reduction |
| **Expert Bundles** | 1.5+ MB | ~200 KB | 85% reduction |
| **Dead Code** | 169 `#[allow(dead_code)]` | TBD | Code audit needed |
| **Test Files** | Multiple scaffolds/duplicates | Consolidated | ~40% reduction |
| **Disk Space** | ~86 MB in dot dirs | ~30 MB | 65% reduction |

**Total estimated time to clean**: 4-6 hours of focused work (can be parallelized)

---

## Priority 1: Immediate Deletions (Safe, Minimal Review)

These files provide no value and can be deleted immediately.

### 1.1 Root Directory - Code Bundles (2,600 lines)

| File | Lines | Reason |
|------|-------|--------|
| `mcp-server-bundle.md` | 1,200 | AI-generated snapshot, code lives in repo |
| `rust-protocol-bundle.md` | 800 | AI-generated snapshot, stale immediately |
| `typescript-sdk-bundle.md` | 600 | AI-generated snapshot, SDK is canonical |

**Command**:
```bash
rm mcp-server-bundle.md rust-protocol-bundle.md typescript-sdk-bundle.md
```

### 1.2 Duplicate/Empty Files (41 lines)

| File | Issue |
|------|-------|
| `@AGENTS.md` | Just redirects to AGENTS.md, no unique value |

**Command**:
```bash
rm @AGENTS.md
```

### 1.3 Expert Bundles - Empty/Obsolete (3 files)

| File | Size | Issue |
|------|------|-------|
| `expert-bundles/actions-system-review.md` | 2 bytes | **Empty file** |
| `expert-bundles/submit-values-review.md` | 205 KB | Raw console dump, no analysis |
| `expert-bundles/kit-setup-bundle.md` | - | Work complete, no longer needed |

**Command**:
```bash
rm expert-bundles/actions-system-review.md
rm expert-bundles/submit-values-review.md
rm expert-bundles/kit-setup-bundle.md
```

### 1.4 Dot Directories - Obsolete Infrastructure

| Path | Issue |
|------|-------|
| `.beads/` | Superseded by `.hive/` - duplicate task tracking |
| `test-screenshots/` (root) | Empty duplicate of `.test-screenshots/` |
| `.opencode/plugin-disabled/` | Orphaned disabled plugin config |
| `.swarm/worktrees/` | Empty directory, no active worktrees |

**Command**:
```bash
rm -rf .beads/
rm -rf test-screenshots/
rm -rf .opencode/plugin-disabled/
rm -rf .swarm/worktrees/
```

### 1.5 Stray Test Files

| File | Issue |
|------|-------|
| `scripts/demo-arg-div.ts` | Demo file, not a real test |
| `scripts/test-terminal-visual.ts` | Wrong SDK import path |
| `scripts/test-monitor-positioning.ts` | Wrong SDK import path |
| `tests/smoke/simple-exit-test.ts` | Orphaned simple test |
| `tests/smoke/test-xcap-screenshot.ts` | Obsolete xcap dependency |
| `tests/smoke/test-screenshot-scale.ts` | Obsolete screenshot approach |

**Command**:
```bash
rm scripts/demo-arg-div.ts scripts/test-terminal-visual.ts scripts/test-monitor-positioning.ts
rm tests/smoke/simple-exit-test.ts tests/smoke/test-xcap-screenshot.ts tests/smoke/test-screenshot-scale.ts
```

---

## Priority 2: Archive Candidates (Move to docs/archive/)

These documents have historical value but are no longer actively needed in root.

### 2.1 GPUI Research Documents (1,126 lines)

| File | Lines | Reason |
|------|-------|--------|
| `GPUI_RESEARCH.md` | 542 | Historical research, findings in AGENTS.md |
| `GPUI_IMPROVEMENTS_REPORT.md` | 584 | Recommendations already applied |

### 2.2 Audit Syntheses (1,499 lines)

| File | Lines | Reason |
|------|-------|--------|
| `DESIGN_AUDIT.md` | 403 | Point-in-time audit, action items extracted |
| `PERF_AUDIT.md` | 292 | Point-in-time audit |
| `SECURITY_AUDIT.md` | 317 | Point-in-time audit |
| `TESTING_AUDIT.md` | 241 | Overlaps with AGENTS.md Section 14 |
| `UX_AUDIT.md` | 246 | Point-in-time audit |

### 2.3 Completed Implementation Plans (1,532 lines)

| File | Lines | Reason |
|------|-------|--------|
| `PANEL_IMPLEMENTATION.md` | 208 | Complete - code in `src/panel.rs` |
| `POLLING.md` | 492 | Complete - patterns in codebase |
| `SELECTED_TEXT.md` | 832 | Complete - `src/selected_text.rs` exists |

**Command**:
```bash
mkdir -p docs/archive/2025-12

# Move GPUI research
mv GPUI_RESEARCH.md GPUI_IMPROVEMENTS_REPORT.md docs/archive/2025-12/

# Move audits
mv DESIGN_AUDIT.md PERF_AUDIT.md SECURITY_AUDIT.md TESTING_AUDIT.md UX_AUDIT.md docs/archive/2025-12/

# Move completed plans
mv PANEL_IMPLEMENTATION.md POLLING.md SELECTED_TEXT.md docs/archive/2025-12/
```

---

## Priority 3: Consolidation (Merge/Condense Files)

### 3.1 Expert Bundles - Merge Related Topics

#### Clipboard System Bundle
Merge these 2 files into `clipboard-system-bundle.md`:
- `clipboard-review.md`
- `clipboard-history-raycast-parity.md`

#### Main Search Input Bundle
Merge these 3 files into `main-search-input-bundle.md`:
- `main-search-input-fixes.md`
- `search-input-cursor-layout.md`
- `input-lag-performance.md`

#### Actions Dialog Bundle
Merge these 2 files into `actions-dialog-bundle.md`:
- `actions-panel-review.md`
- `actions-resize-ideas-bundle.md`

### 3.2 Expert Bundles - Trim Oversized Files

These files are bloated AI dumps that need severe reduction:

| File | Current | Target | Action |
|------|---------|--------|--------|
| `window-visibility-review.md` | **670 KB** | ~50 KB | Extract key insights only |
| `mcp-schema-review.md` | 218 KB | ~30 KB | Keep schema, remove verbose analysis |
| `process-management-review.md` | 154 KB | ~20 KB | Extract actionable patterns |
| `list-rendering-search-perf.md` | 76 KB | ~15 KB | Keep benchmarks, remove narrative |
| `text-input-interactions-bundle.md` | 76 KB | ~15 KB | Extract interaction patterns |
| `snippets-review.md` | 73 KB | ~15 KB | Keep snippet system overview |

### 3.3 Root Markdown - Condense Verbose Plans

| File | Current | Target | Action |
|------|---------|--------|--------|
| `EDITOR_PLAN.md` | 1,138 lines | ~350 lines | Remove completed phases 1-3 |
| `RAYCAST_PARITY.md` | 1,122 lines | ~500 lines | Remove completed features |
| `TERM_PLAN.md` | 587 lines | ~200 lines | Archive completed, keep TODOs |
| `MENU_PLAN.md` | 386 lines | ~150 lines | Condense to checklist |
| `RESIZE_PLAN.md` | 507 lines | ~200 lines | Remove implementation details |
| `GLOSSARY.md` | 324 lines | ~130 lines | Remove SDK function defs (in JSDoc) |

---

## Priority 4: Code Refactoring

### 4.1 main.rs Decomposition (Critical)

**Current state**: 13,047 lines - **GOD OBJECT**

| Issue | Lines | Recommendation |
|-------|-------|----------------|
| `handle_prompt_message()` | 1,200+ | Extract to `src/prompt_handler.rs` |
| `render_script_list()` | 900 | Extract to `src/script_list.rs` |
| `handle_action()` | 263 | Extract to `src/action_handler.rs` |
| `new()` | 238 | Split initialization into modules |
| Logging calls | 466 | Review for verbose/redundant entries |

**Suggested module extraction**:
```
src/
  app.rs              # Core ScriptKitApp struct + state
  prompt_handler.rs   # handle_prompt_message() + helpers
  script_list.rs      # render_script_list() + item rendering
  action_handler.rs   # handle_action() + action dispatch
  window_ops.rs       # Window positioning, bounds, display logic
```

### 4.2 Dead Code Audit (169 annotations)

Files with highest `#[allow(dead_code)]` counts (review for removal):

| File | Count | Action |
|------|-------|--------|
| `src/main.rs` | 30+ | Audit scaffolded features |
| `src/scripts.rs` | 20+ | Check unused Script fields |
| `src/executor.rs` | 15+ | Review execution variants |
| `src/clipboard_history.rs` | 15+ | Check unused clipboard fields |
| Other files | 80+ | Systematic review needed |

**Audit command**:
```bash
rg '#\[allow\(dead_code\)\]' src/ --count-matches | sort -t: -k2 -nr
```

### 4.3 Duplicate Patterns to Extract

| Pattern | Occurrences | Recommendation |
|---------|-------------|----------------|
| `Command::new("open").spawn()` | 8x in main.rs | Extract `utils::open_path()` |
| `Clipboard::new()` | 8x across files | Extract `utils::get_clipboard()` |
| pbcopy stdin pipe | 3x | Extract `utils::set_clipboard()` |

### 4.4 Stale TODOs (9 found)

Search and address:
```bash
rg "TODO|FIXME|HACK|XXX" src/ --glob '*.rs'
```

Known stale TODOs:
- Settings dialog scaffold
- Parse inputs TODO
- Render header/hint/footer placeholders
- NEW PROMPT TYPES scaffold comments

---

## Priority 5: Test Cleanup

### 5.1 Incomplete Test Scaffolds

These tests exist but test existence, not behavior:

| File | Issue |
|------|-------|
| `tests/autonomous/test-core-prompts.ts` | Has assertions but doesn't test behavior |
| `tests/autonomous/test-form-inputs.ts` | Scaffold only |
| `tests/autonomous/test-system-apis.ts` | Scaffold only |
| `tests/autonomous/test-file-apis.ts` | Scaffold only |
| `tests/autonomous/test-media-apis.ts` | Scaffold only |

**Action**: Either implement real tests or delete scaffolds.

### 5.2 Duplicate Test Patterns

| Pattern | Files | Recommendation |
|---------|-------|----------------|
| `TestResult` interface boilerplate | 7 files | Extract to shared `test-utils.ts` |
| Design gallery variants | 6 files | Consolidate into `test-design-gallery.ts` |
| Tailwind tests | 3 files | Merge into `test-tailwind.ts` |
| Actions panel tests | **10 files!** | Consolidate into `test-actions-panel.ts` |

### 5.3 Unused Test Utilities

| File | Exports | Used |
|------|---------|------|
| `tests/autonomous/screenshot-diff.ts` | ~8 exports | Review usage |
| `tests/autonomous/screenshot-utils.ts` | ~20 exports | Review usage |

**Action**: Audit exports, remove unused functions.

---

## Priority 6: Infrastructure Cleanup

### 6.1 .gitignore Additions

Add these patterns to prevent future accumulation:

```gitignore
# AI-generated bundles (ephemeral)
*-bundle.md

# Package caches
.packx_cache/

# Test artifacts (keep .test-screenshots/ but ignore contents)
.test-screenshots/*.png

# Disabled plugins
.opencode/plugin-disabled/
```

### 6.2 Periodic Cleanup Policy

| Directory | Size | Policy |
|-----------|------|--------|
| `.test-screenshots/` | 32 MB | Auto-delete PNGs older than 7 days |
| `.mocks/` | 54 MB | Review visual baselines quarterly |
| `~/.sk/kit/logs/` | Variable | Rotate logs older than 30 days |

**Suggested cron/script**:
```bash
# Add to development workflow or CI
find .test-screenshots -name "*.png" -mtime +7 -delete
find ~/.sk/kit/logs -name "*.jsonl" -mtime +30 -delete
```

### 6.3 Directory Deduplication

| Keep | Delete | Reason |
|------|--------|--------|
| `.test-screenshots/` | `test-screenshots/` | Standard location |
| `.hive/` | `.beads/` | Current system |
| `AGENTS.md` | `@AGENTS.md` | Primary doc |

---

## Implementation Checklist

### Quick Wins (< 1 hour)
- [ ] Delete code bundles (Priority 1.1)
- [ ] Delete @AGENTS.md (Priority 1.2)
- [ ] Delete empty/obsolete expert bundles (Priority 1.3)
- [ ] Remove obsolete dot directories (Priority 1.4)
- [ ] Delete stray test files (Priority 1.5)
- [ ] Update .gitignore (Priority 6.1)

### Medium Effort (2-3 hours)
- [ ] Archive GPUI research docs (Priority 2.1)
- [ ] Archive audit syntheses (Priority 2.2)
- [ ] Archive completed implementation plans (Priority 2.3)
- [ ] Merge clipboard-related bundles (Priority 3.1)
- [ ] Merge search input bundles (Priority 3.1)
- [ ] Merge actions dialog bundles (Priority 3.1)

### Significant Effort (4+ hours)
- [ ] Trim oversized expert bundles (Priority 3.2)
- [ ] Condense verbose root markdown (Priority 3.3)
- [ ] Extract modules from main.rs (Priority 4.1)
- [ ] Audit dead code annotations (Priority 4.2)
- [ ] Extract duplicate patterns (Priority 4.3)
- [ ] Consolidate test files (Priority 5)

---

## Files to Keep (No Changes Needed)

### Root Documentation
- `AGENTS.md` - Primary AI agent reference (canonical)
- `DEV.md` - Human developer quickstart
- `RELEASE.md` - Release process
- `BUNDLING.md` - Cross-platform builds
- `MCP.md` - MCP integration docs

### docs/ Directory
- `docs/PROTOCOL.md` - JSON protocol spec
- `docs/ROADMAP.md` - Future features

### Expert Bundles (Keep)
- `app-script-communication-bundle.md` - Active reference
- `editor-basics-bundle.md` - Active reference
- `div-scrollable-links-bundle.md` - Active reference
- `env-system-review.md` - Active reference
- `hud-review.md` - Active reference
- `terminal-review.md` - Active reference

---

## Verification Commands

After cleanup, verify health:

```bash
# Check no broken imports
cargo check

# Run tests
cargo test

# Verify docs still build/link correctly
# (if using mdbook or similar)

# Count remaining lines
find . -name "*.md" -not -path "./docs/archive/*" -not -path "./.git/*" | xargs wc -l

# Check .gitignore is effective
git status --ignored
```

---

## Notes for Future Maintenance

1. **Bundle files are ephemeral** - Add to .gitignore, regenerate as needed
2. **Audit syntheses age quickly** - Archive after 30 days or when actioned
3. **Implementation plans should shrink** - Remove completed sections regularly
4. **Test scaffolds are tech debt** - Either implement or delete within 2 weeks
5. **Dead code accumulates** - Run quarterly `#[allow(dead_code)]` audits
6. **Expert bundles bloat easily** - Cap at 50KB per file, split if larger

---

*This cleanup plan was synthesized from comprehensive analysis by W1-W6 worker agents.*
