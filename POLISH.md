# The Polish Bar

**Status:** Draft for John's calibration · Authored 2026-07-06
**Companion:** [`.impeccable.md`](.impeccable.md) defines the *taste* (what it should look like). This doc defines the *bar* (measurable budgets, contracts, and how each is proven). A violation of this doc is a **bug**, not a preference — triage it like a crash.

Every statement below was reverse-engineered from evidence: 7,726 commits of git history, plus John's own words mined from ~3,400 Claude Code sessions and ~6,000 Codex sessions (2026-03 → 2026-07). Receipts are cited as commit hashes or dated quotes. Numbers marked **(proposed)** are inferred, not evidenced — calibrate them; everything else was already enforced at least once.

How agents use this doc:

1. **Triage:** any observed violation of a bar below is at least P1 polish debt. Rank findings against these dimensions, in this order — the ordering reflects how often John complains about each.
2. **Fix classes, not instances:** every fix must land one rung up the CLAUDE.md enforcement ladder (compiler → lint → behavior test → devtools probe → source audit). A patched pixel without a lock will regress.
3. **Prove it in the real UI:** a fix must change what renders, not what a test greps — "no fake-green" (`d67df7cb4`).

---

## 1. Latency — "Instant" is the product

The single most repeated word in three years of sessions. The window is a reflex, not an app.

**The bar:**

- Every keystroke in any filter input should paint its result inside the **16ms product bar**. Historical evidence drove worst-case typing latency from 187ms → 14ms: "All inputs now under 18ms (within 16ms frame budget)" (`13a417737`). The current root typing probes do **not** measure input-to-paint, so this bar is not yet a current automated paint gate. It applies to held keys too: "definite 'hiccups' when holding down delete… feel laggy" (2026-05-20).
- **Nothing synchronous and expensive on the keystroke path.** SQLite, history search, osascript probes, browser URL sniffs run off-thread and merge next frame. Evidenced: `@` was blocking the UI thread ~943ms → 4ms (`bf0b87ffc`); ai_vault (avg 99ms) and browser_history (avg 30ms) gated behind explicit filters (`13a417737`). Budget: **≤1ms synchronous work per keystroke (proposed)**.
- **Hotkey → window visible with input focused: ≤50ms (proposed)**, and no keystroke typed immediately after the hotkey is ever dropped. Keyboard-focus readiness is a measured property (`3918ad57d`); state resets *before* the shortcut, "so that it's ready and there's no timers/guards/sense of delay" (2026-06-17).
- **Surface transitions are instant.** Main↔Agent Chat, escape-back, Tab-to-chat: "there should be no reason for any lag" (2026-04-03), "Please make it instant" (2026-05-25). Budget: **one perceived frame, ≤50ms (proposed)**.
- **Openable surfaces come pre-warmed**, never cold-spawned: warm PTY pool (`8929093e3`), Tab AI harness prewarm (`de4eb2789`), focused-text Pi prewarm (`0d5983543`).
- **No artificial delays** on interaction paths (`eb288220a`), and no full-screen loading gate when the input could be live immediately: "input is immediately available — users can start typing while context loads in background" (`f275c1eb3`).
- Arrow-key navigation updates the list in the same frame as the footer — never let chrome update before content ("the footer text changes in time, but the items list doesn't", 2026-05-08).

**Proof:** add a devtools latency probe that measures hotkey→visible, keystroke→paint (including a key-repeat storm), and transition times, with red/green numbers per run. Until input-to-paint instrumentation exists, `stateResult.inputValue` state-echo timings and internal-log durations are **surrogate diagnostics**, not proof that the 16ms paint bar passed. Off-thread rule locked by clippy `disallowed-methods` where expressible, else behavior test.

Search proof must distinguish semantic state, a naturally scheduled completed GPUI draw, and retained framebuffer readback. Join each measurement to its query, committed-result, selection, and window-lifetime revisions. A forced capture can prove that a scene renders, not that the responsible publication scheduled it; offscreen GPUI evidence does not measure WindowServer composition or native activation.

## 2. Layout stability — nothing moves that the user didn't move

Layout shift reads to John as literally *broken*: "it causes the footer layout to shift around so the buttons don't feel 'stable'" (2026-05-21).

**The bar:**

- **Async publication preserves interaction intent, not an obsolete row position.** Implicit global Files completions warm caches without republishing the current query; explicit Files, directory search, and other admitted publishers may update rows. Automatic selection follows the first eligible result; deliberate selection follows its stable identity. Source installation, row reconciliation, preview/footer subject, and revision publication form one transition. A pointer or agent action captured before that transition executes its exact subject or is refused, never a different row at the old position. The earlier blanket query-frame latch (`910c40a07`) is not the policy for every source.
- **Search results grow monotonically as you type.** "'Why is' has more/completely results than 'Why i'. It makes it feel broken" (2026-06-22). No instant-result-then-replaced-after-a-beat (2026-05-09).
- **Every list leads with a persistent section separator — even if it's just "Results".** A list whose first row is a bare item shifts every row down the moment a header-led state (empty query, grouped mode) swaps in, and vice versa. The main menu's stable "Results" header in search mode is the reference fix (`4d76327b8`); the Flows desk got the same treatment 2026-07-10. The separator row never appears/disappears with filtering — only its label may swap. Applies to every launcher-family list, builtin browser, and new surface.
- **No stale-content flash on mode entry.** Typing `/` or `@` must never flash the previous command/mention before clearing (2026-04-16, 2026-04-20 — a repeat regression; deserves a permanent probe).
- **Popups freeze their shell size once open** and scroll inside it. Actions menu resize-while-typing is a standing P1 (layout-stability audit 2026-07-04); content-sized popups are not user-resizable (`76268b2b7`).
- **No loading-skeleton flash when a synchronous first paint is cheap** — the ~59ms skeleton was replaced by a ~0.5ms sync seed (`977cee601`). Threshold: if first paint can be computed in **≤5ms (proposed)**, paint it synchronously.
- **Stable slots for stateful chrome.** Footer actions swap labels, not slots (Agent Chat audit item); reserve lanes for validation messages and status rows instead of inserting them.
- **Overflow clips the disposable end, never the data** ('100 words' must not render as '00 words', `c2b69cad0`). Widths come from real flex layout, never per-character estimates (`8af8eb31f`, `ca4c93d19`; standing flexbox mandate).
- **Footer buttons never shrink or truncate** — "or else they're useless" (2026-07-06). Shortcut buttons keep intrinsic width (`flex_none` / hug-to-content in `footer_chrome`); informational lanes absorb width pressure and may truncate instead.
- Scroll containment: inner regions scroll themselves, never the page around them (2026-06-15).
- **User scrolling is independent of selection.** Refresh preserves a deliberate viewport anchor and fractional offset; an old deferred reveal cannot undo a newer scroll or query. Provider refresh never steals input focus.

**Proof:** devtools rect-diff probe (element rects across frames during type/open/stream); golden screenshots per surface; the remaining 2026-07-04 audit backlog is the burn-down list.

## 3. Escape & dismissal — Escape is sacred

**The bar:**

- **One Escape contract for the whole app**, derived from the `DismissPolicy` table — never per-renderer booleans (`cdb65127c`).
- **Escape unwinds exactly one layer per press**; every branch stops propagation so nothing double-fires (`8900405ee`). Delete has its own ladder in cwd mode (one delete → `/`, two → main menu, 2026-05-28).
- **Destination depends on origin:** opened from the main menu → Escape returns there; opened directly (shortcut/tray/deeplink) → Escape closes the window (`4794e630c`).
- **Escape and Cmd+W have identical data-loss behavior** — Escape saves before closing (`db96a750d`). Semantics stay distinct: Escape = back, Cmd+W = close window (2026-04-02).
- **Destructive dismissal needs a two-press arm + HUD within a 2s window** (`cdb65127c`).
- **Toggles are debounced (300ms)** so close-then-reopen can't strand or double-open (`c9f2c3311`, `2da4365f0`).
- **Popups/dialogs auto-close on focus loss** (2026-03-23) and HUD messages always auto-dismiss (2026-06-11). No redundant confirmation HUDs when the outcome is visible (2026-03-26).
- Escape must never look hung: a paused-but-open dictation UI on Escape is a defect (2026-04-01).

**Proof:** `flows/escape.md` owns this dimension; every surface gets an escape-ladder behavior test; new surfaces cannot ship without a `DismissPolicy` row (compiler rung: exhaustive match, no wildcard).

## 4. Focus & keyboard — focus lands where typing goes

**The bar:**

- **The filter input owns focus by default.** Unclaimed clicks and window drags return focus to the input, never dump it on the list root (`9858f21b8`; "if you type, nothing happens" 2026-06-11).
- **Focus is restored after every popup, action, or dialog dismiss** (`82696857a`; "it doesn't refocus on the ai chat input" 2026-03-21). No pre-attach focus steal (`a6fd4e035`, `a39de7029`).
- **Every displayed shortcut fires.** A rendered keycap that doesn't work is a broken promise ("ensure that all actions with displayed shortcuts have their shortcuts enabled", 2026-06-08).
- **Modals trap the keyboard:** Tab cycles inside, Escape closes, nothing leaks to the surface behind (`225240d4c`; "each 'Tab' is sending values to the ACP Chat behind the scenes" 2026-04-28). The focused button is unmistakable — the user always knows what Enter will do (`3f42543b1`).
- **Paste is paste** — never re-typed character-by-character, never auto-submits (2026-06-19).
- Global keys are scoped: surface-level bindings (Tab, Cmd+P) must not shadow editor semantics ("I *hate* 'Tab' to open ACP Chat… inside of a markdown editor", 2026-04-09).

**Proof:** simulateGpuiEvent focus probes per surface (open → click dead space → type; open popup → dismiss → type); autofocus regressions get permanent probes — they've recurred (2026-03-13, 2026-04-16).

For hidden launcher proof, observe the actual local GPUI input focus handle separately from native window focus (which remains false). The selected row is not the focused input. Exercise both real GPUI key/pointer events and revision-bound semantic actions; selecting the already-selected row deliberately must still establish anchored intent.

## 5. Consistency — one system, zero drift

John's most emotionally-loaded complaint pattern: "I feel like you're misunderstanding the concept of 'reusing components' and 'consistency'" (2026-06-08); "Some jr dev added that feature and I hate it" (2026-06-12).

**The bar:**

- **Shared render paths, not lookalikes.** Same input component in main menu, mini agent, Agent Chat; Today == Notes editor render path (`8b266e444`); ACP input == main menu layout (`2dee3c380`). A surface reimplementing shared UX locally is a defect even if pixel-identical today — drift is the failure mode.
- **Zero hardcoded visual values outside token layers.** Colors, spacing, radii, insets, opacity resolve through `crate::theme` / chrome tokens; one canonical vibrancy opacity token resolved by every window (`f4cca5f55`); accent-derived chrome via alpha overlays (`d7da006cf`). Target: **hardcoded-value count ratchets to zero**.
- **Accent stays on the theme accent.** Hue drift budget: shader hue_shift ≤0.06, warm-side only — 0.10–0.16 rotates gold into green (`9fe049439`).
- **`.impeccable.md` is law:** opacity tiers, Raycast-style text grading, three-key footer, list-item anatomy, surface-layout decision rule, actions-dialog anatomy. Deviations need a documented owner + reason (CLAUDE.md contract).
- **A fix changes the real UI, not the audit** — token added AND applied (`d67df7cb4`).
- Interaction states are calibrated, not binary: hover sits visibly between idle and focused ("we need it like halfway between the focus strength and the 'nothing' strength", 2026-04-13); pressed/toggle == focused style (`.impeccable.md`).

**Proof:** `flows/auditor.md` sweeps enumerate raw values → convert to clippy disallowed-lists + shrink-only ratchet; de-drift contract tests for chrome constants (`66415737d`); component-reuse violations flagged in review against the CLAUDE.md shared-UI checklist.

## 6. Motion — calm, never decorative

Notably: John has *never* complained about missing animation — only about motion that exists and misbehaves. Default to less.

**The bar:**

- **Pulses breathe, never blink:** ≥2s cycle, opacity floor ≥0.6 (`a1e663f08`). Streaming states get a spinner, not a blinking cursor (2026-04-02).
- **No appear animation on modals** (`a158bf910`). No unnecessary animation at all — ".impeccable.md: no unnecessary animation, no visual noise".
- **Ambient effects are always-on subtle; change amplifies** — never fade to nothing at idle, never grab attention (`9fe049439`, `3fbb39b03`).
- **Hover state never churns during scroll** — suppress enter/leave while rows slide under a stationary pointer (`6c7950040`).

**Proof:** motion constants live in tokens (cycle ms, opacity floors) with a de-drift test; scroll-hover churn locked by the existing behavior fix + probe.

## 7. Materials & windows — native or nothing

**The bar:**

- **Vibrancy must actually read.** Material-led, not tint-led: Menu material + tint ≤0.30 + backdrop_saturation 2.6; target ~54% body saturation (Raycast's measured value) — a 0.85 neutral tint crushes it to ~15% (`f4cca5f55`). "Drastically lower the background opacity… so I can actually see the blur" (2026-03-23). Blur parity across ALL windows — main, notes, chat, dialogs (2026-03-24).
- **The footer blur trio is non-negotiable** (native NSVisualEffectView + hitTest:nil + deferred transparent hitbox) — never change one leg independently (memory: footer-blur-architecture). The list visibly scrolls behind the blurred footer (2026-04-07).
- **Non-activating overlay discipline:** the window interacts with the frontmost app Raycast-style (2026-03-25), never steals activation, never lingers on top after work ends (standing AFK feedback), and never ghost-flashes on unrelated shortcuts (2026-03-30). Overlay ordering hides siblings synchronously in the same unsafe block — no async flash gap (`2492acfa9`).
- **Chrome metrics are measured against native macOS, and refuted concerns are dropped, not "fixed"** (`1c71559a7`): 9pt input inset from a real NSTextField (`d67df7cb4`), 12pt footer gap at Apple's soft bezel floor (`ac3345ce9`).

**Proof:** screenshot + saturation measurement probes (the vibrancy work was won by measuring); window show/hide/z-order choreography probes via devtools getState — leave `windowVisible:false` after every pass.

## 8. States — every state is designed

**The bar:**

- **Loading is inline and non-blocking** — never a full-screen takeover when the input could be live (`f275c1eb3`).
- **Errors are dismissable callouts with a real Retry**, never dead-ends (`b85ee77b6`).
- **Zero-match shows a designed empty list**, not the unfiltered set (`0bb6d487a`), and empty states render inside the same container as the list (no list↔EmptyState container swap — audit P1).
- **Capped results say so** — follow established patterns or load more on scroll; never silently truncate (2026-05-12).
- **Sources surface without secret prefixes** — if browser history is enabled, "amazon" finds it without `tabs:` (2026-05-20).
- **Placeholder and ghost text teach**, they don't brand: "Search or type @ / | . ; for commands" (`9515cd69f`); ghost text must be substantive, not filler (2026-05-30).
- Rows show friendly names, never raw sigils (`777b99e8f`).
- Search providers distinguish success, empty, unavailable, failure, cancellation, and stale completion. A disconnected worker is not successful empty data. Empty/header-only/status-only projections have no selectable marker, preview subject, or runnable row; disabled explanations remain non-activatable. An empty rich source sub-search is unarmed until the first Down selects its first eligible row.

**Proof:** state-matrix walkthrough per surface (empty / error / loading / overflow / long-text / zero-match), screenshot each cell; undesigned cells become tickets.

The owned search inventory must account for every required state and schedule. Missing capability, trace overflow, stale framebuffer, blank readback, or failed cleanup invalidates the corresponding proof; none is a skip-as-pass. Use bounded pixel samples from the exact completed frame for the matrix, reserving images for representative or failing cases.

---

## The standing loop

- **Nightly polish sweep:** auditor + escape + perf flows run against this doc; findings ranked by dimension order above; the AFK pipeline consumes the queue.
- **Every complaint becomes a lock.** The scar ledger was empty when this doc was written — every quote above had to be re-mined from raw transcripts. Going forward: friction moment → `/scar` → probe or eval → regression lock. Each irritation is felt exactly once, ever.
- **Recalibration:** the **(proposed)** numbers (50ms open/transition, 1ms sync-per-keystroke, 5ms sync-first-paint) need John's sign-off; everything else already has a receipt. Revisit budgets when hardware or GPUI vendor changes materially.
