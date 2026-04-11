# Verification

This repo prefers the smallest runtime-backed verification that proves a change. UI work should verify the real surface; logic work should stay on the narrowest relevant checks.

## Main menu and footer

`make smoke-main-menu` is the repo's fast launcher and footer smoke target. Use it for main window, footer, built-in menu, and plugin-skill routing changes.

## Targeted checks

Use the smallest check that exercises the touched code:

- `make check` or `cargo check` for compile validation
- `make lint` or `cargo clippy --lib -- -D warnings` for lint-sensitive Rust changes
- `make test` or `cargo nextest run --lib` for library changes
- `make test-all`, `make test-system`, or `make test-slow` only when the touched area justifies them
- `SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui` for direct runtime inspection when you need the app open

For UI changes outside the main launcher/footer path, use the project's agentic runtime verification flow against the real surface instead of guessing from unit tests alone.

## Release gates

`make verify` is the broad validation gate. Use it for release work, CI debugging, or when the change touches shared build/test infrastructure.

`make ship-check` is human-only release validation and should not be run by an AI agent.

## Legacy sources

These docs and commands seeded the verification summary and remain in place while the lattice absorbs the durable rules.

- [CLAUDE.md](../CLAUDE.md)
- [Makefile](../Makefile)
