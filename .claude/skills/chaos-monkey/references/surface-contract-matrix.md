# Surface Contract Matrix — the affordance-parity oracle

**Reference surface: the main menu (Script List).** The main menu establishes
the affordance grammar (user contract, 2026-07-20): every main-window surface
either matches it or carries a **ratified** divergence entry below. A
divergence without a ratification entry is a finding, not a decision — no
matter how deliberate the code comment sounds.

This file is data for the affordance-parity lens (SKILL.md Phase 2) and a
story-harvest source (Phase 1). Update it in the same change that alters any
surface's footer/chord behavior; the divergence-ratification rule (Phase 3)
makes silent drift a red.

## Universal footer affordance contract

Every main-window surface's native footer must expose:

1. **Primary action** — ↵ or ⌘↩, truthfully labeled, dispatching real behavior
   (never a decorative label).
2. **Actions (⌘K)** — the contextual actions menu, unless a ratified
   divergence says otherwise.
3. **Dismiss affordance** — Esc, labeled for what it truthfully does on that
   surface (Cancel / Back / Close).

Verification: enumerate the surface's actual footer buttons via
`getElements`/the automation surface and diff against the table — presence,
keycap, label truthfulness, enabled-state coherence.

## Overlay symmetry contract

For EVERY chord that opens an overlay/popup/menu on a surface (⌘K actions,
pickers, dropdowns, confirm dialogs):

- the **same chord toggles it closed** (open → chord → closed, idempotent);
- **Escape closes it**, consuming exactly one rung of the escape ladder;
- **focus returns** to the pre-open owner (typing works immediately);
- the **underlying footer/chrome survives** beneath and after the overlay;
- the opener's footer button (if any) reflects open state (`selected`) and
  clears on close.

Run as a probe family: surface × registered chord. Open-path-only coverage is
not coverage (OF-61 shipped because only the open path was ever driven).

## Per-surface table

Legend: ✓ = required + verified in code · ✗! = absent, unratified (finding) ·
✗r = absent, ratified divergence · TBV = required by contract, to be verified
by the next campaign's parity lane.

| Surface | Primary | Actions ⌘K | Dismiss | Divergences / notes |
|---|---|---|---|---|
| Script List (main menu) | ✓ Run ↵ | ✓ | ✓ Esc | REFERENCE surface |
| TermPrompt (script terminal) | ✓ Continue ↵ | ✓ (`selected(actions_open)`) | ✓ Esc Cancel | `term_prompt_footer_buttons`, ui_window.rs — matches contract |
| Quick Terminal | ✓ Apply/Agent ⌘↩ + Close ⌘W | **✗! OF-60** | ⌘W (no Esc keycap) | Code says "Run/AI/Actions are intentionally omitted" (ui_window.rs ~1101) — divergence NEVER ratified; also decide Esc-vs-⌘W dismiss labeling |
| About | Esc Back only | ✗ (pending ratification) | ✓ Esc Back | Info surface; single-affordance footer plausibly fine — needs one explicit ratification line |
| MicroPrompt | ✓ submit ↵ | TBV | TBV | Migrated in OF-57 (`01a9286fd`) |
| SDK Reference | ✓ copy ↵ | TBV | TBV | Migrated in OF-57 |
| Script Template Catalog | ✓ use-template ↵ | TBV | TBV | Migrated in OF-57 |
| Create AI Preset | ✓ | TBV | TBV | Migrated in OF-57 |
| Notes Browse | ✓ | TBV | TBV | Migrated in OF-57 |
| Arg / Select / Mini / Path prompts | ✓ | TBV | ✓ Esc | OF-58C surfaces; parity lane must fill ⌘K cells |
| Form / Env / Drop / Template / Naming prompts | ✓ | TBV | ✓ Esc | OF-58AB surfaces |
| Agent Chat | ✓ send | TBV | ✓ Esc ladder | Footer chips contract (Cwd + Agent·Model); UXC-1 (4 affordances vs "three keys") still awaiting human wording |
| Find (file search) | ✓ open ↵ | TBV | ✓ Esc | |
| Clipboard History | ✓ paste ↵ | TBV | ✓ Esc | |
| Dictation History | ✓ | TBV | ✓ Esc | |
| Theme Chooser | ✓ apply ↵ | TBV | ✓ Esc | |
| Emoji Picker | ✓ insert ↵ | TBV | ✓ Esc | |
| Notes window (secondary) | — | — | — | OUTSIDE contract until the NotesWindow-chrome ratification lands (open item, final report §7) |
| ActionsDialog overlay (live ⌘K) | n/a (overlay) | self | ✓ Esc + ⌘K toggle (OF-61!) | Overlay keeps the UNDERLYING footer visible; the footerless full-view variant is dead code pending delete-vs-wire ratification |

## Ratified divergences

| Surface | Divergence | Ratified by | Date | Evidence |
|---|---|---|---|---|
| *(none yet — every current divergence is pending)* | | | | |

Pending ratification: Quick Terminal Actions omission (OF-60 — presumption is
it JOINS the contract, since the user filed its absence as a bug), About
single-affordance footer, Notes secondary-window chrome, ActionsDialog dead
variant.

## Seed findings (boarded 2026-07-20, ledger round 158)

- **OF-60** — Quick Terminal footer omits the Actions ⌘K button. An unratified
  divergence shipped behind a confident code comment; user-reported.
- **OF-61** — Terminal actions menu opened with ⌘K does not close on a second
  ⌘K. Symmetry-contract violation; user-reported. Probe must drive
  open→chord→closed and assert the footer button's `selected` state clears.
