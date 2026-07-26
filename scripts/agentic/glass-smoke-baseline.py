#!/usr/bin/env python3
"""Ingest a preserved glass-entry ABBA session into an economics baseline.

Oracle plan glass-smoke-harness-max-info, work package 1. Reads a preserved
smoke session directory (e.g. .artifacts/glass-entry-abba/smoke3-2026-07-25)
WITHOUT modifying it and emits the information-economics baseline the v2
study harness must beat:

  python3 scripts/agentic/glass-smoke-baseline.py \
    --source .artifacts/glass-entry-abba/smoke3-2026-07-25 \
    --out /tmp/smoke3-baseline-economics.json

Counting rules (deliberately conservative, per the plan):
- "captured" metric-family observations are metric results that exist
  anywhere in the lifecycle artifacts: each scenario whose filmstrip carries
  a metrics result (glass-lifecycle-metrics.py output), plus the promoted
  main-entry color metric when entry-metrics.json exists.
- "promoted" metric-family observations are families actually surfaced into
  runs.jsonl rows: displayed-color, capsule-stage relation drift, and the
  alpha policy — 3 per run when present, 0 when the metric never ran.
This distinction avoids falsely claiming the existing lifecycle scenarios
were wholly unmeasured: they ARE measured, then discarded before promotion.

Timing derives from lifecycle receipt startedAt/finishedAt timestamps, never
from filesystem modification times, when receipts exist.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime
from pathlib import Path

PROMOTED_FAMILIES_PER_GRADED_RUN = (
    "main-entry.displayed-color",
    "main-entry.stage-relation",
    "main-entry.alpha-policy",
)


def _parse_iso(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def build_baseline(source: Path) -> dict:
    runs_path = source / "runs.jsonl"
    rows = [
        json.loads(line)
        for line in runs_path.read_text().splitlines()
        if line.strip()
    ]

    session_hashes: dict[str, str] = {}
    for name in ("runs.jsonl", "session.json", "fixture.json"):
        path = source / name
        if path.is_file():
            session_hashes[name] = _sha256(path)

    starts: list[datetime] = []
    finishes: list[datetime] = []
    summed_lifecycle_ms = 0.0
    captured = 0
    promoted = 0
    run_dispositions: dict[str, str | None] = {}

    for row in rows:
        run_dir = source / row["run"]
        receipt_path = run_dir / "lifecycle" / "receipt.json"
        if receipt_path.is_file():
            session_hashes[f"{row['run']}/lifecycle/receipt.json"] = _sha256(
                receipt_path
            )
            receipt = json.loads(receipt_path.read_text())
            run_dispositions[row["run"]] = receipt.get("disposition")
            started = receipt.get("startedAt")
            finished = receipt.get("finishedAt")
            if started and finished:
                start_at = _parse_iso(started)
                finish_at = _parse_iso(finished)
                starts.append(start_at)
                finishes.append(finish_at)
                summed_lifecycle_ms += (
                    finish_at - start_at
                ).total_seconds() * 1000.0
            for scenario in receipt.get("scenarios") or []:
                filmstrip = scenario.get("filmstrip") or {}
                if filmstrip.get("metrics") is not None or filmstrip.get(
                    "metricsPath"
                ):
                    captured += 1
        else:
            run_dispositions[row["run"]] = None

        entry_metrics = run_dir / "entry-metrics.json"
        if entry_metrics.is_file():
            captured += 1
        if row.get("runMaximumDisplayedEntryDeltaE00") is not None:
            promoted += len(PROMOTED_FAMILIES_PER_GRADED_RUN)

    session_wall_ms = 0.0
    if starts and finishes:
        session_wall_ms = (
            max(finishes) - min(starts)
        ).total_seconds() * 1000.0

    all_ineligible = bool(rows) and all(
        row.get("eligible") is False for row in rows
    )
    qualification = (
        "NON_QUALIFYING_LOAD" if all_ineligible else "MIXED_OR_QUALIFYING"
    )

    def per_unit(count: int) -> float | None:
        if count <= 0 or session_wall_ms <= 0:
            return None
        return session_wall_ms / count

    return {
        "schemaVersion": 1,
        "source": str(source),
        "qualification": qualification,
        "sessionWallMs": session_wall_ms,
        "summedLifecycleWallMs": summed_lifecycle_ms,
        "runCount": len(rows),
        "runDispositions": run_dispositions,
        "capturedMetricFamilyObservations": captured,
        "promotedMetricFamilyObservations": promoted,
        "wallMsPerCapturedMetricFamily": per_unit(captured),
        "wallMsPerPromotedMetricFamily": per_unit(promoted),
        "sourceArtifactSha256": session_hashes,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    source = Path(args.source)
    if not (source / "runs.jsonl").is_file():
        raise SystemExit(f"missing runs.jsonl under {source}")

    baseline = build_baseline(source)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(baseline, indent=2, sort_keys=True) + "\n")
    print(json.dumps(baseline, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
