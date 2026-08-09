# Goal: Audit the AI-authored consistency branch and publish an evidence-first review

Goal: Audit every change between `main` and `consistency/default-recommendations`, distinguish verified improvements from plausible regressions and unproven claims, capture representative real-app screenshots, and publish a comprehensive Paper Fold `/here-now-article` whose largest section is a prioritized, testable next-steps program.

Scope:
- In scope: all commits and changed paths in `main...HEAD`; production Rust and domain crates; tests; devtools/probes; design contracts; generated artifacts; documentation and repository instructions; user-visible launcher, Actions, prompts, built-ins, Agent Chat, Flow, Notes, Dictation, windows, chrome, and reliability behavior.
- In scope: static diff review, history/receipt integrity review, focused repository-native tests, real runtime inspection, representative screenshots, risk classification, and publication to a `here.now` URL.
- Out of scope: changing production behavior, weakening tests, retuning locked glass motion, committing, pushing, merging, or deploying the application. Audit/article files and screenshots may be created; publication of the requested article is authorized.

Baseline and target:
- Baseline: 50 commits and 481 changed paths currently separate the branch from `main`; 411 changed paths are code/test/tooling-oriented. The work was predominantly AI-authored and is not trusted merely because prior receipts say it passed.
- Target: account for 100% of changed paths through six subsystem packages; identify concrete improvements, breakage candidates, proof gaps, and provenance concerns; verify the highest-risk claims with focused tests or runtime evidence; publish an article containing branch metrics, subsystem findings, screenshots with captions explaining what to inspect, a confidence model, and a large ordered next-steps section with owners, commands, stop conditions, and expected evidence.

Suggested starting points:
- `git diff --name-status main...HEAD`, `git diff --numstat main...HEAD`, and `git log main..HEAD`.
- `GLOSSARY.md`, `AGENTS.md`, shared components under `src/components/**`, relevant surface owners under `src/**`, `rules/AI_RELIABILITY.md`, and changed tests/probes under `tests/**` and `scripts/**`.
- Existing consistency ledgers and receipts under `.artifacts/consistency/**`, `.notes/oracle/**`, `design/consistency/**`, and `.goals/finish-consistency-program-75.md`—treated as claims to corroborate, never as proof by themselves.
- `script-kit-devtools` and project runtime probes for real UI evidence; `agent-cargo.sh` for Cargo checks.

Measurement and verification:
- Produce a machine-readable changed-path inventory with exactly one package assignment per path and no unmapped paths.
- For every material claim in the article, link it to a diff, commit, focused command result, runtime receipt, or screenshot; label unverified inference explicitly.
- Run focused tests selected from changed behavior, plus syntax/build checks practical within the campaign. Cargo commands must use `./scripts/agentic/agent-cargo.sh` and explicit timeouts.
- Capture representative real-app screenshots at intended viewports for changed user-visible surfaces; captions must state what improved and what could still be wrong. Do not use screenshots as proof of keyboard routing, lifecycle, or temporal behavior.
- Validate the final article locally, publish through `here-now-article`/`here-now`, and fetch the published URL to verify the deployed content.

Environment:
- Current macOS repository and branch; current working tree must remain ownership-safe.
- Real Script Kit runtime where practical, with test status/probe fixtures isolated from user data.
- Screenshots must come from the branch build/runtime, not mockups presented as runtime evidence. Design mockups may appear only when clearly labeled.

Progress tracking:
- Persist the immutable premise and six-package coverage under `.notes/oracle/branch-trust-audit/`.
- Preserve six Oracle receipts, six worker reports, focused verification receipts, screenshot paths, and one integrated ledger.
- No production commit, push, merge, or app deployment is authorized.

Completion requirements:
- Show final measurements against the baseline and target.
- Explain what improved, what might have broken, what remains unproven, and why confidence differs by subsystem.
- Dedicate the largest substantive section to prioritized next steps, with each item including rationale, owning files/surface, proof command or runtime check, and stop condition.
- Remove failed temporary artifacts that are not useful evidence; preserve bounded receipts and screenshots.
- Publish the article and verify its live URL.
- Summarize residual risks and all lifecycle actions deliberately not performed.

## Open decisions

None. Use conservative evidence labels and prioritize correctness/reliability over defending prior AI work.
