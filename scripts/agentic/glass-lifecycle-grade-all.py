#!/usr/bin/env python3
"""Grade EVERY captured lifecycle scenario from one raw dataset.

Oracle plan glass-smoke-harness-max-info, work package 5. The lifecycle
probe captures five scenarios per invocation and (until now) only main-entry
was promoted into run rows; the other scenarios' metric results were
computed and discarded. This grader consumes either:

- an ordinary INLINE lifecycle receipt (receipt.json), or
- a DEFERRED capture receipt (capture-receipt.json, analysisState=pending),
  in which case each scenario's preserved metricsCommand is re-executed
  verbatim against the same hash-bound artifacts, and a final standard
  receipt.json is written next to the capture receipt.

Every adapter returns the standardized shape:
  { capturePass, observerPass, lifecyclePass, metricPass, hardGatePass,
    disposition, metrics, errors, artifactPaths, artifactSha256 }

Usage:
  python3 scripts/agentic/glass-lifecycle-grade-all.py \
    --receipt <run>/lifecycle/receipt.json --out <run>/scenario-metrics
"""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import statistics
import subprocess
from pathlib import Path

LEGACY_FULL_SCENARIO_ORDER = [
    "main-exit",
    "main-entry",
    "notes-entry",
    "notes-close-before-settle-reopen",
    "dictation-exit-reopen",
]

ALPHA_MATCH_TOLERANCE = 0.025


def sha256_file(path: Path) -> str | None:
    if not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def worker_count() -> int:
    try:
        performance_cores = int(
            subprocess.run(
                ["sysctl", "-n", "hw.perflevel0.physicalcpu"],
                capture_output=True,
                text=True,
                check=False,
            ).stdout.strip()
            or os.cpu_count()
            or 1
        )
    except ValueError:
        performance_cores = os.cpu_count() or 1
    return max(1, min(4, performance_cores - 1))


def validate_scenario_set(receipt: dict) -> list[str]:
    """Exact-set validation: the declared profile's scenarios must each be
    present exactly once, and nothing else may appear."""
    errors: list[str] = []
    required = (
        receipt.get("requestedScenarioNames")
        or receipt.get("requiredScenarioNames")
        or LEGACY_FULL_SCENARIO_ORDER
    )
    observed = [row.get("name") for row in receipt.get("scenarios") or []]
    for name in required:
        count = observed.count(name)
        if count != 1:
            errors.append(f"{name}: expected exactly one, observed {count}")
    for name in observed:
        if name not in required:
            errors.append(f"unexpected scenario: {name}")
    return errors


def verify_frame_hashes(scenario: dict) -> list[str]:
    """Re-hash every filmstrip frame ON DISK against its recorded sha256.
    A corrupted frame byte can never become a product verdict."""
    errors: list[str] = []
    frames = (
        (scenario.get("filmstrip") or {}).get("receipt") or {}
    ).get("frames") or []
    for frame in frames:
        path = Path(frame.get("path", ""))
        recorded = frame.get("sha256")
        if not path.is_file():
            errors.append(f"frame missing on disk: {path}")
            continue
        actual = sha256_file(path)
        if recorded and actual != recorded:
            errors.append(
                f"frame hash mismatch: {path.name} recorded {recorded} on disk {actual}"
            )
    return errors


def compute_cadence(filmstrip_receipt: dict) -> dict:
    """Per-filmstrip capture cadence and residual jitter. ScreenCaptureKit is
    damage-driven, so residual jitter is measured against the NEAREST
    POSITIVE INTEGER MULTIPLE of the display period, never blindly against
    one frame period. Percentile fields are diagnostic; the existing
    capture-health gates are preserved unchanged."""
    refresh = float(filmstrip_receipt.get("refreshRateHz") or 0)
    display_interval_ms = 1000.0 / refresh if refresh > 0 else None
    times = [
        float(frame["displayTimeNs"])
        for frame in filmstrip_receipt.get("frames") or []
        if isinstance(frame.get("displayTimeNs"), (int, float))
    ]
    intervals_ms = [
        (later - earlier) / 1e6
        for earlier, later in zip(times, times[1:])
    ]
    residuals: list[float] = []
    if display_interval_ms:
        for interval in intervals_ms:
            multiple = max(1, round(interval / display_interval_ms))
            residuals.append(abs(interval - multiple * display_interval_ms))

    def percentile(rows: list[float], fraction: float) -> float | None:
        if not rows:
            return None
        ordered = sorted(rows)
        index = min(len(ordered) - 1, int(fraction * (len(ordered) - 1)))
        return ordered[index]

    return {
        "displayIntervalMs": display_interval_ms,
        "intervalCount": len(intervals_ms),
        "p50IntervalMs": statistics.median(intervals_ms) if intervals_ms else None,
        "p95IntervalMs": percentile(intervals_ms, 0.95),
        "maximumIntervalMs": max(intervals_ms, default=None),
        "p50MultipleOfDisplayPeriod": (
            statistics.median(
                max(1, round(interval / display_interval_ms))
                for interval in intervals_ms
            )
            if intervals_ms and display_interval_ms
            else None
        ),
        "p95ResidualJitterMs": percentile(residuals, 0.95),
        "maximumResidualJitterMs": max(residuals, default=None),
        "lateFrameCount": filmstrip_receipt.get("lateFrameCount"),
        "duplicateDisplayTimeCount": filmstrip_receipt.get(
            "duplicateDisplayTimeCount"
        ),
        "maximumConsecutiveDisplayTimeGapNs": filmstrip_receipt.get(
            "maximumConsecutiveDisplayTimeGapNs"
        ),
        "maximumAllowedDisplayTimeGapNs": filmstrip_receipt.get(
            "maximumAllowedDisplayTimeGapNs"
        ),
        "screenDamageCadenceWithinOneDisplayPeriod": filmstrip_receipt.get(
            "screenDamageCadenceWithinOneDisplayPeriod"
        ),
        "captureHealthPass": filmstrip_receipt.get("captureHealthPass"),
    }


def lifecycle_event_counts(scenario: dict) -> dict:
    """Count ACTUAL nativeExit history events. This is deliberately not
    called a style-mutation count: a style count is only accepted when a
    real style ledger exists."""
    counts = {"ticketBegin": 0, "ticketCancel": 0, "ticketCommit": 0, "other": {}}
    for holder_key in ("duringExit", "afterReopen"):
        holder = scenario.get(holder_key) or {}
        history = (
            (holder.get("windowLifecycle") or {}).get("nativeExit") or {}
        ).get("history") or []
        for event in history:
            name = str(event.get("event"))
            if name in counts:
                counts[name] += 1
            else:
                counts["other"][name] = counts["other"].get(name, 0) + 1
    return counts


def entry_exit_alpha_matched_comparison(
    entry_frames: list[dict],
    exit_frames: list[dict],
    tolerance: float = ALPHA_MATCH_TOLERANCE,
) -> dict:
    """Match REAL captured frames by nearest observed alpha within an
    absolute ceiling. Fabricated (interpolated) frames are prohibited: an
    entry frame with no exit frame within tolerance stays unmatched."""
    pairs: list[dict] = []
    unmatched_entry: list = []
    used_exit: set[int] = set()
    for entry in entry_frames:
        entry_alpha = entry.get("windowAlpha")
        if not isinstance(entry_alpha, (int, float)):
            unmatched_entry.append(entry.get("sequence"))
            continue
        best_index: int | None = None
        best_distance = tolerance
        for index, candidate in enumerate(exit_frames):
            if index in used_exit:
                continue
            exit_alpha = candidate.get("windowAlpha")
            if not isinstance(exit_alpha, (int, float)):
                continue
            distance = abs(float(entry_alpha) - float(exit_alpha))
            if distance <= best_distance:
                best_distance = distance
                best_index = index
        if best_index is None:
            unmatched_entry.append(entry.get("sequence"))
            continue
        used_exit.add(best_index)
        matched = exit_frames[best_index]

        def frame_delta(frame: dict) -> float | None:
            value = frame.get("maximumStageDeltaE00")
            return float(value) if isinstance(value, (int, float)) else None

        entry_delta, exit_delta = frame_delta(entry), frame_delta(matched)
        pairs.append(
            {
                "entrySequence": entry.get("sequence"),
                "exitSequence": matched.get("sequence"),
                "entryAlpha": entry_alpha,
                "exitAlpha": matched.get("windowAlpha"),
                "alphaDistance": best_distance,
                "entryMaximumStageDeltaE00": entry_delta,
                "exitMaximumStageDeltaE00": exit_delta,
                "displayedDeltaE00Difference": (
                    abs(entry_delta - exit_delta)
                    if entry_delta is not None and exit_delta is not None
                    else None
                ),
            }
        )
    unmatched_exit = [
        frame.get("sequence")
        for index, frame in enumerate(exit_frames)
        if index not in used_exit
    ]
    differences = [
        pair["displayedDeltaE00Difference"]
        for pair in pairs
        if pair["displayedDeltaE00Difference"] is not None
    ]
    return {
        "matchTolerance": tolerance,
        "pairs": pairs,
        "unmatchedEntrySequences": unmatched_entry,
        "unmatchedExitSequences": unmatched_exit,
        "maximumDisplayedDeltaE00Difference": max(differences, default=None),
        "measurementPass": bool(pairs),
    }


def _base_result(scenario: dict, run_dir: Path) -> dict:
    filmstrip = scenario.get("filmstrip") or {}
    frame_errors = verify_frame_hashes(scenario)
    capture_pass = (
        filmstrip.get("capturePass")
        if filmstrip.get("capturePass") is not None
        else filmstrip.get("pass")
    ) is True and not frame_errors
    observer_pass = filmstrip.get("exitCode") == 0 and not frame_errors
    metrics_path = Path(filmstrip.get("metricsPath") or "")
    receipt_path = Path(filmstrip.get("receiptPath") or "")
    return {
        "capturePass": capture_pass,
        "observerPass": observer_pass,
        "lifecyclePass": scenario.get("structuralPass") is True,
        "metricPass": False,
        "hardGatePass": False,
        "disposition": "EVALUABLE_FAIL",
        "metrics": {
            "cadence": compute_cadence(filmstrip.get("receipt") or {}),
            "lifecycleEventCounts": lifecycle_event_counts(scenario),
        },
        "errors": list(frame_errors),
        "artifactPaths": {
            "metrics": str(metrics_path) if str(metrics_path) != "." else None,
            "filmstripReceipt": str(receipt_path)
            if str(receipt_path) != "."
            else None,
        },
        "artifactSha256": {
            "metrics": sha256_file(metrics_path),
            "filmstripReceipt": sha256_file(receipt_path),
        },
    }


def _finalize(result: dict) -> dict:
    result["hardGatePass"] = (
        result["capturePass"]
        and result["observerPass"]
        and result["lifecyclePass"]
        and result["metricPass"]
    )
    if not result["observerPass"]:
        result["disposition"] = "INVALID_OBSERVER"
    elif result["hardGatePass"]:
        result["disposition"] = "EVALUABLE_PASS"
    else:
        result["disposition"] = "EVALUABLE_FAIL"
    return result


def grade_main_exit(scenario: dict, metrics: dict | None, run_dir: Path) -> dict:
    result = _base_result(scenario, run_dir)
    metrics = metrics or {}
    result["metrics"].update(
        {
            "geometryStable": metrics.get("geometryStable"),
            "geometryStateCount": metrics.get("geometryStateCount"),
            "gutterPass": metrics.get("gutterPass"),
            "alphaProgressionPass": metrics.get("alphaProgressionPass"),
            "hiddenReferencePass": scenario.get("hiddenReferencePass"),
        }
    )
    result["metricPass"] = (
        metrics.get("pass") is True
        and scenario.get("hiddenReferencePass") is True
    )
    return _finalize(result)


def grade_main_entry(scenario: dict, metrics: dict | None, run_dir: Path) -> dict:
    result = _base_result(scenario, run_dir)
    metrics = metrics or {}
    motion_envelope = scenario.get("motionEnvelope") or {}
    result["metrics"].update(
        {
            "motionEnvelopePass": motion_envelope.get("pass"),
            "settledCapturesPass": scenario.get("settledCapturesPass"),
            "captureBoundsMatch": scenario.get("captureBoundsMatch"),
            "alphaProgressionPass": metrics.get("alphaProgressionPass"),
        }
    )
    result["metricPass"] = metrics.get("pass") is True
    return _finalize(result)


def grade_notes_entry(scenario: dict, metrics: dict | None, run_dir: Path) -> dict:
    result = _base_result(scenario, run_dir)
    metrics = metrics or {}
    reveal = scenario.get("bodyOnlyReveal") or {}
    host_clock = reveal.get("hostClockTiming") or {}
    result["metrics"].update(
        {
            "hiddenBeforeVisible": reveal.get("hiddenBeforeVisible"),
            "visibleAfterAnchor": reveal.get("visibleAfterAnchor"),
            "bodyPixelTransition": metrics.get("bodyPixelTransition"),
            "bodyMaskPass": metrics.get("bodyMaskPass"),
            "hostClockOrdered": host_clock.get("ordered"),
            "visibleWithinBounds": host_clock.get("visibleWithinBounds"),
            "completedFrameCount": reveal.get("completedFrameCount"),
        }
    )
    result["metricPass"] = (
        metrics.get("pass") is True and metrics.get("bodyMaskPass") is True
    )
    return _finalize(result)


def _grade_reopen_like(scenario: dict, metrics: dict | None, run_dir: Path) -> dict:
    result = _base_result(scenario, run_dir)
    metrics = metrics or {}
    validation = scenario.get("nativeExitValidation") or {}
    topology = scenario.get("completeNativeTopologyAfterReopen") or {}
    result["metrics"].update(
        {
            "exactOwnerReuse": scenario.get("nativeWindowIdsAfterReopen"),
            "activeExitErrorCount": len(validation.get("activeErrors") or []),
            "cancelledExitErrorCount": len(
                validation.get("cancelledErrors") or []
            ),
            "geometryStateCount": metrics.get("geometryStateCount"),
            "topologyPass": topology.get("pass"),
        }
    )
    result["metricPass"] = metrics.get("pass") is True
    return _finalize(result)


def grade_notes_reopen(scenario: dict, metrics: dict | None, run_dir: Path) -> dict:
    return _grade_reopen_like(scenario, metrics, run_dir)


def grade_dictation_reopen(
    scenario: dict, metrics: dict | None, run_dir: Path
) -> dict:
    result = _grade_reopen_like(scenario, metrics, run_dir)
    result["metrics"]["noMicrophoneCapture"] = scenario.get(
        "noMicrophoneCapture"
    )
    return result


SCENARIO_GRADERS = {
    "main-exit": grade_main_exit,
    "main-entry": grade_main_entry,
    "notes-entry": grade_notes_entry,
    "notes-close-before-settle-reopen": grade_notes_reopen,
    "dictation-exit-reopen": grade_dictation_reopen,
}


def resolve_metrics(scenario: dict, deferred: bool) -> tuple[dict | None, list[str]]:
    """Inline receipts carry graded metrics; deferred captures carry the
    exact preserved grader command, which is re-executed verbatim."""
    filmstrip = scenario.get("filmstrip") or {}
    errors: list[str] = []
    if not deferred:
        return filmstrip.get("metrics"), errors
    command = filmstrip.get("metricsCommand")
    if not command:
        return None, [f"{scenario.get('name')}: deferred capture lacks metricsCommand"]
    proc = subprocess.run(command, capture_output=True, text=True, check=False)
    metrics_path = Path(filmstrip.get("metricsPath") or "")
    metrics = (
        json.loads(metrics_path.read_text()) if metrics_path.is_file() else None
    )
    if proc.returncode != 0 and metrics is None:
        errors.append(
            f"{scenario.get('name')}: deferred metrics command failed "
            f"({proc.returncode}): {proc.stderr.strip()[-300:]}"
        )
    return metrics, errors


def grade_one(scenario: dict, deferred: bool, run_dir: Path) -> dict:
    name = scenario.get("name")
    grader = SCENARIO_GRADERS.get(name)
    if grader is None:
        return {
            "capturePass": False,
            "observerPass": False,
            "lifecyclePass": False,
            "metricPass": False,
            "hardGatePass": False,
            "disposition": "EVALUABLE_FAIL",
            "metrics": {},
            "errors": [f"no grader adapter for scenario {name!r}"],
            "artifactPaths": {},
            "artifactSha256": {},
        }
    metrics, errors = resolve_metrics(scenario, deferred)
    result = grader(scenario, metrics, run_dir)
    result["errors"].extend(errors)
    if errors:
        result["metricPass"] = False
        result = _finalize(result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--out")
    parser.add_argument("--workers", type=int)
    args = parser.parse_args()

    receipt_path = Path(args.receipt).resolve()
    receipt = json.loads(receipt_path.read_text())
    deferred = receipt.get("analysisState") == "pending"
    run_dir = receipt_path.parent
    out_dir = Path(args.out) if args.out else run_dir / "scenario-metrics"
    out_dir.mkdir(parents=True, exist_ok=True)

    set_errors = validate_scenario_set(receipt)
    scenarios = receipt.get("scenarios") or []
    required = (
        receipt.get("requestedScenarioNames")
        or receipt.get("requiredScenarioNames")
        or LEGACY_FULL_SCENARIO_ORDER
    )

    results: dict[str, dict] = {}
    if not set_errors:
        workers = args.workers or worker_count()
        with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
            futures = {
                pool.submit(grade_one, scenario, deferred, run_dir): scenario[
                    "name"
                ]
                for scenario in scenarios
            }
            for future in concurrent.futures.as_completed(futures):
                results[futures[future]] = future.result()

    # Deterministic merge in DECLARED scenario order.
    ordered_results = {
        name: results[name] for name in required if name in results
    }
    for name, result in ordered_results.items():
        (out_dir / f"{name}.json").write_text(
            json.dumps(result, indent=2, sort_keys=True) + "\n"
        )

    symmetry = None
    entry_frames = None
    exit_frames = None
    for scenario in scenarios:
        rows = (
            (scenario.get("filmstrip") or {}).get("metrics") or {}
        ).get("frames")
        if scenario.get("name") == "main-entry":
            entry_frames = rows
        if scenario.get("name") == "main-exit":
            exit_frames = rows
    if entry_frames and exit_frames:
        symmetry = entry_exit_alpha_matched_comparison(
            entry_frames, exit_frames
        )

    overall_pass = (
        not set_errors
        and bool(ordered_results)
        and all(row["hardGatePass"] for row in ordered_results.values())
    )
    merged = {
        "schemaVersion": 1,
        "sourceReceipt": str(receipt_path),
        "sourceReceiptSha256": sha256_file(receipt_path),
        "analysisSource": "deferred" if deferred else "inline",
        "scenarioSetErrors": set_errors,
        "scenarios": ordered_results,
        "entryExitAlphaMatchedComparison": symmetry,
        "pass": overall_pass,
    }
    (out_dir / "scenario-metrics.json").write_text(
        json.dumps(merged, indent=2, sort_keys=True) + "\n"
    )

    if deferred and not set_errors:
        # Write the final standard lifecycle receipt next to the capture
        # receipt. Interference invalidity always dominates.
        final = dict(receipt)
        final["analysisState"] = "offline"
        final["offlineGrading"] = {
            "scenarioMetrics": str(out_dir / "scenario-metrics.json"),
            "pass": overall_pass,
        }
        interference_invalid = (
            (receipt.get("interference") or {}).get("disposition")
            == "INVALID_INTERFERENCE"
        )
        final["pass"] = (
            overall_pass
            and receipt.get("capturePass") is True
            and not interference_invalid
        )
        final["disposition"] = (
            "INVALID_INTERFERENCE"
            if interference_invalid
            else "EVALUABLE_PASS"
            if final["pass"]
            else "INVALID_OBSERVER"
            if receipt.get("error")
            else "EVALUABLE_FAIL"
        )
        (run_dir / "receipt.json").write_text(
            json.dumps(final, indent=2) + "\n"
        )

    print(
        json.dumps(
            {
                "out": str(out_dir / "scenario-metrics.json"),
                "pass": overall_pass,
                "scenarioSetErrors": set_errors,
                "scenarios": {
                    name: row["disposition"]
                    for name, row in ordered_results.items()
                },
            },
            indent=2,
        )
    )
    return 0 if overall_pass else 1


if __name__ == "__main__":
    raise SystemExit(main())
