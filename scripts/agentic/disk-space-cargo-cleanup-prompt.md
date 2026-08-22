# Disk Space Cargo Cleanup

You are an unattended Claude Code maintenance session for Script Kit GPUI.

Goal: restore free disk space on this repo's volume. The trigger threshold is 25 GiB free; the target after cleanup is 35 GiB free or better.

Hard boundaries:
- Work only in the Script Kit GPUI repo.
- Do not edit source files.
- Do not use sudo.
- Do not touch `.git`.
- Do not delete anything outside `target/`, `target-agent/`, or the watcher state/log directories.
- Never terminate `./dev.sh`, `cargo watch`, Cargo, rustc, agent wrappers, or any
  PID listed in `target-agent/.locks/*.lock/pid`.
- Never delete an active build pool, the pinned `agent-debug` warm pool,
  `target-agent/.locks`, `target-agent/shared`, runtime exports, or artifacts.
- Remove only individual stale pools after acquiring their exact ownership lock.

Primary action:
Run the permitted helper command exactly as shown in the runtime section. That
helper performs deterministic, lock-safe cleanup without terminating processes
or deleting active/warm caches, and verifies disk space afterward.

Expected flow:
1. Inspect the runtime facts.
2. Run the helper command.
3. Check `df -h .` and `du -sh target target-agent` afterward.
4. Return a concise summary of what changed and whether free disk is now above threshold.

Do not ask questions. Do not create plans or notes. Do not make commits.
