#!/usr/bin/env python3
"""Measure capsule/main material relation on every real drag filmstrip frame."""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
import statistics
from pathlib import Path

from PIL import Image


def load_contrast_module():
    source = Path(__file__).with_name("glass-contrast-metrics.py")
    spec = importlib.util.spec_from_file_location("glass_contrast_metrics", source)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--trajectory", default="fast-horizontal")
    parser.add_argument("--out")
    args = parser.parse_args()

    metrics = load_contrast_module()
    receipt_path = Path(args.receipt).resolve()
    receipt = json.loads(receipt_path.read_text())
    trial = next(
        (
            row
            for row in receipt.get("trials", [])
            if row.get("trajectory") == args.trajectory
        ),
        None,
    )
    errors: list[str] = []
    if trial is None:
        errors.append(f"required trajectory {args.trajectory!r} is missing")
        frames = []
    else:
        frames = trial.get("filmstrip", {}).get("frames", [])
    motion_frames = [frame for frame in frames if frame.get("phase") == "motion" or float(frame.get("fraction", 0)) < 1]
    settled_frames = [frame for frame in frames if frame.get("phase") == "settled" or float(frame.get("fraction", 0)) > 1]
    if len(motion_frames) < 15 or len(settled_frames) < 3:
        errors.append(
            f"expected at least 15 motion + 3 settled frames, found "
            f"{len(motion_frames)} + {len(settled_frames)}"
        )

    appkit = receipt.get("layout", {}).get("fidelity", {}).get("appKit", {})
    host_width = float(appkit.get("footerContainerFrame", {}).get("width", 0))
    backdrop = appkit.get("mainBackdropFrame")
    foreground_nodes = appkit.get("nodes", [])
    capsule_nodes = metrics.node_capsules(receipt)
    if host_width <= 0 or not backdrop:
        errors.append("exact AppKit host/backdrop geometry is missing")
    if len(capsule_nodes) < 2:
        errors.append(f"expected at least two visible capsules, found {len(capsule_nodes)}")

    frame_rows: list[dict] = []
    for frame in frames:
        image_path = Path(frame.get("path", "")).resolve()
        if not image_path.exists():
            errors.append(f"filmstrip frame missing: {image_path}")
            continue
        image = Image.open(image_path).convert("RGB")
        scale = image.width / host_width if host_width > 0 else 0
        if scale <= 0:
            continue
        capsules = [
            metrics.capsule_metrics(image, node, scale, foreground_nodes)
            for node in capsule_nodes
        ]
        _, stage_y, _, stage_height = metrics.frame_pixels(backdrop, scale, image.height)
        stage_pixels = [
            image.getpixel((x, y))
            for y in range(
                stage_y + max(8, stage_height - round(40 * scale)),
                stage_y + stage_height - 8,
            )
            for x in range(
                round(20 * scale),
                image.width - round(20 * scale),
                max(1, round(2 * scale)),
            )
        ]
        if not stage_pixels:
            errors.append(f"stage sample empty for {image_path}")
            continue
        stage_rgb = metrics.median_rgb(stage_pixels)
        for capsule in capsules:
            local_rgb = metrics.local_stage_rgb(image, capsule, backdrop, scale)
            stage_lab = metrics.rgb_to_lab(local_rgb)
            material_lab = metrics.rgb_to_lab(tuple(capsule["materialMedianRgb"]))
            capsule["stageMedianRgb"] = local_rgb
            capsule["stageDeltaE00"] = metrics.delta_e_2000(material_lab, stage_lab)
            capsule["stageAbsoluteLStarDifference"] = abs(
                material_lab[0] - stage_lab[0]
            )
        frame_rows.append(
            {
                "fraction": frame.get("fraction"),
                "phase": "motion" if float(frame.get("fraction", 0)) < 1 else "settled",
                "path": str(image_path),
                "sha256": frame.get("sha256"),
                "stageMedianRgb": stage_rgb,
                "capsules": capsules,
                "maximumStageDeltaE00": max(
                    (capsule["stageDeltaE00"] for capsule in capsules),
                    default=math.inf,
                ),
                "maximumStageAbsoluteLStarDifference": max(
                    (
                        capsule["stageAbsoluteLStarDifference"]
                        for capsule in capsules
                    ),
                    default=math.inf,
                ),
                "minimumMedianBoundaryLuminanceDifference": min(
                    (capsule["medianBoundaryLuminanceDifference"] for capsule in capsules),
                    default=0,
                ),
                "minimumP10BoundaryLuminanceDifference": min(
                    (capsule["p10BoundaryLuminanceDifference"] for capsule in capsules),
                    default=0,
                ),
                "minimumFractionAtLeast015": min(
                    (capsule["fractionAtLeast015"] for capsule in capsules),
                    default=0,
                ),
            }
        )

    capsule_ids = [str(node.get("id")) for node in capsule_nodes]
    adaptive: dict[str, dict] = {}
    for capsule_id in capsule_ids:
        settled_rows = [
            next((capsule for capsule in row["capsules"] if capsule["id"] == capsule_id), None)
            for row in frame_rows
            if row["phase"] == "settled"
        ]
        settled_rows = [row for row in settled_rows if row is not None][-3:]
        if len(settled_rows) != 3:
            errors.append(f"{capsule_id}: expected three settled relation samples")
            continue
        offsets = [
            tuple(
                material - stage
                for material, stage in zip(
                    metrics.rgb_to_lab(tuple(row["materialMedianRgb"])),
                    metrics.rgb_to_lab(tuple(row["stageMedianRgb"])),
                )
            )
            for row in settled_rows
        ]
        offset = tuple(statistics.median(item[index] for item in offsets) for index in range(3))
        residuals = []
        for row in frame_rows:
            capsule = next(
                (item for item in row["capsules"] if item["id"] == capsule_id), None
            )
            if capsule is None:
                continue
            stage_lab = metrics.rgb_to_lab(tuple(capsule["stageMedianRgb"]))
            expected_lab = tuple(stage_lab[index] + offset[index] for index in range(3))
            actual_lab = metrics.rgb_to_lab(tuple(capsule["materialMedianRgb"]))
            residual = metrics.delta_e_2000(actual_lab, expected_lab)
            capsule["adaptiveExpectedLab"] = expected_lab
            capsule["adaptiveResidualDeltaE00"] = residual
            residuals.append(residual)
        adaptive[capsule_id] = {
            "settledOffsetLab": offset,
            "residualP95DeltaE00": metrics.percentile(residuals, 0.95),
            "residualMaximumDeltaE00": max(residuals, default=math.inf),
            "pass": (
                metrics.percentile(residuals, 0.95) <= 5.0
                and max(residuals, default=math.inf) <= 8.0
            ),
        }
    settled_offsets = [tuple(row["settledOffsetLab"]) for row in adaptive.values()]
    neighboring_relation_differences = [
        metrics.delta_e_2000(
            (50 + left[0], left[1], left[2]),
            (50 + right[0], right[1], right[2]),
        )
        for left, right in zip(settled_offsets, settled_offsets[1:])
    ]
    maximum_neighbor_relation_difference = max(
        neighboring_relation_differences, default=0
    )
    boundary_pass = all(
        row["minimumMedianBoundaryLuminanceDifference"] >= 0.040
        and row["minimumP10BoundaryLuminanceDifference"] >= 0.015
        and row["minimumFractionAtLeast015"] >= 0.80
        for row in frame_rows
    )
    result = {
        "schemaVersion": 1,
        "receipt": str(receipt_path),
        "trajectory": args.trajectory,
        "frameCount": len(frame_rows),
        "motionFrameCount": sum(row["phase"] == "motion" for row in frame_rows),
        "settledFrameCount": sum(row["phase"] == "settled" for row in frame_rows),
        "frames": frame_rows,
        "summary": {
            "adaptiveCapsules": adaptive,
            "maximumNeighboringSettledRelationDeltaE00":
                maximum_neighbor_relation_difference,
            "boundaryPassEveryFrame": boundary_pass,
        },
        "errors": errors,
        "pass": (
            not errors
            and len(motion_frames) >= 15
            and len(settled_frames) >= 3
            and len(adaptive) == len(capsule_ids)
            and all(row["pass"] for row in adaptive.values())
            and maximum_neighbor_relation_difference <= 6.0
            and boundary_pass
        ),
    }
    serialized = json.dumps(result, indent=2) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
