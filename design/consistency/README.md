# Script Kit consistency proposals

`design/consistency/**` is a local, no-build review explorer for the 75 tasks in
`.notes/CONSISTENCY-FIXES.md`.

## Open it

From the repository root:

```sh
python3 -m http.server 4173
```

Then open:

```text
http://127.0.0.1:4173/design/consistency/
```

## Truth model

- **Current · source-derived** is a deterministic emulation of current source or
  contract behavior. It is not a native screenshot unless explicitly linked as
  an unmodified fixture.
- **Proposal · not implemented** is the behavior and visual contract under
  review. It is not shipped product behavior.
- Existing files under `design/mockups/screens/**` are referenced as immutable
  baselines. The explorer never mutates those iframes or generated tokens.
- Browser projection does not prove native GPUI/AppKit pixels, focus, AX, glass,
  or runtime behavior. Proof-gated tasks say so in their evidence rail.

## Decisions

Approve / Revise / Reject controls start unset. Decisions and notes remain in
local storage under `script-kit.consistency.review.v1`. The index can export a
JSON receipt or reset the local review.

## Architecture

- `index.html` — group overview and review progress
- `groups/*.html` — thin entries for the 12 Oracle-audited groups
- `data/groups.json` — checked-in 12-group / 75-task manifest
- `shared/explorer.js` — shared renderer, compare controls, decisions, export
- `shared/explorer.css` — review chrome and token-backed product fragments
- `tests/validate-explorer.mjs` — structural and coverage validation
- `tools-build-data.py` — regenerates the checked-in manifest from the reviewed
  fix ledger; it is not needed to view the explorer

## Scope guardrails

This design pass does not modify product Rust, existing screen fixtures,
generated token outputs, glass values, motion fixtures, thresholds, or proof
envelopes. Product fragments consume generated `--sk-*` tokens; explorer-only
chrome uses `--cx-*` variables.
