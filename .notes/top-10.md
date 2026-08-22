# Script Kit GPUI: the ten shipping-critical product programs

Updated and re-audited: 2026-08-22

Baseline commit: `aee92338d`

Status: audited implementation roadmap, **not a shipping approval**. The
committed baseline and the current uncommitted worktree are different states.
No program below is complete merely because a design, source audit, passing
unit test, partial implementation, or partial DevTools receipt exists.

## Product thesis

Script Kit should make scripts, scriptlets, built-ins, flows, AI actions, apps,
files, notes, and integrations feel like capabilities of one coherent native
product. Extension authors own functionality; the host owns the experience.

The target architecture is:

```text
source adapter
  -> canonical command descriptor
  -> shared interaction planner
  -> host-owned surface and components
  -> execution broker
  -> typed outcome and verifiable receipt
```

Every implementation decision should strengthen this chain. A new command kind
must not create another keyboard grammar, action menu, loading state, error
language, permission model, execution lifecycle, ranking policy, or test-only
automation dialect.

## Committed-baseline audit at `aee92338d`

The original August 2026 audit established the following facts from the
committed baseline. Some findings have since changed in uncommitted work; the
dated current-worktree re-audit below supersedes them wherever noted.

- `bun scripts/devtools/surfaces.ts` reports 37 surface contracts, 53 unique
  `AppView` variants, 54 contract mappings, and 11 coverage profiles.
- Of the 54 mappings, five have a statically declared `Direct` coverage
  binding, 48 are `Derived`, and one is `Unsupported`. The direct bindings
  name Actions Dialog, Agent Chat, Flow Session, Flow Desk, and Chat Prompt;
  they do not by themselves prove a fresh passing runtime receipt. All five
  corresponding coverage profiles are still `partial`.
- Every implemented coverage profile reports `partial`; Dictation History is
  `planned` and lacks six direct runtime primitives.
- At the committed baseline, `scripts/devtools/coverage.ts` mapped the main
  launcher to three nonexistent owner paths: `src/app.rs`,
  `src/app_impl/render_impl.rs`, and `src/widgets/script_list.rs`. Those paths
  are intentionally named here as historical defects, not implementation
  owners. The current uncommitted registry reports zero invalid owner paths.
- `tests/source_audit_inventory.md` currently reports 2,327 app-source reader
  sites across 350 test files; all reader classes total 2,815 sites across 410
  of 505 Rust files under `tests/`.
- `src/ai/agent_chat/ui/view.rs` is 17,839 lines;
  `src/scripts/grouping.rs` is 7,178 lines;
  `src/actions/dialog.rs` is 6,702 lines;
  `src/menu_syntax/main_hint.rs` is 4,396 lines; and
  `scripts/kit-sdk.ts` is 10,009 lines.
- At the committed baseline,
  `src/mcp_resources/mod.rs::SDK_NOT_YET_IMPLEMENTED_IN_GPUI` listed
  14 *claimed* unsupported SDK capabilities: `setStatus`, `keyboard.type`,
  `keyboard.tap`, `mouse.move`, `mouse.click`, `setPanel`, `setPreview`,
  `setPrompt`, `mini`, `micro`, `hotkey`, `widget`, `find`, and `menu`.
  This list is already stale: `hotkey()`, `mini()`, and `micro()` all have
  implemented SDK entries, real prompt routing, and live renderer owners.
  The isolated SDK auto-submit harness passes two `hotkey` cases plus three
  `mini`/`micro` cases; that proves SDK request/response behavior, while the
  native-user-path proof remains a separate runtime requirement. Treat all 14
  as items to reconcile, not 14 proven unsupported APIs.
- At the committed baseline, `bun run scripts/test-runner.ts --parallel`
  failed the existing SDK release gate: 171 cases, 158 passed, one failed, and
  12 skipped. The failing
  case is `tests/sdk/test-window-management.ts::stale-id-rejects`; its focused
  file reports three passed, one failed, and five skipped. This is a proven
  harness/bridge contract mismatch, not proof that native window safety is
  broken: `scripts/test-runner.ts` enables `SDK_TEST_AUTOSUBMIT=1`,
  `scripts/kit-sdk.ts::addPending` immediately supplies `moveWindow` its
  unconditional `{ value: '' }` success, while
  `src/window_control/registry.rs::resolve_legacy_window_id` already rejects
  unknown IDs and native actions return verified transaction outcomes.
- `tests/sdk/test-fields-datetime.ts` still asserts `fields()` is unimplemented
  and claims `Message::Fields` is unhandled, although
  `src/prompt_handler/message_route.rs` routes it to `ShowFields` and
  `src/prompt_handler/mod.rs` handles that prompt. Audit field-subtype runtime
  behavior separately; do not mistake the stale fixture narrative for proof
  that the entire API is missing.
- The root typing benchmark explicitly reports `measuresPaint = false`; its
  proposed 25 ms p50 gate is still marked `USER-RATIFICATION-PENDING`.
- `.github/workflows/perf-gates.yml` syntax-checks the typing benchmark, but
  its runtime gate executes only `root-search-frame-stability.ts`.
- Ordinary pull requests default to policy checks, formatting, and compilation;
  executed Rust tests require main-branch execution or the `full-ci` label.
- At the committed baseline, `.github/workflows/release.yml` delegated its
  Rust test gate to
  `scripts/verify.sh --only test-compile`, which runs `cargo test --no-run`.
- `.notes/CONSISTENCY-FIXES.md` already contains a validated 75-task program;
  `scripts/devtools/consistency.ts` already supplies fail-closed catalog,
  task, family, scope, and full-program auditing. Reuse those task IDs and
  receipts rather than creating a competing consistency ledger.
- The canonical `.notes/CONSISTENCY-FIXES.md` task catalog is currently
  gitignored/untracked even though `.notes/CONSISTENCY-PROGRESS.md` is tracked.
  No current GitHub workflow invokes the consistency auditor. A clean CI
  checkout therefore cannot enforce the existing catalog until the catalog is
  deliberately published to an approved tracked/generated path or provided as
  a trusted input artifact.
- Existing production foundations already include canonical
  `{category}/{identifier}` command IDs, shared conversation-command
  descriptors and dismissal rules, a production-wired pure window
  orchestrator, root-provider generation fences, transaction traces, AI
  first-visible-output tracing, a release manifest, and packaged
  resource/sidecar verification. The Raycast-compatible `ExtensionManifest` /
  `CommandMetadata` / `Command` types existed on disk in
  `src/extension_types/mod.rs`, but **were not compiled by either committed
  crate root**. Treat them as dormant source at the baseline, not as an existing
  production foundation.
- At audit time the Data volume had approximately 24 GiB free, below the
  agent Cargo wrapper's default 25 GiB floor. The small `sk-protocol` test
  suite completed, but full application builds require a fresh disk/capacity
  check rather than an automatic expensive build.

These values are baselines, not permanent constants. Refresh them at the start
of each program and store a dated receipt before making product claims.

## 2026-08-22 implementation checkpoint and safe verification

The owner's other CPU-intensive work initially pushed machine load above 500;
the earlier SDK timeouts were environmental, not a product receipt. After
lightweight capacity checks showed recovery, verification resumed with
`CARGO_BUILD_JOBS=2`, `--jobs 2`, isolated synthetic SDK workers capped at two,
and no competing Cargo-pool owners. No check launched the application, opened
or focused a window, injected native input, captured the screen, accessed the
microphone/camera, contacted an AI provider, or altered another process.

Evidence in this checkpoint is intentionally time- and source-qualified:

- At 09:37 local time, machine load rose again to **48.77 / 20.27 / 11.27**,
  so favorite-model, private AI-preset, and provider transport work continued
  source-only. The requested approximately 20-minute recheck at 09:56 showed
  recovery to **6.51 / 6.78 / 8.47** with **63 GiB free**. One bounded,
  two-job, explicitly noninteractive Cargo compile then succeeded; the current
  harness contains **15,149 executable library cases**, including all **28 new
  regressions**. Six exact single-threaded offline filters execute **20 passing
  cases**: six model favorites, six private system-prompt preset owners, seven
  typed provider result/cancellation/retry owners, and one final-only response
  owner. No app, window, provider, capture, real clipboard, external agent, or
  native input was started. The full application/library check, exact strict
  release Clippy command, and complete screened offline Rust suite were then
  genuinely refreshed against this same source: **497 passing behavior cases
  across 133 explicit filters**, zero strict-library lint warnings, and the
  same eight longstanding binary-only warnings. The larger library test
  harness retains 12 preexisting test-only warnings.
- A later source audit found that AI chats, Notes, Brain, and clipboard SQLite
  databases were opened before owner-only hardening; the clipboard background
  worker did not harden its files at all; existing permission fixes followed
  symbolic links and silently ignored failure. A shared replacement now
  prepares each primary/legacy sidecar as `0600` through an opened no-follow
  descriptor before SQLite access, adds SQLite's `SQLITE_OPEN_NOFOLLOW`,
  rejects planted primary/sidecar/parent links, rechecks materialized WAL/SHM
  files fail-closed, removes the duplicate clipboard owner, and prevents Notes/
  Brain recovery from moving hostile links. SQLite's no-follow flag also
  rejected macOS's ordinary `/var -> /private/var` alias, so the production
  owner resolves only the verified parent while keeping the final database
  filename no-follow. **All ten new isolated regressions pass**, along with
  five existing SQLite/recovery compatibility cases. The strict release
  Clippy gate was refreshed successfully against the final database owner.
- A subsequent private-document and semantic-owner audit found that canonical
  Brain/Notes/Day Page markdown, actual Brain indexing and day switching,
  fragment provenance, trash restore, transaction/element fingerprints, AI
  diagnostics, Dictation receipts, and legacy DevTools outputs still crossed
  weaker ownership boundaries. The real production owners now create/repair
  `0700` directories and `0600` no-follow documents before reads; reject
  hostile links, foreign roots, fragment traversal, and unsafe restore
  destinations; preserve repeated same-name trash entries; and replace
  publicly guessable private SHA-256/FNV hashes with the existing
  process-private keyed fingerprint owner. Dictation History's painted rows,
  semantic projection, and tracked scroll now share one actual entry ID.
  Launcher and Dictation History have truthful direct static owners, raising
  the complete 54-mapping inventory to **7 Direct / 47 Derived / 0
  Unsupported** while retaining **0/54 direct runtime proofs**. **Ten Brain
  substrate/indexer cases, 19 additional screened Rust behaviors, and 50
  DevTools tests with 212 assertions pass** without launching the app or
  touching the operator's computer. The 11:19 source-freshness audit correctly
  reduced acceptance from the earlier committed **15/75** to **0/75** while
  the follow-up was uncommitted: all 15 earlier offline receipts were
  `BLOCKED_STALE_GENERATION`, and all 60 runtime receipts remained absent.
  Regenerate actual safe producer receipts after the final grouped commits;
  do not reuse, timestamp-edit, or relabel stale evidence.
- A subsequent document-conflict audit found two real user-data loss paths:
  the main Day Page used a predictable, second-resolution recovery filename
  and could overwrite an earlier conflict; the separate Notes-window day
  editor discarded a non-append external rewrite without preserving it at
  all. Both real save paths now call the same Brain-owned conflict writer,
  which validates the source/root, creates private `0700` trash and exclusive
  `0600`/`O_NOFOLLOW` recovery files, suffixes every same-second collision,
  refuses hostile directory links, preserves planted-file targets, and logs
  only process-keyed path fingerprints. **Three isolated production-owner
  regressions, the real repeated Day Page save, its existing non-append
  compatibility case, and all 13 private Brain behaviors pass** without
  starting an application or touching live user data.
- A follow-up review found the same integrity failures in both private
  conversation indexes: Dictation History returned successful-looking IDs
  after failed writes, hydrated an empty provider over valid data after a
  failed read, and could discard malformed records during deletion; Agent
  Chat silently ignored completed-conversation persistence errors, parsed
  malformed index lines as if they never existed, and could delete a saved
  conversation before discovering its index was unsafe; private composer
  prompt recall had the same malformed-line and JSONL-boundary failures.
  Both owners now
  reject malformed JSONL without exposing private lines, repair legacy
  missing-newline boundaries through the opened owner-only descriptor,
  serialize save/delete/compaction ownership, reject invalid root snapshots,
  and preserve existing files before any unsafe rewrite. Dictation returns a
  real typed persistence result and leaves unavailable History actions
  disabled; Agent Chat saves the complete conversation and index together,
  protects submitted composer prompts with the same serialized private owner,
  shows safe recovery notices on either persistence failure, and replaces raw
  conversation-title, current-app automation query, generated-command name,
  launcher filter, grouped-cache-key, captured-window-title, and screenshot-
  artifact-path diagnostics with process-keyed fingerprints; provider/setup,
  background-title, model-selection, queued-submit, rewind, and Brain-ingest
  failures receive the same private diagnostic treatment. The separate
  Script Kit Selfie owner now encodes image bytes in memory and persists both
  screenshot/receipt through preflighted no-follow `0600` atomic files in a
  repaired `0700` directory; the grandfathered source audit no longer requires
  the previously unsafe `std::fs::write` implementation. The real Agent Chat
  Downloads export now creates owner-only exclusive files, suffixes repeated
  exports instead of overwriting earlier conversations, refuses hostile
  directories/destination links, and sanitizes session-derived filenames.
  Both focused-window and full-screen AI screenshot paths now also repair
  their shared temporary directory to `0700` and refuse hostile directory
  links before accepting any private image bytes; process/sequence-qualified
  Selfie identities prevent same-millisecond captures from overwriting each
  other. User-requested webcam photos now use the same shared exclusive
  no-follow `0600` owner as conversation exports, suffix every same-second
  collision instead of destroying an earlier image, reject hostile Desktop
  destinations, and expose only keyed diagnostic identities. The first real
  Dictation regression run then exposed an additional production mismatch:
  saved entries publish `preview`/`text` and `target`, while the shared
  provider-item reader previously accepted only `title`, silently hiding
  valid spoken history from generic provider consumers. The actual reader now
  resolves Dictation previews/text and destination labels while preserving
  strict Calendar/Notifications title rules. Thirty-one isolated regressions
  cover hostile links, corrupt records, durable-provider preservation,
  boundary repair, preflight ordering, and concurrent saves/deletes. The
  owner's machine load rose to **68.53 / 30.73 / 18.39** immediately after
  starting the bounded compile, so that compile was stopped; the requested
  approximately 20-minute recheck at 12:27 worsened to **281.47 / 149.11 /
  83.88**; a second recheck at 12:46 improved to **43.49 / 19.09 / 31.07**.
  Once load recovered, the genuine application harness completed and all
  **31/31 isolated Rust regressions passed**: six Dictation-history, two real
  provider-projection, seven Agent Chat persistence, four conversation-export,
  four synthetic Selfie artifact, two screenshot-directory, and six private
  exclusive/unique-file cases. The first six reran directly in **0.08s**;
  every group ran single-threaded without an app, capture, provider, camera,
  microphone, native input, or real clipboard. The provider correction is
  committed as `31e3940ff`. Lightweight,
  genuinely rerun facade-governance suites did execute **50 passing Bun tests
  with 115 assertions**, and the existing non-GUI design exporter produced
  byte-identical generated outputs; refreshed source-bound facade and export
  receipts restored the exact current clean commit to **15/75 accepted,
  60 missing, zero stale/invalid/failed receipts, and zero auditor errors**.
  This verifies the safe DevTools/code-generation obligations and the named
  isolated Rust behavior, not any interactive runtime task. Subsequent
  build-system/domain extraction changes require their own fresh application
  compile and source-bound receipt regeneration before being called complete.
- A lightweight owner audit then promoted eight heavily used launcher
  surfaces whose actual production renderer, semantic collector, and layout
  owner were already present but hidden behind host-wide Derived coverage:
  Clipboard History, Browser History, Notes Browse, File Search, Day Page,
  Current App Commands, Agent Chat History, and Webcam. Exact AppView-bound
  profiles now describe those real owners without claiming runtime behavior;
  File Search legitimately covers two contract mappings. The refreshed
  complete census is **37 kinds / 54 mappings / 53 variants / 19 profiles**,
  with **16 Direct / 38 Derived / 0 Unsupported** and still **0/54 direct
  runtime proofs**. Three inexpensive Bun suites execute **15 passing behavior
  cases and 150 assertions**, including explicit anti-fabrication checks;
  the complete source-fresh offline producer lane then executes **283 passing
  Bun cases / 1,103 assertions across 17 files**, restoring **15/75 accepted,
  60 missing, zero stale/invalid/failed receipts, and zero auditor errors**.
  None launches the app, captures the screen/camera, or requires Cargo.
- The actual full library and application check completed successfully through
  the prescribed two-job Cargo wrapper. The main binary still reports eight
  preexisting binary-only unused-import warnings; the release-required
  library target is warning-clean. The current check includes production
  launcher/Agent Chat command-catalog ranking and refresh, exact-identity
  attachments, source-owned shortcut/alias preferences, hidden-command
  exclusion, owner-bound destructive confirmations, symlink-safe private
  conversation/prompt/transcript-attachment persistence, shared owner-only
  Dictation/Flow transcript storage, collision-safe Notes recovery copies,
  source-owned Notes/Todos/Clipboard/Dictation/conversation background
  refresh, private preflight/Tab AI/current-app automation receipts and memory,
  owner-only screenshot files, AI phase/Quick AI trace streams, complete
  exported/handoff prompts, wrapper artifacts, private handoff receipts,
  Claude MCP/API-key/bearer-header configuration, custom agent credentials,
  private project history, serialized authentication-state persistence,
  owner-only atomic model favorites and user-authored AI preset system
  prompts/imports/exports, truthful favorite-save failure, typed persistent
  provider cancellation/error/retry, and final-only response delivery,
  transactional Quick AI startup cleanup, Notes/AI diagnostic redaction,
  shared Claude credential and
  system-prompt isolation, generation-safe browser providers, genuinely
  library-tested Brain Inbox freshness, present-surface footer authorization,
  permission-safe Agent Chat dismissal, and generated-script validation.
- The exact release command
  `./scripts/agentic/agent-cargo.sh clippy --jobs 2 --locked --lib --no-deps -- -D warnings`
  passed with zero warnings/errors after reducing the original 280-error
  release failure. macOS Metal cache access required the documented sandbox
  escalation; no application was launched, and this pass includes the newest
  favorite-model, AI-preset, and provider-result changes.
- The two release-required app-independent domain crates execute **60 passing
  tests**: 51 `sk-protocol` and nine `sk-clipboard`. The application library
  test target compiles all **15,149 library cases**, and a current,
  noninteractive, single-threaded behavior selection executes **497 passing
  tests across 133 explicitly screened filters**, including the real
  fresh-install seeded starter,
  canonical command readiness, scriptlet retention, typed AI recovery,
  clean-question context, password-manager clipboard privacy, exact owned
  process-group cancellation, root-provider generations, shared Escape,
  collision/symlink-safe generated files and receipts, both generation-path
  security policies, provider redaction, private JSON suppression, bare
  provider-token masking, complete private-key removal, canonical history
  match projection, stale browser cancellation, safe footer authorization,
  accepted-prompt preservation, subprocess-group ownership, transaction
  replay/privacy, genuinely lossless bounded same-process private-result
  recovery, credential- and system-prompt-free process arguments,
  non-reversible process-keyed launcher diagnostics, complete Brain Inbox row
  freshness, stable tied search rankings, side-effect-free unavailable
  commands, permission-pending conversation safety, rich browser-history
  routing, hidden-script suppression, exact ranking before truncation,
  scriptlet aliases/skill identifiers, refreshed Agent Chat command snapshots,
  duplicate-owner attachment identity and simultaneous owner preservation,
  collision-safe prompt plans, alias-aware sync/highlights/chips, hostile
  conversation/prompt storage, owner-only legacy-file migration, complete
  private-history deletion across all four owned stores, owner-only/no-follow
  transcript attachments and per-conversation cleanup, collision-safe private
  Notes conflict recovery, reusable owner-only/no-follow private-file
  read/append/boundary-safe JSONL/atomic replacement, private Dictation
  transcripts, owner-only AI preflight receipts, real current-app automation
  upserts, private Tab AI intent/generated-source memory and execution
  receipts, synthetic owner-only screenshot bytes, owner-only/no-follow shared
  and Quick AI traces, unguessable model-answer/reasoning/provider-error
  fingerprints, single-record concurrent trace writes without injected fsync
  latency, actual in-memory Pi and Flow transport phase/cancellation routing,
  actual owner-only/no-follow prompt exports, handoff wrappers and private
  receipts, hostile export-directory/file/receipt symlink rejection,
  cryptographically keyed prepared-context fingerprints, owner-only Claude
  MCP bearer/API-key configurations, preserved unmanaged servers, private
  custom-agent credential catalogs and project MRUs, non-clobbering
  concurrent authentication-state updates, malformed-state refusal, keyed
  export/gist diagnostic locations, and actual redacted screenshot/Notes/AI
  configuration diagnostic events, and nine real SDK-reference/capability
  inventory/permission/topology owner checks,
  legacy-permission repair, symlink-safe Flow primary and migration paths,
  source-owned Notes/Todos/Clipboard/Dictation/Agent Chat refresh,
  current-snapshot validation, legitimate empty-cache readiness, bounded
  explicit Notes result expansion, exact
  Notes/Force Quit/per-conversation
  confirmation, Notes/AI/sync log suppression, inert-row rejection, safe
  template escaping, transactional Quick AI startup, collision-resistant
  cross-project Flow transcript identity, owner-verified atomic legacy
  adoption, failed-claim privacy, exact source-owned same-name script and
  scriptlet selection/dispatch/deep-links, independently owned
  shortcut/alias preferences with legacy read/remove fallback, compatibility
  aliases, and the locked production glass-motion fixture. The six actual
  release integration targets execute **55 passing tests**; the production
  generated-script creation contract adds **four**, public Tab AI
  execution/memory contracts add **42**, and both screenshot-identity
  contracts add **ten**, for **111 passing cases across eleven targets**.
  The two current passive-search/source-filter structural suites add **21
  passing source-owner audits** without new source-reader sites. The exact
  lifecycle/deep-link compatibility audits and two transaction integration
  cases add **four more exact passing checks** without touching owner
  transaction logs,
  launching an app, or terminating another process.
- The strict synthetic SDK release suite executes **215 passing cases, zero
  failures, zero skips across 40 files**, retaining its real five-second
  timeout. Its screenshot helper consumes only a synthetic 1×1 fixture,
  requires synthetic auto-submit, writes to an OS temporary directory, and
  removes that directory afterward. Independently rerun filtered receipts
  prove eight real date/time/search `fields()` SDK cases, two `hotkey()`
  cases, and five editor/mini/micro cases; nine pure Rust SDK-reference and
  capability-catalog cases confirm implemented prompts are supported,
  all **18** genuinely unsupported capability entries are marked unavailable,
  native permission/platform boundaries are explicit, and catalog indexing
  stays stable until intentional invalidation. None of these synthetic
  receipts claims native user-path or permission proof.
- The current Quick AI latency benchmark's operator-safety precondition is
  already implemented and directly verified without a provider: `--help`
  exits successfully, `--describe-contract` truthfully returns
  `STATIC_INVENTORY` / `LIVE_AI` and `measuresPaint=false`, and a normal
  attempted run under `SCRIPT_KIT_NONINTERACTIVE=1` fails before provider
  startup even if a live opt-in is present. The historical unsafe-`--help`
  findings below describe the older committed/worktree snapshots, not the
  current production script.
- The complete noninteractive proof-contract lane executed **662 passing
  tests across 35 files with 2,492 assertions**. The release archive coverage
  includes one actual macOS `/usr/bin/ditto -c -k --keepParent` ZIP generated
  from an isolated synthetic signed application containing the required
  executable/Pi/SDK/Info.plist, migration and icon resources, a zero-byte
  signed resource, a real framework symlink, and its CodeResources signature
  envelope; no application is started or displayed. Full-workspace formatting
  passed, and the app-source-audit inventory reported no new guarded reader
  sites relative to `HEAD`.
- The actual standalone non-GUI `export_design_tokens` binary regenerated
  both design outputs into an isolated temporary directory. Its JSON and CSS
  bytes exactly matched the checked-in artifacts; the receipt binds the exact
  source commit, exporter binary, six current source-owner hashes, both
  output hashes, strict operator-safety facts, and completed cleanup. The
  new release evidence gate accepted that real receipt.
- The canonical consistency auditor currently proves **15/75** tasks:
  **2/2 static inventories, 12/12 offline unit-behavior tasks, and 1/1
  deterministic fixture task**. Tightening the shared receipt schema correctly
  invalidated the original receipts; each real offline producer was then
  rerun sequentially, including one brief bounded direct `rustc` proof, only
  after a fresh capacity check. The current auditor reports **zero blocked,
  invalid, or failed offline tasks** and **60 direct-runtime tasks still
  missing**. No receipt was replayed, relabeled, or made fresh without actual
  verification.
  Protected-source, generated-output, conflict-lifecycle, façade-lifecycle,
  privacy, and cleanup governance flags still pass.
  The GOV-001 owner task validates ten canonical ownership groups, five actual
  consumers, 21 bounded exact source identities, and eight intentional
  compatibility exceptions through 40 passing mutation/production tests.
  **0/60 direct-runtime tasks are proven**. The auditor correctly refuses a
  shipping verdict; static or synthetic evidence has not been relabeled as
  runtime proof.
- Focused synthetic security verification additionally closed four genuine
  noninteractive-tooling bypasses: unapproved subprocess commands, forged
  isolated-launch environment authority, unsafe outer batch envelopes, and
  mismatched requested/resolved target identity. Their bounded compatibility
  suite passed **133 tests and 622 assertions** before the final ownership
  tightening. That tightening also prevents `session.sh` or an attached
  transport from mutating/hiding an existing operator window; **10 current
  purely in-memory tests and 106 assertions** pass without starting even a
  subprocess, and the current combined proof lane passes all **701 tests**.
  Generated scripts now use exclusive final-byte creation, collision-safe
  actual identities, secret-redacted atomic receipts, and one shared
  precreation shell/slug policy; the shared AI diagnostic vault now suppresses
  JSON with no safe allowlisted fields instead of falling back to its raw
  private payload, also masks passwords/passphrases/bare bearer credentials
  and multiline private keys while preserving real `sk-protocol` crate names,
  and provider/persistent-session/local-model logs no longer expose raw
  prompts, credentials, response bodies, or stderr. The current Rust
  behavior selection includes direct passing tests for each new security
  contract.
- The actual main-menu `@scripts:`, `@scriptlets:`, and `@skills:` builders
  previously performed discovery-order substring filtering, truncated before
  scoring, fabricated score-zero matches, ignored aliases/skill identifiers,
  and exposed scripts explicitly marked hidden. A new library-tested Spine
  catalog owner delegates to the exact canonical root-search scorer, applies
  its hidden-command and metadata policies, preserves complete match evidence,
  sorts before the eight-row limit, and keeps deterministic tie order. An
  attempted refresh fix targeting the already-retired visual picker had no
  production consumer and was rejected. Instead, actual launcher startup plus
  full, incremental, and plugin-skill refresh publish one coherent immutable
  in-memory host snapshot, and the real detached Agent Chat Spine composer
  consumes it to replace all three previous "Coming soon" sections with
  selectable ranked results. The existing store-backed resolver still rejects
  launcher-only sources unless an explicit host snapshot is supplied,
  preserving its architectural decision lock. Selection resolves both the
  friendly token **and exact canonical resolution identity**, so plugins with
  the same visible label cannot attach another owner's file or content.
  A later adversarial check also proved that selecting each duplicate owner
  separately was insufficient: simultaneously selecting both still overwrote
  the alias map and collapsed them to the same `@file:SKILL.md` token during
  sync. The real launcher, detached composer, and attachment portal now share
  stable variant-plus-source owner identity and deterministic unbounded
  `-2`/`-3` suffixing; the alias parser preserves that exact identity, sync
  retains/removes the intended owner, both inline tokens highlight correctly,
  duplicate context chips stay hidden, and the final prompt preserves both
  complete files/scriptlet bodies. Built-in `@context` still canonicalizes to
  `@here`, and reattaching the same owner remains idempotent. Four canonical
  ranking, five snapshot/section/alias, three collision, two cross-owner
  parser/sync/chip, and one final prompt-plan tests pass without plugin
  discovery, disk reads, subprocesses, GUI, or native input.
- A later exact-owner audit proved that two scripts or scriptlets inside the
  same plugin still shared `script/{plugin}:{display-name}` or
  `scriptlet/{plugin}:{display-name}` even when they came from different
  source files/anchors. Passive reorder, semantic row IDs, canonical
  descriptors, share links, and first-match deep-link dispatch could therefore
  select or execute the wrong automation. Public name IDs and exact-query
  history remain backward-compatible, while actual selection, descriptors,
  copied share links, and opt-in config hotkey precedence now use stable
  length-framed SHA-256 of the lexically normalized source path plus a
  scriptlet's real anchor/command. Existing bare and plugin/name links remain
  accepted; source IDs round-trip through the real command parser and select
  only their exact owner without exposing private filesystem paths. The
  source-less compatibility case, mutable-display-name stability, duplicate
  script/scriptlet dispatch, normalized-path equivalence, deep-link
  round-trip, and existing descriptor aliases are covered by real library
  behavior tests. A prospective test in the app binary's disabled local test
  module was rejected; the actual production share-link planner instead lives
  in the compiled library and is consumed by the real launcher action.
  A follow-up audit found that Add/Update/Remove Shortcut and Alias still
  persisted the colliding legacy plugin/name ID even after execution identity
  was fixed. The real action owners now write and remove the selected
  command's exact source-owned ID; focused rows, preview panels, alias
  editing, alias execution, and the live alias registry all resolve the exact
  owner first while retaining historical plugin/name values as a read/remove
  fallback. Same-name sources keep independent preferences, removing either
  cannot delete its sibling's binding, and a source-less command retains its
  original compatible ID.
- Flow transcript persistence previously flattened path punctuation to `-`,
  discarded all but the last 160 path characters, and silently rewrote the
  stored `flow_id`/`flow_path` to whichever project loaded the resulting
  colliding file. The same flaw affected the FIFO writer's initial revision;
  a failed id-only legacy persist could also disclose one private transcript
  to multiple projects. The actual production store now uses a bounded,
  readable filename plus a full length-framed SHA-256 of the original flow ID
  and full definition path. Current snapshots fail closed on wrong embedded
  owner; old path-qualified files migrate only for the exact embedded
  ID/path; legitimate v0-v3 id-only files with an empty legacy path can be
  claimed once only after an atomic same-directory rename. Foreign, future,
  malformed, or unclaimed histories are never returned, relabeled,
  overwritten, or deleted. Four legacy-version migrations, same-ID separate
  projects, punctuation/truncation/ID collisions, hostile primary snapshots,
  exact-owner path migration, foreign legacy IDs, failed atomic claims, and
  normal malformed-thread canonicalization all pass using isolated temporary
  directories; no Flow process or provider is started.
- Force Quit previously hid the launcher and started termination immediately;
  permanent Notes deletion re-read the current selection after confirmation;
  individual conversation deletion executed immediately on Cmd+Backspace;
  and Agent Chat "clear history" could run without confirmation while leaving
  submitted prompt history behind. Force Quit now uses the actual owning
  window's destructive confirmation and revalidates the full immutable app
  name/bundle/path before any HUD, hide, subprocess, or termination. Permanent
  deletion uses only the originally confirmed still-trashed note; cancellation,
  owner loss, changed target, restored notes, and missing notes are inert.
  Individual conversation deletion now also requires its real parent-window
  modal and refuses to run if its selected session changed before acceptance.
  Detached Agent Chat global deletion uses the same owner-bound confirmation, preflights
  every exact owned store before mutation, rejects symlink/wrong-type targets,
  and deletes saved conversation files, the conversation index, typed prompt
  history, **and private generated history attachments** before invalidating
  its in-memory cache. Shared grouped-list
  navigation also skips nonselectable status/reserved rows rather than treating
  every non-header entry as executable.
- Conversation identifiers now use one explicit validation policy across
  save/load/existence/rename/delete: actual `warm:<uuid>` and Unicode IDs stay
  valid, while empty/dot/traversal/drive/control-byte names fail before any
  mutation. Conversation directories, indexes, and per-session files reject
  symlinks; payload identity must match its requested session; `O_NOFOLLOW`,
  owner-only `0600`, same-directory exclusive temporary files, and atomic
  replacement prevent hostile path redirection. Prompt-history append/load
  now apply that same policy. A shared opened-file-descriptor guard also
  repairs preexisting world/group-readable conversation indexes, prompts, and
  conversation files to owner-only permissions **before** their private data
  can be read or appended. Previously overlooked generated Markdown
  summaries/full transcripts now use the same validated session/payload
  identity and exclusive atomic `0600` no-follow writer inside an opened,
  symlink-rejecting `0700` attachment directory; an older permissive
  directory is repaired before the private write. Deleting one conversation
  preflights and removes only its own summary/transcript, while global clear
  preflights all four stores before removing any of them. Prompt history preserves
  trimmed consecutive deduplication and oldest-to-newest ordering,
  and atomically compact to the newest 200 private prompts. AI template
  rendering escapes JSON string values in one nonrecursive pass, preserving
  quotes/newlines/literal placeholders without code injection; first-run
  starter scripts render all final bytes before exclusive final-path creation
  and never reopen an attacker-replaceable destination. Notes same-second
  conflict backups likewise use a brand-new `0600`/`O_NOFOLLOW` handle, sync
  their complete private contents, and suffix real collisions `-2`, `-3`,
  rather than truncating an existing recovery file or following a planted
  symlink; diagnostic errors contain only keyed path fingerprints.
  Conversation/prompt migration, attachment privacy, selective/global
  deletion, Notes collision/symlink resistance, generated-file, and template
  behavior cases use only isolated temporary directories or in-memory data.
- A cross-surface persistence audit then found that Dictation stored entire
  spoken transcripts through ordinary `File::create`/append (`0644` under a
  typical umask), followed planted symlinks on read/write, and truncated the
  old transcript before a compaction rewrite; Flow's new exact-owner filenames
  still followed primary and both legacy symlink targets. The shared
  `src/atomic_file.rs` private-file contract now rejects nonregular/symlink
  destinations, repairs older permissive files through their already-open
  `O_NOFOLLOW` descriptor **before** exposing/appending private contents,
  creates new files at `0600`, and writes complete replacements through a
  unique exclusive `0600` sibling plus sync and atomic rename. Its shared
  directory owner creates each private directory at `0700` from its first
  appearance, rejects symbolic links/non-directory targets, and repairs
  legacy permissions through the same opened `O_NOFOLLOW | O_DIRECTORY`
  handle before exposing private children. Actual
  Dictation append/load/delete/compaction and Flow primary/path-qualified/
  id-only migration all use this real shared production owner. The same owner
  now protects AI preflight audit append/read/atomic compaction, current-app
  automation prompts/raw queries/recipes, Tab AI natural-language intent and
  generated source, Tab AI execution receipts, focused/full-screen PNG
  files, actual exported Agent Chat prompts, external-agent handoff prompts,
  executable handoff wrappers, both handoff/export receipt types, Claude MCP
  configurations containing private bearer headers/API-key environment
  variables, managed-MCP ownership state, custom Agent Chat agent credential
  catalogs, private project-directory MRUs, and authentication/runtime-state
  snapshots. The newly edited favorite-model owner also uses the same private
  `0600` no-follow reader and atomic writer, refuses malformed/symlinked state,
  serializes complete read/toggle/write transactions, and surfaces a truthful
  retryable UI message instead of pretending an unsaved favorite succeeded;
  all four new regressions plus two existing compatibility cases pass. A second
  owner applies the same private atomic
  contract to actual AI-preset system prompts and user-selected import/export
  files, repairs permissive legacy stores before exposing prompts, refuses
  symlinked stores/imports/exports, preserves malformed recoverable prompts
  across create/import/delete, serializes concurrent preset creators, and
  removes raw preset names/paths/errors from real launcher diagnostics; all six
  isolated regressions pass.
  The actual persistent/spawned Claude transport now classifies explicit
  provider `is_error` facts and error subtypes as failures, preserves structured
  provider failure messages for the existing redaction/classification owner,
  rejects missing/blank final responses, and forwards a final-only answer
  exactly once when no streaming chunks were emitted. The persistent provider
  also honors the real callback's user-stop decision, kills/reaps only its
  owned child before returning typed cancellation, never converts Stop into
  provider failure, and refuses to duplicate an accepted provider failure or
  an already-partially-streamed request through fallback. Seven pure
  parser/cancellation/retry cases and one final-delivery case pass without any
  provider startup.
  Independent auth-state background workers now acquire one owner
  lock before reading, merging, and atomically replacing the private file;
  eight simultaneous agents retain all eight records, stale initialization
  cannot downgrade authenticated state, and malformed/foreign state is never
  silently overwritten. Its JSONL append repairs an unterminated older
  record through the
  same opened no-follow descriptor, without rereading the full audit log;
  its separate observability append keeps the same no-follow/owner-only/
  single-record guarantees without adding fsync latency to measured AI
  phases. Both shared cross-surface and Quick AI trace writers now use that
  owner, repair older permissive traces before append, reject hostile
  symlinks, and fingerprint private model answers, reasoning, provider errors,
  and query text with ephemeral process-keyed HMAC instead of publicly
  guessable SHA-256. Actual prepared-context receipts use the same
  cryptographically random process key instead of a predictable PID/timestamp
  salt. Actual six-path launcher/Agent Chat handoff diagnostics preserve the
  exact private interoperability receipt while exposing only keyed prompt,
  export-path, and private-gist URL fingerprints. Errors and actual
  screenshot/configuration diagnostics contain only keyed fingerprints,
  never private paths, window titles, client/profile names, prompts,
  credentials, or provider prose.
  Empty/malformed stores and legitimate older migrations preserve
  compatibility; hostile symlinks never expose, replace, or corrupt another
  file. Eight shared directory/file, seven complete preflight, four real
  current-app automation, three Tab AI persistence, three synthetic
  screenshot, 14 cross-surface phase-trace, five Quick AI trace, three actual
  Pi/Flow in-memory phase/cancellation transport, eight real prompt
  export/handoff/receipt/wrapper/diagnostic, two actual prepared-context
  fingerprint, 20 production Claude MCP/custom-agent/project-history/auth
  owners and compatibility cases, 18 complete isolated Dictation, and two
  targeted Flow privacy cases pass without screenshot capture, microphone,
  providers, clipboard access, external-agent execution, or GUI.
- Quick AI startup now atomically reserves its exact thread generation and
  transfers an owned child, registered process group, and scratch directory
  through a fail-closed RAII guard. Stdout/stderr/thread-start failure cleans
  up only the verified owned generation; uncertain cleanup remains tracked;
  stale cleanup cannot erase a newer turn; and retry becomes available only
  after ownership is safely released. Its user-search trace fingerprint now
  uses the same ephemeral keyed HMAC as other private diagnostics instead of
  reversible public SHA-256. Actual Notes search/external refresh/save/error
  logs and Agent Chat slash, file, mention, storage, tab-context, and
  automation-memory events likewise retain only keyed identity and byte counts;
  the shared mention-sync event no longer dumps the complete raw token set;
  never typed search text, private paths, prompts, provider errors, or labels.
  Pure production-event subscribers prove the real structured events rather
  than asserting on source strings.
- A later production-path audit found that stale browser tabs/history
  completions published global snapshots and launched favicon work before the
  query fence, while conversation/dictation long-text matches discarded their
  existing visible-field evidence. Browser providers now check the current
  generation first, discard only the matching obsolete in-flight request,
  preserve snapshots/backoff, re-arm the actual current query, and never run
  stale favicon publication. Their direct helpers are now strictly
  snapshot-only: they cannot spawn an unfenced worker, replace a newer query's
  results, inspect browsers, or start favicon networking. Root
  `@browser-history` cold searches are scheduled through the existing
  generation-fenced host coordinator, using six executable library-owned
  eligibility tests. A follow-up audit found the same latent UX failure in
  all three other private passive providers: Clipboard, Dictation, and Agent
  Chat each spawned its own untracked cache thread from a supposedly read-only
  render path, published before the live query fence, and never notified the
  active launcher after cold results arrived. A subsequent full-provider audit
  found the same ownership defect in Notes and Todos, including explicitly
  filtered SQLite queries and scans of up to 30 day-page files on the actual
  keystroke/render path. All five actual launcher providers now use one
  library-owned source-plus-generation worker lifecycle; at most one refresh
  per provider runs, stale/foreign completions cannot release or publish
  another worker, Notes cache epochs and private snapshots are revalidated
  before install, both typing/immediate-set paths reconcile visible results
  while preserving the selected command, and passive **and explicit** source
  filters perform cache-only foreground grouping. Successfully loaded empty
  Notes/Todos/Clipboard caches remain warm rather than respawning forever.
  Explicit `notes:`/rich Notes searches now honor their requested bounded
  result count instead of silently reclamping every query to five rows.
  Notes successful/metadata/fallback/failure events no longer emit raw
  search text, database paths, or errors; real tracing subscribers verify
  keyed identity and byte counts.
  Real in-memory/isolated-filesystem cases cover stale generations, foreign
  providers, nonzero wraparound, changed private snapshots, and empty caches
  without touching the real clipboard, microphone, or screen. History
  projections retain the real visible title/subtitle indices
  and report hidden transcript matches without inventing highlights. Brain
  Inbox snapshots now compare all eight actual item fields, replace edited
  same-identity rows without moving selection, and bump the epoch exactly
  once; their production comparator lives in the library-tested root-search
  owner rather than the binary's disabled test module. Brain lexical/vector
  fusion, cosine ranking, and signal-topic aggregation previously sorted tied
  randomized `HashMap` values by score alone and then truncated them, so the
  selected document or retained topic could change between identical refreshes.
  All three real production paths now sort by descending score and ascending
  stable document/topic identity **before** truncation; four executable pure
  tests prove reversed input, tied top-one selection, tied top-16 membership,
  and preservation of stronger-score precedence. All browser/cache,
  eligibility, inbox, and canonical-ranking behavior tests pass without
  launching providers or contacting the network.
- Default-on launcher and Agent Chat logging previously retained full search
  queries, terminal commands, notes, URLs, private paths, clipboard/context
  labels, prior conversation titles, and even entire first-message AI prompts.
  A shared private log value now retains only original byte length and an
  RFC 2104 **HMAC-SHA-256 keyed with a private ephemeral UUIDv4 per process**;
  the key is never logged, exported, or persisted. A public unsalted SHA-256
  would still expose progressively typed filters: an observer could test each
  possible next character against consecutive digest records. The keyed
  representation prevents that prefix-reconstruction attack while retaining
  same-process diagnostic correlation. Production filter, Brain/file search,
  fallback execution, history, attachment, portal, handoff, export, and
  ghost-text owners use the same representation instead of cleartext. Per-key
  Agent Chat telemetry was another reconstruction channel: individual
  characters are now represented only by length and structural facts, never
  raw text or a trivially reversible one-character digest. Executable pure
  tests prove both RFC 4231 HMAC vectors, long-key normalization, process-local
  stability, resistance to public typed-prefix guesses, absence of canary
  contents, and safe production structured-JSON output.
- Native footer callbacks previously executed stale or invisible Run/AI/Stop
  actions after their owning surface disappeared. One pure shared
  authorization planner now admits only currently enabled buttons, exact
  currently visible left affordances, or independently verified current
  header context chips. Missing configuration, disabled actions, arbitrary
  header overrides, and stale sensitive actions fail before popup dismissal
  or any other side effect. Four adversarial pure planner tests pass; the
  locked glass-motion calibration remains independently unchanged.
- The real launcher submit route previously added and saved private search
  history, invalidated grouped results, and reached clipboard/attachment
  branches **before** rejecting a disabled command. Production now checks the
  exact existing canonical availability reason immediately after resolving the
  selected row and before any history, cache, clipboard, portal, frecency, or
  execution side effect. The same actionable refusal still reaches the user;
  three executable library tests cover ready, malformed, unsupported,
  permission-pending, and zero-side-effect rejected submissions.
- Agent Chat previously treated a tool-permission request as idle: Retry,
  New Conversation, and Close could be offered, while Escape, Cmd+W, and the
  native close callback could destroy the still-pending turn. One shared
  library-tested active-turn planner now treats waiting approval as active,
  disables Send/Retry/New/Close with the exact `WaitingForPermission` reason,
  and routes both production dismissal paths through the same overlay-first
  resolver. Pending approval never advertises Stop because the actual stop
  handler only supports streaming; normal streaming keeps its real Stop,
  idle conversations retain all valid actions, and overlays still dismiss one
  at a time. Four executable pure tests cover pending, streaming, idle,
  contradictory status flags, typed refusals, and overlay precedence.
- Process cancellation previously unregistered live process groups, allowed
  an orphaned Codex server to be replaced, and destroyed already accepted AI
  prompts on reader/thread failure. Exact owned positive PID/group identity
  now remains tracked through graceful termination and bounded escalation;
  zero, invalid, untracked, uncertain, or wrong-group signals fail closed;
  terminal cleanup requires an observed dead owned group; and queued prompt,
  model, sandbox, and mission context survive recoverable transport failure.
  The focused tests use pure synthetic liveness observations and never signal,
  inspect, launch, or terminate a real process.
- Both persistent and ordinary Claude providers previously copied API/OAuth
  settings into their command-line arguments; the persistent provider also
  logged its full raw argument vector. Both now use one validated launch
  contract that moves supported credentials only into the private child
  environment, leaves inline settings limited to nonsecret isolation/tool
  policy, rejects conflicting or unsupported sole authentication sources,
  and logs safe metadata only. The exact locally installed Claude binary also
  contains the `--system-prompt-file` option, and an installed CLI definition
  describes that option as reading the system prompt from a file. Both production
  launchers now supply `--system-prompt-file /dev/fd/N` through the same
  anonymous, bounded Unix pipe: the private prompt never appears in argv,
  environment variables, logs, or a temporary file; unrelated descriptors
  remain close-on-exec; partial writes are deadline-bounded; and delivery
  failure reaps only the newly owned child. Hostile-canary and real anonymous
  pipe tests prove the transport without launching Claude or contacting a
  provider. Actual installed-provider execution and streaming remain
  intentionally unproven under the current no-live-AI constraint.
- The transaction recorder's supposed fingerprints previously stored complete
  JSON command payloads, while disk traces, MCP resources, poll snapshots,
  semantic IDs, replay results, timeout logs, and provider errors could expose
  private user data. Fingerprints are now real SHA-256 digests; all immediate,
  persisted, compacted, historical, and MCP traces apply one idempotent safe
  projection; failures retain typed actionable summaries without private
  details; replay honors the current `off`/`onFailure` policy on both executor
  and live prompt-handler paths; continued batches retain their first failure;
  unsupported commands remain present in replay receipts; and schema/examples
  now advertise the real `onFailure` wire value. Successful same-process
  selection replay now restores the exact original private result from an
  in-memory-only vault keyed by request ID, canonical command fingerprint,
  and result index; publication occurs only after safe trace persistence.
  Entries expire after 120 seconds and are bounded to 128 entries, 256 KiB
  total, 32 KiB per value, and 512-byte request IDs; wrong identities,
  malformed digests, failed/empty results, expired entries, persistence
  failures, and poisoned locks fail closed. Five executable pure tests prove
  lossless warm replay without exposing user content on disk, in trace JSON,
  or through MCP. Cold restart/cross-process replay intentionally cannot
  recover private values without a separately designed secure persistence
  policy.
- Performance enforcement now accepts only exact owner-ratified approval
  status plus a real positive observed sample count; `UNRATIFIED`, invented
  approval labels, omitted samples, NaN, and fractional counts fail closed.
  Hidden runtime-target generations must come exclusively from canonical
  `state.surfaceContract.targetIdentity`; top-level spoofing or conflicting
  secondary identities can no longer manufacture valid target proof. The two
  exact no-runtime suites pass **21 tests with 135 assertions**.
- A later receipt-privacy audit additionally found that unclassified password,
  API-key, bearer-token, cookie, and private-key fields could remain cleartext,
  and approved synthetic-cleartext mode also exposed credentials. Both now
  fail closed; receipt registry/producer fingerprints include the exact shared
  privacy, evidence-classification, and task-policy sources, while both
  producer validation and the consistency auditor verify every executed test
  suite and production owner's actual current bytes. Focused pure suites pass
  **105 tests and 520 assertions**. The shared-policy source change correctly
  invalidated earlier receipts; all real offline producers were rerun
  sequentially. The later provider/target/footer changes correctly invalidated
  the GOV-001/PF-002 source-bound receipts and the protected footer inventory;
  both genuine producers and the explicit static protected-source manifest
  were regenerated only after their relevant behavior/calibration checks
  passed. The later Agent Chat privacy changes again correctly invalidated
  GOV-001/GOV-002 and the two-façade lifecycle ledger; the actual façade
  inventory was regenerated and both genuine task behavior producers were
  rerun against their current source owners. The latest actual GOV-001 producer
  executes **40 passing ownership tests**, and GOV-002 executes **50 passing
  façade lifecycle tests**; neither receipt was copied or timestamp-refreshed
  without running its real producer. The final auditor again reports
  **15 passing offline tasks, zero stale/blocked/invalid/failed offline tasks,
  all governance/privacy/cleanup checks passing, and exactly 60 missing
  real-runtime tasks**. The newest strict lint, domain/integration tests, SDK
  suite, proof suite, formatting, and source-audit guard were independently
  refreshed against the final source snapshot.
- Packaged verification now compares every root/icon/font/license/SDK/
  migration asset against its authoritative bytes and rejects symbolic-link
  escapes for bundled executables, manifests, resources, and owning
  directories. The 54-surface release gate independently derives canonical
  contracts, reconstructs coverage from raw schema-validated primitive
  receipts, and joins exact AppView/window/host/parent/generation/source/
  binary/transaction identities; labels or producer-supplied pass flags alone
  cannot qualify. Its latest focused no-launch synthetic validator run passes
  **21 tests and 98 assertions**. Synthetic fixtures prove rejection logic,
  not real runtime coverage: actual candidate proof remains **0/54**.
- Release audits first proved that a plain-text file could masquerade as the
  signed ZIP, then that verifying only four important archived files still
  allowed migration scripts, icons, framework content, file permissions, or
  the signing envelope to be replaced without detection. Manifest schema
  **v3** now binds both the exact executable/Pi/SDK/`Contents/Info.plist`
  identities and a canonical SHA-256 tree covering **every signed app file
  and framework symlink**: normalized sorted paths, entry kind, Unix mode,
  exact byte count, and content/symlink-target hash are length-framed before
  hashing. The actual verified `.app` tree must match the ZIP at capture;
  final Ubuntu verification independently rebuilds the full tree from the
  archive and requires the same tree hash/count without needing `--app`.
  The bounded, dependency-free central-directory inspector also requires the
  real `Contents/_CodeSignature/CodeResources` envelope and rejects plain
  text, added/deleted/tampered signed files, changed permissions or framework
  link targets, missing/duplicate or Unicode/case-aliased entries, APFS-style
  aliased ancestor symlinks, traversal, foreign app roots, unsupported
  ZIP64/multidisk/encrypted/compression forms, inconsistent local headers,
  and symlinked required members. Real macOS `ditto` output, genuine framework
  symlinks, empty resources, deflate/data-descriptor archives, and app-less
  downstream verification pass in isolated offline tests, including forged
  outer ZIP hash/size negative controls. Existing signing/notarization/runtime
  requirements are not weakened; exact signed candidate journeys and 54
  runtime targets still remain unproven.
- Exact-candidate first-install, permissions, migration, and mock-AI packaged
  journeys; the complete 54-mapping direct-runtime matrix; signed/notarized
  distribution; and owner-ratified genuinely painted latency evidence do not
  exist. The overnight grouped commits deliberately publish the portable
  75-task catalog and all new proof/source owners instead of leaving them
  ignored or untracked. Committing those inputs removes the earlier
  clean-checkout source blocker; it does **not** fabricate the missing live
  behavior, packaged-journey, signing, or painted-latency evidence.

Current program status:

| Program | Honest status | Present implementation and remaining gate |
| --- | --- | --- |
| 01. Command contract | Partial | Typed durable descriptors reject malformed/forged identities, project real availability into launcher rows, Actions, preflight, footer, and command doctor, and fail closed before process spawn; same-name scripts/scriptlets now use source-owned SHA-256 identity for actual selection, descriptors, share links, exact dispatch, independent shortcut/alias creation/edit/removal, focused/preview display, alias execution, and hotkey precedence while preserving historical plugin/name read/removal and query-memory aliases; actual context searches preserve hidden-command policy, canonical scores, metadata evidence, and exact owner identity; launcher submits refuse blocked rows before any side effect. Full source-family/runtime parity remains unproven. |
| 02. Navigation and focus | Partial | Physical bubble, simulated launcher Escape, Actions dismissal, current-surface footer/header dispatch, and inert-row navigation share behavior-tested pure planners; Agent Chat gesture/native close preserve pending permission work; destructive Force Quit, permanent Notes deletion, individual conversation deletion, and complete Agent Chat history deletion require actual owner-bound confirmation plus immutable target revalidation. Full real-host keyboard/focus/confirmation matrices remain unexecuted. |
| 03. Design system | Partial | Shared presentation and canonical conversation/popup owners have freshly regenerated façade/conflict governance evidence, and actual exporter JSON/CSS bytes match exactly; surface-by-surface rendered parity remains unproven. |
| 04. Deterministic search | Partial | Root generation contracts now cover browser tabs/history plus real Notes/Todos/Clipboard/Dictation/Agent Chat workers: each source has one owned generation, rejects stale/foreign snapshots before publication, refreshes the actual launcher without moving its selected command, and never blocks passive or explicitly filtered foreground grouping on SQLite/JSONL/30-day markdown scans; empty Notes/Todos/Clipboard caches cannot respawn forever, and explicit Notes result limits no longer silently clamp at five. Same-name script/scriptlet rows retain exact source-owned selection and independent aliases across asynchronous reorder; direct lookups cannot spawn unfenced work; actual launcher attachments rank before truncation, suppress hidden scripts, honor aliases/skill metadata, and retain canonical evidence; refreshed immutable snapshots update detached Agent Chat families; duplicate owners survive suffixing, sync, and prompts; Brain rankings resolve ties by stable identity. Full indexed-provider coverage and realistic catalog/paint gates remain unproven. |
| 05. End-to-end latency | Blocked honestly | Observation kinds, exact owner approval, and actual positive sample counts are fail-closed; no owner-ratified painted measurements or permission to run visible performance journeys exist. |
| 06. Execution lifecycle | Partial | Exact owned process groups remain tracked until verified dead, invalid/unowned PID zero cannot be signaled, Codex server replacement cannot orphan a predecessor, and real Quick AI startup reserves its exact generation before side effects and unwinds owned child/group/scratch failures transactionally; same-name command dispatch resolves the exact normalized source; Flow FIFO initialization cannot adopt a colliding project's revision/transcript, follow primary/legacy symlinks, or persist world-readable private turns; transaction traces redact private content, same-process replay is bounded/lossless, and continued batches retain their first failure. Secure cross-process replay and complete command-family receipts remain unproven. |
| 07. AI consistency | Partial | Actual detached Agent Chat discovers scripts/scriptlets/skills through refreshed launcher snapshots, preserves multiple same-name selected-owner attachments, confirms individual/complete history deletion across all four stores, and protects conversation/prompt/full-transcript attachment files with validated IDs, owner-only `0700`/`0600` no-follow migration, spoof rejection, and atomic writes. Flow, Dictation, AI preflight receipts, current-app automation prompts/recipes, Tab AI intent/generated-source memory/execution receipts, actual screenshot PNGs, exported/handoff prompts, executable handoff wrappers, handoff/export receipts, Claude MCP bearer/API-key config and ownership state, custom agent credential catalogs, private project MRUs, authentication state, model favorites, private user-authored AI system-prompt presets/imports/exports, and real shared/Quick AI traces now share owner-only, no-follow private directory/file ownership and safe legacy repair. Auth-state, favorite, and preset workers serialize actual read/merge/atomic replacement so simultaneous agents cannot erase one another; malformed private state refuses destructive replacement and failed favorite saves surface an honest retry. Provider-declared error facts, empty/missing final responses, exact owned-child cancellation, nonduplicating fallback, and final-only response delivery are behavior-proven without live providers. Private answer/thought/query/diagnostic/prepared-context/prompt/export-path/gist/preset fingerprints use the cryptographically keyed process owner, trace appends remain concurrent-safe without injecting fsync latency, and AI diagnostics never expose private paths/window titles/project names/errors. Actual in-memory Pi and Flow transport paths emit complete private phase milestones and distinguish cancellation from completion. Flow retains exact ID/path SHA-256 identity and one-time legacy adoption. Notes/Todos/Clipboard/Dictation/conversation cold search has generation-fenced launcher-owned refresh. Notes conflict recovery remains private/collision-safe; all success/fallback/failure AI/Notes diagnostics use keyed fingerprints. Existing coverage also proves clean-chat entry, typed recovery, pending-permission safety, accepted-prompt preservation, credential-free Claude argv, and anonymous system prompts. Actual provider-backed transport and live target-scoped recovery remain unproven. |
| 08. SDK compatibility | Partial | The full isolated suite passes 215/0/0; separate filtered runs prove eight fields date/time/search cases, two hotkey cases, and five editor/mini/micro cases; nine real Rust capability owners mark implemented prompts supported, mark all 18 genuinely unavailable capabilities unsupported, and enforce explicit native permission/platform boundaries. Generated scripts validate capabilities/topology and shared shell/slug policy, escape hostile JSON template values nonrecursively, and write complete final starter bytes through the original exclusive file handle; real seeded/template scripts declare parseable supported capabilities; Rust behavior plus the four repaired creation-contract cases prove collision/symlink-safe files and receipts. Native prompt/window/permission proof remains absent. |
| 09. Behavioral proof | Partial | All 15 static/unit/fixture task receipts, the two-façade lifecycle inventory, and protected-source hashes are current; hidden target identity rejects fabricated fallback generations; the full proof lane passes 701 tests; the canonical 75-task catalog and every CLI subcommand resolve the now-committed portable catalog; and the auditor reports 15/75 with no stale/invalid/failed tasks after genuinely rerunning both invalidated governance producers and the façade ledger. The catalog and real library-tested owners are now tracked, but all 60 direct-runtime obligations remain honestly unproven. |
| 10. Packaged release | Blocked honestly | Current strict Clippy, formatting, 497 real safe Rust behavior cases, 111 app-free integration cases, complete SDK/proof suites, exact generated JSON/CSS byte parity, canonical 54-target validators, packaged-asset byte parity, private AI/screenshot/trace/handoff/credential/auth-state/model-favorite/system-prompt/SQLite storage, truthful provider cancellation/failure/final-output ownership, and symlink rejection pass; version-3 release manifests independently attest every signed archive file/link/mode, the CodeResources envelope, and executable/Pi/SDK/Info.plist, including a real `ditto` framework-symlink archive and app-less Ubuntu verification. Actual packaged journeys, the full direct matrix, signing/notarization, and ratified paint still prevent publication even after the source/catalog are committed. |

Next permitted work: close genuinely provable offline ownership gaps, retain
CPU-capped source/library checks, and keep the portable catalog plus all proof
owners visible as required clean-source release inputs. Every visible,
native-input, provider-backed, packaged, or genuinely painted requirement
remains blocked until its separate explicit authorization and exact-artifact
evidence exist; no synthetic fixture can discharge those obligations.

The follow-up static surface inventory contains **16 Direct, 38 Derived, and
0 Unsupported bindings**. Direct is a production-owner relation only:
**0 of 54 surface mappings and 0 of 17 supported prompt families have a fresh
direct runtime receipt**. Source-generation changes invalidate all 15 existing
offline receipts until their real producers are run again.

## 2026-08-22: predictable Rust builds and verification throughput

### Observed failure chain, not a generic Rust checklist

1. One reviewed Dictation filter linked an application harness containing
   **15,207 tests** even though only six were requested.
2. `.cargo/config.toml` globally injected `CARGO_INCREMENTAL=1`; after the
   official prebuilt sccache package was installed, sccache immediately
   refused `rustc -vV` instead of caching any compilation.
3. Cargo's `test` profile inherited `[profile.dev.package."*"] opt-level=2`,
   so correctness-only checks optimized hundreds of third-party packages as
   if they were part of a frame-sensitive interactive binary.
4. `transcribe-rs` unconditionally enabled both Whisper C++ and ONNX; the
   application-wide harness therefore rebuilt native speech/model dependencies
   even for private JSONL, symlink, file-permission, and provider tests.
5. `gpui_macos/build.rs` let Metal pick `$HOME/.cache/clang/ModuleCache`.
   A cold sandboxed build failed because that path was not writable.
6. The emergency cleaner deliberately terminated live agent Cargo owners;
   its pruning helper could delete whole `target-agent/pools` parents rather
   than individually locked caches. During the observed run that destroyed a
   warm cache, produced hundreds of impossible dependency errors, and forced
   an entirely new cold compile.
7. Disk policy only warned below its 25-GiB reserve and still began a build,
   guaranteeing a race against the 25-GiB emergency watcher.
8. The local machine had no `sccache`, while CI already used both rust-cache
   and sccache; adding another CI cache layer would not fix the actual local
   absence or CI's incremental-compilation incompatibility.
9. The first real bounded timing report measured **1,233 total build units,
   1,228 rebuilt units, 2-way concurrency, and 257.2 seconds total**. Its
   longest actual owners were the monolithic app test harness (**67.0s**),
   Whisper's native build script (**17.2s**), `objc2-app-kit` (**13.2s**),
   and two separate compilations of the identical GPUI crate (**7.25s +
   6.28s**). The duplicate existed because GPUI's Metal build script pulled
   the complete GPUI application crate into its build dependencies just to
   read a sibling directory constant.
10. The app build script watched `.git/HEAD`, packed refs, and the active
    branch ref even in local debug/test profiles. Every grouped commit,
    including documentation-only checkpoints, therefore invalidated the
    entire **67-second** application test harness without a compiled-source
    or bundled-asset change.
11. The release verifier, CI performance lane, and offline-proof producer
    invoked `bun test scripts/...` without an explicit `./` prefix. On the
    installed Bun **1.3.14**, that form enters repository-wide filter mode,
    traverses unrelated build trees, and can exceed Darwin's **10,240-file
    descriptor** ceiling. The resulting missing child-process stdout/stderr
    produced **128 false failures** and a measured **156.13** machine-load
    spike even though the same bounded tests and their child processes were
    healthy. This exact Bun/macOS failure is independently documented by
    the upstream project; it was not an application regression.

### Ten implemented improvements and their verification contracts

1. **Extract GPUI-free persistence and search ownership.** `crates/sk-storage` now owns
   atomic writes, private directories/files, safe JSONL, symlink refusal, and
   collision-proof exports; `src/atomic_file.rs` preserves all app-facing
   paths. `cargo tree -p sk-storage` contains only `libc`, `tempfile`, `uuid`,
   and their small transitive dependencies: no GPUI, Metal, Whisper, or ONNX.
   Four initial storage regressions compiled in **1.69s**; the subsequent
   **14 isolated private-storage tests** ran against a **15-test harness** in
   **0.20s warm**, rather than linking the 15,207-test app harness. The full
   **15/15 domain suite**, including a real concurrent no-tearing regression,
   now finishes its behavior in **0.64s**; the former 12,000-write stress
   workload requires explicit `SCRIPT_KIT_STORAGE_FULL_STRESS=1`. The normal
   release domain lane initially executed **75/75 passing app-independent
   cases**. The pure provider coordinator and source-owned refresh lifecycle
   now also live in `crates/sk-protocol/src/search_contract.rs`, with their
   original app paths retained as compatibility adapters. Seven stale-query,
   duplicate-worker, cross-provider, and exact-generation regressions moved
   out of the 15,000-case app harness. The complete **984-line Unicode-aware
   sentence-query matcher** and its **19 existing word-boundary, stopword,
   ranking, prefix, metadata, and truthful-highlight regressions** then moved
   into `crates/sk-protocol/src/sentence_search.rs` with every app-facing
   import preserved. The expanded protocol domain executes **77 passing
   cases in 0.03s** without GPUI, Metal, Whisper, or ONNX. The complete
   domain inventory is now **101 cases**: nine Clipboard, 77 Protocol, and
   15 Storage behaviors.
2. **Separate test optimization from interactive rendering.** Keep every
   existing frame-sensitive dev dependency at `-O2`; explicitly compile
   correctness-test dependencies and vendored GPUI crates at `-O0`. Release
   settings and calibrated user-facing animation/render performance stay
   unchanged. Local debug/test builds no longer track changing Git refs;
   release, CI `GITHUB_SHA`, and explicit `SCRIPT_KIT_TRACK_GIT_HEAD=1`
   preserve exact provenance. Executable checks cover both profiles and
   every Git-invalidation policy.
3. **Use a real compiler cache.** Install the official prebuilt sccache
   package, configure a persistent repo-owned local cache and Unix socket,
   and normalize the repository base directory. Preserve explicit existing
   wrappers. Auto mode clearly explains sandbox fallback; required mode fails
   closed before Cargo starts if the cache cannot execute `rustc`. The
   observed cold rebuild populated **more than 1,100 real cache files**.
4. **Remove the incremental/sccache contradiction.** Delete the global
   `CARGO_INCREMENTAL=1` injection while preserving local dev/test profile
   incrementality. CI/release explicitly use `CARGO_INCREMENTAL=0`, allowing
   disposable runners to cache eligible Rust units instead of bypassing
   sccache. Keep in mind that linked binaries, proc macros, and incremental
   path crates remain non-cacheable by design.
5. **Persist shader modules inside the writable workspace.** The wrapper and
   vendored GPUI Metal build agree on `target-agent/shared/clang-modules` and
   pass Clang's documented `-fmodules-cache-path` flag. The identical
   production Metal compile succeeded in the restricted sandbox in **0.16s**,
   produced real `.pcm` modules under that directory, and linked a valid
   `.metallib` without touching `$HOME/.cache`. The shader build now resolves
   the adjacent vendored GPUI path directly, eliminating the full duplicate
   GPUI build-dependency graph exposed by the timing report.
6. **Protect live and warm build caches.** A shared lock-owner contract makes
   incomplete leases fail closed, acquires the exact pool lock before
   deletion, preserves `agent-debug`, and never deletes pool parents,
   `.locks`, shared compiler/shader caches, exports, or artifacts. Both
   ordinary and emergency pruning now remove only individually unlocked
   stale pools. No automatic cleaner kills Cargo, rustc, a dev watcher, or a
   user-owned process.
7. **Fail before predictable disk contention.** Preserve the 25-GiB floor,
   inspect free space after safe eviction, and refuse to start below that
   reserve unless an explicit low-disk override was provided. Executable
   negative tests prove the compiler is never invoked when the floor cannot
   be met.
8. **Bound CPU and capture actual timing receipts.** Agent Cargo defaults to
   two workers while preserving an intentional caller override. Every run
   records cache state, pool, worker count, exit status, elapsed seconds, and
   before/after free space. `SCRIPT_KIT_AGENT_TIMINGS=1` emits Cargo's real
   critical-path HTML plus a fail-closed machine-readable summary of actual
   hot units, duplicate compilations, bounded concurrency, and specific next
   actions instead of relying on impressions about slow crates.
9. **Reuse only current reviewed test binaries.** The dedicated harness
   runner refuses missing executables and binaries older than source,
   workspace crates, vendored code, Cargo configuration, or lockfiles. It
   accepts explicit reviewed filters only and disables app launch, visible
   probes, screen takeover/capture, native input, and live AI before running
   each group single-threaded. It cannot claim a stale build is current.
   APFS artifact export also recognizes that Cargo's built-in `test`
   correctness profile places named binaries in `debug/`; non-GUI proof
   tools can reuse cheap correctness-profile dependencies without losing a
   stable, source-identifiable artifact. A disposable real-binary behavior
   test rejects the former incorrect `target/test/` lookup.
10. **Lock the contract into real behavior and release gates.** Dedicated
    low-cost Bun cases operate disposable fake pools, live/incomplete leases,
    missing/unusable sccache, disk starvation, stale/current harnesses,
    optimized-vs-test profiles, and CI safety flags. The release verifier now
    also refuses native-input/screen-capture overrides and includes the
    storage domain plus build-policy tests in its normal verification lanes.
    Every real explicit Bun test invocation now roots its paths with `./`,
    including both shared offline-proof launchers and the actual recorded
    receipt command; the receipt validator rejects any unrooted command
    while preserving canonical source paths and fingerprints. A live
    child-output regression proves subprocess evidence is observable, and
    both extra native-input/screen-capture opt-ins are forcibly disabled.
    The shared DevTools safety boundary now rejects inherited native-input
    and screen-capture permission for protocol requests, driver launches,
    and existing-session transports. Direct `session.sh` invocation crosses
    that same policy before creating a session directory, resolving an app
    binary, mutating a FIFO, or disturbing an existing operator session;
    reviewed read-only status remains available. Its isolated standalone
    suite passes **32 cases and 230 assertions**, including red/green
    proofs for every previously unguarded direct shell entry point.
    The source-current full nonintrusive release lane then executed
    **701 passing tests, zero failures, and 2,741 assertions across 37 files
    in 9.61s**, without the previous repository scan or load spike. The
    focused build/proof-contract lane separately passed **62 tests and 320
    assertions in 0.77s**. None touches the operator's computer.

The measured post-deduplication build contained **985 total units instead of
1,233**, rebuilt only **119 units instead of 1,228**, retained exactly two
workers, and completed in **103.5s instead of 257.2s**. Its timing report
contains one GPUI compilation instead of two. Its remaining critical path is
the application test harness (**66.63s**) and Whisper's native build
(**16.68s**), which is why app-independent domain extraction remains the
highest-return next architectural move. After Git-reference invalidation was
removed, a real app-harness incremental rebuild completed in **17.38s**;
all **25 current app-only privacy regressions** then passed from that fresh
binary in **0.45s**, and all **15 Storage regressions** passed separately.
After the storage extraction and four grouped source commits, a genuinely
source-current application harness rebuilt in **22.04s** with exactly two
workers; all 25 app regressions passed again, and the complete **75-case
domain lane** rebuilt in **0.41s** and passed every case without a GUI.
The required strict application-library release gate also passed on this
source with **zero warnings** via the bounded two-worker agent wrapper;
its first populated Clippy metadata cache completed in **74s**.
The actual shipping application binary separately passed a two-worker
compile check; its eight existing binary-only unused-import warnings remain
explicitly distinguished from the zero-warning required library gate.
The previously deleted standalone design-token exporter was then rebuilt in
the existing cheap test profile: Cargo observed **984 total units but only two
dirty units**, rebuilt and linked the actual non-GUI exporter in **45.86s**,
and APFS-cloned it into the protected stable artifact directory. Running the
real binary reproduced both checked-in generated outputs byte for byte; the
fresh, exact-source consistency auditor reported **15 accepted / 60 missing,
zero invalid or failed receipts, zero auditor errors, and passing privacy,
protected-source, and generated-byte gates**. The 60 remaining obligations
are real-runtime interactions and were not misrepresented as offline proof.

### Remaining architecture, without overstating completion

- Additional pure command/search/provider/AI-policy owners should follow the
  proven `sk-storage` pattern; each extraction must preserve app paths and
  prove its dependency tree excludes GPUI and native media backends.
- The application-wide root harness still legitimately includes Whisper and
  ONNX. Making speech backends optional requires a separate typed runtime
  boundary, unchanged default packaged capabilities, and explicit no-feature,
  default-feature, and packaged-Dictation verification; simply disabling a
  dependency would silently break a shipped command.
- The lockfile contains duplicate `nix`, `dirs`, `png`, `hashbrown`, and
  Windows-platform dependency generations. Any consolidation must be driven
  by the generated timing critical path and verified compatibility, not a
  blanket lockfile upgrade.
- nextest and CI rust-cache already exist. Use nextest archive/reuse only for
  an actually measured build-once/multiple-runner workflow; do not duplicate
  caches or misrepresent execution as fresh behavior proof.
- A changed source still invalidates previous top-ten receipts; final
  acceptance requires the fresh app/library build, reviewed offline behavior,
  direct safe-producer reruns, and exact-current-source audit. Interactive,
  painted, provider-backed, signed, packaged, and 54-surface runtime
  requirements remain separately unproven without explicit authorization.

Primary technical references:

- [Cargo build-cache layout and compiler wrappers](https://doc.rust-lang.org/cargo/reference/build-cache.html)
- [Cargo profiles, inheritance, package overrides, and incremental compilation](https://doc.rust-lang.org/stable/cargo/reference/profiles.html)
- [Cargo critical-path build timing reports](https://doc.rust-lang.org/stable/cargo/reference/timings.html)
- [Cargo build-script change detection](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [Mozilla sccache: supported storage, Unix sockets, and non-cacheable Rust units](https://github.com/mozilla/sccache)
- [Clang modules and explicit module-cache locations](https://clang.llvm.org/docs/Modules.html)
- [Cargo optional dependencies and feature contracts](https://doc.rust-lang.org/stable/cargo/reference/features.html)
- [Nextest build archiving and reuse](https://nexte.st/docs/ci-features/archiving/)
- [Bun macOS test-filter descriptor exhaustion and silent child-process output loss](https://github.com/oven-sh/bun/issues/32067)

## Historical 2026-08-21 re-audit: superseded worktree snapshot

The remainder of this dated section preserves the August 21 findings and
receipts as historical audit evidence. Its counts, failures, missing
production consumers, dirty-file totals, and readiness labels are **not the
current source state**; the August 22 implementation checkpoint above is the
authoritative current verification record.

### Audit scope and status vocabulary

The committed repository remains at `aee92338d`; the inspected working tree
contained **41 modified tracked paths and 20 untracked paths**. Existing
changes must be preserved, but none of them can be treated as available to a
clean GitHub checkout until they are deliberately reviewed and committed.
`.notes/top-10.md` is itself ignored by `.gitignore:78`; its presence does not
mean the roadmap is committed.

Use these labels consistently:

- `COMMITTED`: exists at `aee92338d`; a clean checkout can see it.
- `WORKTREE-VERIFIED`: exists only in current changes and has the exact listed
  nonzero behavior receipt. It is not committed or package-proven.
- `WORKTREE-SOURCE-ONLY`: exists only in current changes; source inspection or
  an unrelated test is not behavior proof.
- `STATIC-INVENTORY`: proves catalog/schema/source relationships, never actual
  app behavior, visible pixels, first-run readiness, or successful execution.
- `NOT STARTED`: no current implementation or matching receipt exists.
- `BLOCKED`: an identified prerequisite, incompatible contract, missing
  primitive, missing approval, or absent candidate prevents acceptance.

### Requirement-by-requirement validity and August 21 readiness

| Program | Strategic verdict | Currently proven | Mandatory correction before implementation can be accepted |
| --- | --- | --- | --- |
| 01 — command contract | Valid, partially prototyped | The app-independent command model covers 24 source families and passes domain behavior tests. Launcher, extension, and conversation projection methods exist in the worktree, and both app targets compile. | The committed deeplink/command parser supports only six categories; 18 new identities cannot automatically round-trip through it. Extension types were dormant at the committed baseline, and the new descriptor methods have no production launcher/footer/actions/AI/DevTools consumers. Unify identity ownership and migrate real consumers before claiming one command contract. |
| 02 — interaction | Valid, partially prototyped | The existing orchestrator is already production-wired through `src/app_impl/window_orchestrator_bridge.rs`; a pure Escape planner exists and one capture-preemption path calls it. | Physical bubble handling, the simulated-key mirror, Actions, portals, focus restoration, and Cmd+K still do not share that planner. Treat all visible/native-input parity probes as approval-gated, never default verification. |
| 03 — design system | Valid, not yet proven | Existing shared input, row, shell, footer, conversation, and theme owners are real; the generated 37-surface registry is current. | No complete component-ownership/presentation matrix or cross-surface migration receipt exists. Preserve the two sanctioned general-purpose row owners, specialized renderers, native footer geometry, and locked glass calibration. |
| 04 — deterministic search | Valid, partially prototyped | Shared provider fences and ranking fixtures pass domain tests; the compiled worktree adds query/generation rejection to browser-tab and browser-history refreshes. | `RootSearchStore` is included only by `src/main.rs`, while `Cargo.toml` sets that binary's `test = false`; the proposed `cargo test --lib root_search_store_tests` cannot execute those tests. Expose a library-testable owner or an explicit integration harness before accepting provider claims. |
| 05 — latency | Valid, blocked on safety, measurement, and owner ratification | The typing contract truthfully reports `STATE_ECHO`, `measuresPaint=false`, visible-runtime approval, and an unratified 25/50/150 ms p50/p95/max proposal. The frame-stability contract is hidden-only semantic-state proof, not paint. | `quick-ai-latency-bench.ts --help` still falls through to six live provider calls. Add fail-closed CLI/operator guards before any benchmark use; obtain separate explicit permission for visible paint measurement and separate product-owner approval before enforcing a budget. |
| 06 — execution | Valid, but current shared prototype is semantically incomplete | Domain behavior proves one terminal receipt, blocked preflight, and cleanup-required cancellation receipts; existing Flow runners already implement truthful process-group termination. | The new shared reducer transitions directly from running to `Cancelled`, while production Flow correctly requires `Cancelling -> verified dead -> Cancelled`. Add a nonterminal cancellation-request phase and identity-bound cleanup acknowledgement before connecting real runners. |
| 07 — AI coherence | Valid, existing foundations only | Current source already owns typed recovery records, safe user messages, context-staging policy, clean-chat constructors, conversation commands, and first-visible-output phase events. | The named `quick_question_entry_suppresses_all_implicit_context` test also lives only in the `test = false` binary module, so `cargo test --lib <that-name>` is a zero-test false gate. Extract a testable contract and obtain target-scoped recovery/handoff receipts without paid/live calls or implicit sensitive context. |
| 08 — authoring/SDK | Valid, meaningful isolated progress | The strict noninteractive SDK harness passes **192 cases, 0 failures, 8 explicit skips across 37 files**; stale window IDs reject, genuine mini/micro/hotkey/fields support is reconciled, SDK TypeScript reports zero errors, typed unsupported APIs reject safely, and the Rust capability catalog compiles in both app targets. | This is synthetic SDK behavior, not native prompt/window/permission proof. The eight skips include file search, four clipboard-history actions, menu-bar type verification, and two unavailable-network cases. The catalog still requires nonzero focused Rust behavior proof, exhaustive export reconciliation, validator wiring, topology checks, and native receipts. |
| 09 — direct proof | Valid, static/proof tooling partially verified | Current census is **37 kinds / 54 mappings / 53 variants / 11 profiles**: 5 static Direct, 48 Derived, 1 Unsupported, and **0/54 fresh runtime-proven mappings**. Owner validation now reports zero errors; the candidate catalog validates all 75 task IDs. | The supposedly clean-checkout-safe catalog is still **untracked**; `git ls-files scripts/devtools/consistency-catalog.md` returns no path. In addition, `consistency.ts verify-task` still hardcodes ignored `.notes/CONSISTENCY-FIXES.md`. Commit the approved catalog, route every subcommand through it, and bind runtime receipts to exact source, binary, target, transaction, privacy, and cleanup. |
| 10 — release | Valid, **currently blocked by a real required gate** | The current safe proof lane executes **271 tests, 0 failures, 796 assertions across 20 files**; workflow YAML parses, generated contracts match, fixture evidence rejects compile-only/zero-test/stale/wrong-binary/unsafe receipts, and the isolated SDK release suite is green. | The actual required release Clippy command fails with **280 errors** under `-D warnings`, even though both app targets compile. No GitHub workflow, full application Rust suite, integration-test suite, signed/notarized real candidate, first-install journey, permissions/migration/AI-recovery journey, or release publication has been executed. The proposed Rust release job uses `--lib`, which still excludes integration tests. One hidden root-semantic receipt cannot stand in for the complete packaged product. |

### Cross-program defects that must be corrected first

1. **Separate durable identity from deeplink support.** The new protocol
   `CommandSource::ALL` has 24 families; the committed
   `config::command_ids::SUPPORTED_COMMAND_CATEGORIES` has six. Specify which
   identities are persisted, bindable, deep-linkable, internal-only, or
   passive before migrating any action, shortcut, or history consumer.
2. **Make dormant and binary-only owners honestly testable.** The committed
   extension module was not reachable from either crate root. Both
   `root_search_store_tests` and the locked quick-question test are in binary
   modules excluded by `test = false`. Every advertised filter must execute a
   nonzero test count against the actual owning module.
3. **Do not flatten cancellation semantics.** A user request to stop is not
   proof that a child process or its descendants have stopped. The shared
   lifecycle needs nonterminal `Cancelling`, process-group ownership,
   escalation, and verified teardown before terminal `Cancelled`.
4. **Close every clean-checkout catalog path.** A file is not tracked because
   its own prose says "tracked." Require `git ls-files` proof for the approved
   catalog and remove the remaining ignored-notes dependency from
   `verify-task`, not only from `catalog`, `verify-scope`, and `verify-all`.
5. **Install the safety boundary before expanding proof.** Under
   `SCRIPT_KIT_NONINTERACTIVE=1`, reject window reveal/focus, global or native
   input, screenshots/pixel capture, microphones/cameras, live AI, copied
   credentials, real-data mutations, focused-window selectors, and unsafe
   nested batches. `inspectAutomationWindow` is not passive: its protocol
   explicitly includes screenshot dimensions and pixel probes.
6. **Classify every latency observation truthfully.** Hidden semantic-state
   stability, state-echo latency, GPUI frame callbacks, compositor-visible
   paint, and screenshot evidence are distinct. No visible/compositor test
   runs on the operator's computer without separate explicit permission.
7. **Preserve a complete release evidence chain.** A passing test fixture
   proves the verifier's rejection logic, not a signed production artifact.
   Require the exact candidate source SHA, app SHA, sidecar SHA, SDK schema,
   generated-contract identity, nonzero executed suites, safe real-candidate
   journeys, cleanup, signing, notarization, stapling, and Gatekeeper.
8. **Preserve a repeatable macOS build environment.** The first sandboxed app
   compile failed because Apple's Metal compiler tried to write
   `~/.cache/clang/ModuleCache`. The exact prescribed wrapper command passed
   after approved cache access. Future isolated/CI agents need either approved
   access or an explicitly supported writable Clang module-cache policy; no
   app launch is necessary to verify it.
9. **Restore the actual required Clippy release gate.** The exact release
   command `./scripts/agentic/agent-cargo.sh clippy --locked --lib --no-deps
   -- -D warnings` fails with **280 errors**. Findings include unused imports
   and dead code, duplicate attributes, forbidden production `unwrap`/`expect`,
   missing unsafe-function safety documentation, excessive argument counts,
   large error/enum variants, and malformed rustdoc lists. Triage committed
   debt separately from new worktree debt, fix errors with their actual owners,
   preserve locked glass motion, and never silence the gate globally.

### Historical August 21 noninteractive audit receipts

The following checks were rerun against the current worktree after inspecting
their safety boundaries. None launched the app, changed focused windows,
captured a display, drove native input, accessed a microphone/camera, or
called a live AI provider.

```text
SCRIPT_KIT_CARGO=./scripts/agentic/agent-cargo.sh \
  bash scripts/verify.sh --skip-bundle --only proof-contracts
  -> generated surface contracts current
  -> 271 passed; 0 failed; 796 assertions; 20 files

SCRIPT_KIT_NONINTERACTIVE=1 SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER=0 \
SCRIPT_KIT_ALLOW_VISIBLE_PROBES=0 SCRIPT_KIT_ALLOW_LIVE_AI=0 \
  bun run scripts/test-runner.ts --parallel --json
  -> 192 passed; 0 failed; 8 explicitly skipped; 37 files

./scripts/agentic/agent-cargo.sh test -p sk-protocol
  -> 45 passed; 0 failed; the separate doc-test target executes 0 tests

./scripts/agentic/agent-cargo.sh check --lib --bin script-kit-gpui
  -> both app targets compile; 143 library warnings; 22 binary warnings
  -> first sandbox attempt failed at ~/.cache/clang/ModuleCache
  -> approved cache access passed; COMPILE-ONLY; no app launched

./scripts/agentic/agent-cargo.sh clippy --locked --lib --no-deps -- -D warnings
  -> FAILED: could not compile script-kit-gpui (lib) due to 280 previous errors
  -> this is the exact configured required release gate, not a speculative risk
  -> first sandbox attempt hit the same Metal module-cache boundary
  -> approved cache access reached and exposed the actual lint failures

bun run scripts/check-sdk-types.ts
  -> 0 TypeScript errors

./node_modules/.bin/tsc --noEmit <the two menu-syntax fixture files>
  -> passed; COMPILE-ONLY, not executable behavior

bun scripts/devtools/consistency.ts catalog \
  --fixes scripts/devtools/consistency-catalog.md
  -> 75/75 valid; STATIC_INVENTORY; no runtime behavior proven

bun scripts/devtools/surfaces.ts
  -> 37 kinds; 54 mappings; 53 variants; 11 profiles
  -> 5 static Direct; 48 Derived; 1 Unsupported; 0/54 runtime proven

bun scripts/devtools/coverage.ts
  -> 10 partial profiles; 1 planned; 0 supported; 0 owner-path errors

bun scripts/agentic/root-typing-lag-benchmark.ts --describe-contract
  -> RUNTIME_VISIBLE when executed; STATE_ECHO; measuresPaint=false
  -> 25/50/150 ms proposal remains USER_RATIFICATION_PENDING

bun scripts/agentic/root-search-frame-stability.ts --describe-contract
  -> RUNTIME_HIDDEN when executed; semantic_frame_identity only
  -> no reveal, focus, native input, or screenshot; measuresPaint=false

ruby YAML.parse_file <ci.yml, perf-gates.yml, release.yml>
  -> all three workflows parse; none has executed on GitHub Actions

bash -n scripts/verify.sh scripts/verify-macos-bundle.sh
  -> shell syntax valid; no package or signing proof

git ls-files scripts/devtools/consistency-catalog.md
  -> empty: candidate catalog is untracked and absent from a clean checkout

df -h /System/Volumes/Data
  -> approximately 104 GiB free at audit time
```

### Go/no-go decision

**GO for incremental implementation only after preserving the current dirty
worktree, establishing the noninteractive safety contract, selecting the
approved clean-checkout consistency catalog, and assigning an explicit owner
for the 280-error required Clippy gate.**

**NO-GO for shipping while the exact required Clippy gate is red; also NO-GO
for claims of 54-surface coverage, claims of measured paint, live AI or
visible probes, complete keyboard parity, native SDK parity, truthful shared
cancellation, or packaged-product readiness.** Those claims remain blocked by
the explicit program gates below.

## Non-negotiable implementation rules

1. Work directly in the checkout. Project flows are disabled; never dispatch
   `md flows/<name>.md`. Existing flow documents are ownership references only.
2. Inspect `AGENTS.md`, `GLOSSARY.md`, current source, generated contracts, the
   dirty worktree, and relevant repo-local skills before changing a surface.
3. Use `./scripts/agentic/agent-cargo.sh` for every repository Cargo command.
4. Preserve the locked glass-motion calibration. No program in this roadmap
   authorizes animation retuning or screen takeover.
5. For live Agent Chat probes, run `bash scripts/agentic/ensure-pi-sidecar.sh`
   before launch and use sandboxed Driver sessions; seed sandbox auth only
   when the scenario explicitly requires authenticated provider calls.
6. Prefer compiler/type guarantees, lints, behavior tests, and real runtime
   receipts over new app-source-reading tests.
7. Never count `Derived`, inferred, source-only, metadata-only, compile-only,
   missing-primitive, blocked, or invalid-interference receipts as product
   proof.
8. Establish a red proof before a fix, then rerun the same user path, binary,
   target selector, and metrics for green proof.
9. Protect secrets and user data: sandbox mutable scenarios, redact provider
   payloads, obtain explicit approval for screen takeover, and expose clear
   permission requirements before execution.
10. Separate proposed budgets from ratified budgets. Never quietly relax a
    threshold, replace paint with state echo, or omit a failed trial.
11. Reuse existing command, identity, dismissal, window, provider, trace,
    receipt, release-manifest, and consistency-ledger owners. New names in
    this roadmap describe desired semantics, not permission to introduce
    parallel systems when an existing owner can be extended.
12. Classify every verification command before running it: static inventory,
    source/type check, executed unit behavior, future-only test filter, hidden
    runtime proof, visible/native-input proof, provider-backed AI benchmark,
    or packaged-app-only check.
13. `--help`, `--list`, and registry generation prove command usability or
    inventory only, never product behavior. A Bun/Cargo command that reports
    zero executed tests is not a passing behavioral gate.
14. Live AI benchmarks, Driver launches, native input, focus changes, screen
    takeover, package execution, and real-user-data mutation require their
    stated environment/safety prerequisites. Do not treat a CLI flag as safe
    until the actual parser confirms it exists.
15. Standard planning, implementation, CI, and local verification must not
    take over the operator's computer. Set `SCRIPT_KIT_NONINTERACTIVE=1`, keep
    screen-takeover/visible-probe/native-input/screen-capture/live-AI opt-ins
    disabled, and fail closed on nested unsafe protocol commands plus direct
    session-shell lifecycle bypasses before any filesystem side effect.
    Separate owner-approved sessions are
    required for any visible, native-input, camera, microphone, or paid-AI
    proof.
16. Distinguish committed source, uncommitted source, compiled source,
    executed behavior, hidden runtime proof, and packaged-product proof. A
    clean-checkout dependency is not satisfied until `git ls-files` confirms
    it is tracked.

## Historical committed-baseline verdicts and required corrections

The original roadmap was reviewed against commit `aee92338d`. The table below
preserves why each initiative exists; the current-worktree re-audit above is
authoritative whenever an existing defect has already changed locally.

| Program | Verdict | Existing owner or correction that must come first |
| --- | --- | --- |
| 01 — command contract | Valid after reuse correction | Start with `src/extension_types/mod.rs`, `src/config/command_ids.rs`, and `src/components/conversation_actions.rs`; do not create competing command, availability, or identity systems. |
| 02 — interaction | Valid after reuse correction | Extend `LauncherSurfaceContract`, `DismissPolicy`, `resolve_conversation_dismissal`, and `window_orchestrator::reduce`; preserve existing one-layer and active-work decisions. |
| 03 — design consistency | Valid with bounded scope | Reuse the existing 75-task `UX-*`, `GEO-*`, and `GOV-*` work; preserve both sanctioned general-purpose row owners and legitimate specialized renderers. |
| 04 — search | Valid after reuse correction | Extend `RootSearchStore`, existing frozen passive frames, provider tokens, generation fences, caches, and match evidence; do not rebuild working provider guarantees. |
| 05 — latency | Valid after safety correction | Existing AI phase traces already emit `FIRST_VISIBLE_OUTPUT`; `quick-ai-latency-bench.ts --help` is unsafe because no help flag exists. Use `--print-command` for non-executing inspection, and add an explicit live-AI refusal before further work. |
| 06 — execution | Valid after reuse correction | Extend existing process-group ownership, AI reducers, Flow run registry, transaction executor, and transaction trace; preserve their different semantics. |
| 07 — AI | Valid but already partially implemented | Reuse shared conversation command/availability/dismissal contracts, typed AI reliability reducers, entry requests, provenance, and phase trace; first close missing green recovery activation receipts. |
| 08 — SDK | Valid after capability-truth and red-gate correction | At the committed baseline, SDK auto-submit made `stale-id-rejects` fail even though native registry resolution rejected stale IDs. The current uncommitted strict suite now passes 192 cases. `hotkey()`, `mini()`, `micro()`, and `fields()` already had real routes despite stale labels. The two menu-syntax `*.test.ts` fixtures remain compile-only. |
| 09 — direct proof | Valid after grading and tracking corrections | A `Direct` binding is static ownership, not a current runtime receipt. The candidate 75-task catalog is in a tracking-eligible path but remains untracked, and `verify-task` still reads the ignored notes file. Close both gaps before claiming clean-checkout CI. |
| 10 — release | Valid after blocker and reuse corrections | Baseline SDK and compile-only release defects have local candidate fixes, but no workflow or full Rust suite has run. Preserve native stale-ID protection, execute real library and integration tests, commit the approved catalog, and add exact packaged-candidate receipts. |

### Existing consistency-program alignment

Use `scripts/devtools/consistency-catalog.md` as the proposed portable task
catalog and `.notes/CONSISTENCY-PROGRESS.md` as the existing tracked progress
ledger. The catalog path is currently untracked, so it is not a real CI
authority until committed. `.notes/CONSISTENCY-FIXES.md` is local historical
context only and must not remain a required verification dependency. Map
roadmap work onto these IDs:

| Roadmap program | Existing consistency task families to reuse |
| --- | --- |
| 01 — command contract | `UX-003`, `UX-004`, `WF-005`, `GOV-001`, `GOV-002` |
| 02 — interaction | `SAFE-002`, `SAFE-004`, `UX-014`, `WF-006`, `WF-008`, `WF-023` |
| 03 — design system | `UX-001` through `UX-018`, `GEO-001` through `GEO-009` |
| 04 — search | `PF-008`, `PF-010`, `UX-008`, `UX-009`, `GOV-001` |
| 05 — latency | `PF-005`, `PF-006`, `PF-008`, `PF-011`; add new latency gates only where the ledger has no owner |
| 06 — execution | `PF-002`, `SAFE-003`, `WF-005`, `WF-007`, `WF-011` |
| 07 — AI | `SAFE-001`, `WF-001` through `WF-011`, `WF-016`, `WF-022` |
| 08 — SDK | `PF-004`, `WF-010`; capability reconciliation needs an explicitly scoped additional track |
| 09 — direct proof | `PF-001` through `PF-012`, `GOV-004`, `GOV-005`, `GOV-006` |
| 10 — release | `GOV-006` plus a separately scoped release-execution and packaged-runtime track |

A mapped task is not complete because its ID exists or a progress note claims
completion. Run the existing auditor against fresh, identity-matched evidence
before relying on any task's state. Because `.notes/CONSISTENCY-FIXES.md` is
gitignored, a GitHub runner cannot rely on it; because
`scripts/devtools/consistency-catalog.md` is still untracked, runners cannot
yet see that candidate either. First commit the approved portable catalog,
verify `git ls-files` returns it, make `verify-task` consume the same catalog
as every other subcommand, then run clean-checkout catalog and receipt tests.
Do not force-add private `.notes/` files or quietly make a local-only gate look
like a clean-checkout gate.

### Verification evidence classes

- `STATIC-INVENTORY`: reports the existence of files, mappings, fixtures, or
  cases. Examples: `surfaces.ts`, `coverage.ts`, matrix `--list`, and `--help`.
- `COMPILE-ONLY`: confirms source/type correctness. The two menu-syntax
  fixtures require `tsc --noEmit`; `bun test` reports `0 tests` and is invalid.
- `UNIT-BEHAVIOR`: runs a nonzero number of deterministic Bun, Python, or Rust
  tests. Capture and report the actual passed/failed count.
- `SDK-BEHAVIOR`: runs the safe auto-submit SDK harness; system-input tests
  remain excluded unless the user explicitly authorizes them.
- `RUNTIME-HIDDEN`: launches a pinned app binary with `sandboxHome: true` and
  proves semantic/protocol behavior without native input or focus takeover.
- `RUNTIME-VISIBLE`: changes frontmost windows, drives real keys/pointers,
  captures screens, or checks actual paint; run serially with permission and
  interference guards.
- `LIVE-AI`: makes real provider requests, may consume paid/account quota,
  requires valid credentials, and must not run as a casual CLI inspection.
- `PACKAGED-APP`: requires an already-built bundle with the real sidecar and
  confirms the exact candidate that would be published.
- `FUTURE-ONLY`: names a test filter, probe, or contract that does not exist
  yet. It cannot pass acceptance until implemented and proven to select at
  least one real test or scenario.

### Historical committed-baseline receipts

```text
bun test <7 DevTools / AI test files>             149 passed, 0 failed
python3 -B -m unittest <3 policy scanners>        45 passed, 0 failed
./scripts/agentic/agent-cargo.sh test -p sk-protocol
                                                  27 passed, 0 failed
bun run scripts/check-sdk-types.ts                0 TypeScript errors
tsc --noEmit <2 menu-syntax fixture files>        passed; compile-only
bun run scripts/test-runner.ts --filter hotkey    2 passed, 0 failed
bun run scripts/test-runner.ts --filter editor    5 passed, 0 failed;
                                                  includes 3 mini/micro cases
bun run scripts/test-runner.ts --parallel         FAIL: 171 total; 158 passed,
                                                  1 failed, 12 skipped;
                                                  stale-id-rejects
bun run scripts/test-runner.ts --filter window-management
                                                  FAIL: 3 passed, 1 failed,
                                                  5 skipped; same stale-ID case
bun scripts/devtools/consistency.ts catalog ...   75/75 task IDs valid
filterable-surface-matrix.ts --list               12 cases; inventory only
attached-popup-surface-matrix.ts --list            4 cases; inventory only
quick-ai-latency-bench.ts --print-command         command inspected; no AI call
```

The failing SDK counts above describe the committed baseline only. They must
not overwrite the current **215 passed / 0 failed / 0 skipped** worktree receipt
or imply the uncommitted improvements are present on `HEAD`. The intermediate
August 21 **192 passed / 0 failed / 8 skipped** result is also historical.

## Program sequencing and dependency graph

```text
Wave 0: baseline census + release truthfulness
  09 direct behavioral proof ───────┐
  10 actual release gates ──────────┤
                                    v
Wave 1: host-owned foundations
  01 canonical command descriptor -> 02 shared interaction state machine
                                 -> 04 unified search and ranking
                                 -> 06 shared execution lifecycle
                                 -> 08 authoring/capability validation
                                    |
                                    v
Wave 2: product consistency
  03 shared design system + 07 coherent AI experience
                                    |
                                    v
Wave 3: calibrated shipping bar
  05 end-to-end latency budgets + 09 full proof matrix + 10 release approval
```

Recommended first implementation order:

1. Preserve the complete grouped-commit checkpoint. Keep verification
   noninteractive, recheck free disk, retain the now-passing two-target app
   compile, preserve the now-passing strict Clippy gate after eliminating its
   historical 280 errors, and keep the approved portable catalog tracked.
   Every auditor subcommand already defaults to that committed catalog.
2. Preserve the verified SDK stale-ID/negative-response fix, safe capability
   boundaries, corrected owner paths, and executed-release-test tooling;
   validate them from a clean checkout without relabeling synthetic behavior
   as native proof.
3. Make root-search and quick-question contracts reachable by real nonzero
   library/integration tests. Resolve the existing six-category versus new
   24-family identity/deeplink contract before adopting descriptors.
4. Correct shared cancellation to `Cancelling -> verified cleanup ->
   Cancelled`, then route launcher/footer/actions/AI and all physical/simulated
   keyboard paths through their existing canonical owners.
5. Extend generation-safe providers, capability validation, shared
   presentation, and typed AI context/recovery one ownership-safe slice at a
   time.
6. Expand direct hidden runtime receipts; schedule any visible/native-input or
   paid-AI proof only with separate explicit approval. Ratify actual paint
   budgets and require the exact packaged candidate's full release evidence
   chain before publishing.

Each numbered program below is a separately deliverable initiative. Phases are
ordered and should land as small independently verifiable changes, not one
cross-repository rewrite.

---

## 01. One canonical command contract

### Outcome

Every callable thing has the same host-owned identity, capability declaration,
action contract, search projection, execution plan, and failure semantics.

### Primary ownership

- `src/scripts/types.rs`
- `src/scripts/command_contract.rs`
- `src/scripts/validation.rs`
- `src/extension_types/mod.rs` for existing Raycast-compatible
  `ExtensionManifest`, `CommandMetadata`, arguments, preferences, and `Command`
- `src/config/command_ids.rs` for canonical categories, parsing, validation,
  and deeplink round-tripping
- `src/components/conversation_actions.rs` for existing command descriptors,
  typed disabled reasons, semantic action IDs, and execution outcomes
- `src/plugins/types.rs` and `src/plugins/manifest.rs`
- `src/menu_syntax/payload.rs` and `src/menu_syntax/handler_index.rs`
- `src/actions/types/action_model.rs`
- `src/app_impl/execution_scripts.rs` and
  `src/app_actions/handle_action/files.rs` for real exact-owner dispatch/share
- `src/app_actions/handle_action/shortcuts.rs`,
  `src/app_render/focused_info.rs`, `src/app_render/preview_panel.rs`,
  `src/app_impl/alias_input.rs`, `src/app_impl/selection_fallback.rs`, and
  `src/app_impl/registries_state.rs` for real source-owned preference
  persistence, display, editing, registry, and alias execution
- `src/hotkeys/mod.rs` for legacy-versus-source-owned shortcut precedence
- `src/spine/types.rs`, `src/spine/list.rs`, and source-specific adapters
- `crates/sk-protocol/src/` for app-independent serialized primitives

### Step-by-step implementation

1. Inventory the existing `extension_types::Command`, `CommandMetadata`,
   `ExtensionManifest`, `config::command_ids::CommandCategory`,
   `ConversationCommandDescriptor`, `ConversationCommandAvailability`, and
   `ActionAvailability`. Verify each owner is actually reachable from the
   intended crate root: the committed extension module was dormant. Document
   ownership and compile both app targets before depending on it.
2. Enumerate every source represented by `SearchResult`: scripts, scriptlets,
   flows, skills, built-ins, apps, windows, files, notes, Brain, inbox, todos,
   conversation history, AI Vault, clipboard, dictation, browser tabs/history,
   fallbacks, validation issues, and Spine projections.
3. Build a source-to-field matrix covering stable ID, origin, display title,
   subtitle, icon, aliases, keywords, shortcut, arguments, availability,
   permissions, dependencies, authentication, context policy, action list,
   execution mode, cancellation, confirmation, and output type.
4. Extend or project the existing owners into a versioned command descriptor;
   `CommandDescriptor` below is a proposed semantic contract, not a required
   new parallel struct. Reuse existing typed arguments, preferences, command
   mode, permissions, disabled reasons, and action metadata. Use app-local
   projection first; move only genuinely app-independent wire primitives into
   `sk-protocol` after consumers and crate boundaries justify extraction.
   Candidate semantic facets are
   `CommandIdentity`, `CommandAvailability`, `CommandCapability`,
   `CommandAction`, `CommandInputSchema`, `CommandExecutionPolicy`, and
   `CommandContextPolicy`.
5. Reconcile the six existing persisted/deeplink categories with the new
   24-source descriptor projection. Classify every family as deep-linkable,
   bindable, internal-only, passive, or intentionally unavailable; extend the
   existing `{category}/{identifier}` parser deliberately where needed and
   preserve `scriptkit://commands/...` round trips. Never derive identity from
   row index, mutable title, filter text, or a transient provider response.
   The current implementation now retains historical
   `script/{plugin}:{display-name}` and `scriptlet/{plugin}:{display-name}`
   as compatibility aliases while binding selection, descriptors, new copied
   links, exact command dispatch, and independently editable shortcut/alias
   preferences to full SHA-256 of the normalized source file plus scriptlet
   anchor/command. New Add/Update/Remove actions, focused rows, preview,
   alias editor, live alias registry, fallback execution, and config hotkeys
   must select the exact source first; existing plugin/name preferences remain
   readable/removable compatibility aliases until intentionally replaced.
   Preserve empty-source fallback, path privacy, existing query memory, exact
   parser round trips, and both source/legacy config-hotkey precedence.
6. Reuse the existing typed disabled-reason models when projecting an
   exhaustive state such as `Ready`, `MissingAuth`,
   `MissingPermission`, `MissingDependency`, `UnsupportedCapability`,
   `TemporarilyUnavailable`, and `RequiresConfirmation`, each with safe
   user-facing copy and a structured recovery reason.
7. Add adapters for built-ins and ordinary scripts first. Produce descriptors
   beside the existing models without replacing existing rendering or routing.
8. Adapt scriptlets, flows, skills, and AI commands next; preserve existing
   source-specific execution internals behind adapter implementations.
9. Adapt passive result families such as files, notes, conversations, and
   browser history as command-bearing entities with explicit primary actions.
10. Build actions, footer hints, source chips, and detail previews from the
   descriptor rather than independently recomputing title, shortcuts, state,
   and availability in each consumer. Prove real production call sites: a
   projection method with no non-test consumers is not integration.
11. Expose a redacted descriptor through protocol/DevTools so agents can ask
    why a command is available, unavailable, selected, or routed to AI.
12. Introduce compatibility adapters for existing `SearchResult`, `Action`,
    plugin-manifest, and menu-syntax consumers. Migrate one family per change.
13. Only remove duplicate fields after all active consumers read the canonical
    descriptor and behavior-level parity proves identical visible outcomes.

### Deliverables

- A versioned descriptor with documented serialization and identity rules.
- Source adapters for every supported command family.
- One availability/recovery explanation consumed by launcher, actions, footer,
  AI context, and DevTools.
- A fixture catalog containing at least one representative of every family.

### Verification and exit gate

```bash
./scripts/agentic/agent-cargo.sh test -p sk-protocol
./scripts/agentic/agent-cargo.sh check --lib --bin script-kit-gpui
bun test ./scripts/devtools/consistency.test.ts
bun scripts/devtools/surfaces.ts | jq '.totals'
```

- Crate compilation is `COMPILE-ONLY`; it does not prove a descriptor is used
  by a rendered launcher, Actions, footer, AI context, or DevTools collector.
- The `command_descriptor` app test filter is `FUTURE-ONLY` until its behavior
  tests exist and the owning module is reachable from the selected test
  target; require a nonzero `running N tests` receipt.
- Every deep-linkable new identity round-trips through the existing command-ID
  parser; internal/passive-only identities are explicitly not advertised as
  bindable or deep-linkable.
- Every supported family yields a stable descriptor with no fabricated fields.
- Duplicate IDs, missing required arguments, unsupported capabilities, and
  unavailable actions fail closed.
- Two same-name commands from the same plugin stay independently selectable,
  survive asynchronous reorder/display-title edits, produce distinct private
  source-fingerprinted share links, and execute only their exact original
  source; independently assigned shortcuts and aliases cannot replace/remove
  a sibling owner's preference; historical plugin/name and bare-name links
  and preference read/remove fallback continue working.
- Existing user-visible ordering and primary-action behavior remain unchanged
  until a later program deliberately changes them.

---

## 02. One navigation, focus, and keyboard interaction model

### Outcome

Enter, Escape, Tab, arrows, Cmd+K, Cmd+Enter, Cmd+W, selection, focus restore,
popup dismissal, and return routes feel predictable on every supported host.

### Primary ownership

- `src/main_sections/app_view_state.rs`
- `src/components/conversation_actions.rs` for the existing shared
  `resolve_conversation_dismissal` ladder and command availability
- `src/window_orchestrator/mod.rs` and `src/window_orchestrator/tests.rs` for
  the existing pure visibility/focus/window reducer
- `src/app_navigation/`
- `src/app_impl/startup.rs` and `src/app_impl/startup_new_actions.rs`
- `src/render_script_list/`
- `src/actions/window.rs`, `src/actions/dialog.rs`, and
  `src/actions/command_bar.rs`
- `src/app_impl/agent_handoff/agent_chat_entry.rs`
- `src/menu_syntax/trigger_picker_keys.rs`
- `scripts/devtools/keyboard.ts`, `focus.ts`, `act.ts`, and `actions.ts`

### Step-by-step implementation

1. Inventory every physical keyboard entry path: global capture interceptors,
   surface bubble handlers, popup handlers, and automation key-dispatch mirrors.
2. Enumerate host/surface combinations and record existing semantics for Escape,
   Cmd+W, Enter, Tab, Shift+Tab, Cmd+K, Cmd+Enter, arrows, Backspace, and
   active selection. Include intended differences such as editor data-loss
   confirmation and explicit quick-chat context suppression.
3. Map proposed `InteractionIntent`, `InteractionContext`,
   `InteractionOutcome`, `FocusOwner`, and `ReturnRoute` semantics onto the
   existing `LauncherSurfaceContract`, `DismissPolicy`, `FocusToken`,
   `WindowEvent`, `WindowCommand`, and conversation-dismissal owners before
   introducing any additional types.
4. Extend `resolve_conversation_dismissal` and `window_orchestrator::reduce`
   where their ownership fits. The existing orchestrator is already
   production-wired through `src/app_impl/window_orchestrator_bridge.rs`;
   extend that bridge instead of creating a second window state machine.
5. Extract the Script List Escape ladder into one pure ordered planner:
   close object selector, dismiss trigger picker, cancel attachment portal,
   clear visible filter, return to origin, or close/reset.
6. Encode exceptions explicitly, including legitimate
   `opened_from_main_menu == true` launcher states, mini-to-full transitions,
   attachment portals, and user-confirmed destructive dismissal.
7. Route physical capture, surface bubble, actions-window handling, and
   automation simulation through the same planner or its canonical outcome.
8. Define one ownership policy for Cmd+K: an actions hint is shown only when
   the current host can open a real actions menu with executable actions.
9. Make every transition capture and restore the correct focus owner, selected
   semantic ID, input draft, and return destination.
10. Add property-style behavior tests for legal/illegal transitions and
    table-driven parity across all keyboard entry paths.
11. Expand runtime matrices for filterable surfaces, attached popups, Notes,
    Agent Chat, Flow, prompts, and nested attachment portals.
12. Remove redundant handler branches and replace brittle source-audit locks
    with planner tests plus real-key/automation parity receipts.

### Verification and exit gate

```bash
./scripts/agentic/agent-cargo.sh test --lib window_orchestrator::tests
./scripts/agentic/agent-cargo.sh test --lib dismissal_resolver_closes_one_overlay_before_considering_active_work
bun test ./scripts/devtools/operator-safety.test.ts
bun scripts/agentic/filterable-surface-matrix.ts --list
bun scripts/agentic/attached-popup-surface-matrix.ts --list
```

- The two `--list` commands are `STATIC-INVENTORY` only. The separate
  `main-menu-escape-after-agent-chat-probe.ts` and
  `main-escape-visible-input-probe.ts` journeys are `RUNTIME-VISIBLE` until
  proven otherwise and are intentionally absent from the default command
  list. Run them only in a separately approved isolated session with their
  sidecar, binary, focus, screen, and interference prerequisites satisfied.
- Physical and automated key paths agree on visible outcomes.
- Exactly one interaction layer closes per Escape; no swallowed extra press.
- No advertised shortcut is unavailable, routed to the wrong host, or hidden
  behind stale focus.
- Dirty drafts and explicit context survive transitions according to policy.

---

## 03. A host-owned design system for every command surface

### Outcome

Command families can look different where their content requires it, but input,
rows, sections, footer hints, actions, empty states, loading, error copy, and
chrome always read as one native Script Kit product.

### Primary ownership

- `src/components/mod.rs`
- `src/components/text_input.rs` and `src/components/text_input/`
- `src/components/unified_list_item/`
- `src/components/prompt_layout_shell.rs`, `prompt_container.rs`,
  `prompt_footer.rs`, and `minimal_prompt_shell.rs`
- `src/components/footer_chrome.rs`, `hint_strip.rs`, `info_state.rs`, and
  `non_list_state.rs`
- `src/components/conversation_style.rs` and `conversation_text.rs`
- `src/ui/chrome/tokens.rs`, `src/theme/`, and `src/designs/core/`
- `tests/hardcoded_visual_inventory.py`

### Step-by-step implementation

1. Produce a real inventory of input, row, footer, empty/loading/error state,
   popup, and conversation-rendering owners for all 37 surface contracts.
2. Classify each current implementation as shared, compatible legacy,
   justified divergence, or accidental local duplicate. Preserve the two
   existing sanctioned general-purpose row owners (`crate::list_item` for
   launcher/built-ins/Actions and `UnifiedListItem` for select prompts), plus
   bounded specialized compact rows; the rule is no third general-purpose row
   system, not a forced migration to a single row implementation.
3. Extend the existing `LauncherSurfaceContract`, vocabulary, generated
   surface contracts, and active consistency ledger with presentation facets.
   `SurfacePresentationContract` names the desired semantics, not permission
   to invent a second competing surface registry.
4. Establish a canonical anatomy for launcher rows, command rows, actions rows,
   searchable popups, information states, and conversation turns.
5. Extend shared components where current consumers cannot express legitimate
   requirements; do not invent surface-local lookalikes or a third row system.
6. Move recurring spacing, color, opacity, typography, radius, border, and
   glyph values into existing theme/chrome/design-token layers.
7. Migrate the highest-visibility surfaces first: main launcher, actions menu,
   built-in lists, script prompts, Agent Chat/Flow, then secondary windows.
8. Preserve the existing calibrated glass-motion contract exactly. Visual
   consistency work is not authorization to retune production animation.
9. Split oversized owner modules only along behavior/ownership boundaries and
   only after characterization tests and direct runtime receipts exist.
10. Add semantic component metadata identifying the shared primitive used by a
    visible element, enabling structural parity checks without source-string
    tests.
11. Extend the existing hardcoded visual-value guard only for reviewed,
    meaningful deltas. Do not add broad allowlists or update fixtures to hide
    a visual regression.
12. Capture theme, dark/light, dense-data, accessibility-text, empty, loading,
    disabled, and error-state fixtures for every migrated surface.

### Verification and exit gate

```bash
python3 -B -m unittest tests/hardcoded_visual_inventory_test.py
bun test ./scripts/devtools/consistency.test.ts
bun scripts/devtools/surfaces.ts | jq '.totals'
./scripts/agentic/agent-cargo.sh test --lib components
```

- New command surfaces inherit shared components by construction.
- Shared theme/token changes propagate without per-surface patching.
- Existing accessibility, focus, footer glyph alignment, and glass calibration
  remain unchanged unless separately and explicitly approved.
- Any intentional divergence names its owner, rejected shared alternative, and
  product reason.

---

## 04. One indexed, explainable, deterministic search pipeline

### Outcome

Every keystroke produces stable, fast, explainable results across all command
and passive-data providers without stale updates, unexpected jumps, or
misleading highlights.

### Primary ownership

- `src/scripts/types.rs`, `src/scripts/grouping.rs`, and `src/scripts/search/`
- `crates/sk-protocol/src/search_contract.rs` for the pure source-owned
  worker, query-generation coordinator, and stale-completion lifecycle;
  `src/scripts/root_search_contract.rs` preserves app-service adapters and
  the existing app-facing compatibility path
- `crates/sk-protocol/src/sentence_search.rs` for pure Unicode-aware
  natural-language matching, proximity, evidence, and truthful highlights;
  `src/scripts/search/sentence.rs` preserves its existing compatibility path
- `src/main_sections/root_search_store.rs`
- `src/app_impl/filtering_cache.rs` and filter-input handlers
- `src/clipboard_history/cache.rs`, `src/dictation/history.rs`, and
  `src/ai/agent_chat/ui/history.rs` for private passive history snapshots
- `src/notes/storage.rs` and `src/menu_syntax/artifacts.rs` for source-owned
  Notes SQLite queries and bounded Todos day-page snapshots
- `src/menu_syntax/query.rs`, `payload.rs`, and `handler_index.rs`
- `src/spine/catalog_filter.rs` and `src/spine/input_spans.rs`
- `src/watcher/`
- provider owners under files, notes, Brain, conversations, clipboard,
  dictation, browser tabs, browser history, windows, apps, and AI Vault

### Step-by-step implementation

1. Enumerate all root providers, their backing storage, current latency,
   invalidation mechanism, result identities, query syntax, and failure mode.
2. Inventory the existing `RootSearchStore`, `RootPassiveFrame`,
   `root_windows_refresh_token`, `root_brain_search_generation`,
   `root_file_search_generation`, frozen source frames, caches, semantic
   selection keys, and `MatchEvidence`. Map `SearchQuery`, `ProviderRequest`,
   `ProviderGeneration`, `SearchCandidate`, `RankingEvidence`, and
   `SearchSnapshot` semantics onto those owners before adding new types.
   Reuse the actual GPUI-free coordinator and source-owned worker in
   `crates/sk-protocol/src/search_contract.rs`: their seven real generation,
   cancellation, exact-query, and stale-worker regressions execute directly
   in the protocol crate. The full pure sentence matcher and 19 Unicode,
   stopword, exact-word, live-prefix, ranking, and highlight regressions also
   execute in that same **77-case GPUI-free crate**.
   `src/scripts/root_search_contract.rs` and `src/scripts/search/sentence.rs`
   retain the app-dependent Notes/Todos/Brain adapters and compatibility
   re-exports. The GPUI-bearing `RootSearchStore` remains binary-owned; its
   old binary-local tests are still excluded by `Cargo.toml`'s `test = false`.
3. Reuse `CommandDescriptor` identity and capability metadata from Program 01
   instead of duplicating source IDs, actions, aliases, and permissions.
4. Build immutable provider indexes in the background; publish complete
   generation-tagged snapshots atomically.
5. Normalize exact matches, prefixes, fuzzy matches, aliases, shortcuts,
   frecency, context relevance, user preferences, and source weights into one
   explicit ranking policy.
6. Preserve the field-level `MatchEvidence` that admitted each candidate and
   use that exact evidence for row highlighting and previews.
7. Generalize the generation/token rejection that already exists for Brain,
   root Windows, and root Files to providers that still lack it. Preserve the
   existing `stale_semantic_batches_are_rejected_and_current_batches_install`
   behavior rather than rewriting already-correct provider families. Notes,
   Todos, Clipboard, Dictation, and Agent Chat now use one real
   library-owned source/generation
   worker contract: start refresh only from either actual input update owner,
   never from the foreground cache getter; reject stale/foreign completions
   before snapshot publication; reject an outdated Notes storage epoch;
   reconcile only the live query while preserving the selected row; treat a
   successfully loaded empty cache as warm; keep explicit and passive source
   grouping free from synchronous JSONL/SQLite/30-file markdown reads; honor
   explicit Notes result limits up to the overall bounded launcher maximum;
   and fingerprint every actual Notes success/fallback/failure event.
8. Partition visible output into explicit stable sections so a late passive
   provider cannot unexpectedly reorder already selected primary commands.
9. Assign source-specific deadlines and partial-result policies. A failed or
   slow browser/file/network-backed provider must never block local typing.
10. Support transparent result explanations: winning field, relevance tier,
    provider, context boost, frecency boost, and user preference.
11. Add deterministic fixtures for 10, 100, 1,000, and 10,000 command catalogs
    with slow, failing, duplicate, invalid, and concurrently changing sources.
12. Wire filesystem/watch invalidation to narrow affected provider indexes and
    prove that changing one script does not trigger a full synchronous rebuild.
13. Expand the semantic frame-stability gate to cover selection identity,
    section stability, stale generation refusal, and user-visible paint.

### Verification and exit gate

```bash
./scripts/agentic/agent-cargo.sh test -p sk-protocol
./scripts/agentic/agent-cargo.sh test --lib scripts::search
./scripts/agentic/agent-cargo.sh test --lib scripts::root_search_contract::root_search_store_tests
bun test ./scripts/agentic/root-search-frame-stability.test.ts
bun scripts/agentic/root-search-frame-stability.ts --help
bun scripts/agentic/root-typing-lag-benchmark.ts --help
```

- The real library-reachable
  `scripts::root_search_contract::root_search_store_tests` owner already
  executes **11 nonzero cases** covering provider ownership/generation fences,
  stale completions, legitimate empty snapshots, source isolation, and Brain
  Inbox identity/content freshness. Do not confuse this extracted owner with
  the separately disabled GPUI-bearing binary-local tests.
- The two `--help` commands are safe `STATIC-INVENTORY` checks, not search or
  latency proof. Real gates require a pinned binary, sandbox session, explicit
  output receipt, and user-path-specific result assertions.
- The same query and catalog generation produce the same order and highlights.
- A slow or failed passive provider cannot drop input, reorder the selected
  command, or replace a newer query with stale results.
- Every result exposes provenance and ranking evidence.
- Search performance stays within the ratified painted-outcome budgets from
  Program 05 across realistic catalog sizes.

---

## 05. Real, ratified end-to-end latency budgets

### Outcome

“Instant” is a measurable product property: input, visible content, actions,
surface transitions, command launch, and AI acknowledgement stay within
calibrated p50/p95 budgets on representative hardware.

### Primary ownership

- `src/main_sections/app_state.rs`
- `src/main_entry/app_run_setup.rs` and `src/startup_profile.rs`
- `src/app_impl/filtering_cache.rs` and filter-input handlers
- `src/ai/phase_trace.rs` and `src/ai/agent_chat/ui/thread.rs`
- `scripts/devtools/perf.ts` and `scripts/devtools/driver.ts`
- `scripts/agentic/root-typing-lag-benchmark.ts`
- `scripts/agentic/root-search-frame-stability.ts`
- `scripts/agentic/multisurface-keystroke-latency-probe.ts`
- `scripts/agentic/multisurface-scroll-perf-probe.ts`
- `scripts/agentic/quick-ai-latency-bench.ts`
- `.github/workflows/perf-gates.yml`

### Step-by-step implementation

Precondition status: **implemented and verified in the current worktree**.
`quick-ai-latency-bench.ts` now supports safe `--help`,
`--describe-contract`, and `--print-command` inspection; unknown options
fail closed, paid/network execution requires `SCRIPT_KIT_ALLOW_LIVE_AI=1`,
and `SCRIPT_KIT_NONINTERACTIVE=1` categorically refuses provider startup even
when the live opt-in is present. Preserve these guards; actual provider or
visible paint runs remain separately permission-gated and unproven.

1. Define exact observation points for hotkey-to-visible-frame,
   hotkey-to-input-ready, key-to-paint, Enter-to-visible-acknowledgement,
   Cmd+K-to-actionable-menu, transition-to-focus, AI-submit-to-acknowledgement,
   and AI-submit-to-first-readable-content.
2. Clearly distinguish native event receipt, GPUI state update, next-frame
   callback proxy, compositor-visible paint, and actual screenshot evidence.
   Never label state echo or callback proxy as confirmed paint.
3. Record machine model, display refresh rate, build profile, active background
   load, binary SHA, scenario, fixture size, provider state, and sample count.
4. Capture at least ten repeatable cold and warm baseline runs for each user
   journey on an agreed reference machine.
5. Present candidate p50/p95/max thresholds for explicit product-owner
   ratification. Keep the existing 25 ms p50 proposal marked pending until
   approved; do not silently adopt historical notes as production budgets.
6. Extend the existing root benchmark with a separate, honestly named painted
   observation path and retain its current state-echo metric for diagnostics.
7. Build artifact-pinned probes for hotkey readiness, first-key delivery,
   actions-menu readiness, command launch, and surface morph transitions.
8. Extend the existing `src/ai/phase_trace.rs` lifecycle, which already emits
   `TURN_START`, `FIRST_PROVIDER_EVENT`, `FIRST_VISIBLE_OUTPUT`,
   `FIRST_THOUGHT`, `TOOL_CALL_STARTED`, `TERMINAL`, and `TEARDOWN`. Separate
   any genuinely missing launch/prewarm/auth/paint timings without recreating
   an AI phase trace or claiming semantic visible output is compositor paint.
   Preserve the production `0600`/`O_NOFOLLOW`, keyed-fingerprint,
   single-record-append contract; observability writes deliberately omit
   fsync so the instrument never fabricates its own latency regression.
9. Measure realistic catalogs, long previews, asynchronous providers, slow
   browser/file sources, heavy conversations, and scrolling while streaming.
10. Run semantic/non-timing determinism gates on pull requests. Run calibrated
    hardware-sensitive percentile gates on dedicated macOS runners or a
    controlled nightly lane.
11. Upload signed/attributed JSON receipts containing metric kind, observation
    point, target identity, p50/p95/max, sample counts, budget, binary SHA,
    source SHA, environment, and classification.
12. Prove every gate can go red by injecting a deterministic regression; retain
    a passing recovery run with the same observation point and scenario.
13. Profile only after a reproducible gate fails. Prioritize synchronous UI
    work, repeated index construction, row-wide layout invalidation, duplicate
    filtering, blocking filesystem access, and avoidable child startup.

### Verification and exit gate

```bash
bun scripts/agentic/root-typing-lag-benchmark.ts --help
bun scripts/agentic/quick-ai-latency-bench.ts --help
bun scripts/agentic/quick-ai-latency-bench.ts --describe-contract
bun scripts/agentic/quick-ai-latency-bench.ts --print-command
bun test ./scripts/agentic/root-typing-lag-benchmark.test.ts
bun test ./scripts/agentic/root-search-frame-stability.test.ts
bun test ./scripts/agentic/quick-ai-latency-bench.test.ts
bun test ./scripts/agentic/ai-phase-trace-report.test.ts
```

- `quick-ai-latency-bench.ts --help`, `--describe-contract`, and
  `--print-command` are separately verified non-provider paths.
  `SCRIPT_KIT_NONINTERACTIVE=1` always refuses an attempted provider run,
  and interactive live runs require explicit `SCRIPT_KIT_ALLOW_LIVE_AI=1`.
  Keep historical unsafe-flag findings clearly dated; do not treat them as
  an outstanding defect in the current worktree.
- `driver-benchmark.ts` immediately launches an app and runs up to 100 Driver
  scenarios plus legacy sessions. Run it only as an explicitly classified
  `RUNTIME-HIDDEN` benchmark after sidecar, binary, sandbox, and disk checks;
  it is not a harmless static validation command.
- Every enforced threshold is calibrated and explicitly ratified.
- Benchmark CLI inspection and noninteractive execution fail closed before
  any app launch, focus change, system input, capture, provider call, or
  credential copy.
- State echo, frame-callback proxy, and confirmed paint are reported as
  distinct metrics.
- The CI workflow executes—not merely syntax-checks—the relevant gates.
- A release candidate fails closed when a ratified critical latency budget
  regresses under the documented environment.

---

## 06. A common command-execution lifecycle and receipt

### Outcome

Every command exposes a truthful lifecycle, understandable progress, reliable
cancellation, bounded resources, typed failure recovery, and a durable,
redacted execution receipt.

### Primary ownership

- `src/executor/runner.rs`, `scriptlet.rs`, `errors.rs`, and `telemetry.rs`
- `src/app_execute/` and `src/app_impl/execution_scripts.rs`
- `src/prompt_handler/`
- `src/flows/runner.rs`, `src/flows/run_registry.rs`, and `src/flows/session.rs`
- `src/ai/reliability/`
- `src/protocol/transaction_trace.rs` and `transaction_executor.rs`
- `src/mcp_resources/transaction_resources.rs`

### Step-by-step implementation

1. Enumerate execution paths for built-ins, Bun scripts, shell scriptlets,
   external applications, background flows, prompt-hosted scripts, AI turns,
   and commands requiring OS permissions.
2. Map the existing `AiPhase` / `AiOutcome` reducer, Flow run registry,
   process-group lifecycle, transaction executor, and persisted
   `TransactionTrace` onto a shared lifecycle: `Ready -> Preparing -> Running
   -> Streaming / Progress -> Completed | Failed`, with cancellation following
   the separate nonterminal `Running / Streaming -> Cancelling ->
   cleanup-confirmed -> Cancelled` path. This contract is already implemented
   in `crates/sk-protocol/src/execution_contract.rs` and consumed by actual
   `src/flows/run_registry.rs::FlowRun::execution_receipt`; eight domain
   behavior cases plus the 20-case production Flow registry suite prove
   nonterminal cancellation, exact run/process-group cleanup ownership,
   rejection of premature terminals, and privacy-safe final receipts.
   Extend these owners for other command families without replacing richer
   AI/Flow guarantees with a lossy generic state.
3. Document which operations are synchronous, backgroundable, streaming,
   cancellable, retryable, destructive, or user-confirmation gated.
4. Introduce a host execution broker that accepts `CommandDescriptor` plus
   validated arguments/context and delegates actual execution to existing
   source-specific runners.
5. Run availability, dependency, permission, authentication, argument, and
   confirmation checks before starting child processes or side effects.
6. Preserve process-group ownership, graceful SIGTERM/SIGKILL escalation,
   orphan detection, duplicate-submit suppression, and timeout identity.
   Do not expose terminal `Cancelled` on SIGTERM dispatch; confirm the exact
   owned process group has exited first, matching the existing Flow contract.
7. Emit one structured event stream for preparing, progress, output,
   completion, typed failure, user cancellation, cleanup, and retry.
8. Normalize UI behavior: immediate acknowledgement, stable progress copy,
   action availability, safe backgrounding, one terminal toast when relevant,
   and preserved user drafts on recoverable failures.
9. Extend the existing `TransactionTrace` schema/persistence and DevTools
   receipt envelope where suitable. Persist a redacted execution receipt with
   run ID, descriptor ID, timings, final classification, safe user message,
   diagnostic fingerprint, cleanup status, and relevant output metadata; do
   not create a second uncorrelated flight recorder.
10. Surface execution history and current operations through DevTools/MCP
    without exposing secret arguments, provider payloads, or raw transcripts.
11. Add fault-injection fixtures for spawn failure, broken pipes, child crash,
    timeout, ignored SIGTERM, nested subprocesses, provider refusal, user
    cancellation, duplicate submission, and app shutdown during execution.
12. Migrate command families incrementally, keeping compatibility shims until
    real behavior and receipt parity pass for each existing route.

### Verification and exit gate

```bash
./scripts/agentic/agent-cargo.sh test -p sk-protocol
./scripts/agentic/agent-cargo.sh test --lib executor
./scripts/agentic/agent-cargo.sh test --lib ai::reliability
./scripts/agentic/agent-cargo.sh test --lib flows
```

- Every started command reaches exactly one terminal state.
- A cancellation request is visible as nonterminal `Cancelling` until the
  exact child/process group is proven dead; cleanup cannot be supplied as an
  optimistic caller-owned boolean.
- Cancellation does not appear as a failure and leaves no live owned process
  group.
- Recoverable failure preserves inputs/context and exposes a real action.
- Every visible completion claim corresponds to the run's verified terminal
  receipt, not a child-spawn event or an optimistic UI update.

---

## 07. One coherent, trustworthy AI experience

### Outcome

Quick AI, Agent Chat, Flow conversations, rewrite tools, note handoffs,
script-provided AI commands, and focused-text actions feel like intentional
modes of the same reliable assistant.

### Primary ownership

- `src/app_impl/agent_handoff/agent_chat_entry.rs` and
  `src/app_impl/agent_handoff/mod.rs`
- `src/ai/entry_contract.rs` for the library-reachable, production-used
  clean-question and explicit-context policy
- `src/ai/agent_chat/launch.rs`, `profiles.rs`, and `ui/`
- `src/ai/agent_chat/ui/config.rs` for owner-only Claude MCP credential
  synchronization, custom-agent catalogs, private project history, and
  serialized authentication/runtime-state persistence
- `src/ai/agent_chat/ui/favorite_models.rs` and `ui/thread.rs` for private,
  atomic, serialized model favorites and user-visible persistence failure
- `src/ai/presets.rs` and `src/render_builtins/ai_presets.rs` for private
  user-authored system prompts, safe preset import/export and mutation,
  and keyed launcher-side preset diagnostics
- `src/ai/agent_chat/ui/history.rs`, `history_attachment.rs`, `thread.rs`, and
  `chat_window.rs` for all four owner-bound private-history stores, truthful
  completed-turn persistence, and visible private-storage failure recovery
- `src/ai/agent_chat/ui/{export.rs,view.rs}` for owner-only, collision-safe,
  no-follow user-requested conversation exports and truthful export failure
- `src/ai/session.rs` and `src/ai/providers.rs` for typed provider-error
  facts, exact user cancellation, fail-closed retry ownership, complete
  nonempty response validation, and final-only output delivery
- `src/ai/reliability/` and `crates/sk-protocol/src/ai_reliability/`
- `src/ai/phase_trace.rs` and `src/ai/agent_chat/codex_exec.rs` for private
  shared cross-surface and Quick AI timing/diagnostic trace ownership
- `src/ai/message_parts.rs` for cryptographically keyed prepared-context
  content fingerprints and private model-bound context assembly
- `src/ai/agent_prompt_handoff.rs` for actual external-agent prompt files,
  user-exported prompts, executable handoff wrappers, and private receipts
- `src/atomic_file.rs` for reusable no-follow owner-only directory/file
  creation, permission repair, append, bounded JSONL repair, unsynced
  single-record observability append, and unique atomic replacement
- `src/utils/db_permissions.rs`, `src/ai/storage.rs`, `src/notes/storage.rs`,
  `src/brain/store.rs`, and both `src/clipboard_history/{database.rs,
  db_worker/mod.rs}` owners for private SQLite primary/WAL/SHM lifecycle
- `src/brain/substrate/{io.rs,paths.rs,day.rs,trash.rs}`,
  `src/brain/{indexer.rs,day_trace.rs}`, `src/day_page/{document.rs,
  sediment.rs}`, `src/notes/{storage.rs,day_switcher.rs}`, and
  `src/notes/window/{init.rs,notes.rs}` for canonical private Brain/Notes/day
  markdown, owner-only readers, indexing, fragment provenance, and safe trash
- `src/protocol/transaction_trace.rs`, `src/protocol/types/`,
  `src/ai/reliability/{diagnostics.rs,devtools.rs}`, `src/dictation/runtime.rs`,
  `src/main_sections/day_page_{actions.rs,context_round_trip.rs}`, and
  `scripts/devtools/lib/privacy.ts` for non-guessable process-keyed private
  receipt, semantic, AI, transcript, and automation fingerprints
- `src/ai/preflight_audit.rs` for private AI preparation receipts and atomic
  bounded log compaction
- `src/ai/current_app_automation_memory/` and `src/ai/tab_context.rs` for
  private user intent, prompts, generated source, automation recipes, and
  execution receipts
- `src/ai/harness/screenshot_files.rs` for owner-only image files and
  fingerprint-only screenshot/window/error diagnostics
- `src/platform/selfie_capture.rs` for owner-only, no-follow screenshot
  images/receipts, private capture directories, and synthetic-byte-only
  artifact verification without screen capture
- `src/app_impl/webcam_actions.rs` and `src/atomic_file.rs` for shared
  collision-safe, exclusive, no-follow `0600` user exports and webcam photos;
  webcam verification uses synthetic bytes and never accesses a camera
- `src/flows/session.rs`, `codex_client.rs`, and `runner.rs`
- `src/dictation/history.rs` for complete private spoken-transcript history
- `src/mcp_resources/mod.rs` for Dictation-native `preview`/`text` and
  `target` projection into the shared provider-item/search contract
- `src/notes/storage.rs` for private, collision-safe Notes conflict recovery
- `src/notes/window/ai_handoff.rs`
- `src/components/conversation_style.rs`, `conversation_text.rs`,
  `conversation_actions.rs`, and `ai_recovery.rs`
- `rules/AI_RELIABILITY.md`

### Step-by-step implementation

1. Enumerate every AI entry point and classify its user intent: clean quick
   question, contextual answer, ongoing conversation, explicit row handoff,
   note handoff, selected-text rewrite, script assistance, or autonomous task.
2. Extend existing `AgentChatEntryRequest`, `AgentChatEntryIntent`,
   `AgentChatContextPolicy`, typed reliability operations, and
   `ConversationCommandDescriptor`. `AiIntent` describes the target semantics;
   it must not become a second entry protocol when existing request types can
   carry origin, target, profile, model, context, disposition, return route,
   and approval requirements.
3. Preserve the locked clean-chat rule:
   `AgentChatEntryRequest::quick_question()` opens an empty composer with no
   auto-selected launcher row, hidden chips, or automatic submission. Extend
   the existing production-used `src/ai/entry_contract.rs::AiEntryPolicy`
   owner: its library-reachable clean-question and ambient-context rejection
   regressions execute through `cargo test --lib`, while the older duplicated
   binary-only regression still cannot serve as an independent test gate.
4. Define explicit context sources and provenance for selected text, active
   app, current file, launcher item, note, screenshot, clipboard, browser tab,
   Brain memory, uploaded attachment, and user-entered text.
   Persist Flow transcript ownership using both original flow ID and complete
   definition path, never a lossy display slug or truncated project prefix;
   reject a snapshot whose embedded owner differs, and atomically claim
   legitimate id-only/path-qualified legacy history before returning private
   turns. Route every Flow and Dictation primary/legacy transcript through the
   shared opened-descriptor `0600`/`O_NOFOLLOW` private-file contract; repair
   older permissive files before exposing private data, reject symlinks at all
   names, repair missing JSONL boundaries before appending, reject malformed
   records instead of silently dropping them, and atomically replace complete
   compacted history. Serialize Dictation/Agent Chat save, deletion, and
   compaction ownership so concurrent writers cannot erase adjacent private
   work. Publish Dictation provider data and saved IDs only after actual
   durable persistence; retain the previous valid resource on load failure.
   Save a completed Agent Chat conversation and its private index together,
   refuse malformed existing indexes before mutating any conversation, and
   show a safe recovery notice whenever durable storage fails. Apply the same
   strict private JSONL boundary, malformed-record rejection, serialized
   mutation, and truthful visible failure policy to submitted composer prompt
   recall without blocking delivery of the active message. Preserve v0-v3
   migration and fail closed on failed/foreign claims.
   Apply that same contract to preflight audits, both current-app/Tab AI
   automation memories, Tab AI execution receipts, and focused/full-screen
   screenshot PNGs; repair JSONL append boundaries through the already-open
   descriptor, never expose path/window-title/provider-error text, and verify
   image persistence with synthetic PNG bytes instead of capturing the screen.
   Prepare the shared focused/full-screen screenshot temporary directory as
   `0700` through the no-follow owner before either capture writes its image;
   repair permissive legacy directories and reject symlinked roots.
   Keep Script Kit Selfie screenshots and their provenance receipts in a
   repaired `0700` directory, preflight both no-follow artifact targets
   before writing either, replace both through private `0600` atomic files,
   and refuse foreign destinations or planted links. Remove any historical
   source-audit expectation that unsafe `std::fs::write` remain present;
   executable synthetic-byte permission/link tests are the behavior contract.
   Persist user-requested Agent Chat Downloads exports in exclusive `0600`
   no-follow files; suffix repeated exports, reject symlinked directories and
   destinations, sanitize session-derived names, and keep raw OS errors out
   of the user-visible export recovery message. Reuse that same shared
   exclusive owner for user-requested webcam photos: preserve every same-second
   capture, reject symlinked Desktop destinations, fingerprint private paths,
   and verify the actual production writer with synthetic bytes only.
   Create prompt-export/handoff directories at `0700` from first appearance;
   repair legacy permissions through an opened no-follow directory handle;
   reject hostile export-directory, prompt-file, wrapper, and receipt
   symlinks; write complete private prompts and receipts through unique
   exclusive `0600` atomic siblings; and add owner-only execute permission to
   wrapper scripts only after their private bytes are fully persisted.
   Fingerprint actual prepared-context receipts with the same ephemeral
   cryptographically random HMAC key, never PID/timestamp-derived salt.
   Persist token-bearing Claude MCP configuration, managed-server ownership,
   custom-agent API-key catalogs, private project MRUs, and auth/runtime
   state through the same no-follow `0600` owner; repair permissive legacy
   stores before reading secrets and reject malformed/symlinked state without
   overwriting another owner. Serialize the entire background auth-state
   read/merge/atomic-write transaction so concurrent agent saves preserve
   every owner and never regress established authentication facts.
   Apply identical private, no-follow atomic ownership to Agent Chat model
   favorites; repair permissive legacy files before reading, reject malformed
   and symlinked stores without replacing another owner's bytes, serialize
   every read/toggle/write transaction, and report save failures visibly
   instead of optimistically repainting an unpersisted favorite. The owner
   change and all four isolated regressions are compiled and passing; the two
   existing normalization/compatibility cases pass in the same owner.
   Treat saved AI-preset system prompts and explicit preset imports/exports as
   private `0600` no-follow artifacts too. Repair permissive legacy files
   before reading private prompts; keep exported prompt bundles owner-only;
   refuse planted links; reject malformed owned state instead of replacing it
   with an empty list; serialize complete create/import/delete transactions;
   and fingerprint user-created preset names/IDs and storage errors in real
   launcher diagnostics. Six path-isolated owner regressions pass without
   launching an AI provider or touching non-isolated user files.
   Protect private AI chat, Notes, Brain, and clipboard SQLite primaries from
   first creation rather than repairing them after schema initialization:
   prepare existing/new files as `0600` via no-follow descriptors; repair
   existing WAL/SHM permissions before private bytes are read; open SQLite
   with `SQLITE_OPEN_NOFOLLOW`; reject hostile primary, sidecar, and parent
   links; resolve macOS's trusted `/var` alias without weakening final-path
   no-follow; recheck newly materialized WAL/SHM files fail-closed; use the same
   owner in both real clipboard database paths; and never let Notes/Brain
   corruption recovery rename a planted foreign link. All ten focused isolated
   regressions pass, along with five existing owner/recovery compatibility
   cases, without opening the app or touching the real clipboard.
   Extend that same owner-only opened-descriptor contract to every canonical
   Brain/Notes/day markdown root and document, not only SQLite/transcript
   sidecars. Repair older directory/file permissions before indexing, parsing,
   switching, AI context assembly, or fragment traversal; reject hostile
   directory/document links, foreign roots, and path escapes without reading
   or mutating another owner's bytes; preserve same-name trash collisions;
   and allow restore only inside the verified private Brain tree. Keep the
   actual Notes window, Day Page editor, Brain indexer, day switcher, AI day
   traces, and fragment provenance on that production owner rather than
   adding a synthetic test-only file path.
   Route semantic element fingerprints, private Choice IDs, transaction
   payload markers, AI diagnostic vault/DevTools identities, Dictation
   transcript/microphone/device summaries, Day Page handoff receipts, and
   legacy DevTools main/focus/text/dictation receipts through the existing
   process-private cryptographic owner. Preserve protocol wire shape where
   required, but reject publicly computable SHA-256/FNV private markers; the
   only retained public SHA is the existing nonprivate AppKit footer label
   required for cross-runtime accessibility parity.
   Classify actual persistent-Claude `result` records from provider-stated
   `is_error` and error-subtype facts; preserve structured failure detail for
   safe diagnostic handling; refuse missing/empty final responses in both
   persistent and spawned transports; and deliver final-only successful
   responses to the visible chunk owner exactly once. Honor a real streaming
   callback's Stop request as typed cancellation only after killing/reaping
   the exact owned child; never resubmit an accepted provider failure or a
   response which already emitted visible chunks. Seven pure
   parser/cancellation/retry cases plus one callback-delivery case pass
   entirely in memory without starting Claude.
   Preserve exact prompt SHA only in private interoperability receipts;
   all six launcher/Agent Chat diagnostics expose keyed prompt fingerprints
   and keyed export-path/private-gist identities instead.
   Route shared cross-surface and Quick AI traces through the owner-only,
   no-follow, single-record observability writer without fsync; repair
   permissive legacy traces, reject symlink targets, retain first-milestone
   latching and exact native record boundaries, and replace publicly
   guessable private-answer/reasoning/query/provider-error SHA-256 digests
   with ephemeral process-keyed HMAC fingerprints.
   Persist generated conversation summaries/full transcripts with the same
   validated owner, atomic `0600` no-follow file policy, and private `0700`
   directory; repair older permissive directories before writing. Per-session
   deletion must remove only that session's summaries/transcripts; global
   clear must preflight and remove conversations, index, prompt history,
   and generated attachments as four owned stores. Notes conflict recovery
   must exclusively create private no-follow files and suffix same-second
   collisions without overwriting an existing recovery artifact. Route both
   the main Day Page session and the independent Notes-window day editor
   through the same validated Brain-owned private conflict writer: a
   non-append external edit must be preserved before either editor replaces
   the bound file, hostile source/trash paths fail closed, same-second copies
   remain independent, and diagnostics expose only keyed path fingerprints.
5. Show included context before submission and distinguish user-selected
   context from context that is merely available to add.
6. Introduce one profile/model readiness contract that validates sidecar
   availability, login, provider configuration, selected-model compatibility,
   runtime health, and supported tool capabilities.
7. Refuse stale selections honestly. Never silently replace a user-selected
   profile/model with an unrelated fallback or claim readiness from a metadata
   endpoint when a real request path is broken.
8. Share visible phases through the existing `AiPhase` reducer and
   `PhaseTrace`, including preparing, authenticating, thinking, tool activity,
   streaming, approval required, completed, cancelled, and recovery available.
9. Reuse one conversation renderer, copy action, scroll affordance, reasoning
   disclosure policy, approval presentation, and accessible semantic IDs.
10. Carry the typed `AppFailureRecord` from provider/runtime boundary to the
    visible recovery state; never classify formatted prose or safe copy.
11. Mount actionable recovery controls only where the host can actually
    perform them: footer, actions menu, or blocking modal as appropriate.
    Prioritize the concrete current DevTools gap: a green recovery-action
    activation receipt is missing for Agent Chat and Chat Prompt; Flow lacks
    green rethread/restart/reattach action receipts.
12. Add fixture-backed failures for missing login, stale model, absent sidecar,
    unsupported provider capability, dropped stream, closed runtime, network
    timeout, denied permission, and mid-turn cancellation.
13. Measure warm/cold setup, provider readiness, submit-to-acknowledgement,
    first token, first readable paint, tool duration, and recovery completion.
14. Add cross-surface handoff proofs showing that Notes, Flow, launcher,
    detached chat, focused text, and script errors target the intended host and
    preserve exactly the declared context.
15. Split oversized Agent Chat owners by responsibility only after shared
    intent, context, recovery, and renderer contracts provide safe seams.

### Verification and exit gate

```bash
./scripts/agentic/agent-cargo.sh test -p sk-protocol
./scripts/agentic/agent-cargo.sh test --lib ai::reliability
bun test ./scripts/agentic/ai-phase-trace-report.test.ts
bun scripts/devtools/coverage.ts --surface agent-chat
```

- `src/ai/entry_contract.rs` already supplies a library-reachable
  `quick_question_entry_suppresses_all_implicit_context` regression and an
  additional hostile ambient-context/seed/selection/submission rejection
  case; require their nonzero matching behavior receipts. The older test
  duplicated in the `test = false` binary module remains insufficient alone.
- Every AI entry has explicit intent, context provenance, host target, and
  open-versus-submit behavior.
- Provider/runtime failures preserve user work and offer a genuinely executable
  recovery action.
- Distinct projects with equal Flow IDs, sanitized punctuation aliases, or a
  shared long path suffix can never read, rewrite, adopt, or overwrite one
  another's transcript or FIFO revision.
- A stopped user turn is `Cancelled`, never an error.
- Live Agent Chat, Flow, and Quick AI success/failure paths have direct
  target-scoped semantic receipts and truthful phase timings.

---

## 08. Capability-aware script authoring and compatibility

### Outcome

Authors know what the host supports before a script runs; schemas, prompts,
actions, permissions, compatibility, diagnostics, and examples remain aligned
with the same runtime command contract.

### Primary ownership

- `scripts/kit-sdk.ts`
- `kit-init/types/menu-syntax.d.ts`
- `kit-init/sdk/menu-syntax.ts` and its tests
- `src/scripts/types.rs`, `validation.rs`, and metadata/schema parsers
- `src/menu_syntax/payload.rs`, `metadata.rs`, `doctor.rs`, and
  `capture_schema.rs`
- `src/mcp_resources/mod.rs`
- `tests/sdk/test-window-management.ts` and `test-fields-datetime.ts`
- `src/window_control/registry.rs` and `actions.rs`
- `src/setup/mod.rs` and `kit-init/examples/`
- `scripts/check-sdk-types.ts` and `scripts/test-runner.ts`

### Step-by-step implementation

1. Preserve and independently reverify the current worktree's repair of the
   formerly failing SDK release gate with
   `SCRIPT_KIT_NONINTERACTIVE=1 bun run scripts/test-runner.ts --filter
   window-management`. Preserve the native registry's existing stale-ID
   rejection; prove every stale/unknown window-action ID rejects, valid
   synthetic IDs succeed, and the complete strict suite stays green without
   weakening assertions, enabling system input, or fabricating native proof.
2. Inventory every SDK export, prompt, action, metadata field, permission,
   dependency, message shape, and command schema consumed by the Rust host.
3. Reconcile SDK exports, generated SDK references, starter templates,
   TypeScript declarations, runtime dispatch handlers, and visible docs.
4. Publish a versioned capability catalog with `supported`, `experimental`,
   and `unsupported` states, minimum host version, required permissions,
   platform limits, alternatives, and migration notes.
5. Preserve the already-reconciled current inventory: `hotkey()`, `mini()`,
   `micro()`, and `fields()` are marked supported rather than incorrectly
   unsupported; the authoritative denied list now contains **18** genuinely
   unavailable capabilities, each with an actionable explanation. Existing
   focused SDK receipts prove two hotkey cases, five editor/mini/micro cases,
   and eight date/time/search `fields()` cases; the nine capability/reference
   Rust owners verify both supported and unsupported projections. Keep the
   rewritten subtype-specific `test-fields-datetime.ts` behavior coverage;
   classify any remaining name against SDK export, protocol variant, message
   routing, renderer, semantic collector, and direct proof. Isolated
   auto-submit is still not native UI or permission evidence.
6. Generate TypeScript author-facing types from canonical command/prompt
   schemas, or generate both languages from one reviewed schema definition.
7. Extend script validation to flag unsupported SDK capabilities, malformed
   command schemas, invalid shortcuts, duplicate aliases/keywords/triggers,
   absent dependencies, impossible permission combinations, and unavailable
   action callbacks.
8. Present author diagnostics as actionable launcher rows with source path,
   precise field, safe explanation, conflict owner, and suggested repair.
9. Model the actual execution route before previewing author capabilities:
   ordinary TypeScript scripts and launcher-routed TypeScript scriptlets have
   interactive SDK prompt transport; legacy synchronous scriptlet invocation
   does not; shell/Python scriptlets do not gain SDK access. Derive topology
   from the real host path, never the file extension alone, and reject only
   genuinely impossible interactive prompts before a command can hang.
10. Add a local command preview harness that renders a descriptor's row,
   argument form, loading state, action menu, success, and representative
   failure without performing the command's side effects.
11. Make generated starter scripts and AI-authored scripts consume the same
   capability catalog so examples never imply an unavailable API works.
12. Add backward-compatibility fixtures for realistic old/new metadata,
    scriptlets, nested action definitions, long labels, unusual Unicode,
    malformed schemas, and missing external runtimes.
13. Define explicit compatibility/migration policy for renamed prompts,
    changed argument semantics, removed SDK helpers, and unsupported legacy
    APIs. Never silently swallow an unsupported message.
14. Add a command doctor that reports readiness per script, plugin, dependency,
    permission, schema, action, and host version with repair instructions.
15. Require each newly supported API to add author-facing type coverage,
    runtime behavior proof, failure semantics, DevTools visibility, and at
    least one checked-in executable fixture.

### Verification and exit gate

```bash
bun run scripts/check-sdk-types.ts
./node_modules/.bin/tsc --noEmit --lib ES2022 --target ES2022 --types node --moduleResolution bundler --module ES2022 --skipLibCheck kit-init/sdk/menu-syntax.test.ts kit-init/types/menu-syntax.test.ts
SCRIPT_KIT_NONINTERACTIVE=1 bun run scripts/test-runner.ts --filter hotkey
SCRIPT_KIT_NONINTERACTIVE=1 bun run scripts/test-runner.ts --filter editor
SCRIPT_KIT_NONINTERACTIVE=1 bun run scripts/test-runner.ts --filter fields-datetime
SCRIPT_KIT_NONINTERACTIVE=1 bun run scripts/test-runner.ts --filter window-management
SCRIPT_KIT_NONINTERACTIVE=1 bun run scripts/test-runner.ts --parallel --json
bun test ./tests/sdk/runner-safety.test.ts
./scripts/agentic/agent-cargo.sh test --lib scripts::validation
./scripts/agentic/agent-cargo.sh test --lib mcp_resources
./scripts/agentic/agent-cargo.sh test --lib mcp_resources::tests::sdk_reference_marks_
./scripts/agentic/agent-cargo.sh test --lib mcp_resources::tests::sdk_capability_catalog_
```

- The two `kit-init/**/menu-syntax.test.ts` files are explicitly documented as
  compile-only fixtures. `bun test` executes zero tests and must never be
  accepted as proof. The `tsc` command above is `COMPILE-ONLY`; the filtered
  SDK runners are `SDK-BEHAVIOR` and currently execute two hotkey cases,
  five editor/mini/micro cases, and eight fields date/time/search cases.
  They use SDK auto-submit, not a live app, so native capture, focus, sizing,
  dismissal, and date-picker rendering still require direct runtime receipts.
- The strict complete worktree suite currently reports **215 passed, 0 failed,
  and 0 skipped across 40 files**. The previously skipped file search,
  clipboard-history, menu-bar, and unavailable-network cases now have
  explicit safe synthetic behavior coverage. Preserve exact stale-ID
  negatives; never remove/skip an assertion, declare SDK auto-submit to be
  native proof, or relax native generation/unknown-ID rejection to keep the
  gate green.
- The expanded `mcp_resources` Rust catalog is independently verified by the
  passing full application compile and **nine nonzero focused Rust behavior
  cases**; TypeScript and Bun success alone would still be insufficient.
  Preserve supported/unsupported projection, exact once-per-capability
  catalog coverage, native permission/platform boundaries, and explicit
  cache invalidation in those owning Rust tests.
- SDK docs, generated types, capability catalog, examples, and runtime behavior
  agree for every advertised API.
- Unsupported capabilities fail before execution with a useful alternative.
- Duplicate bindings and malformed author metadata never silently remove a
  script from the launcher.
- Every shipped starter/template script passes the same compatibility checks
  as third-party scripts.

---

## 09. Direct behavioral proof across the real surface matrix

### Outcome

A product claim is backed by direct target-scoped runtime evidence. Agents can
inspect, drive, measure, and compare every supported user experience without
mistaking source-text assertions or inherited host coverage for proof.

### Primary ownership

- `scripts/devtools/driver.ts`, `surfaces.ts`, `coverage.ts`, and `schema.ts`
- `scripts/devtools/elements.ts`, `layout.ts`, `scroll.ts`, `focus.ts`,
  `keyboard.ts`, `actions.ts`, `surface.ts`, `act.ts`, and `compare.ts`
- `scripts/devtools/lib/receipt-schema.ts` and `target-identity.ts`
- `src/app_layout/collect_elements.rs` and `build_layout_info.rs`
- `src/stdin_commands/` and app-independent protocol definitions
- `scripts/agentic/filterable-surface-matrix.ts`
- `scripts/agentic/attached-popup-surface-matrix.ts`
- `tests/source_audit_inventory.py`
- `scripts/devtools/consistency-catalog.md` for the now-committed portable
  75-task catalog available to clean checkouts
- `.notes/CONSISTENCY-PROGRESS.md` and `scripts/devtools/consistency.ts` for
  the existing progress ledger/auditor

### Step-by-step implementation

1. Validate the existing 75-task consistency catalog, identify the relevant
   `PF-*` and `GOV-*` owners, and inspect their current identity-matched
   receipts before adding a new proof task or ledger entry. Verify the
   approved portable catalog appears in `git ls-files` before calling any
   auditor operation clean-checkout-safe. Every subcommand, including
   `verify-task`, now uses the same committed portable catalog by default;
   direct runtime receipts, not source tracking, remain the proof blocker.
2. Regenerate the surface census and record exact counts of contract kinds,
   mappings, variants, coverage profiles, and `Direct`/`Derived`/`Unsupported`
   classifications. A static `Direct` relation is not a runtime pass; record
   its profile status and the exact current runtime receipt separately.
3. Preserve the real launcher, Dictation History, Clipboard History, Browser
   History, Notes Browse, File Search, Day Page, Current App Commands, Agent
   Chat History, and Webcam direct selectors plus their actual production
   renderer/semantic/layout `sourceFiles`; keep painted Dictation rows,
   semantic projection, and tracked scrolling on the same actual row ID; and
   retain behavior-oriented registry validation that refuses nonexistent,
   duplicate, absolute, or escaping owner paths.
4. Enumerate the **38 Derived mappings** that currently lack a direct profile
   binding after promoting genuine production surface owners; separately
   enumerate all **54 mappings without fresh direct runtime proof**, including
   the 16 static Direct bindings. Rank both by
   user frequency, failure severity, state mutation, privacy exposure, and
   likelihood of interaction drift.
5. Define the minimum direct receipt for every surface: exact target identity,
   host, semantic tree, focused node, selected node, active actions, visible
   state, and safe transition/cleanup outcome.
6. Extend `getElements` and layout projection from the same state the renderer
   uses; do not invent test-only semantics or claim invisible controls exist.
7. Add missing main-launcher text fit, scroll geometry, overlap pairs, and
   focus-ring bounds; report absence as an explicit typed blocker.
8. Add Dictation History fixture store identity, row generation, preview
   generation, redacted transcript fingerprint, audio-path privacy proof, and
   selection/scroll anchor measurements. Ensure fixture generation itself
   receives a real durable saved-entry result; a rejected write, malformed
   private index, missing provider payload, or stale invalid root snapshot is
   an explicit unavailable/failure state, never an invented passing identity.
9. Extend the existing layout joins, clipping checks, overlap analyses,
   semantic projection grading, receipt privacy, and AX primitives where a
   concrete runtime owner is missing. Add text truncation, clipping, overlap,
   contrast, accessible role/name,
   semantic-to-AX parity, and tab-order inspection where runtime access is
   actually available.
10. Reuse the existing nine consistency fixture families and `PF-010` catalog;
    extend that deterministic fixture catalog to span scripts, scriptlets,
   built-ins, custom actions, prompts, notes, flows, AI errors, permissions,
   large catalogs, missing dependencies, and invalid author metadata.
11. Run generated scenario combinations through the persistent `Driver`
    instead of paying subprocess/session startup for every single key.
12. Capture red and green receipts with the same path, surface, target, binary,
    scenario, semantic IDs, observation points, and metric names.
13. Classify each result as directly proven, blocked by missing primitive,
    unsafe, invalid interference, environment unavailable, or product failure.
14. Replace source-audit assertions with pure behavior tests or direct probes
    one high-churn surface at a time; preserve the rare architectural audits
    that cannot be expressed at a higher enforcement rung.
15. Extend the existing consistency auditor and coverage receipts with a
    scorecard that reports direct mapped-surface coverage,
    supported prompt-family coverage, missing primitives, stale owner paths,
    scenario failures, runtime duration, and privacy violations.
16. Gate high-risk pull requests on a small direct behavior matrix and require
    the full supported-surface matrix before release.

### Verification and exit gate

```bash
bun scripts/devtools/surfaces.ts | jq '.totals'
bun scripts/devtools/coverage.ts | jq '[.surfaces[] | {id, status, missing: (.missingRuntimePrimitives | length)}]'
bun scripts/devtools/consistency.ts catalog --fixes scripts/devtools/consistency-catalog.md
git ls-files scripts/devtools/consistency-catalog.md
bun test ./scripts/devtools/surface.test.ts ./scripts/devtools/elements.test.ts ./scripts/devtools/layout.test.ts ./scripts/devtools/receipt-schema.test.ts ./scripts/devtools/coverage.test.ts ./scripts/devtools/runtime-coverage.test.ts
python3 -B -m unittest tests/source_audit_inventory_test.py
```

- Surface/coverage generation and consistency `catalog` are
  `STATIC-INVENTORY`; the Bun/Python suites execute real unit behavior. Fresh
  target-scoped proof additionally requires the appropriate task/family
  verification and a current matching runtime receipt.
- The `git ls-files` command must return the exact candidate catalog path; its
  current empty output is a release-blocking clean-checkout failure.
- Every supported high-frequency surface has direct target-scoped evidence.
- No coverage registry points at missing owners or reports a derived mapping
  as directly proven.
- Privacy-sensitive data appears only as redacted fingerprints in receipts.
- The source-audit inventory declines without weakening behavior coverage.

---

## 10. Release gates that prove the actual packaged product

### Outcome

A signed/notarized build ships only after executed tests, packaged-app user
journeys, dependency readiness, migrations, permissions, AI recovery, privacy,
and ratified performance gates all pass against the release candidate.

### Primary ownership

- `.github/workflows/ci.yml`
- `.github/workflows/perf-gates.yml`
- `.github/workflows/release.yml`
- `scripts/verify.sh`
- `scripts/release-evidence.ts` and `scripts/release-evidence.test.ts`
- `scripts/verify-macos-bundle.sh`
- `scripts/prepare-pi-sidecar.sh`
- `scripts/install-pi-sidecar-into-bundle.sh`
- `src/setup/mod.rs`, `src/permissions_wizard.rs`, and `src/updates.rs`
- DevTools direct proof and performance receipts from Programs 05 and 09

### Step-by-step implementation

1. Preserve the independently verified **zero-warning strict Clippy gate**
   after eliminating its historical 280-error failure. Rerun
   `./scripts/agentic/agent-cargo.sh clippy --locked --lib --no-deps --
   -D warnings` against each candidate source without broad lint suppression,
   weakening `unwrap`/`expect` policy, or touching locked glass-motion
   calibration. A green compile alone remains insufficient.
2. Preserve the current candidate repair of the previously failing
   `validate-sdk-tests` release dependency: rerun every stale-ID mutation,
   preserve native registry/generation rejection, and require a complete
   noninteractive SDK receipt with its real passed/failed/skipped counts.
3. Preserve the current candidate replacement of the release-only Rust
   `test-compile` gate with executed app and domain tests. Add a deliberate
   integration-test lane: `cargo test --lib` alone excludes integration
   targets. Keep compile-only as an optional fast preflight, never the release
   acceptance criterion; require parsed nonzero results for every lane.
4. Add a focused required pull-request behavior lane covering domain tests,
   interaction planner tests, AI reliability tests, command descriptor tests,
   and SDK capability checks, while retaining cheap formatting/compile feedback.
   If the existing consistency auditor is required, preserve its committed
   approved portable catalog and prove every subcommand uses that tracked
   source. A local-only catalog or progress shortcut is not a clean-checkout
   CI gate.
5. Extend the existing release manifest produced in
   `.github/workflows/release.yml`; it already records version, tag, release
   ZIP name, platform, SHA-256, and size. Add exact source SHA, binary SHA,
   bundle identity, sidecar identity, SDK schema version, generated contract
   version, and gate-receipt identities rather than creating a parallel
   manifest. Manifest **v3** now additionally binds the exact
   `Contents/Info.plist` identity and one canonical, length-framed SHA-256
   tree covering **every app file and framework symlink**, including path,
   entry kind, permissions, exact size, content/link-target hash, and the
   required `Contents/_CodeSignature/CodeResources` envelope. Creation must
   compare the entire verified `.app` tree to the actual final ZIP; final
   Ubuntu verification must independently recompute and compare that whole
   archive tree without `--app`. Checking only four binaries/manifests or the
   outer ZIP filename/hash/size is insufficient. Preserve bounded
   central-directory parsing, real `ditto`/deflate/data-descriptor/empty-file
   compatibility, legitimate framework links, exact app root, Unicode/case
   ancestor-alias defenses, and duplicate/traversal/unsafe-symlink rejection.
6. Build a stable artifact and run a sandboxed first-launch journey against the
   actual packaged `.app`, not only a development binary.
7. Prove first install creates its intended directory structure, discovers the
   bundled SDK, indexes starter scripts, and reaches the main ready-to-type
   state without relying on the developer's existing home directory.
8. Exercise Accessibility, Screen Recording, microphone, and other permission
   states through safe fixtures/passive checks. Missing permissions must show
   precise recoverable guidance without pretending they are granted.
9. Verify Bun/runtime discovery, bundled Pi sidecar, executable permissions,
   profile readiness, and truthful failure behavior when an optional external
   dependency is absent.
10. Run packaged-app smoke scenarios: open launcher, type/filter, execute one
   script, open Actions, navigate/cancel, open Notes, submit mock AI, recover a
   mock AI failure, and close without leaked processes.
11. Add upgrade fixtures for legacy configuration, existing scripts, duplicate
   bindings, previous SDK versions, persisted conversations, notes, and Brain
   storage. Prove user data is preserved.
12. Run direct behavior matrices and ratified latency gates against the same
    binary and fixture generation referenced by the release manifest.
13. Add privacy gates for diagnostic vault redaction, AI provider secrets,
    clipboard/history exposure, transcript receipts, screen-capture opt-in,
    and permission-state reporting.
14. Preserve existing signing, notarization, stapling, sidecar codesigning,
    bundle verification, and Gatekeeper checks; append runtime readiness rather
    than replacing distribution security.
15. Make the publish job depend on every gate and explicitly reject missing,
    stale, wrong-binary, skipped, compile-only, derived-only, or blocked
    receipts.
16. Upload one readable release scorecard listing all journeys, direct proof,
    latency metrics, migration outcomes, dependency checks, security checks,
    source SHA, and bundle SHA.
17. Inject one known failing behavior and one missing sidecar in controlled
    verification to prove the release path fails closed before publication.

### Verification and exit gate

```bash
./scripts/agentic/agent-cargo.sh clippy --locked --lib --no-deps -- -D warnings
./scripts/agentic/agent-cargo.sh test --lib
./scripts/agentic/agent-cargo.sh test -p sk-clipboard -p sk-protocol -p sk-storage
bun run scripts/check-sdk-types.ts
SCRIPT_KIT_NONINTERACTIVE=1 bun run scripts/test-runner.ts --parallel --json
bash scripts/verify-macos-bundle.sh '<packaged-app-path>'
```

- The first command is the exact zero-warning release lint gate; the
  second is a full app-crate test run and must not start until the shared
  Cargo pool, expected build size, and free-disk floor are checked. The last
  command is `PACKAGED-APP` only and requires a real bundle containing both
  executable binaries; the quoted placeholder is not runnable as written.
- The required strict Clippy command currently passes with zero warnings and
  errors; its historical 280-error failure is not a current blocker. Preserve
  the actual source-bound gate rather than substituting fixtures or compile.
- The full SDK command is an existing required release job; the current
  uncommitted candidate passes **215 cases, with zero failures and zero skips**.
  Shipping still requires the same exact green result from the actual
  committed release source without reduced safety or fabricated native proof.
- Release Rust tests actually execute; `--no-run` cannot satisfy readiness.
- Library-only Rust execution is not integration coverage; binary-only source
  tests behind `test = false` cannot count as executed acceptance tests.
- The packaged candidate—not a sibling debug binary—passes the representative
  user-journey suite.
- The exact published bundle has valid signing/notarization/Gatekeeper proof,
  a working sidecar, clean first-run behavior, direct surface receipts, and
  ratified performance receipts.
- Final publication independently verifies that the exact outer archive
  contains the already-attested executable, sidecar, SDK, Info.plist,
  CodeResources signing envelope, and complete signed file/symlink/mode tree;
  plain text, replacement bundles, added/deleted migration or resource
  files, modified permissions or framework-link targets,
  Unicode/case-aliased ancestor symlinks, traversal, and mismatched inner
  bytes fail even when the outer ZIP hash and size are rewritten.
- Publishing fails closed on missing, stale, blocked, or wrong-artifact proof.

---

## First 12 implementation slices

These slices are intentionally narrow enough to verify independently while
moving the broader architecture forward.

1. Preserve the grouped source checkpoint and retain the passing full library **and** main
   binary compile. Preserve the zero-warning strict Clippy release gate
   through the prescribed Cargo wrapper; resolve build-environment blockers
   without launching the application.
2. Commit the approved portable 75-task catalog, require
   `git ls-files scripts/devtools/consistency-catalog.md` to return its path,
   and preserve every auditor subcommand's existing portable-catalog default.
3. Add safe Quick AI `--help`/`--describe-contract` handling plus an explicit
   live-provider opt-in; prove all transport, SDK, benchmark, and release
   entry points reject takeover, focus/input, capture, microphone/camera, and
   live AI in noninteractive mode.
4. Preserve the current 215-case, zero-skip SDK fix and owner-registry repair;
   rerun stale
   window-ID negatives, supported mini/micro/hotkey/fields cases, runner
   failure controls, corrected owners, and truthful zero-runtime coverage.
5. Extract/re-export binary-only root-search and clean-quick-question contracts
   into owners whose focused library/integration tests actually execute a
   nonzero case count.
6. Reconcile the existing six deeplink categories against all 24 descriptor
   families. Project one built-in and one script through real launcher,
   action, footer, semantic, and context consumers without changing persisted
   identity, visible labels, shortcuts, or selection behavior.
7. Route physical capture, physical bubble, simulated-key mirror, popup, and
   portal dismissal through the existing orchestrator/shared planner; verify
   pure one-layer/focus parity before any separately approved visible proof.
8. Correct the shared execution contract to preserve nonterminal
   `Cancelling`, exact process-group ownership, bounded escalation, and
   verified dead-group cleanup before terminal `Cancelled`.
9. Generalize provider-generation/query rejection beyond protected sources;
   prove late browser/file/clipboard results cannot steal selection, and keep
   exact match evidence/highlights without claiming hidden-state checks prove
   painted latency.
10. Wire capability catalog/topology diagnostics into actual script validation,
    launcher readiness, author docs, and generated examples; classify the
    preserve the current zero-skip SDK result and add native proof only when safely
    isolated.
11. Extend existing transaction traces, typed AI context/recovery, and the
    direct surface scorecard using identity-matched redacted hidden receipts;
    require actual clean quick question, explicit row handoff, Notes handoff,
    and recovery-owner behavior instead of static Direct grades.
12. Add executed library **and integration** release lanes, first-install /
    permissions / migration / mock-AI packaged journeys, exact signed app/Pi /
    source / SDK / schema evidence, truthful hardware-ratified paint gates,
    and a fail-closed publish scorecard for the exact candidate.

## Product-wide definition of done

The product is ready to ship when all of the following are true:

- Every supported command has one stable host-owned descriptor and readiness
  explanation.
- Every supported surface follows the same declared keyboard, focus, actions,
  loading, empty-state, recovery, and return-route contracts.
- Search results are fast, explainable, generation-safe, and selection-stable.
- Every command execution has truthful progress, cancellation, cleanup, and a
  typed terminal receipt.
- AI entry, context, model selection, streaming, approvals, cancellation, and
  recovery are explicit and consistent across hosts.
- Every advertised SDK capability is supported, accurately experimental, or
  blocked with an actionable alternative before it harms the user experience.
- High-frequency surfaces have direct semantic, layout, keyboard, focus,
  accessibility, privacy, and lifecycle proof.
- Latency budgets measure their named observation point, are ratified by the
  product owner, and fail on real regressions.
- Release gates execute tests, operate the actual packaged candidate, preserve
  user data, validate distribution security, and refuse stale or missing
  evidence.
- The exact required strict Clippy gate passes with no blanket suppression;
  compilation warnings are not silently converted into a releasable build.
- Default tests and verification never take over the operator's computer;
  any approved visible/native-input/provider-backed capture is an isolated,
  explicitly consented exception rather than a hidden side effect.
- A user can reasonably expect any command or custom script to feel like part
  of the same instant, predictable, trustworthy, pleasant product.
