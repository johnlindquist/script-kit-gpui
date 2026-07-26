#!/usr/bin/env python3
"""WP8 (glass-smoke-harness-max-info): paired block statistics and
failure-only early stopping must be fail-closed in BOTH directions —
no invalid/unpaired observation can produce a terminal product failure,
and no amount of apparent success can shortcut the full acceptance quota.

Run from the repo root:
  python3 -m unittest scripts.agentic.test_glass_smoke_summary
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

_spec = importlib.util.spec_from_file_location(
    "glass_smoke_summary",
    Path(__file__).with_name("glass-smoke-summary.py"),
)
summary = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(summary)

CONTROL = "alpha000"
CANDIDATE = "alpha090"


def run_row(
    *,
    run_id: str,
    build_id: str,
    block_id: str,
    slot_position: int,
    kind: str = "scheduled",
    epoch: int = 0,
    disposition: str = "EVALUABLE_PASS",
    accepted: bool = True,
    load_mean: float = 2.0,
    maximum: float = 4.0,
    hard_gate_failures: list | None = None,
    below_floor: int = 0,
    zero_alpha: int = 0,
    capture_valid: bool = True,
    interference_pass: bool = True,
    load_eligible: bool = True,
    thermal_eligible: bool = True,
) -> dict:
    return {
        "runId": run_id,
        "buildId": build_id,
        "role": "negative-control" if build_id == CONTROL else "candidate",
        "kind": kind,
        "blockId": block_id,
        "scheduleEpoch": epoch,
        "slotPosition": slot_position,
        "disposition": disposition,
        "acceptedForInference": accepted,
        "loadMean": load_mean,
        "maximumDisplayedEntryDeltaE00": maximum,
        "hardGateFailures": hard_gate_failures or [],
        "captureValid": capture_valid,
        "interferencePass": interference_pass,
        "loadEligible": load_eligible,
        "thermalEligible": thermal_eligible,
        "visibleFramesBelowAlphaFloor": below_floor,
        "visibleZeroAlphaFrames": zero_alpha,
        "scenarioHardGates": {"main-entry": disposition == "EVALUABLE_PASS"},
    }


def green_study(blocks: int = 5) -> list[dict]:
    """ABBA blocks: control at mirrored outer positions, candidate inner.
    The negative control is red for the EXACT intended reason."""
    rows: list[dict] = []
    counter = 0
    for index in range(blocks):
        block = f"block-{index:02d}"

        def add(build, position, **kwargs):
            nonlocal counter
            counter += 1
            rows.append(
                run_row(
                    run_id=f"run-{counter:03d}",
                    build_id=build,
                    block_id=block,
                    slot_position=position,
                    **kwargs,
                )
            )

        add(
            CONTROL, 0,
            disposition="EVALUABLE_FAIL",
            hard_gate_failures=["alpha-policy"],
            below_floor=3,
        )
        add(CANDIDATE, 1)
        add(CANDIDATE, 2)
        add(
            CONTROL, 3,
            disposition="EVALUABLE_FAIL",
            hard_gate_failures=["alpha-policy"],
            below_floor=3,
        )
    return rows


def summarize(rows):
    return summary.summarize(
        rows, reference_build=CONTROL, reference_role="negative-control"
    )


class PairedBlockTests(unittest.TestCase):
    def test_complete_pass_meets_the_full_quota(self) -> None:
        result = summarize(green_study())
        verdict = result["verdict"]
        self.assertTrue(verdict["studyValid"])
        candidate = verdict["builds"][CANDIDATE]
        self.assertEqual(candidate["verdict"], "PASS")
        self.assertEqual(candidate["validBlockCount"], 5)
        self.assertEqual(candidate["eligiblePassingRunCount"], 10)
        self.assertEqual(len(result["pairedBlockComparisons"]), 5)
        comparison = result["pairedBlockComparisons"][0]
        self.assertTrue(comparison["pairLoadPass"])
        self.assertEqual(comparison["withinBlockReplicateSpreadDeltaE00"], 0.0)

    def test_valid_early_failure_is_terminal(self) -> None:
        rows = green_study()
        # One eligible, paired, evaluable candidate hard-gate failure.
        victim = next(
            row for row in rows if row["buildId"] == CANDIDATE
        )
        victim["disposition"] = "EVALUABLE_FAIL"
        victim["hardGateFailures"] = ["displayed-color"]
        verdict = summarize(rows)["verdict"]
        self.assertEqual(
            verdict["builds"][CANDIDATE]["verdict"], "TERMINAL_FAILED"
        )
        self.assertIn(
            victim["runId"], verdict["builds"][CANDIDATE]["terminalRunIds"]
        )

    def test_invalid_observer_never_triggers_terminal_failure(self) -> None:
        rows = green_study()
        victim = next(row for row in rows if row["buildId"] == CANDIDATE)
        victim["disposition"] = "INVALID_OBSERVER"
        victim["hardGateFailures"] = ["displayed-color"]
        victim["acceptedForInference"] = False
        verdict = summarize(rows)["verdict"]
        self.assertEqual(verdict["builds"][CANDIDATE]["verdict"], "INCOMPLETE")

    def test_invalid_interference_never_triggers_terminal_failure(self) -> None:
        rows = green_study()
        victim = next(row for row in rows if row["buildId"] == CANDIDATE)
        victim["disposition"] = "INVALID_INTERFERENCE"
        victim["hardGateFailures"] = ["displayed-color"]
        victim["acceptedForInference"] = False
        verdict = summarize(rows)["verdict"]
        self.assertEqual(verdict["builds"][CANDIDATE]["verdict"], "INCOMPLETE")

    def test_load_pair_mismatch_never_triggers_terminal_failure(self) -> None:
        rows = green_study()
        victim = next(row for row in rows if row["buildId"] == CANDIDATE)
        victim["disposition"] = "EVALUABLE_FAIL"
        victim["hardGateFailures"] = ["displayed-color"]
        victim["loadMean"] = 5.5  # 3.5 above the control's 2.0 -> unpaired
        verdict = summarize(rows)["verdict"]
        self.assertNotEqual(
            verdict["builds"][CANDIDATE]["verdict"], "TERMINAL_FAILED"
        )

    def test_failing_warmup_never_triggers_terminal_failure(self) -> None:
        rows = green_study()
        rows.append(
            run_row(
                run_id="warmup-x",
                build_id=CANDIDATE,
                block_id="warmup",
                slot_position=0,
                kind="warmup",
                disposition="EVALUABLE_FAIL",
                hard_gate_failures=["displayed-color"],
            )
        )
        verdict = summarize(rows)["verdict"]
        self.assertEqual(verdict["builds"][CANDIDATE]["verdict"], "PASS")

    def test_negative_control_red_for_wrong_reason_invalidates_study(
        self,
    ) -> None:
        rows = green_study()
        wrong = next(row for row in rows if row["buildId"] == CONTROL)
        # Red, but NOT via the alpha floor / zero-alpha mechanism — e.g. a
        # generic metric failure. The legacy broad fallback would accept
        # this; the v2 contract must not.
        wrong["visibleFramesBelowAlphaFloor"] = 0
        wrong["visibleZeroAlphaFrames"] = 0
        result = summarize(rows)
        verdict = result["verdict"]
        self.assertFalse(verdict["studyValid"])
        self.assertIn(
            "not red for the intended alpha-floor/zero-alpha reason",
            verdict["studyInvalidReasons"][0],
        )

    def test_missing_block_keeps_the_candidate_incomplete(self) -> None:
        rows = [row for row in green_study() if row["blockId"] != "block-04"]
        verdict = summarize(rows)["verdict"]
        candidate = verdict["builds"][CANDIDATE]
        self.assertEqual(candidate["verdict"], "INCOMPLETE")
        self.assertEqual(candidate["validBlockCount"], 4)

    def test_removing_one_of_ten_success_runs_keeps_incomplete(self) -> None:
        rows = green_study()
        victim = next(row for row in rows if row["buildId"] == CANDIDATE)
        victim["acceptedForInference"] = False
        verdict = summarize(rows)["verdict"]
        self.assertEqual(verdict["builds"][CANDIDATE]["verdict"], "INCOMPLETE")

    def test_no_variance_or_margin_produces_early_success(self) -> None:
        # Three perfect zero-variance blocks with a huge apparent margin —
        # still INCOMPLETE. Success can never stop early.
        rows = green_study(blocks=3)
        for row in rows:
            if row["buildId"] == CANDIDATE:
                row["maximumDisplayedEntryDeltaE00"] = 0.001
        verdict = summarize(rows)["verdict"]
        self.assertEqual(verdict["builds"][CANDIDATE]["verdict"], "INCOMPLETE")

    def test_schedule_epochs_are_labeled_and_never_merged(self) -> None:
        rows = green_study(blocks=3)
        epoch_rows = []
        counter = 0
        for index in range(2):
            block = f"block-{index:02d}"  # SAME block ids as epoch 0
            for build, position, kwargs in (
                (CONTROL, 0, dict(
                    disposition="EVALUABLE_FAIL",
                    hard_gate_failures=["alpha-policy"],
                    below_floor=3,
                )),
                (CANDIDATE, 1, {}),
                (CANDIDATE, 2, {}),
                (CONTROL, 3, dict(
                    disposition="EVALUABLE_FAIL",
                    hard_gate_failures=["alpha-policy"],
                    below_floor=3,
                )),
            ):
                counter += 1
                epoch_rows.append(
                    run_row(
                        run_id=f"epoch1-{counter:03d}",
                        build_id=build,
                        block_id=block,
                        slot_position=position,
                        epoch=1,
                        maximum=9.0,
                        **kwargs,
                    )
                )
        result = summarize(rows + epoch_rows)
        comparisons = result["pairedBlockComparisons"]
        # 3 comparisons from epoch 0 + 2 from epoch 1; block-00 appears in
        # both epochs as SEPARATE labeled comparisons, never merged.
        self.assertEqual(len(comparisons), 5)
        block00 = [row for row in comparisons if row["blockId"] == "block-00"]
        self.assertEqual(
            sorted(row["scheduleEpoch"] for row in block00), [0, 1]
        )
        means = {
            row["scheduleEpoch"]: row["blockMeanMaximumDisplayedEntryDeltaE00"]
            for row in block00
        }
        self.assertEqual(means[0], 4.0)
        self.assertEqual(means[1], 9.0)

    def test_next_schedule_epoch_removes_failed_candidates(self) -> None:
        plan = summary.next_schedule_epoch(
            [CONTROL, CANDIDATE, "alpha095"], ["alpha095"], current_epoch=0
        )
        self.assertEqual(plan["scheduleEpoch"], 1)
        self.assertEqual(plan["buildIds"], [CONTROL, CANDIDATE])
        self.assertEqual(plan["removedBuildIds"], ["alpha095"])
        self.assertFalse(plan["crossEpochComparisonAllowed"])

    def test_terminal_failure_predicate_is_the_plan_exact_shape(self) -> None:
        base = {
            "acceptedForInference": True,
            "pairLoadPass": True,
            "disposition": "EVALUABLE_FAIL",
            "hardGateFailures": ["displayed-color"],
        }
        self.assertTrue(summary.terminal_failure(base))
        for mutation in (
            {"acceptedForInference": False},
            {"pairLoadPass": False},
            {"disposition": "EVALUABLE_PASS"},
            {"disposition": "INVALID_INTERFERENCE"},
            {"hardGateFailures": []},
        ):
            self.assertFalse(summary.terminal_failure({**base, **mutation}))


if __name__ == "__main__":
    unittest.main()
