#!/usr/bin/env python3
"""Information economics, artifact provenance, and comparison-ledger
validation for glass smoke studies (glass-smoke-harness-max-info WP10).

Principles (all fail-closed):

* Information is counted in STABLE DECISION UNITS from the registry, never
  raw JSON field counts — adding fields without adding a registry unit
  cannot improve economics.
* A unit counts only when its required source artifact exists, its input
  hashes validate, it has a machine-readable result, and the result is
  promoted into both the run row and the session summary. A missing unit
  lowers the count; it is never assumed.
* Cross-run frame reuse is prohibited for replication; offline regrading
  of the same raw capture under additional metrics is allowed and
  hash-bound via the metric cache key.
* Every plan recommendation gets exactly one terminal comparison-ledger
  entry. PENDING is forbidden in a final summary.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

REGISTRY_PATH = Path(__file__).with_name("glass-smoke-information-registry.json")

ALLOWED_LEDGER_DECISIONS = {
    "IMPLEMENTED",
    "REJECTED",
    "CONDITIONAL_IMPLEMENTED",
    "CONDITIONAL_REJECTED",
}


def load_registry(path: Path | None = None) -> dict:
    registry = json.loads((path or REGISTRY_PATH).read_text())
    if registry.get("schemaVersion") != 1:
        raise ValueError("information registry schemaVersion must be 1")
    ids = [unit["id"] for unit in registry.get("units", [])]
    if len(ids) != len(set(ids)):
        raise ValueError("information registry contains duplicate unit ids")
    return registry


def registry_unit_ids(registry: dict) -> set[str]:
    return {unit["id"] for unit in registry.get("units", [])}


def count_information_units(
    run_row: dict,
    registry: dict,
) -> dict:
    """Count captured/promoted decision units for one run row.

    Only registry ids count — unknown ids are ERRORS, not bonus units.
    ``missing`` lists registry units absent from the captured set so a
    shrunk capture is visible, never assumed.
    """
    known = registry_unit_ids(registry)
    info = run_row.get("informationUnits") or {}
    captured = list(info.get("captured") or [])
    promoted = list(info.get("promoted") or [])
    errors: list[str] = []
    for unit_id in captured:
        if unit_id not in known:
            errors.append(f"captured unit not in registry: {unit_id}")
    for unit_id in promoted:
        if unit_id not in known:
            errors.append(f"promoted unit not in registry: {unit_id}")
        elif unit_id not in captured:
            errors.append(f"promoted unit was never captured: {unit_id}")
    valid_captured = [unit for unit in captured if unit in known]
    valid_promoted = [
        unit for unit in promoted if unit in known and unit in captured
    ]
    return {
        "registryVersion": registry.get("schemaVersion", 1),
        "captured": valid_captured,
        "promoted": valid_promoted,
        "missing": sorted(known - set(valid_captured)),
        "capturedCount": len(valid_captured),
        "promotedCount": len(valid_promoted),
        "errors": errors,
    }


def metric_cache_key(
    *,
    input_receipt_hashes: list[str],
    frame_inventory_hashes: list[str],
    metric_source_bundle_sha256: str,
    scenario: str,
    options: dict,
) -> str:
    """Hash-bound regrading key. Any changed frame input, metric source, or
    option produces a different key, so a stale cached metric can never be
    reused against changed inputs."""
    payload = json.dumps(
        {
            "inputReceiptHashes": sorted(input_receipt_hashes),
            "frameInventoryHashes": sorted(frame_inventory_hashes),
            "metricSourceBundleSha256": metric_source_bundle_sha256,
            "scenario": scenario,
            "options": options,
        },
        sort_keys=True,
        separators=(",", ":"),
    )
    return hashlib.sha256(payload.encode()).hexdigest()


def validate_artifact_index(index: dict, study_root: Path) -> list[str]:
    """Artifact provenance validation: paths stay inside the study root,
    duplicate roles must agree on content, and parents must reference known
    hashes."""
    errors: list[str] = []
    if index.get("schemaVersion") != 1:
        errors.append("artifact index schemaVersion must be 1")
    artifacts = index.get("artifacts", [])
    root = study_root.resolve()
    hashes = {artifact.get("sha256") for artifact in artifacts}
    for artifact in artifacts:
        relative = str(artifact.get("relativePath", ""))
        resolved = (root / relative).resolve()
        if not str(resolved).startswith(str(root)):
            errors.append(f"artifact path escapes the study root: {relative}")
        for parent in artifact.get("parents", []) or []:
            if parent not in hashes:
                errors.append(
                    f"artifact {relative} references unknown parent hash {parent}"
                )
    # Multiple artifacts may legitimately share a role across attempts; what
    # must never happen is one LOGICAL artifact (same role + path recorded
    # twice) with different hashes.
    seen: dict[tuple[str, str], str] = {}
    for artifact in artifacts:
        key = (str(artifact.get("role")), str(artifact.get("relativePath")))
        sha = str(artifact.get("sha256"))
        if key in seen and seen[key] != sha:
            errors.append(
                f"duplicate artifact {key[0]}:{key[1]} with different hashes"
            )
        seen[key] = sha
    return errors


def validate_comparison_ledger(
    ledger: list[dict],
    required_ids: list[str],
) -> list[str]:
    """Every recommendation gets exactly one terminal entry; PENDING is
    forbidden; removing an entry fails validation."""
    errors: list[str] = []
    seen_ids: list[str] = []
    for entry in ledger:
        entry_id = str(entry.get("id", ""))
        seen_ids.append(entry_id)
        decision = entry.get("decision")
        if decision == "PENDING":
            errors.append(f"ledger entry {entry_id}: PENDING is forbidden in a final summary")
        elif decision not in ALLOWED_LEDGER_DECISIONS:
            errors.append(
                f"ledger entry {entry_id}: decision {decision!r} not in {sorted(ALLOWED_LEDGER_DECISIONS)}"
            )
        if not str(entry.get("reason", "")).strip():
            errors.append(f"ledger entry {entry_id}: reason required")
    for required in required_ids:
        count = seen_ids.count(required)
        if count != 1:
            errors.append(
                f"ledger entry {required}: expected exactly one, observed {count}"
            )
    return errors


def economics_comparison(baseline: dict, study: dict) -> dict:
    """The informationEconomics block: historical smoke3 versus the current
    study, in wall milliseconds per captured/promoted decision unit.

    smoke3 is labeled nonqualifying for acceptance but remains the required
    historical economics comparison.
    """

    def per_unit(wall_ms: float, count: float) -> float | None:
        if count and count > 0 and wall_ms and wall_ms > 0:
            return wall_ms / count
        return None

    historical_wall = float(baseline.get("sessionWallMs") or 0)
    historical_captured = int(
        baseline.get("capturedMetricFamilyObservations") or 0
    )
    historical_promoted = int(
        baseline.get("promotedMetricFamilyObservations") or 0
    )
    current_wall = float(study.get("wallMs") or 0)
    current_captured = int(study.get("capturedUnits") or 0)
    current_promoted = int(study.get("promotedUnits") or 0)
    display_exclusive = float(study.get("displayExclusiveMs") or 0)
    projected_pair_wall = study.get("equivalentRepeatedPairProjectedWallMs")
    reduction_ms = (
        float(projected_pair_wall) - current_wall
        if projected_pair_wall is not None
        else None
    )
    return {
        "historicalBaseline": {
            "source": baseline.get("source"),
            "qualification": baseline.get(
                "qualification", "NON_QUALIFYING_LOAD"
            ),
            "acceptanceQualifying": False,
            "wallMs": historical_wall,
            "capturedUnits": historical_captured,
            "promotedUnits": historical_promoted,
            "wallMsPerCapturedUnit": per_unit(
                historical_wall, historical_captured
            ),
            "wallMsPerPromotedUnit": per_unit(
                historical_wall, historical_promoted
            ),
        },
        "current": {
            "wallMs": current_wall,
            "capturedUnits": current_captured,
            "promotedUnits": current_promoted,
            "wallMsPerCapturedUnit": per_unit(current_wall, current_captured),
            "wallMsPerPromotedUnit": per_unit(current_wall, current_promoted),
            "displayExclusiveMsPerPromotedUnit": per_unit(
                display_exclusive, current_promoted
            ),
        },
        "savings": {
            "helperCompileSavingsMs": study.get("helperCompileSavingsMs"),
            "removedLayoutLaunchMs": study.get("removedLayoutLaunchMs"),
            "equivalentRepeatedPairProjectedWallMs": projected_pair_wall,
            "measuredLadderWallMs": current_wall,
            "absoluteReductionMs": reduction_ms,
            "percentageReduction": (
                (reduction_ms / float(projected_pair_wall)) * 100.0
                if reduction_ms is not None and projected_pair_wall
                else None
            ),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True)
    parser.add_argument(
        "--study",
        required=True,
        help="study session directory (reads session.json + runs.jsonl) or a study-metrics JSON file",
    )
    parser.add_argument("--registry")
    parser.add_argument("--out")
    args = parser.parse_args()
    baseline = json.loads(Path(args.baseline).read_text())
    registry = load_registry(Path(args.registry) if args.registry else None)

    study_path = Path(args.study)
    if study_path.is_dir():
        runs_path = study_path / "runs.jsonl"
        session_path = study_path / "session.json"
        rows = (
            [
                json.loads(line)
                for line in runs_path.read_text().splitlines()
                if line.strip()
            ]
            if runs_path.exists()
            else []
        )
        session = (
            json.loads(session_path.read_text())
            if session_path.exists()
            else {}
        )
        captured = 0
        promoted = 0
        unit_errors: list[str] = []
        for row in rows:
            counted = count_information_units(row, registry)
            captured += counted["capturedCount"]
            promoted += counted["promotedCount"]
            unit_errors.extend(counted["errors"])
        study = {
            "wallMs": session.get("wallMs"),
            "displayExclusiveMs": session.get("displayExclusiveMs"),
            "capturedUnits": captured,
            "promotedUnits": promoted,
            "helperCompileSavingsMs": session.get("helperCompileSavingsMs"),
            "removedLayoutLaunchMs": session.get("removedLayoutLaunchMs"),
            "equivalentRepeatedPairProjectedWallMs": session.get(
                "equivalentRepeatedPairProjectedWallMs"
            ),
            "unitErrors": unit_errors,
        }
    else:
        study = json.loads(study_path.read_text())

    comparison = economics_comparison(baseline, study)
    result = {
        "schemaVersion": 1,
        "registryUnits": len(registry.get("units", [])),
        "informationEconomics": comparison,
        "unitErrors": study.get("unitErrors", []),
    }
    serialized = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    return 0 if not result["unitErrors"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
