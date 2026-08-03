# Workflow Safety Execution Ledger

- premise: `.notes/oracle/cons-flow-ux/premise.md`
- assigned coverage: SAFE-001..SAFE-004, WF-001..WF-024 (28 tasks)
- consult slug: `cons-flow-ux`
- consult count: 1 / 1
- plan status: complete (`plan.md`, 28/28 task IDs covered)
- protocol/profile: v2 / `profile-b`
- execution status: C01 complete (`SAFE-001`); starting C02 (`WF-001`, `WF-003`)
- audit verdict: C01 local audit PASS; whole-premise audit pending

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

- **Status:** Starting after the C01 atomic commit.
