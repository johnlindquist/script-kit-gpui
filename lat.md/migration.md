# Documentation Migration

The lattice migration replaces sprawling internal markdown with a smaller set of durable, code-backed knowledge files while preserving a few root entrypoints for humans and tools.

## Keep-in-place documents

`README.md`, `CLAUDE.md`, `AGENTS.md`, and `.impeccable.md` stay at the repo root as explicit entrypoints during this migration. The lattice should absorb broad internal knowledge without breaking those tool-facing contracts.

## First migration wave

The first wave replaces the authored wiki with current-code-backed pages for [[overview]], [[architecture]], [[scripting]], [[protocol]], [[ai-context]], [[acp-chat]], [[design]], [[windowing]], and [[verification]].

## Wiki retirement

The repo contract now points future agents to `lat.md/lat.md` instead of `wiki/index.md`. The authored wiki is no longer the active knowledge base.

## Planning docs removed

The repo no longer keeps the old planning buckets under `plan/`, `plans/`, root `*_PLAN.md`, or the obvious plan-and-recommendation files that were sitting in `docs/`.

## Docs cleanup status

The `docs/` cleanup has advanced past the exploratory material. `docs/archive/`, `docs/research/`, `docs/ux/`, and `docs/audits/` are gone, along with the top-level research, roadmap, recommendation, and improvement docs that were duplicating the same planning layer.

The remaining `docs/` reference material has now been distilled into the lattice or removed. The old rem-sizing, vibrancy-overlay, protocol, context, performance, and planning notes no longer need a separate `docs/` tree.

## Distill instead of mirroring

Historical plans, audits, research notes, and session artifacts should not be copied into `lat.md/` one file at a time. Move only the durable facts and active constraints that still match current code.

## Explicit exclusions

Generated wiki snapshots, wiki ingest logs, vendored upstream docs, test fixtures, and session archives are not first-class lattice content. They can remain in the repo or be archived separately without becoming part of the knowledge graph.

## Inventory snapshot

At the start of this migration, the repo tracked 1,279 committed markdown files, including 173 under `docs/`, 52 plans, 50 audits, 7 authored wiki pages, and 483 task or session artifacts. Those counts explain why the migration must be selective.

## Remaining migration backlog

The next cuts are the remaining top-level markdown docs that still sit outside the explicit keep list and outside tool-owned directories. Keep only the files that still act as live entrypoints or tool contracts; distill everything else into `lat.md/` before removal.

At the current root-doc boundary, the intended survivors are `README.md`, `CLAUDE.md`, `AGENTS.md`, `.impeccable.md`, and the still-useful `GPUI.md` dispatch deep dive.
