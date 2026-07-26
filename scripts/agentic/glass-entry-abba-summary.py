#!/usr/bin/env python3
"""Summarize a glass-entry ABBA session into the step-4 headline numbers.

Usage: python3 scripts/agentic/glass-entry-abba-summary.py <out-dir>

Headline: BEFORE = median of the 10 accepted baseline (A) per-run maxima;
AFTER = median of the 10 accepted candidate (B) per-run maxima. The median
alone is not sufficient: every accepted candidate run's WORST frame must pass.
Pair rule: within each A,B,B,A block, adjacent A/B pairs must have 1-minute
load averages within 1.0 of each other, or the pair is discarded.
"""

from __future__ import annotations

import json
import statistics
import sys
from pathlib import Path

MAX_DISPLAYED = 5.0
MAX_RELATION_DRIFT = 5.0


def main() -> int:
    out = Path(sys.argv[1])
    rows = [
        json.loads(line)
        for line in (out / "runs.jsonl").read_text().splitlines()
        if line.strip()
    ]
    accepted = [row for row in rows if row.get("accepted")]
    eligible = [row for row in accepted if row.get("eligible")]

    # Pair load rule: walk accepted runs in order, pair A/B neighbors per
    # block (A-B and B-A within each A,B,B,A block).
    discarded_pairs = []
    ordered = [row for row in accepted]
    for i in range(0, len(ordered) - 1, 2):
        left, right = ordered[i], ordered[i + 1]
        delta = abs(left["preLoad1"] - right["preLoad1"])
        if delta > 1.0:
            discarded_pairs.append((left["run"], right["run"], round(delta, 2)))

    def build_rows(tag: str) -> list[dict]:
        return [row for row in eligible if row["build"] == tag]

    def maxima(rows: list[dict]) -> list[float]:
        return [
            row["runMaximumDisplayedEntryDeltaE00"]
            for row in rows
            if row.get("runMaximumDisplayedEntryDeltaE00") is not None
        ]

    a_rows, b_rows = build_rows("A"), build_rows("B")
    a_max, b_max = maxima(a_rows), maxima(b_rows)

    candidate_pass = (
        len(b_rows) >= 10
        and all(
            row.get("runMaximumDisplayedEntryDeltaE00") is not None
            and row["runMaximumDisplayedEntryDeltaE00"] <= MAX_DISPLAYED
            for row in b_rows
        )
        and all(
            (row.get("maximumCapsuleStageRelationDriftDeltaE00") or 0)
            <= MAX_RELATION_DRIFT
            for row in b_rows
        )
        and all(row.get("visibleFramesBelowAlphaFloor") == 0 for row in b_rows)
        and all(row.get("visibleZeroAlphaFrames") == 0 for row in b_rows)
        and all(
            (row.get("unmeasurableVisibleFrameCount") or 0) == 0 for row in b_rows
        )
        and all(row.get("alphaPolicyPass") is True for row in b_rows)
    )
    # The probe must detect the original defect: baseline runs must FAIL the
    # alpha policy (visible sub-0.85 frames) — a green baseline means the
    # metric is not actually gating displayed color.
    baseline_red = bool(a_rows) and all(
        row.get("alphaPolicyPass") is False or row.get("metricPass") is False
        for row in a_rows
    )

    summary = {
        "acceptedRuns": len(accepted),
        "eligibleRuns": len(eligible),
        "rejectedRuns": [
            row["run"] for row in accepted if not row.get("eligible")
        ],
        "discardedPairsByLoadDelta": discarded_pairs,
        "before": {
            "runs": len(a_rows),
            "medianMaxDisplayedEntryDeltaE00": (
                statistics.median(a_max) if a_max else None
            ),
            "worstMaxDisplayedEntryDeltaE00": max(a_max, default=None),
            "loadRange": [
                min((r["preLoad1"] for r in a_rows), default=None),
                max((r["postLoad1"] for r in a_rows), default=None),
            ],
        },
        "after": {
            "runs": len(b_rows),
            "medianMaxDisplayedEntryDeltaE00": (
                statistics.median(b_max) if b_max else None
            ),
            "worstMaxDisplayedEntryDeltaE00": max(b_max, default=None),
            "worstRelationDrift": max(
                (
                    row.get("maximumCapsuleStageRelationDriftDeltaE00") or 0
                    for row in b_rows
                ),
                default=None,
            ),
            "loadRange": [
                min((r["preLoad1"] for r in b_rows), default=None),
                max((r["postLoad1"] for r in b_rows), default=None),
            ],
        },
        "candidatePass": candidate_pass,
        "baselineRedForTheRightReason": baseline_red,
        "pass": candidate_pass and baseline_red and not discarded_pairs,
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    return 0 if summary["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
