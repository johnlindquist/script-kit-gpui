# scripts/devtools — receipt-first app instrumentation

CLIs and a library for inspecting and driving a running script-kit-gpui app
over its stdin/stdout JSON protocol. Inspection commands emit a final JSON
receipt with a fail-closed `classification`. `design loop` emits protocol
responses and `design watch` emits progress JSONL before their final receipt.

## Noninterfering owned evaluation

Prefer the owned evaluator for agent verification. It mounts production views
in real hidden GPUI/Metal windows, dispatches local application events, and
uses synthetic state without desktop input, operator clipboard, or providers.
The older session commands below do not grant permission to show or activate
operator windows.

```bash
bun scripts/devtools/devtools.ts build-ops discover
bun scripts/devtools/devtools.ts build-ops inspect
bun scripts/devtools/devtools.ts build-ops act app-build --features owned-ui-evaluation
bun scripts/devtools/devtools.ts build-ops act libtest-build
bun scripts/devtools/devtools.ts build-ops act exporter-build
```

Save the emitted `artifact` object as a reference JSON file; do not select a
binary by mtime. Each reference binds immutable executable bytes, compiler
inputs, toolchain, and build configuration. Changed Rust inputs require a new
build/reference. Use a fresh output directory for each run.

```bash
bun scripts/devtools/design.ts discover --artifact app.reference.json --out .test-output/design-catalog
bun scripts/devtools/design.ts run --artifact app.reference.json --out .test-output/design-matrix --scenario production-family-matrix
bun scripts/devtools/design.ts watch --artifact app.reference.json --out .test-output/design-watch
bun scripts/devtools/stories.ts run --scope core --libtest libtest.reference.json --app app.reference.json --out .test-output/core-stories
```

`design loop` accepts the existing JSONL protocol for composable inspection,
queries, actions, waits, mounts, and teardown. `discover` and `diagnose` expose
the current catalogue, availability, targets, and limits. Always target the
exact instance ID and lifetime generation. For owned `waitFor` conditions of
type `completedFrame`, omit `expected` to observe the current identity atomically;
supplying it enforces owner/revision freshness. Explicit `null` is invalid.
The `owned-evaluation:batch:` request-ID prefix is reserved for internal step
correlation; external requests using it are rejected before dispatch.

Live watch reads `edits.json` inside its owned output directory, shaped as
`{"edits":[{"tokenId":"theme.colors.accent.selected","value":7520680}]}`.
Only advertised live tokens reload in process; structural Rust changes still
require compilation. Invalid edits retain the last good revision. Send SIGINT
to the watch controller (not its whole process tree) to revert and close its
owned windows. Require the final receipt's cleanup to be closed before treating
the run as complete; a timeout, failed assertion, or missing frame is not proof.

Readback proves GPUI pixels, not WindowServer composition, AppKit glass/glyphs,
native focus, OS IME, global input, or live providers/devices. Transparent PNGs
must be viewed with alpha compositing; a synthetic background is visualization,
not native material evidence. Glass motion calibration remains unchanged.

Build recovery uses `build-ops diagnose locks`, `query retention`, and
`query keep-set`; mutation schemas and exact identity/revision requirements
come from `discover`. Never delete a live or unknown lease, prune protected
caches, or substitute another owner's cleanup identity. Resource refusal is
fail-closed: retire only explicitly eligible owned outputs or obtain a scoped
budget decision; do not disable the free-disk floor.

### Controlled launcher search

The `launcher-ranking-provider` design journey and
`scripts/agentic/launcher-selection-stability-probe.ts` use the same runner in
`scripts/agentic/launcher-search-recipes.ts`. Its finite inventory is the
acceptance scope: case ID, source, query route, completion schedule, selection
intent, expected transition, proof level, and resource bounds. Missing
capabilities and unexecuted cases are uncovered, not skipped successes. Record
the inventory hash and generated/executed/reduced/blocked/failed counts, plus
structural inapplicability with its evidence. Mutually exclusive work cannot
be reported as simultaneous completion; an empty list cannot prove anchoring.

```bash
bun scripts/agentic/launcher-selection-stability-probe.ts spec
bun scripts/devtools/devtools.ts build-ops query storage
bun scripts/devtools/devtools.ts build-ops query keep-set
bun scripts/devtools/devtools.ts build-ops act app-build --features owned-ui-evaluation --artifact-out .test-output/search-contract.reference.json
bun scripts/devtools/devtools.ts design run --artifact .test-output/search-contract.reference.json --out .test-output/search-contract --scenario launcher-ranking-provider --search-case automatic-higher-arrival
```

`--artifact-out` must name a new file in the canonical `.test-output` or temporary
roots. If compilation/publication succeeded but saving that reference failed,
save the exact returned `artifact` object in an admitted destination and verify it
with `build-ops query artifact --reference <path>`; do not rebuild valid bytes to
repair a reference-file destination.

Use new reference/output destinations or a current verified reference; do not
rebuild merely because a recipe or evidence destination changed. The manifest's
default feature is `ocr`; owned search fixtures do not require `local-llm`, live
models, credentials, or a Pi provider. One evaluator runs at a time, within the
existing eight-window, 4,096-request, 2,048-frame, ten-minute and retained-image
limits. Split the finite schedule inventory into bounded deterministic shards;
exceeding a limit never authorizes sampling it down to a passing subset.

Finish managed artifact publication before starting a native campaign. Publication
and runtime cleanup share a metadata lease; contention can invalidate cleanup
even after every process and window has closed. Preserve that failed receipt and
protected task record rather than treating it as UI success. The existing
`native-task-finalize` path requires the original live supervisor; there is no
supported dead-owner finalization operation. Do not edit task records or forge
process identities to bypass that boundary.

The native metadata bridge has a 20-second deadline covering the existing lease
acquisition, identity check, and lease-release limits; the parent allows 25 seconds
for startup acknowledgement. This does not extend the evaluator's ten-minute
lifetime. Bind failures retain `native_task_bind_timeout` or
`native_task_bind_failed` alongside a missing-startup-identity failure, instead of
discarding the underlying handoff failure. A failed acknowledgement still cannot
prove that native windows or managed references closed.
Search receipts classify the launch wrapper's underlying error, preserving typed
protocol failures such as `response_timeout` while retaining the wrapper's actual
cleanup evidence. A wrapper message is not a replacement for the failure cause.

Start with the case matching the reported behavior, not the exhaustive inventory.
Use `launcher-selection-stability-probe.ts run --artifact <reference> --out
<new-directory> --case automatic-higher-arrival` for the late-result/Enter race.
`--case <case-id>` and `--shard <index>` select bounded checks: inspect their
executed-case results. Unrequested schedules remain explicitly uncovered and
cannot produce an overall inventory acceptance pass. Omitting both selectors
executes the entire inventory; reserve that for an explicitly requested campaign.

Version 5 preserves the first selected identity even without arrow-key or pointer
navigation (`automaticAnchor`, distinct from `explicitAnchor`). For an unchanged
query, displayed rows retain their order and late matches appear below them under
the shared **More results** section. A new query uses normal ranking again. If
the selected result disappears or becomes ineligible, selection clears and Enter
does not fall back to an unrelated result; typing or deliberate navigation can
choose another target.

`automatic-higher-arrival` holds a real provider completion until the initial
selection is painted, releases the late match, checks the original selected
identity and marker position, then dispatches Enter and verifies the original
subject. Provider-order comparisons require equal candidate contents, not equal
arrival-dependent display order. No timing grace period or extra loading delay is
used to protect the target.

The sentence corpus includes `sentence-typing`: 32 compiled natural-language inputs crossed
with seven schedules (224 scenarios): burst typing, deterministic paced typing,
word pauses, reversed completions, typo/restore ABA, mid-sentence cursor edits, and
deliberate selection before late arrivals. `src/design_evaluation/search_sentences.json`
is shared by the native mock providers and the finite schedule generator. Each
fixture supplies matching source records; an all-fallback result cannot satisfy
the final matching-source assertion. Text is inserted through individual GPUI
key events, not select-all/setInput replacement. Each text mutation captures a
naturally completed search frame and checks literal input, query ownership, and
selection. Cursor-only movement records the input child's scheduled frames;
it does not demand a root search notification for an unchanged query. The
subsequent insertion/deletion must prove the exact mid-sentence edit.
Spaces, punctuation, numbers, composed/decomposed accents, emoji, and mixed-script
text are included. This is owned application input, not OS IME or global typing.
Entry is explicit: 217 profiles type forward; the seven leading-space profiles
type the first non-space character, move left, insert the literal leading space,
then resume at the end. Empty-launcher Space intentionally opens Day Page; the
caret-prefix route preserves that shortcut instead of misclassifying it as search.
Use `--case sentence-typing --shard <index>` for a focused typing profile. The
full version-5 inventory also retains the source-order and interaction schedules.

Long typing runs losslessly retain each capture's complete frame page, then
call `acknowledgeFrames` when retained trace bytes reach a quarter of the native
byte capacity. Below that threshold, they avoid an unnecessary wire request.
The request carries the mounted target,
accepted target expectation, and exact trace/frame cursor. Native history strictly
before that frame is released; the cursor frame remains the next scheduled-capture
baseline. Read cursors remain passive. Stale, retired, future, or nonexistent
acknowledgements fail without drawing, clearing a trace error, or changing limits.
Use the captured frame, not a newer page cursor; acknowledgement cannot pass
`expected.frameGeneration`, even when newer frames already exist natively.
Discovery publishes `frameAcknowledgement` version 1; `schema --json` describes
the command and counters. Receipts retain every delivered frame and acknowledgement.

Notes subtitles use the same held display clock as other controlled sources.
The native fixture publishes `expectedNoteSubtitleFingerprint` for its fixed
one-hour-old, unpinned, 64-character Notes. The runner compares each committed
Notes row's semantic descriptor with that independent expectation. Both use
the native process-private HMAC key; the `sha256:` prefix is not a public-SHA
contract. No note text or private key is returned.

Output packing admits at most three independent schedules and 384 declared
requests per runtime; an oversized independent schedule runs alone. Every
three-schedule pair comparison and six-order cohort owns an exclusive runtime
and stays whole on that fixture root. This preserves all 1,006 schedules and every
comparison family; request counts are a conservative packing proxy, not proof
that a group fits the unchanged 64 MiB child stdout+stderr limit. The separate
8 MiB retained `app.log` ceiling cannot establish that fit.

Cohort admission reserves four units for three ordered events plus normalization
under both selection intents. A complete six-order cohort declares 3,072 requests
and 480 seconds, below the reserved runtime ceilings. These per-case estimates
do not enlarge the runtime limits or prove wire-byte fit. Revising an estimate
changes `caseSetHash`; earlier receipts remain historical evidence, not a pass
for the revised inventory.

Each launched runtime retains a `search-runtime-output` effect with
`observedReceivedOutputBytes`, configured `maxOutputBytes`, the separate
`maxRetainedLogBytes`, and its schedule IDs. The received counter counts the
original child stdout/stderr bytes, before protocol payload decompression; it
excludes supervisor framing and output the supervisor did not forward. It is
sampled after close and immediately after safety setup. `streamsDrained` and `cleanupClosed`
remain explicit: a limit failure or incomplete cleanup never becomes a pass.
If an indivisible comparison family exceeds the cap, it cannot be repaired by
splitting or normalizing away its required comparisons.

Discovery advertises the opt-in `responseEncoding` contract. The owned client
validates it before requesting `zlib-json-base64-v1`: complete JSON responses are
compressed independently, with unchanged 6 MiB decoded and 4 MiB compressed
per-response bounds. Strict decoding precedes existing identity/frame checks;
missing, malformed, or unrequested encoding fails closed. Legacy requests and
unsolicited lifecycle records remain ordinary JSON. Log consumers use
`normalizeProtocolResponse` from `driver.ts`; physical `app.log` remains the
original wire evidence. Compression does not omit frames or increase any cap.

After each runtime closes, the runner finalizes its complete comparison family
and persists a required `search-shard-N.json` artifact before starting the next
runtime. The final observation contains case summaries and exact shard/schedule
references, not duplicate frame pools or safety snapshots. Each artifact stays
within the existing 64 MiB detail limit; the compact receipt stays within 1 MiB.
A persistence failure stops further launches and preserves earlier artifacts.
`resolveSearchJourneyReceipt` in `scripts/agentic/launcher-search-receipt.ts`
verifies the enclosing receipt, artifact ownership/hashes, one-to-one schedule
bindings and summary equality before reconstructing the complete case evidence.
Missing, changed or mismatched shard evidence cannot qualify a passing summary.

Historical qualification (2026-09-01): the complete version-2 inventory passed
781/781 runnable schedules across all 28 case families and 20 controlled root
providers. The sole structural exclusion is Files + directory same-turn
completion: those scopes share one physical owner. No schedules were reduced
or blocked. All 377 runtimes closed their owned windows, processes, streams,
and managed references. The run retained 14,627 frame entries and performed
7,756 captures; peak received wire output was 31,542,566 bytes against the
unchanged 67,108,864-byte cap. The authoritative receipt is
`.test-output/search-contract-final-v20-full-20260901/receipt.json`; the matching
artifact, Rust/Bun checks, and cache-only controls are indexed in
`.test-output/search-contract-verification-20260901.json`. This qualifies the
declared hidden production-GPUI inventory, not live providers or native OS effects.

Two failed historical v19 runs still have protected, unfinalized task records.
Their exact processes were verified absent, but their original supervisors are
gone and the supported finalization path cannot recover them. Those records and
failed receipts remain unchanged; the version-2 green run does not retroactively
claim their cleanup succeeded.

Mount the dedicated search fixture advertised by the catalog. Its strict
`fixtureControl` search family prepares only compiled scenarios, releases a
nonempty unique array of exact observed run IDs atomically, and advances
bounded logical time. Use the returned `sourcePlans` and `fileViewInputs`, not
guessed paths. Inspection does not advance that clock. Explicit protocol waits
let GPUI timers progress without advancing the search fixture's logical clock
or releasing held providers. Synchronous sources use explicit source-change
admissions and real reads, never invented workers;
disconnect is refused for a source without a worker. Gates replace external IO,
not production filtering, grouping, installation, input events, or selection.
The catalog separately declares a held display clock: `2026-05-01T01:00:00Z`,
reported as `searchProviders.displayUnixMs`. History and Clipboard subtitles use
that reference through the production formatter, so wall-clock minute changes
cannot alter comparison content while provider orders run. Logical source-time
advances do not move this display clock; ordinary production search still uses
the real wall clock. This does not change GPUI timers or measured latency.

A run may warm a cache without publishing rows: inspect both terminal outcome
and publication policy. Source failure retains the last-good snapshot; a
successful empty read replaces it. Unmount/remount creates a new lifetime.
Already released old work can finish late but cannot mutate its successor;
retired held work cannot be released again.

The compiled `eligibility-portal` scenario initializes the real ScriptSearch
attachment portal with a provider-free, hidden Chat return host. Releasing its
owned script source exercises production grouping and the actual reserved slot,
not injected rows. The recipe checks slot semantics, paint, refused selection,
navigation and portal cancellation, then prepares the launcher for calculator
submission. This is not proof of live Chat entry, Pi transport or Chat dismissal.

For sources advertised by `searchProviderWait`, use the existing `waitFor`
transport with condition `{ type: "searchProvider", source, query, afterRunId }`.
The query is the exact observed lifetime/revision/scope stamp. Its compact
`searchProvider` result distinguishes a current admitted run, exact held
blockers, and genuinely settled work. Admitted and settled run IDs must exceed
`afterRunId`; use zero when any current run is acceptable. A missing IO-gate
record is not quiescence: production debounce and queued latest-desired work
remain pending. Blockers are reported, never automatically released. Explicit
fixture-control release must target those observed IDs, then wait again. Stale
queries/targets and malformed conditions fail closed. Synchronous source-change
admissions keep their explicit fixture-control path; they do not gain fake
workers. No provider wait advances the fixture clock or forces a draw.

Final normalization may opt into `acceptCached: true`, but only with
`afterRunId: 0`; combining it with a positive fresh-run bound is invalid.
The distinct `cached` result requires a real, present source snapshot or keyed
cache entry accepted by the production reuse predicate for the current query.
A present empty cache is valid; a missing cache or failed read is not.
`getState.searchObservation.sourceCacheReadiness` exposes the same native proof.
Discovery names supported cache sources in `searchProviderWait.cacheSources`
(currently `tabs`, `files`, `directory`, `history`, `notes`, `todos`, `clipboard`,
`dictation`, `conversations`, and `windows`), separately from worker-wait coverage.
The proof's `cacheIdentity` is sensitive and redacted in retained receipts;
`cacheStateRevision` is an actual source-state revision or null, never a producer
run ID. `rowCount` counts cached source entries, not visible matching rows.
Cached readiness has null `owner`/`run`: it does not revive a detached worker
consumer or invent cache producer provenance. Fresh worker acquisition leaves
this opt-in disabled.
The recipe explicitly drains exact retired held owners when required; cache
readiness never reattaches them. Final ranking normalization also settles
requested query-independent catalogues, even when their fixture entry query
differs. App icon completion remains part of the painted content fingerprint.

File Search uses existing `waitFor` conditions advertised by
`fileSearchStreamWait` and `fileSearchPreviewWait`:
`{ type: "fileSearchStream", generation, query }` waits for the exact real stream
terminal; `{ type: "fileSearchPreview", generation, query, workSequence }` waits
for the exact decoder completion held at its gate. Take these identities from
the current target, never from a former query. The preview result includes
`phase: "held"`, path, decoded status/content hash, logical time and due time.
These waits pump real GPUI receivers without advancing fixture logical time.
Only explicit clock advancement releases the decoder; selection/sequence fences
still decide whether it installs. Stale identities and malformed conditions are
refused. Pixel checks use the same capture's flattened `layout.windowWidth` /
`layout.windowHeight` and raster dimensions, not an assumed display scale.

The owned catalog advertises `frameCursor: { version: 1, operation: "getState", captureFrame: true }`.
`OwnedEvaluationClient.inspect(target, frameCursor?)` accepts the exact
`{ traceGeneration, afterFrameGeneration }` cursor and requires its response echo
plus `latestFrameGeneration`. Omission retains the existing full bounded trace;
null, unknown keys and malformed numbers are refused. Stale, retired or future
cursors fail closed without retrying a full read or refreshing caller authority.
Pages contain ordered retained stamps newer than the cursor. Native negative
readback isolation can legitimately leave gaps in completed generations.

Search recipes retain every returned page in their numeric case pools before
acknowledging its cursor. The acknowledgement is bound to the exact runtime,
client, mounted target and trace lifetime, and may carry across comparison
orders whose earlier frames remain in earlier case pools. Successful prepare or
retirement clears it; unexpected lifetime changes never heal it. An over-budget
page remains explicitly unacknowledged in its failed receipt, even if later
preparation retires that work. `captureFrame(target, includeImage, scheduled?,
frameCursor?)` applies an explicit cursor to both logical history pages:
`frameEvidence.completedFrames` and `state.frameEvidence.completedFrames`.
Recipes retain both pages and the complete current-frame facts in one pool
transaction before advancing the acknowledgement. Notification/cause, paint
bindings, pixels and exact target expectations remain authoritative. Omitted
cursors preserve forced full captures and the existing scheduled-baseline
behavior; there is no implicit refresh, forced fallback or reread. The existing
runtime safety prelude sends malformed/stale/future `getState` and capture
cursor probes over the owned native transport, then checks unchanged frame,
target, search and provider authority. Client rejection tests are separate;
none of these negatives creates a schedule or grants screen/input authority.

Explicit-cursor captures advertise `frameCursor.captureHistoryBundle` and may
return `frameHistoryBundle: {version: 1, captureFrameCount, stateFrameCount}`.
Both wire pages then carry `historyScope: "captureBundle"`: they are disjoint
parts of the current-frame/history union, **not standalone complete histories**.
Only exact duplicate frames are omitted. The client validates the full bundle's
identities, cursors, bounds, and original page counts, then reconstructs both
complete histories with shared frame references before returning state to callers.
Standalone inspection rejects partial-page markers. Default/no-cursor captures
retain their existing representation; acknowledgements still follow retention.

Explicit-cursor state/capture frames may replace duplicate `search` metadata with
`searchMetadataRef`, an index into that frame's own `paintBindings`. The referenced
binding must be the unique `mainSearch` / `main-search` root; native packing
requires exact metadata equality.
`frameCursor.searchMetadataRef: {version: 1, paintBindingIndex: true}` advertises
support. The client restores the complete search snapshot before validating
frames or reconstructing history bundles. Mixed, dangling, wrong-kind, and
unopted default-response references are rejected.

If a scheduled capture reuses a frame already acknowledged by inspection, its
delta can legitimately omit that stamp. The recipe still requires the exact
current completion in returned history or its retained case pool, matching the
full frame identity and trace lifetime. Unbundled current capture facts alone, a matching
frame number from another owner, or an old trace cannot supply that proof; no
full reread is used to repair missing evidence.
Owned `getState` exposes canonical complete preflight at
`state.mainWindowPreflight`, not a duplicate `state.searchObservation.preflight`.
Completed frame search facts still carry their complete `search.preflight`.
The native safety baseline checks that complete frame value against the
top-level state value; existing selection/preflight checks use the canonical
top-level location. No frame facts or preflight checks are removed.

`unmount(target, expected?)` follows the explicit-expectation action contract.
In-case and final runtime retirement first cursor-inspect and retain the returned
page, then supply that exact identity without a hidden second read, refresh or
retry. Final retirement retains any newly observed frames in a
`search-runtime-retirement` effect before unmounting and clears its cursor only
after confirmed retirement. This cleanup observation supports no positive case
assertion; all of its output still counts against the runtime received-byte
ceiling. Omitted expectations preserve the SDK's existing full/default inspection,
but the search runner never uses omission to reread already acknowledged history.

Carry the observed target expectation into each action. Use real local GPUI
text/key/pointer events for user-path proof and semantic actions for agent-path
proof; never substitute native input. Natural frame evidence must follow the
responsible publication and match query/result/selection and window identities.
Forced capture is a separately labeled rendering check, not notification proof.
Pixel probes sample the retained completed frame without drawing again or
capturing the screen. Join selected-marker, row-content, and preview evidence to
that same frame; blank, stale, clipped, missing, or overflowed evidence cannot
prove a visible selection.

Each case owns a versioned `evidence.framePool` (`version: 1`). Capture phases
use `frameEvidence: {frameRef: n}`; observation/atomic phases retain ordered
`completedFrames` references, and counterexamples use the same references.
The zero-based IDs are local to that case: `frames[n]` contains frame `facts`
and ordered `paintBindingRefs`; `paintBindings[n]` contains the complete retained
`binding` plus an optional `metadataRef` into `metadata`. Repeated observations
share one frame entry, and unchanged bindings/metadata share their entries,
without dropping intermediate frames, causal facts, geometry, or pixel evidence.
The entire tables and all references count against the existing per-case byte
bound and failure reserve; an oversized insertion is rejected atomically.
Frame entries may also carry `factRefs` for exact repeated `search`,
`pixelEvidence`, and `nativeWindow` values, plus `ownerRef` for immutable frame
owner provenance. These references use the same metadata table; per-frame
identity remains explicit. Mixed inline/reference forms and wrong-kind entries
are invalid rather than silently substituted.

Use `reconstructSearchFrame(pool, reference)` from
`scripts/agentic/launcher-search-recipes.ts` to expand one retained frame.
`validateSearchFramePool(pool, references)` checks the version, every table
reference, and conflicting physical-frame identities. Numeric references remain
valid after the producer's privacy transformation: decoding a saved receipt
reconstructs its privacy-processed facts, not the original private values.
Deduplication hashes are internal only; no emitted-content hash is compared
against pre-redaction data.

Exact reused state/element observations may use a backward `observationRef`
within the same case's phases. Phase IDs and newly drained frame deltas remain
explicit; fresh observations never reuse an older record. Capture pixel evidence
may use `nativeSamplesFrame` to share the same retained frame's samples rather
than duplicating them. `resolveSearchJourneyReceipt` materializes phase, pixel,
and frame-fact references for callers without changing saved artifacts.
Low-level consumers can use `reconstructSearchFrameFacts`,
`validateSearchObservationPhases`, `reconstructSearchObservationPhase`, and
`reconstructSearchCapturePixels`. Forward, chained, mixed, dangling, and
cross-frame references fail closed.

Finalize compact owned receipts only after end/unmount, native process-group
exit, drained streams, and exact owned cleanup. Invalid cleanup overrides a
behavioral pass. Preserve the bounded counterexample and causal trace on failure.
An unsupported platform or unqualified hidden renderer is a specific blocked
capability, never permission to show the main application over the desktop.

## Entry points

```bash
bun scripts/devtools/devtools.ts list                 # dispatcher: all tools
bun scripts/devtools/devtools.ts elements snapshot --main --start
bun scripts/devtools/targets.ts inspect --focused    # or call files directly
```

Two transports:

- **Session CLIs** (`targets`, `elements`, `focus`, `keyboard`, `text`,
  `scroll`, `layout`, `surface`, `act`, `events`, ...) talk to a
  `scripts/agentic/session.sh` session (FIFO + response file, ~0.5–2s per
  command). Good for one-shot receipts and cross-process workflows.
- **Driver** (`driver.ts`) is event-driven over pipes (~10–50ms per step).
  `Driver.launch()` owns a fresh app process (sandboxed HOME available);
  `Driver.attach({session})` joins a running session.sh session and never
  kills the app. Use the driver for multi-step probes and fast loops.

```bash
bun scripts/devtools/driver.ts smoke                  # launch + rpc timing proof
bun scripts/devtools/driver.ts attach-smoke default   # join a running session
```

## Sessions

Session CLIs default to the shared session `default`. For parallel loops pass
`--session <unique>` or set `SCRIPT_KIT_DEVTOOLS_SESSION`; using `--start`
on the implicit shared session emits a receipt warning. Sessions live under
`/tmp/sk-agentic-sessions/<name>` (override root with `SCRIPT_KIT_SESSION_DIR`).

```bash
bash scripts/agentic/session.sh start my-probe        # start/reuse a session
bash scripts/agentic/session.sh health my-probe
bash scripts/agentic/session.sh stop my-probe
```

## Receipts

Migrated CLIs build through `lib/client.ts` `finishReceipt` and validate through
`lib/receipt-schema.ts`. Ordinary outputs use `emitValidatedReceipt` and
`ReceiptEnvelopeV2`: stable primitive, run/task, repository/producer, binary,
fixture/transaction, recursive privacy, evidence, assertion, interference,
cleanup, disposition, and validation fields. Only `EVALUABLE_PASS` exits zero;
blocked receipts exit 3 and invalid receipts exit 4.

Owned Design/Stories commit that sanitized detail once in `observation.json`.
Their final JSON/stdout is `script-kit-owned-receipt`, `receiptFormatVersion: 1`,
with a scalar summary, owner-marker digest, and existing artifact identities.
`lib/receipt-artifact.ts` verifies the retained files before semantic validation:
one expansion, canonical nonsymlink paths, exact owner/hash/size, a 1 MiB wire
limit, and 64 MiB per artifact. Keep the complete evidence tree at its recorded
location; summaries or copied receipt JSON alone are not proof. Missing,
tampered, stale-owner, nested, and obsolete inline-owned references fail closed.
Historical files are not migrated or deleted. The small exporter release proof
remains standalone V2, written once without a duplicate observation file.

Use `design diagnose --receipt <receipt.json>` for retained-proof validation;
it does not rerun the application. The classification vocabulary lives in
`schema.ts` — notable values:

- `ok` / `reproduced` / `fixed` / `not-reproduced` — proof outcomes
- `blocked-by-session-lifecycle` — session/forwarder/app process is gone;
  restart the session, don't retry the CLI
- `blocked-by-session-queue`, `blocked-by-response-timeout`,
  `blocked-by-parse-error` — precise transport failures
- `blocked-by-missing-primitive` — the app didn't expose what the tool needs

## Target selection (shared flags)

`--session <name> --target-id <id> | --target-kind <kind> [--target-index n]
| --target-title <text> | --target-json <json> | --focused | --main`
plus `--strict`, `--surface <SurfaceKind>`, `--timeout <ms>`, `--start`,
`--show`. Parsed by `lib/client.ts` `parseTargetArgs`; target resolution
happens in-process via `lib/target-identity.ts` (no subprocess hop).

## Library layout

- `lib/client.ts` — transport (`run`, `rpc`), arg parsing, receipt envelope,
  error classification, binary fingerprint. Start here for a new CLI.
- `lib/target-identity.ts` — window listing/inspection and strict target
  identity (`resolveTargetReceipt`, `maybeStartAndShow`).
- `lib/transport-errors.ts` — session.sh error-code → classification map.
- `driver.ts` — `ProtocolCore` (typed protocol surface) + `Driver` (owned
  process) + `AttachedDriver` (running session). Both support `await using`.

## Tests

```bash
cd scripts/devtools && bun test ./__tests__/
```

Run the complete proof-contract gate **exclusively**, with other Cargo builds
and native evaluator campaigns idle. It uses source-bound artifact fixtures
and may invoke reviewed Cargo contracts itself. Native Bun JUnit bytes bind
passing testcases to their actual files even when console output omits headings;
nested verifier output remains separate from the parent receipt/log.

```bash
bash scripts/verify.sh --only proof-contracts
```

Dirty-worktree gate receipts are diagnostic, never publishable release proof.
When requesting `SCRIPT_KIT_VERIFY_RECEIPT`, explicitly set
`SCRIPT_KIT_ALLOW_DIRTY_DIAGNOSTIC_EVIDENCE=1` and the reviewed colon-separated
`SCRIPT_KIT_DIRTY_EVIDENCE_OWNER_PATHS`; clean release evidence requires the
committed release source identity instead.

## Gotchas

- Codex-imp/seatbelt sandboxes cannot launch the GUI app. Launch the session
  outside the sandbox, then attach (`Driver.attach` / session CLIs) from
  inside. A wall of rpc timeouts right after a sandboxed launch is
  `blocked-by-sandbox`, not an app bug.
- Never run bare `cargo` here while `./dev.sh` may be running — build via
  `./scripts/agentic/agent-cargo.sh` (see CLAUDE.md).
- Owned evaluation always requires an immutable artifact reference. The legacy
  driver still supports explicit `SCRIPT_KIT_GPUI_BINARY`/`binary` configuration;
  that mutable path is not interchangeable with qualified owned proof.
