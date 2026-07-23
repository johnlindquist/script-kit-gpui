#!/usr/bin/env python3
"""Measure capsule/main material relation on every real drag filmstrip frame."""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
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
    motion_frames = [frame for frame in frames if float(frame.get("fraction", 0)) < 1]
    settled_frames = [frame for frame in frames if float(frame.get("fraction", 0)) > 1]
    if len(motion_frames) != 15 or len(settled_frames) != 3:
        errors.append(
            f"expected 15 motion + 3 settled frames, found "
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
            }
        )

    maximum_delta = max(
        (row["maximumStageDeltaE00"] for row in frame_rows), default=math.inf
    )
    maximum_lstar = max(
        (row["maximumStageAbsoluteLStarDifference"] for row in frame_rows),
        default=math.inf,
    )
    motion_relations = [
        row["maximumStageDeltaE00"]
        for row in frame_rows
        if row["phase"] == "motion"
    ]
    motion_relation_range = (
        max(motion_relations) - min(motion_relations)
        if motion_relations
        else math.inf
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
            "maximumStageDeltaE00": maximum_delta,
            "maximumStageAbsoluteLStarDifference": maximum_lstar,
            # AppKit intentionally gives compact glass a fixed adaptive
            # separation from a window-sized backdrop. Motion stability is
            # therefore the bounded change in that relationship, not a false
            # demand that two different glass geometries render identical.
            "motionRelationRangeDeltaE00": motion_relation_range,
        },
        "errors": errors,
        "pass": (
            not errors
            and len(frame_rows) == 18
            and maximum_delta <= 25.0
            and maximum_lstar <= 18.0
            and motion_relation_range <= 10.0
        ),
    }
    serialized = json.dumps(result, indent=2) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
