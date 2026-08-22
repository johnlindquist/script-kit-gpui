# Script Kit consistency verification catalog

This tracked catalog is the clean-checkout-safe authority for the 75-task
consistency program. Task identifiers, exact headings, and section content are
part of receipt identity: changing a task invalidates receipts bound to its
previous section fingerprint. A catalog entry is an obligation, never proof of
completion. Only a fresh, privacy-safe, target-scoped `EVALUABLE_PASS` receipt
can discharge it. Local `.notes/` documents may add investigation context but
are not required to load or validate this catalog.

## Reporting and product safety

### RPT-001 — Publish evidence status and corrected inventory language

- Acceptance: distinguish source inventories, executable behavior, runtime evidence, surface kinds, mappings, aliases, and coverage profiles.

### SAFE-001 — Resolve every AI context part through one sanitized preparation path

- Acceptance: bound and sanitize all context, retain typed failures, and never expose raw paths, URIs, payloads, or provider errors.

### SAFE-002 — Replace time-dependent Dictation Escape with one safe dismissal ladder

- Acceptance: Escape dismisses exactly one layer and never discards recording or transcript data without explicit confirmation.

### SAFE-003 — Make New Conversation non-destructive and Delete explicit

- Acceptance: New, Open, Close, and runtime termination preserve history; only an explicitly confirmed Delete erases it.

### SAFE-004 — Make `NotesAction` the executable shortcut owner

- Acceptance: every displayed Notes shortcut resolves to exactly one available action with matching destructive policy.

## Fail-closed proof primitives

### PF-001 — Make DevTools receipt schemas executable

- Acceptance: producing tools reject missing fields, failed assertions, unavailable primitives, and invalid or blocked dispositions.

### PF-002 — Bind each proof stack to one target and generation transaction

- Acceptance: all evidence agrees on process, binary, window instance, automation target, surface, and generation identities.

### PF-003 — Redact generic semantic and text receipts by default

- Acceptance: recursive output contains no live note, clipboard, AI, transcript, path, secret, or external-window cleartext.

### PF-004 — Make AppView semantic projection exhaustive and quality-graded

- Acceptance: every AppView has explicit projection ownership; partial projections and duplicate semantic IDs cannot pass proof.

### PF-005 — Join intended contract, model truth, and rendered truth

- Acceptance: same-target, same-frame joins expose mismatched, clipped, or overlapping model and rendered geometry independently.

### PF-006 — Add shared text-fit, glyph, and occlusion measurements

- Acceptance: text proof identifies glyph bounds, clipping, intended truncation, occlusion, frame generation, and safe content fingerprints.

### PF-007 — Add semantic-to-AX parity, focus graph, and activation proof

- Acceptance: accessibility peers, reciprocal focus traversal, enabled state, and target-scoped activation agree without mutating permissions.

### PF-008 — Standardize list and scroll geometry proof

- Acceptance: selected semantic identity, rendered viewport visibility, footer exclusion, and selection generations are directly measured.

### PF-009 — Generate typed coverage bindings for all 37 kinds and 54 mappings

- Acceptance: census and source owners are valid; every mapping has one honest binding, and a static Direct grade is never runtime proof.

### PF-010 — Build deterministic family fixtures instead of one probe per surface

- Acceptance: all nine fixture families cover their mapped states using deterministic, network-free, sandboxed fixtures and cleanup.

### PF-011 — Make glass observers fail closed without changing motion

- Acceptance: missing observations and interference remain invalid, measured regressions fail, and protected glass calibration never changes.

### PF-012 — Execute browser-pixel `rectEquals` story assertions

- Acceptance: browser geometry assertions execute against the declared fixture and fail on a deterministic injected rectangle mismatch.

## Shared interaction and presentation

### UX-001 — Render shortcuts, syntax, triggers, and labels as distinct cue types

- Acceptance: each visible cue uses its correct semantic kind and cannot falsely advertise an unavailable keyboard action.

### UX-002 — Establish one canonical shortcut token stream

- Acceptance: menus, rows, footers, and actions derive displayed shortcuts and handlers from the same normalized token stream.

### UX-003 — Give Actions typed availability and disabled explanations

- Acceptance: disabled actions expose one safe reason, remain nonactivatable, and agree across action models and semantic projection.

### UX-004 — Drive footer verbs, shortcuts, enabled state, and handlers from one descriptor

- Acceptance: footer copy, keycaps, action availability, and activation all resolve from the same executable command state.

### UX-005 — Give context chips, identity badges, and destination selectors explicit roles

- Acceptance: staged prompt context, immutable conversation identity, and output destination have distinct safe semantics and affordances.

### UX-006 — Share one row-state palette across general and compact families

- Acceptance: sanctioned list-row owners consume the same tokenized focus, selection, hover, and disabled-state language.

### UX-007 — Resolve selected-row visual policy only at the shared owner

- Acceptance: selection paint is controlled by the shared owner without changing approved visual policy or calibrated motion.

### UX-008 — Give section headers one semantic grammar and stable slot

- Acceptance: section headings retain stable identity and nonactivatable semantics as asynchronous results update.

### UX-009 — Replace header-index shortcuts with explicit selection eligibility

- Acceptance: selection and activation use explicit row eligibility rather than assuming headers occupy fixed indexes.

### UX-010 — Remove silently discarded `UnifiedListItem` API branches

- Acceptance: every public list-item option is rendered truthfully, explicitly unsupported, or removed after consumers migrate.

### UX-011 — Render menu-syntax forms through shared form fields

- Acceptance: script-authored forms reuse existing field, validation, disabled, accessibility, and focus primitives.

### UX-012 — Route Actions search through an existing input owner

- Acceptance: Actions search reuses a supported shared input and follows the same editing, focus, and shortcut policy as its host.

### UX-013 — Put searchable Actions at top and freeze the shell

- Acceptance: action filtering keeps the search field discoverable and preserves popup geometry and pointer-target stability.

### UX-014 — Complete popup lifecycle: attach, dismiss one layer, restore focus

- Acceptance: popups keep their owner identity, close one layer per dismissal, and restore the correct semantic focus target.

### UX-015 — Remove fake pointer affordances from static hints

- Acceptance: informational hints never display button interaction, pointer cursor, or nonexistent activation semantics.

### UX-016 — Make Button, Toast, and shortcut feedback keyboard-operable and uniquely identified

- Acceptance: actionable controls have stable semantic IDs, truthful keyboard behavior, and accessible labels.

### UX-017 — Make `InfoStateTone` affect semantics and restrained anatomy

- Acceptance: informational, warning, setup, and recovery tones expose distinct accessible meaning and shared tokenized anatomy.

### UX-018 — Correct the semantic-state vs compositional-layout boundary

- Acceptance: shared semantic state owns status and recovery while rich compositional layouts remain in their existing owner.

## Cross-surface workflows

### WF-001 — Add provenance, state, and lifetime to staged context

- Acceptance: each staged context item records explicit origin, readiness, scope, removal policy, and privacy-safe identity.

### WF-002 — Return a truthful typed result from Agent Chat entry dispatch

- Acceptance: chat entry reports actual host, intent, staged context, readiness, and failure rather than optimistic dispatch success.

### WF-003 — Define attachment lifecycle through send, retry, host switch, and new thread

- Acceptance: attachment ownership, retries, cleanup, and thread transitions preserve user intent and never silently submit.

### WF-004 — Separate context, conversation identity, and delivery destination semantics

- Acceptance: prompt attachments, model/profile identity, and output destinations cannot be confused or interchanged.

### WF-005 — Make conversation commands executable descriptors

- Acceptance: visible conversation commands share typed identity, availability, shortcuts, recovery actions, and execution results.

### WF-006 — Make Escape, Background, Back, and Close origin-aware

- Acceptance: dismissal follows its declared origin and return route, unwinds one layer, and preserves active work and drafts.

### WF-007 — Align Send, Stop, cancellation, Retry, and recovery phases

- Acceptance: conversation lifecycle phases are explicit, Stop records cancellation rather than failure, and recovery remains executable.

### WF-008 — Bind auto-submit to explicit entry verbs

- Acceptance: Open, Add, and Continue only stage context; submission occurs solely for an explicit Ask, Send, or equivalent intent.

### WF-009 — Normalize copy, selection, and branch-edit affordances

- Acceptance: supported conversation surfaces expose consistent accessible copy, selection, editing, and shortcut behavior.

### WF-010 — Audit real ChatPrompt capabilities before claiming parity

- Acceptance: ChatPrompt advertises only actually installed callbacks, actions, cancellation, and recoveries.

### WF-011 — Make Flow identity, retention, and setup truth visible

- Acceptance: flow identity, engine readiness, transcript retention, run state, and recovery are explicit and preserve history.

### WF-012 — Share Notes search results while naming host-specific destinations

- Acceptance: Notes and Today share result identity and clearly disclose whether activation opens locally, externally, or attaches.

### WF-013 — Give Notes and Today the same visible `@` grammar

- Acceptance: the shared editor offers equivalent mention/context discovery while preserving legitimate host-specific choreography.

### WF-014 — Name Notes/Today AI scope precisely

- Acceptance: AI handoffs disclose document, line, or selection scope and stage only the explicitly selected context.

### WF-015 — Preserve state and focus across Notes/Today host switches

- Acceptance: switching hosts retains editor content, selection, scroll position, active document, and intended return focus.

### WF-016 — Make Notes→Agent Chat staging transactional per attachment

- Acceptance: primary and supplemental note attachments report per-item outcomes and never drop user content on partial failure.

### WF-017 — Make Notes Browse truthful standalone and in portals

- Acceptance: Notes Browse uses its actual source owners and distinguishes Open from Attach without implicit submission.

### WF-018 — Consolidate Dictation target descriptors

- Acceptance: every dictation destination derives label, availability, identity, and delivery verb from one typed descriptor.

### WF-019 — Make Dictation destination chips selection-only

- Acceptance: choosing a destination changes selection only; transcript delivery requires an explicit primary action.

### WF-020 — Freeze actual Dictation destination identity

- Acceptance: delivery binds to the selected app, window, document, or prompt generation and rejects stale identities.

### WF-021 — Implement one explicit Dictation delivery contract per target

- Acceptance: each destination has a typed, capability-aware insertion or send contract with no silent fallback.

### WF-022 — Preserve transcript and expose real Dictation delivery recovery

- Acceptance: delivery failures retain transcript content and offer only executable retry, retarget, or copy actions.

### WF-023 — Restore focus and return path after Dictation

- Acceptance: finishing or dismissing dictation restores its declared host, focus owner, and return route safely.

### WF-024 — Align Dictation History portal, copy, AI, and result-count grammar

- Acceptance: dictation history preserves safe transcript fingerprints, truthful counts, explicit attachment, and supported actions.

## Geometry and governance

### GEO-001 — Name geometry by semantic layer

- Acceptance: intended, model, rendered, native, and host geometry are identified explicitly and never compared across unlike roles.

### GEO-002 — Derive Arg mini/full sizing from active rendered metrics

- Acceptance: Arg prompt sizing follows its actual presentation mode, rendered row metrics, visible content, and approved chrome.

### GEO-003 — Repair Confirm model to renderer truth

- Acceptance: Confirm prompt geometry and semantic projection match its actual rendered shell and actions.

### GEO-004 — Derive Notes reservation/autosize from its painted footer action row

- Acceptance: Notes footer reservation and editor sizing match the same painted action row and active presentation mode.

### GEO-005 — Fix shared Notes/Today heading glyph clipping

- Acceptance: heading glyphs fit their measured clip bounds in both shared-editor hosts without unapproved theme changes.

### GEO-006 — Give Settings one action descriptor and Open language

- Acceptance: Settings primary action, footer, shortcut, and semantics share a descriptor whose truthful verb is Open.

### GEO-007 — Make Settings section metrics and iconless policy explicit

- Acceptance: Settings rows declare their section metrics and intentional iconless layout without latent spacing shifts.

### GEO-008 — Remove inert writable Actions row-style fields

- Acceptance: Actions style options either affect the shared renderer truthfully or disappear once all callers migrate.

### GEO-009 — Require explicit list presentation mode for predictive metrics

- Acceptance: predicted list geometry resolves the current dense, expanded, prompt, or themed presentation mode explicitly.

### GOV-001 — Freeze state ownership and migrate only legitimate consumers

- Acceptance: existing shared owners stay authoritative; migrations preserve sanctioned exceptions and avoid duplicate systems.

### GOV-002 — Delete compatibility façades when callers reach zero

- Acceptance: migrated production and test callers use the canonical owner and zero-caller compatibility façades are removed.

### GOV-003 — Introduce explicit authored alpha-byte typing

- Acceptance: authored alpha channels use explicit typed semantics without changing calibrated opacity or visible glass motion.

### GOV-004 — Validate owner-map paths and fix Notes Browse

- Acceptance: every coverage, feature, and Notes Browse owner path resolves inside the checkout; stale or escaping paths fail closed.

### GOV-005 — Give every generated design-contract conflict a lifecycle

- Acceptance: generated conflicts have stable identity, owner, decision, and resolution status rather than silent suppression.

### GOV-006 — Add a final consistency completion auditor

- Acceptance: all 75 tasks require fresh identity-matched passing receipts; invalid, blocked, missing, private, or unclean evidence fails.

### GOV-007 — Reconcile the protected glass veil contradiction without retuning

- Acceptance: documentation and approved fixture truth agree while protected timing, geometry, alpha, curves, and thresholds remain unchanged.
