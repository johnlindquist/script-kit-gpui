# Immutable premise

## Verbatim user request

> audit all the changes between this branch and the main branch and write up a comprehensive /here-now-article w/ screenshots explaining what to look for, what might have broken, and what improved. Dedicate a large section to what we should do next. Note: I don't fully trust all these changes, they were all made with ai agents.

## Governing intent

Produce and publish an evidence-first audit of every change in `main...consistency/default-recommendations`, treating AI-authored commits, generated ledgers, and prior receipts as untrusted claims until corroborated; explain verified improvements, plausible regressions, proof gaps, and what readers should inspect; include representative real-app screenshots; and make the largest substantive section a prioritized, executable next-steps program.

## Falsifiable acceptance criteria

1. Every changed path in `git diff --name-only main...HEAD` has exactly one audit package owner, with no unmapped paths.
2. The audit distinguishes Observed/Verified evidence from Inferred/Unverified risk and never treats prior AI self-reports as completion authority.
3. Production code, tests, tooling, generated artifacts, repository instructions, and commit history are all covered.
4. Representative user-visible changes are exercised in the real branch runtime and captured in screenshots; screenshots are captioned with what to inspect and are not overclaimed as temporal/keyboard proof.
5. The highest-risk claims receive focused repository-native verification, using `./scripts/agentic/agent-cargo.sh` for Cargo and explicit timeouts.
6. The article explains improvements, possible breakage, and residual uncertainty by subsystem, with traceable evidence paths or commands.
7. The largest substantive section is an ordered next-steps program. Every item names rationale, owner/surface, proof command or runtime receipt, and a stop condition.
8. The Paper Fold article is published through the requested here-now article flow and the live URL is fetched to verify deployed content.
9. No production behavior is changed, no glass calibration is retuned, and no commit, push, merge, or application deploy occurs. Article publication is authorized.
