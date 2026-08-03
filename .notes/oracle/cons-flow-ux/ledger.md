# Workflow Safety Execution Ledger

- premise: `.notes/oracle/cons-flow-ux/premise.md`
- assigned coverage: SAFE-001..SAFE-004, WF-001..WF-024 (28 tasks)
- consult slug: `cons-flow-ux`
- consult count: 1 / 1
- plan status: complete (`plan.md`, 28/28 task IDs covered)
- protocol/profile: v2 / `profile-b`
- execution status: C01–C02 complete (`SAFE-001`, `WF-001`, `WF-003`); starting C03 (`WF-002`, `WF-008`)
- audit verdict: C01–C02 local audit PASS; whole-premise audit pending

## Receipts

- Bundle: 34 files, 887,556 bytes, non-empty, below 1 MiB.
- Prompt: prepared; exact size validated before submission.

## Step ledger

### C01 — SAFE-001 canonical sanitized preparation

- **Status:** Complete; ready for atomic commit `fix(ai-context): unify sanitized message preparation [SAFE-001]`.
- **Decision branch:** The protocol had no typed context-unavailable fact. Added `InputFailure::ContextUnavailable` and `AiFailureCode::ContextUnavailable`, with typed classification, safe presentation, immediate/manual-retry planning, exhaustive model coverage, and raw detail confined to the diagnostic vault.
- **Implementation:** Routed all six `AiContextPart` variants through `resolve_context_part_sanitized`; added recursive JSON `base64Data` stripping regardless of MIME, character-safe truncation, XML attribute escaping, explicit primary/supplemental roles, private non-serializable `PreparedUserMessage`, and privacy-safe v2 receipts. Preserved unreadable-existing-file metadata fallback and Flow-specific skill wording/wrapping. Migrated Agent Chat, prompt handoff, preflight audit, storage, protocol input, and runtime reconstruction callers.
- **Compatibility:** Legacy receipt loading ignores v1 content-bearing fields. Audit schema v3 persists character counts rather than raw authored/final content, and startup migration clears legacy v1/v2 audit content columns.
- **Self-audit correction:** The first Agent Chat migration crossed the compatibility string boundary and reconstructed a failure from safe prose. Added the crate-private typed resolver and migrated Agent Chat to carry the original `AppFailureRecord`. Also restored Flow wording and `<flow>` tags, escaped focused-label metadata, extended exhaustive protocol/presentation matrices, and asserted legacy database columns directly before final proof.
- **Focused receipts:** `ai::message_parts` 40/40 PASS; `ai::reliability` 28/28 PASS; `ai::preflight_audit` 4/4 PASS; storage round-trip 1/1 PASS; `ai::agent_prompt_handoff` 21/21 PASS; redacted probe wire contract 1/1 PASS; `sk-protocol` 24/24 PASS; library check PASS.
- **Build:** `SCRIPT_KIT_AGENT_ARTIFACT_NAME=cons-flow-safe001 ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui` PASS. Stable artifact: `target-agent/artifacts/cons-flow-safe001/script-kit-gpui`; SHA-256 `c11299fd6d0a1e2c887eb028ca52b7635046ebc683ed765c07ffa1f3e1765745`.
- **Runtime receipt:** `.artifacts/consistency/cons-flow-ux/safe001-canonical-v2/SAFE-001/receipt.json` → `RUNTIME-CONFIRMED`. Oversized nested JSON: Ready, binary stripped, nonbinary preserved, bounded 335-character payload. Missing primary: Blocked and `acceptedSend:false`. Missing supplemental with authored text: Partial and `acceptedSend:true`. Safe user copy exact; raw content/source canaries absent from serialized receipts and visible Agent Chat semantics.
- **Cleanup:** Driver `try/finally` receipt reports `processExited:true`, `streamsDrained:true`, `logWriterClosed:true`, exact artifact-path `ownedProcessCount:0`, and final owned-process count zero. Clipboard untouched; no signal used.
- **Governance:** Source-audit inventory remains 2,818 reader sites with no new guarded reader. Hardcoded-visual inventory has no additions. `git diff --check` passes. Protected glass diff is empty. Test-only inspection route rejects fixtures outside `SCRIPT_KIT_TEST_STATUS=1`.
- **Adversarial audit:** PASS. Every variant reaches the canonical sanitizer; no production model-bound caller bypass was found; Agent Chat preserves the original typed failure; receipts and `Debug` output cannot serialize model content; legacy content is discarded/redacted; Flow behavior is preserved; failed primary content cannot send; and runtime-owned processes are gone.

### C02 — WF-001 and WF-003 accepted-send lifecycle

- **Status:** Complete; ready for atomic commit `feat(ai-context): model staged provenance and retry lifetime [WF-001 WF-003]`.
- **Implementation:** Added `StagedContextItem` with stable ID, generation, canonical identity, explicit provenance/role/state/lifetime/removability, in-place priority upgrades, and redacted `Debug`. Migrated active, queued, draft, portal, host-handoff, export, and protocol carriers to typed items. Schema v7 exposes only safe lifecycle facts and separates immutable receipts from pending context.
- **Ingress decision:** Every production entry now names provenance. Deliberate inline/spine/slash selection is `UserMention`; attachment/history portals are `AttachmentPortal`; explicit cross-surface transfer is `HostHandoff`; launcher focus is `ImplicitFocused`; ambient capture is `DeferredAmbient`. Primary wins; equal-role priority follows the Oracle plan.
- **Accepted-send decision:** Background preparation leaves composer/transcript/context intact. Primary failure, capability refusal, adapter refusal, and reliability-start refusal restore exact snapshots. Accepted runtime start alone commits resolved items to immutable `ThreadReceipt` records. Transition-owned item snapshots cover queued turns even when their chips are no longer in the active composer collection.
- **Retry decision:** `last_prepared_turn` stores the accepted blocks/attachments/display text and a run-scoped fingerprint. Retry and recovered-resume submit those stored blocks directly; current files, focused target, ambient capture, and pending chips are not re-resolved.
- **Portal decision:** Portal sessions snapshot typed pending items plus consumption state; Escape restores draft text, cursor/selection, picker state, exact IDs/generations, and focus.
- **Focused receipts:** `ai::staged_context` 4/4 PASS; thread 92/92 PASS; full Agent Chat 630/630 PASS; Agent Chat state 51/51 PASS; wait/probe protocol 46/46 PASS; message parts 40/40 PASS; Quick Question suppression 1/1 PASS; embedded draft fixture 1/1 PASS; library check PASS.
- **Build:** `SCRIPT_KIT_AGENT_ARTIFACT_NAME=cons-flow-c02 ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui` PASS. Stable artifact SHA-256 `6c5152045e079e239d46eba4cfa92088fb3631e301fef94017fa9bb0c33c8486`.
- **Runtime receipts:** `.artifacts/consistency/cons-flow-ux/c02-context-lifecycle-v1/{WF-001,WF-003}/receipt.json` → `RUNTIME-CONFIRMED`. Dedupe: one upgraded pending item. Partial accepted: one immutable primary receipt plus one failed supplemental and non-empty immutable payload fingerprint. Fresh thread: zero pending/receipt/payload. Portal Escape: exact draft/cursor/selection/picker/item ID/generation/focus restored.
- **Cleanup:** `processExited:true`, `streamsDrained:true`, `logWriterClosed:true`, exact artifact-path `ownedProcessCount:0`, clipboard untouched, no signal.
- **Escalations:** The first current-tree bin test exposed a stale `pending_context_parts` draft fixture in `src/app_impl/actions_dialog.rs`; migrated it to a typed staged item and proved the owning test. The first runtime probe failed closed because `openAgentChatKitchenSinkFixture` was deleted in the current tree; switched only the opening command to the supported `openAiWithMockData` fixture and reran the same assertions. An adversarial pass found capability refusal could return before restoring `Resolving` state and queued transitions depended on the active composer collection; both were corrected and locked by transition-snapshot tests.
- **Governance:** Source-audit inventory remains 2,818 sites with no guarded additions. Hardcoded-visual inventory has no additions. Receipt canary scan passes. `git diff --check` passes. Protected glass owner diff is empty. Exact process inventory is zero.
- **Adversarial audit:** PASS. No production pending carrier stores a raw `Vec<AiContextPart>`; preparation-only vectors are ephemeral resolver inputs. All ingress provenance is explicit. Failed primary cannot reach adapter start. Every pre-accept exit restores state. Accepted start alone commits receipts. Failed supplementals remain pending. Retry uses one immutable payload. Quick Question stays empty. Portal cancellation restores the full snapshot. Saved-message reload clears pending, receipt, and retry state; the persisted history schema has no legacy pending-context field to resurrect. Protocol/runtime receipts contain no raw path, URI, context body, or failure detail.
