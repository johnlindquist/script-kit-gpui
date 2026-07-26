#!/usr/bin/env python3
"""WP5 (glass-smoke-harness-max-info): batch-grading every lifecycle
scenario from one raw capture must be fail-closed — a missing scenario,
missing frame, corrupted frame byte, or dead grader can never become a
product pass — and deferred grading must reproduce inline grading exactly.

Run from the repo root:
  python3 -m unittest scripts.agentic.test_glass_lifecycle_grade_all
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
GRADER = REPO / "scripts" / "agentic" / "glass-lifecycle-grade-all.py"

_spec = importlib.util.spec_from_file_location("glass_grade_all", GRADER)
grade_all = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(grade_all)

SCENARIOS = [
    "main-exit",
    "main-entry",
    "notes-entry",
    "notes-close-before-settle-reopen",
    "dictation-exit-reopen",
]


def _sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build_synthetic_run(root: Path, *, deferred: bool = False) -> Path:
    """A minimal five-scenario capture with real on-disk frames and metrics.

    In deferred mode each scenario carries a metricsCommand that re-writes
    the same metrics content, so inline and deferred grading must agree.
    """
    run_dir = root / "run"
    run_dir.mkdir(parents=True)
    scenarios = []
    for index, name in enumerate(SCENARIOS):
        scenario_dir = run_dir / name
        scenario_dir.mkdir()
        frames = []
        for frame_index in range(2):
            frame_path = scenario_dir / f"frame-{frame_index:04d}.png"
            frame_path.write_bytes(
                f"synthetic-frame-{name}-{frame_index}".encode()
            )
            frames.append(
                {
                    "sequence": frame_index,
                    "path": str(frame_path),
                    "sha256": _sha(frame_path),
                    "displayTimeNs": 1_000_000_000
                    + frame_index * 8_333_333
                    + index,
                    "windowAlpha": 0.85 + frame_index * 0.05,
                    "maximumStageDeltaE00": 1.0 + frame_index,
                }
            )
        metrics_content = {
            "scenario": name,
            "pass": True,
            "bodyMaskPass": True,
            "bodyPixelTransition": True,
            "geometryStable": True,
            "geometryStateCount": 2,
            "gutterPass": True,
            "alphaProgressionPass": True,
            "frames": frames,
        }
        metrics_path = scenario_dir / "metrics.json"
        metrics_path.write_text(json.dumps(metrics_content))
        filmstrip_receipt = {
            "captureHealthPass": True,
            "refreshRateHz": 120,
            "lateFrameCount": 0,
            "duplicateDisplayTimeCount": 0,
            "maximumConsecutiveDisplayTimeGapNs": 8_333_333,
            "maximumAllowedDisplayTimeGapNs": 9_333_333,
            "screenDamageCadenceWithinOneDisplayPeriod": True,
            "frames": frames,
        }
        receipt_path = scenario_dir / "receipt.json"
        receipt_path.write_text(json.dumps(filmstrip_receipt))
        filmstrip = {
            "exitCode": 0,
            "capturePass": True,
            "pass": not deferred,
            "metricsPath": str(metrics_path),
            "receiptPath": str(receipt_path),
            "receipt": filmstrip_receipt,
            "metrics": None if deferred else metrics_content,
            "metricsCommand": [
                sys.executable,
                "-c",
                (
                    "import json,sys;"
                    "json.dump(json.loads(sys.argv[1]), open(sys.argv[2], 'w'))"
                ),
                json.dumps(metrics_content),
                str(metrics_path),
            ],
        }
        scenarios.append(
            {
                "name": name,
                "structuralPass": True,
                "hiddenReferencePass": True,
                "settledCapturesPass": True,
                "captureBoundsMatch": True,
                "motionEnvelope": {"pass": True},
                "bodyOnlyReveal": {
                    "hiddenBeforeVisible": True,
                    "visibleAfterAnchor": True,
                    "completedFrameCount": 3,
                    "hostClockTiming": {
                        "ordered": True,
                        "visibleWithinBounds": True,
                    },
                },
                "nativeExitValidation": {
                    "activeErrors": [],
                    "cancelledErrors": [],
                },
                "completeNativeTopologyAfterReopen": {"pass": True},
                "nativeWindowIdsAfterReopen": [777],
                "noMicrophoneCapture": True,
                "duringExit": {
                    "windowLifecycle": {
                        "nativeExit": {
                            "history": [
                                {"event": "ticketBegin"},
                                {"event": "ticketCancel"},
                            ]
                        }
                    }
                },
                "filmstrip": filmstrip,
            }
        )
    receipt = {
        "schemaVersion": 2,
        "requestedScenarioNames": SCENARIOS,
        "scenarios": scenarios,
        "capturePass": True if deferred else None,
        "analysisState": "pending" if deferred else "inline",
        "interference": {"pass": True, "disposition": "EVALUABLE_PASS"},
    }
    receipt_name = "capture-receipt.json" if deferred else "receipt.json"
    (run_dir / receipt_name).write_text(json.dumps(receipt))
    return run_dir / receipt_name


def run_grader(receipt_path: Path, out_dir: Path) -> tuple[int, dict]:
    proc = subprocess.run(
        [sys.executable, str(GRADER), "--receipt", str(receipt_path), "--out", str(out_dir)],
        capture_output=True,
        text=True,
        check=False,
    )
    merged_path = out_dir / "scenario-metrics.json"
    merged = json.loads(merged_path.read_text()) if merged_path.exists() else None
    return proc.returncode, merged


class GradeAllTests(unittest.TestCase):
    def test_green_synthetic_run_passes_all_scenarios(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            receipt = build_synthetic_run(Path(tmp))
            code, merged = run_grader(receipt, Path(tmp) / "out")
            self.assertEqual(code, 0)
            self.assertTrue(merged["pass"])
            self.assertEqual(
                {name: row["disposition"] for name, row in merged["scenarios"].items()},
                {name: "EVALUABLE_PASS" for name in SCENARIOS},
            )
            symmetry = merged["entryExitAlphaMatchedComparison"]
            self.assertTrue(symmetry["measurementPass"])
            self.assertEqual(len(symmetry["pairs"]), 2)

    def test_missing_scenario_fails_the_finalizer(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path = build_synthetic_run(Path(tmp))
            receipt = json.loads(receipt_path.read_text())
            receipt["scenarios"] = [
                row
                for row in receipt["scenarios"]
                if row["name"] != "notes-entry"
            ]
            receipt_path.write_text(json.dumps(receipt))
            code, merged = run_grader(receipt_path, Path(tmp) / "out")
            self.assertEqual(code, 1)
            self.assertFalse(merged["pass"])
            self.assertIn(
                "notes-entry: expected exactly one, observed 0",
                merged["scenarioSetErrors"],
            )

    def test_duplicate_scenario_fails_exact_set_validation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path = build_synthetic_run(Path(tmp))
            receipt = json.loads(receipt_path.read_text())
            receipt["scenarios"].append(receipt["scenarios"][0])
            receipt_path.write_text(json.dumps(receipt))
            code, merged = run_grader(receipt_path, Path(tmp) / "out")
            self.assertEqual(code, 1)
            self.assertIn(
                "main-exit: expected exactly one, observed 2",
                merged["scenarioSetErrors"],
            )

    def test_missing_frame_is_observer_failure_never_product_pass(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path = build_synthetic_run(Path(tmp))
            frame = Path(tmp) / "run" / "main-entry" / "frame-0000.png"
            frame.unlink()
            code, merged = run_grader(receipt_path, Path(tmp) / "out")
            self.assertEqual(code, 1)
            row = merged["scenarios"]["main-entry"]
            self.assertEqual(row["disposition"], "INVALID_OBSERVER")
            self.assertFalse(row["hardGatePass"])
            self.assertTrue(
                any("frame missing on disk" in error for error in row["errors"])
            )

    def test_corrupted_frame_byte_is_a_hash_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            receipt_path = build_synthetic_run(Path(tmp))
            frame = Path(tmp) / "run" / "main-exit" / "frame-0001.png"
            frame.write_bytes(frame.read_bytes() + b"\0")
            code, merged = run_grader(receipt_path, Path(tmp) / "out")
            self.assertEqual(code, 1)
            row = merged["scenarios"]["main-exit"]
            self.assertFalse(row["hardGatePass"])
            self.assertTrue(
                any("frame hash mismatch" in error for error in row["errors"])
            )

    def test_deferred_and_inline_grading_agree_on_every_gating_field(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_inline, tempfile.TemporaryDirectory() as tmp_deferred:
            inline_receipt = build_synthetic_run(Path(tmp_inline))
            deferred_receipt = build_synthetic_run(
                Path(tmp_deferred), deferred=True
            )
            inline_code, inline_merged = run_grader(
                inline_receipt, Path(tmp_inline) / "out"
            )
            deferred_code, deferred_merged = run_grader(
                deferred_receipt, Path(tmp_deferred) / "out"
            )
            self.assertEqual(inline_code, 0)
            self.assertEqual(deferred_code, 0)

            def gating(merged: dict) -> dict:
                return {
                    name: {
                        key: row[key]
                        for key in (
                            "capturePass",
                            "observerPass",
                            "lifecyclePass",
                            "metricPass",
                            "hardGatePass",
                            "disposition",
                        )
                    }
                    for name, row in merged["scenarios"].items()
                }

            self.assertEqual(gating(inline_merged), gating(deferred_merged))
            self.assertEqual(inline_merged["pass"], deferred_merged["pass"])
            # The deferred run must ALSO have written the final standard
            # receipt next to the capture receipt.
            final = json.loads(
                (deferred_receipt.parent / "receipt.json").read_text()
            )
            self.assertEqual(final["analysisState"], "offline")
            self.assertTrue(final["pass"])
            self.assertEqual(final["disposition"], "EVALUABLE_PASS")

    def test_symmetry_never_interpolates_fabricated_frames(self) -> None:
        entry = [
            {"sequence": 1, "windowAlpha": 0.50, "maximumStageDeltaE00": 4.0}
        ]
        exit_frames = [
            {"sequence": 9, "windowAlpha": 0.60, "maximumStageDeltaE00": 3.0}
        ]
        comparison = grade_all.entry_exit_alpha_matched_comparison(
            entry, exit_frames
        )
        self.assertEqual(comparison["pairs"], [])
        self.assertEqual(comparison["unmatchedEntrySequences"], [1])
        self.assertEqual(comparison["unmatchedExitSequences"], [9])
        self.assertFalse(comparison["measurementPass"])

    def test_cadence_measures_jitter_against_nearest_display_multiple(
        self,
    ) -> None:
        receipt = {
            "refreshRateHz": 120,
            "captureHealthPass": True,
            "lateFrameCount": 0,
            "duplicateDisplayTimeCount": 0,
            "frames": [
                {"displayTimeNs": 0},
                {"displayTimeNs": 8_333_333},  # exactly one period
                {"displayTimeNs": 25_000_000},  # ~2 periods later (damage gap)
            ],
        }
        cadence = grade_all.compute_cadence(receipt)
        self.assertEqual(cadence["intervalCount"], 2)
        # The 16.67ms interval is two display periods; residual jitter is
        # measured against 2x the period, not blindly against one period.
        self.assertLess(cadence["maximumResidualJitterMs"], 0.5)
        self.assertEqual(cadence["displayIntervalMs"], 1000.0 / 120)


if __name__ == "__main__":
    unittest.main()
