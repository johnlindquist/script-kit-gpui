#!/usr/bin/env python3
"""Paired block statistics + failure-only early stopping for glass smoke
studies (glass-smoke-harness-max-info WP8).

Statistical design (decided by the Oracle plan, not renegotiable here):

* Acceptance keeps the FULL current bar: five complete valid blocks, ten
  eligible runs per candidate, every candidate run passing all existing
  hard gates. Reducing the successful-run requirement weakens acceptance
  and is rejected.
* The added value is the PAIRING the legacy headline discarded: candidate
  forward occurrences pair with the reference build's forward occurrence
  in the same block (reverse with reverse), preserving the legacy <= 1.0
  load-delta limit per comparison.
* No load-adjusted regression, no small-sample p-values: raw paired block
  data, medians, and spreads are more honest at this sample size.
* Success can NEVER stop a study early. Only an eligible, paired,
  evaluable candidate hard-gate failure is terminal; only an eligible
  negative control that is not red for the EXACT intended reason
  invalidates the study.

Run rows (produced by the study runner / receipts v2) must carry:
  runId, buildId, role, kind (warmup|scheduled), blockId, scheduleEpoch,
  slotPosition, disposition, acceptedForInference, loadMean,
  maximumDisplayedEntryDeltaE00, hardGateFailures (list),
  captureValid, interferencePass, loadEligible, thermalEligible,
  visibleFramesBelowAlphaFloor, visibleZeroAlphaFrames,
  scenarioHardGates ({scenario: bool}).
"""

from __future__ import annotations

import argparse
import json
import statistics
from collections import defaultdict
from pathlib import Path

PAIR_LOAD_DELTA_LIMIT = 1.0
REQUIRED_VALID_BLOCKS = 5
REQUIRED_ELIGIBLE_RUNS = 10


def terminal_failure(row: dict) -> bool:
    """The plan's exact critical stopping predicate."""
    return (
        row["acceptedForInference"] is True
        and row["pairLoadPass"] is True
        and row["disposition"] == "EVALUABLE_FAIL"
        and bool(row["hardGateFailures"])
    )


def negative_control_red_for_intended_reason(row: dict) -> bool:
    """The EXACT intended negative-control reason. The legacy broad
    fallback ("any metric failure means red for the right reason") is
    deliberately NOT retained."""
    return (
        row.get("captureValid") is True
        and row.get("interferencePass") is True
        and row.get("loadEligible") is True
        and row.get("thermalEligible") is True
        and (
            row.get("visibleFramesBelowAlphaFloor", 0) > 0
            or row.get("visibleZeroAlphaFrames", 0) > 0
        )
    )


def _inference_rows(rows: list[dict]) -> list[dict]:
    return [row for row in rows if row.get("kind") == "scheduled"]


def _occurrences(rows: list[dict], build_id: str, block_id: str) -> list[dict]:
    """Forward/reverse occurrences of a build within one block, ordered by
    slot position: first is forward, second is reverse (mirrored)."""
    hits = sorted(
        (
            row
            for row in rows
            if row["buildId"] == build_id and row["blockId"] == block_id
        ),
        key=lambda row: row["slotPosition"],
    )
    return hits


def paired_block_comparisons(
    rows: list[dict],
    reference_build: str,
) -> list[dict]:
    """One comparison row per (candidate build, block, epoch). Pairs are
    formed ONLY within a schedule epoch — cross-epoch pairing would compare
    different temporal neighborhoods."""
    scheduled = _inference_rows(rows)
    comparisons: list[dict] = []
    builds = sorted({row["buildId"] for row in scheduled})
    epochs = sorted({row.get("scheduleEpoch", 0) for row in scheduled})
    for epoch in epochs:
        epoch_rows = [
            row for row in scheduled if row.get("scheduleEpoch", 0) == epoch
        ]
        blocks = sorted({row["blockId"] for row in epoch_rows})
        for build_id in builds:
            if build_id == reference_build:
                continue
            for block_id in blocks:
                candidate = _occurrences(epoch_rows, build_id, block_id)
                reference = _occurrences(epoch_rows, reference_build, block_id)
                if len(candidate) != 2 or len(reference) != 2:
                    continue
                forward, reverse = candidate
                ref_forward, ref_reverse = reference
                values = [
                    forward["maximumDisplayedEntryDeltaE00"],
                    reverse["maximumDisplayedEntryDeltaE00"],
                ]
                ref_values = [
                    ref_forward["maximumDisplayedEntryDeltaE00"],
                    ref_reverse["maximumDisplayedEntryDeltaE00"],
                ]
                load_delta_forward = abs(
                    forward["loadMean"] - ref_forward["loadMean"]
                )
                load_delta_reverse = abs(
                    reverse["loadMean"] - ref_reverse["loadMean"]
                )
                pair_load_pass = (
                    load_delta_forward <= PAIR_LOAD_DELTA_LIMIT
                    and load_delta_reverse <= PAIR_LOAD_DELTA_LIMIT
                )
                accepted = all(
                    row.get("acceptedForInference") is True
                    for row in (forward, reverse, ref_forward, ref_reverse)
                )
                comparisons.append(
                    {
                        "blockId": block_id,
                        "scheduleEpoch": epoch,
                        "buildId": build_id,
                        "forwardRunId": forward["runId"],
                        "reverseRunId": reverse["runId"],
                        "blockMeanMaximumDisplayedEntryDeltaE00":
                            statistics.mean(values),
                        "blockWorstMaximumDisplayedEntryDeltaE00": max(values),
                        "withinBlockReplicateSpreadDeltaE00": abs(
                            values[0] - values[1]
                        ),
                        "referenceBlockMeanMaximumDisplayedEntryDeltaE00":
                            statistics.mean(ref_values),
                        "pairedDeltaE00":
                            statistics.mean(values) - statistics.mean(ref_values),
                        "pairLoadDeltaForward": load_delta_forward,
                        "pairLoadDeltaReverse": load_delta_reverse,
                        "pairLoadPass": pair_load_pass,
                        "acceptedForInference": accepted,
                    }
                )
    return comparisons


def annotate_pair_load_pass(rows: list[dict], comparisons: list[dict]) -> None:
    """Stamp each scheduled run with its block comparison's pairLoadPass so
    terminal_failure can require a load-matched pairing."""
    by_key: dict[tuple, bool] = {}
    for comparison in comparisons:
        key = (
            comparison["buildId"],
            comparison["blockId"],
            comparison["scheduleEpoch"],
        )
        by_key[key] = comparison["pairLoadPass"]
    for row in rows:
        key = (
            row.get("buildId"),
            row.get("blockId"),
            row.get("scheduleEpoch", 0),
        )
        row.setdefault("pairLoadPass", by_key.get(key, False))


def summarize_build(rows: list[dict], comparisons: list[dict], build_id: str) -> dict:
    scheduled = [
        row for row in _inference_rows(rows) if row["buildId"] == build_id
    ]
    eligible = [
        row for row in scheduled if row.get("acceptedForInference") is True
    ]
    maxima = [row["maximumDisplayedEntryDeltaE00"] for row in eligible]
    build_comparisons = [
        row
        for row in comparisons
        if row["buildId"] == build_id and row["acceptedForInference"]
    ]
    deltas = [row["pairedDeltaE00"] for row in build_comparisons]
    spreads = [
        row["withinBlockReplicateSpreadDeltaE00"] for row in build_comparisons
    ]
    loads = [row["loadMean"] for row in eligible]
    scenario_pass_counts: dict[str, int] = defaultdict(int)
    for row in eligible:
        for scenario, passed in (row.get("scenarioHardGates") or {}).items():
            if passed:
                scenario_pass_counts[scenario] += 1
    return {
        "buildId": build_id,
        "eligibleRunCount": len(eligible),
        "legacyComparableMedianOfPerRunMaxima": (
            statistics.median(maxima) if maxima else None
        ),
        "worstPerRunMaximum": max(maxima) if maxima else None,
        "medianPairedBlockDeltaE00": (
            statistics.median(deltas) if deltas else None
        ),
        "allPairedBlockDeltasE00": deltas,
        "medianWithinBlockReplicateSpread": (
            statistics.median(spreads) if spreads else None
        ),
        "maximumWithinBlockReplicateSpread": max(spreads) if spreads else None,
        "loadP50": statistics.median(loads) if loads else None,
        "loadMaximum": max(loads) if loads else None,
        "scenarioHardGatePassCounts": dict(scenario_pass_counts),
    }


def study_verdict(
    rows: list[dict],
    comparisons: list[dict],
    *,
    reference_build: str,
    reference_role: str,
) -> dict:
    """Failure-only early stop + per-candidate acceptance.

    Success can never stop early, and no combination of low variance or
    strong apparent margin produces early success: acceptance is only
    counted from complete valid blocks and the full eligible-run quota.
    """
    scheduled = _inference_rows(rows)
    builds = sorted({row["buildId"] for row in scheduled})
    verdicts: dict[str, dict] = {}
    study_invalid_reasons: list[str] = []

    for build_id in builds:
        build_rows = [row for row in scheduled if row["buildId"] == build_id]
        if build_id == reference_build and reference_role == "negative-control":
            # An ELIGIBLE, EVALUABLE negative control must be red for the
            # exact intended reason; anything else invalidates the study.
            for row in build_rows:
                eligible = (
                    row.get("acceptedForInference") is True
                    and row.get("disposition") in ("EVALUABLE_PASS", "EVALUABLE_FAIL")
                )
                if eligible and not negative_control_red_for_intended_reason(row):
                    study_invalid_reasons.append(
                        f"negative control run {row['runId']} is not red for the intended alpha-floor/zero-alpha reason"
                    )
            verdicts[build_id] = {
                "role": "negative-control",
                "verdict": (
                    "CONTROL_VALID" if not study_invalid_reasons else "CONTROL_INVALID"
                ),
            }
            continue

        terminal_rows = [row for row in build_rows if terminal_failure(row)]
        if terminal_rows:
            verdicts[build_id] = {
                "role": "candidate",
                "verdict": "TERMINAL_FAILED",
                "terminalRunIds": [row["runId"] for row in terminal_rows],
            }
            continue
        eligible_pass = [
            row
            for row in build_rows
            if row.get("acceptedForInference") is True
            and row.get("disposition") == "EVALUABLE_PASS"
            and not row.get("hardGateFailures")
        ]
        valid_blocks = {
            (row["blockId"], row.get("scheduleEpoch", 0))
            for row in comparisons
            if row["buildId"] == build_id and row["acceptedForInference"]
        }
        complete = (
            len(valid_blocks) >= REQUIRED_VALID_BLOCKS
            and len(eligible_pass) >= REQUIRED_ELIGIBLE_RUNS
        )
        verdicts[build_id] = {
            "role": "candidate",
            "verdict": "PASS" if complete else "INCOMPLETE",
            "validBlockCount": len(valid_blocks),
            "eligiblePassingRunCount": len(eligible_pass),
            "requiredValidBlocks": REQUIRED_VALID_BLOCKS,
            "requiredEligibleRuns": REQUIRED_ELIGIBLE_RUNS,
        }

    return {
        "studyValid": not study_invalid_reasons,
        "studyInvalidReasons": study_invalid_reasons,
        "builds": verdicts,
    }


def next_schedule_epoch(
    surviving_builds: list[str],
    failed_builds: list[str],
    current_epoch: int,
) -> dict:
    """After a terminal candidate failure: remove the candidate, increment
    scheduleEpoch, regenerate the mirrored schedule for the survivors.
    Paired deltas across epochs must stay labeled, never merged."""
    return {
        "scheduleEpoch": current_epoch + 1,
        "buildIds": [
            build for build in surviving_builds if build not in failed_builds
        ],
        "removedBuildIds": failed_builds,
        "crossEpochComparisonAllowed": False,
    }


def summarize(rows: list[dict], *, reference_build: str, reference_role: str) -> dict:
    comparisons = paired_block_comparisons(rows, reference_build)
    annotate_pair_load_pass(rows, comparisons)
    scheduled = _inference_rows(rows)
    builds = sorted({row["buildId"] for row in scheduled})
    return {
        "schemaVersion": 1,
        "referenceBuild": reference_build,
        "referenceRole": reference_role,
        "pairLoadDeltaLimit": PAIR_LOAD_DELTA_LIMIT,
        "pairedBlockComparisons": comparisons,
        "builds": {
            build_id: summarize_build(rows, comparisons, build_id)
            for build_id in builds
        },
        "verdict": study_verdict(
            rows,
            comparisons,
            reference_build=reference_build,
            reference_role=reference_role,
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", required=True, help="runs.jsonl path")
    parser.add_argument("--reference-build", required=True)
    parser.add_argument(
        "--reference-role",
        choices=["negative-control", "baseline"],
        default="negative-control",
    )
    parser.add_argument("--out")
    args = parser.parse_args()
    rows = [
        json.loads(line)
        for line in Path(args.runs).read_text().splitlines()
        if line.strip()
    ]
    result = summarize(
        rows,
        reference_build=args.reference_build,
        reference_role=args.reference_role,
    )
    serialized = json.dumps(result, indent=2) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    verdict = result["verdict"]
    return 0 if verdict["studyValid"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
