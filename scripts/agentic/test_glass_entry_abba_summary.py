#!/usr/bin/env python3
"""Characterization lock for the LEGACY pair-v1 ABBA summary.

Oracle plan glass-smoke-harness-max-info, work package 1. These tests FREEZE
the current semantics of scripts/agentic/glass-entry-abba-summary.py so the
additive v2 study harness cannot silently reinterpret the in-flight alpha
arc's verdict rules. Every assertion is named legacyPairV1... on purpose:
compatibility is the goal, not endorsement — the v2 summary must NOT import
these semantics by default.

Run from the repo root:
  python3 -m unittest scripts.agentic.test_glass_entry_abba_summary
  (or) python3 -m unittest scripts/agentic/test_glass_entry_abba_summary.py
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SUMMARY = REPO / "scripts" / "agentic" / "glass-entry-abba-summary.py"
FIXTURES = REPO / "scripts" / "agentic" / "fixtures" / "glass-entry-abba"


def legacy_row(
    run: str,
    build: str,
    *,
    accepted: bool = True,
    eligible: bool = True,
    pre_load: float = 2.0,
    post_load: float = 2.1,
    max_displayed: float = 1.5,
    relation_drift: float = 1.0,
    below_floor: int = 0,
    zero_alpha: int = 0,
    unmeasurable: int = 0,
    alpha_pass: bool | None = True,
    metric_pass: bool | None = True,
) -> dict:
    return {
        "run": run,
        "build": build,
        "accepted": accepted,
        "eligible": eligible,
        "preLoad1": pre_load,
        "postLoad1": post_load,
        "thermLimited": False,
        "lifecycleExit": 0,
        "metricExit": 0,
        "metricPass": metric_pass,
        "runMaximumDisplayedEntryDeltaE00": max_displayed,
        "maximumCapsuleStageRelationDriftDeltaE00": relation_drift,
        "firstVisibleEntryAlpha": 0.8511,
        "minimumVisibleEntryAlpha": 0.8511,
        "visibleFramesBelowAlphaFloor": below_floor,
        "visibleZeroAlphaFrames": zero_alpha,
        "unmeasurableVisibleFrameCount": unmeasurable,
        "alphaPolicyPass": alpha_pass,
        "errors": [],
    }


def green_session_rows() -> list[dict]:
    """3 warmup pairs (accepted false) + 5 blocks of A,B,B,A, all eligible.

    Baseline A rows are red for the intended reason (alpha policy fail with
    sub-floor visible frames); candidate B rows are fully green.
    """
    rows: list[dict] = []
    for i in range(1, 4):
        rows.append(
            legacy_row(
                f"warmup-A-{i}",
                "A",
                accepted=False,
                alpha_pass=False,
                metric_pass=False,
                below_floor=4,
                max_displayed=46.0,
            )
        )
        rows.append(legacy_row(f"warmup-B-{i}", "B", accepted=False))
    n = 0
    for _ in range(5):
        for tag in ("A", "B", "B", "A"):
            n += 1
            if tag == "A":
                rows.append(
                    legacy_row(
                        f"run-{n:02d}-A",
                        "A",
                        alpha_pass=False,
                        metric_pass=False,
                        below_floor=4,
                        max_displayed=46.0,
                    )
                )
            else:
                rows.append(legacy_row(f"run-{n:02d}-B", "B"))
    return rows


def run_summary(rows: list[dict]) -> tuple[int, dict]:
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp)
        (out / "runs.jsonl").write_text(
            "".join(json.dumps(row) + "\n" for row in rows)
        )
        proc = subprocess.run(
            [sys.executable, str(SUMMARY), str(out)],
            capture_output=True,
            text=True,
            check=False,
        )
        summary = json.loads((out / "summary.json").read_text())
        return proc.returncode, summary


class LegacyPairV1SummaryLock(unittest.TestCase):
    def test_legacyPairV1_green_session_passes(self) -> None:
        code, summary = run_summary(green_session_rows())
        self.assertEqual(code, 0)
        self.assertTrue(summary["pass"])
        self.assertTrue(summary["candidatePass"])
        self.assertTrue(summary["baselineRedForTheRightReason"])
        self.assertEqual(summary["after"]["runs"], 10)
        self.assertEqual(summary["before"]["runs"], 10)

    def test_legacyPairV1_pinned_fixture_reproduces_exactly(self) -> None:
        """The checked-in fixture pair is the frozen contract: running the
        real summary over legacy-runs.jsonl must reproduce legacy-summary.json
        byte-for-byte (both files are generated artifacts of this repo)."""
        rows = [
            json.loads(line)
            for line in (FIXTURES / "legacy-runs.jsonl")
            .read_text()
            .splitlines()
            if line.strip()
        ]
        code, summary = run_summary(rows)
        expected = json.loads((FIXTURES / "legacy-summary.json").read_text())
        self.assertEqual(summary, expected)
        self.assertEqual(code, 0 if expected["pass"] else 1)

    def test_legacyPairV1_requires_ten_candidate_rows(self) -> None:
        """Negative control from the plan: deleting one B row must leave the
        summary red rather than silently lowering the required sample size."""
        rows = green_session_rows()
        b_rows = [r for r in rows if r["build"] == "B" and r["accepted"]]
        rows.remove(b_rows[-1])
        code, summary = run_summary(rows)
        self.assertEqual(code, 1)
        self.assertFalse(summary["candidatePass"])
        self.assertFalse(summary["pass"])
        self.assertEqual(summary["after"]["runs"], 9)

    def test_legacyPairV1_any_load_delta_discard_is_session_red(self) -> None:
        rows = green_session_rows()
        accepted = [r for r in rows if r["accepted"]]
        accepted[0]["preLoad1"] = 1.0
        accepted[1]["preLoad1"] = 2.5  # delta 1.5 > 1.0
        code, summary = run_summary(rows)
        self.assertEqual(code, 1)
        self.assertTrue(summary["discardedPairsByLoadDelta"])
        self.assertFalse(summary["pass"])

    def test_legacyPairV1_every_candidate_hard_field_gates(self) -> None:
        breakers = [
            {"max_displayed": 5.01},
            {"relation_drift": 5.01},
            {"below_floor": 1},
            {"zero_alpha": 1},
            {"unmeasurable": 1},
            {"alpha_pass": False},
            {"alpha_pass": None},
        ]
        for overrides in breakers:
            with self.subTest(overrides=overrides):
                rows = green_session_rows()
                victim = next(
                    r
                    for r in rows
                    if r["build"] == "B" and r["accepted"]
                )
                replacement = legacy_row(
                    victim["run"], "B", **overrides
                )
                rows[rows.index(victim)] = replacement
                code, summary = run_summary(rows)
                self.assertEqual(code, 1)
                self.assertFalse(summary["candidatePass"])

    def test_legacyPairV1_baseline_red_reason_is_broad(self) -> None:
        """The legacy baseline check accepts EITHER alpha-policy failure OR
        any metric failure ("broad fallback"). The v2 summary must NOT adopt
        this breadth (its negative control demands the exact alpha reason);
        this test only pins what legacy does today."""
        rows = green_session_rows()
        for row in rows:
            if row["build"] == "A" and row["accepted"]:
                row["alphaPolicyPass"] = True  # wrong reason...
                row["metricPass"] = False  # ...but broad fallback accepts it
        code, summary = run_summary(rows)
        self.assertEqual(code, 0)
        self.assertTrue(summary["baselineRedForTheRightReason"])

        for row in rows:
            if row["build"] == "A" and row["accepted"]:
                row["metricPass"] = True  # fully green baseline
        code, summary = run_summary(rows)
        self.assertEqual(code, 1)
        self.assertFalse(summary["baselineRedForTheRightReason"])
        self.assertFalse(summary["pass"])

    def test_legacyPairV1_ineligible_rows_are_retained_not_dropped(
        self,
    ) -> None:
        rows = green_session_rows()
        accepted_b = [
            r for r in rows if r["build"] == "B" and r["accepted"]
        ]
        accepted_b[0]["eligible"] = False
        code, summary = run_summary(rows)
        self.assertEqual(code, 1)
        self.assertIn(accepted_b[0]["run"], summary["rejectedRuns"])
        self.assertEqual(summary["after"]["runs"], 9)


if __name__ == "__main__":
    unittest.main()
