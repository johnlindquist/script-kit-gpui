#!/usr/bin/env python3
"""Measure capsule/main material relation on every real drag filmstrip frame."""

from __future__ import annotations

import argparse
import copy
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


def adaptive_relation_summary(
    frame_rows: list[dict],
    capsule_ids: list[str],
    metrics,
) -> tuple[dict[str, dict], dict[str, float], float, list[str]]:
    errors: list[str] = []
    adaptive: dict[str, dict] = {}
    settled_relation_scalars: list[float] = []
    for capsule_id in capsule_ids:
        motion_sample_count = sum(
            row["phase"] == "motion"
            and any(
                capsule["id"] == capsule_id for capsule in row["capsules"]
            )
            for row in frame_rows
        )
        settled_rows = [
            next(
                (capsule for capsule in row["capsules"] if capsule["id"] == capsule_id),
                None,
            )
            for row in frame_rows
            if row["phase"] == "settled"
        ]
        settled_rows = [row for row in settled_rows if row is not None][-3:]
        if len(settled_rows) != 3:
            errors.append(f"{capsule_id}: expected three settled relation samples")
            settled_relation_scalars.append(math.inf)
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
        offset = tuple(
            statistics.median(item[index] for item in offsets) for index in range(3)
        )
        residuals = []
        for row in frame_rows:
            capsule = next(
                (item for item in row["capsules"] if item["id"] == capsule_id),
                None,
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
            "motionSampleCount": motion_sample_count,
            "residualP95DeltaE00": metrics.percentile(residuals, 0.95),
            "residualMaximumDeltaE00": max(residuals, default=math.inf),
            "pass": (
                motion_sample_count >= 1
                and metrics.percentile(residuals, 0.95) <= 5.0
                and max(residuals, default=math.inf) <= 8.0
            ),
        }
        settled_values = [row["stageDeltaE00"] for row in settled_rows]
        settled_relation_scalars.append(statistics.median(settled_values))
    settled_relations = dict(zip(capsule_ids, settled_relation_scalars))
    neighboring_relation_differences = [
        abs(left - right)
        for left, right in zip(
            settled_relation_scalars,
            settled_relation_scalars[1:],
        )
    ]
    maximum_neighbor_relation_difference = max(
        neighboring_relation_differences, default=0
    )
    return (
        adaptive,
        settled_relations,
        maximum_neighbor_relation_difference,
        errors,
    )


def boundary_pass_every_frame(frame_rows: list[dict]) -> bool:
    return bool(frame_rows) and all(
        row["minimumMedianBoundaryLuminanceDifference"] >= 0.040
        and row["minimumP10BoundaryLuminanceDifference"] >= 0.015
        and row["minimumFractionAtLeast015"] >= 0.80
        for row in frame_rows
    )


def window_bounds_tuple(value: object) -> tuple[float, float, float, float] | None:
    if (
        not isinstance(value, list)
        or len(value) != 2
        or not all(isinstance(row, list) and len(row) == 2 for row in value)
    ):
        return None
    try:
        return (
            float(value[0][0]),
            float(value[0][1]),
            float(value[1][0]),
            float(value[1][1]),
        )
    except (TypeError, ValueError):
        return None


def transform_appkit_geometry_for_display_frame(
    appkit: dict,
    actual_window_bounds: tuple[float, float, float, float],
    capture_bounds: dict,
    image_size: tuple[int, int],
) -> dict:
    """Project settled AppKit geometry into one fixed display-stream crop.

    AppKit fidelity frames use bottom-left window coordinates. Display-stream
    PNGs use top-left crop pixels, while entry morphs temporarily change the
    window frame. Projecting every node through the actual per-frame window
    bounds keeps rounded masks and foreground exclusions attached to the
    material they measure instead of sampling stale settled coordinates.
    """
    transformed = copy.deepcopy(appkit)
    base_window = appkit.get("windowBounds", {})
    base_width = float(base_window.get("width", 0))
    base_height = float(base_window.get("height", 0))
    capture_x = float(capture_bounds.get("x", 0))
    capture_y = float(capture_bounds.get("y", 0))
    capture_width = float(capture_bounds.get("width", 0))
    capture_height = float(capture_bounds.get("height", 0))
    actual_x, actual_y, actual_width, actual_height = actual_window_bounds
    image_width, image_height = image_size
    if min(
        base_width,
        base_height,
        capture_width,
        capture_height,
        actual_width,
        actual_height,
        image_width,
        image_height,
    ) <= 0:
        raise ValueError("entry geometry has a non-positive window or crop dimension")
    pixel_scale_x = image_width / capture_width
    pixel_scale_y = image_height / capture_height
    if abs(pixel_scale_x - pixel_scale_y) > 0.01:
        raise ValueError("entry display-stream crop has non-uniform pixel scale")
    pixel_scale = (pixel_scale_x + pixel_scale_y) / 2.0
    scale_x = actual_width / base_width
    scale_y = actual_height / base_height

    def project(frame: object) -> dict | None:
        if not isinstance(frame, dict):
            return None
        x = float(frame.get("x", 0))
        y = float(frame.get("y", 0))
        width = float(frame.get("width", 0))
        height = float(frame.get("height", 0))
        pixel_x = (actual_x - capture_x + x * scale_x) * pixel_scale
        pixel_y = (
            actual_y
            - capture_y
            + (base_height - (y + height)) * scale_y
        ) * pixel_scale
        pixel_width = width * scale_x * pixel_scale
        pixel_height = height * scale_y * pixel_scale
        # Convert the projected top-left pixel rect back to the bottom-left
        # logical coordinate shape consumed by glass-contrast-metrics.py.
        return {
            "x": pixel_x / pixel_scale,
            "y": (image_height - pixel_y - pixel_height) / pixel_scale,
            "width": pixel_width / pixel_scale,
            "height": pixel_height / pixel_scale,
        }

    for node in transformed.get("nodes", []):
        projected = project(node.get("screenshotFrame"))
        if projected is not None:
            node["screenshotFrame"] = projected
        layer = node.get("layer")
        if isinstance(layer, dict) and "cornerRadius" in layer:
            layer["cornerRadius"] = float(layer["cornerRadius"]) * min(scale_x, scale_y)
    projected_backdrop = project(transformed.get("mainBackdropFrame"))
    if projected_backdrop is not None:
        transformed["mainBackdropFrame"] = projected_backdrop
    transformed["footerContainerFrame"] = {
        "x": 0,
        "y": 0,
        "width": capture_width,
        "height": capture_height,
    }
    transformed["projectedPixelScale"] = pixel_scale
    return transformed


def lifecycle_entry_frames(
    lifecycle_receipt: dict,
    scenario_name: str,
) -> tuple[list[dict], dict, dict, list[str]]:
    errors: list[str] = []
    scenario = next(
        (
            row
            for row in lifecycle_receipt.get("scenarios", [])
            if row.get("name") == scenario_name
        ),
        None,
    )
    if scenario is None:
        return [], {}, {}, [f"required lifecycle scenario {scenario_name!r} is missing"]
    filmstrip = scenario.get("filmstrip", {})
    frames = filmstrip.get("receipt", {}).get("frames", [])
    metric_rows = {
        row.get("sequence"): row
        for row in filmstrip.get("metrics", {}).get("frames", [])
    }
    visible_frames: list[dict] = []
    for frame in frames:
        metric = metric_rows.get(frame.get("sequence"), {})
        if not metric.get("stageVisible") or not metric.get("footerVisible"):
            continue
        row = dict(frame)
        row["_lifecycleMetrics"] = metric
        visible_frames.append(row)
    if len(visible_frames) < 4:
        errors.append(
            f"{scenario_name}: expected visible entry material frames, found {len(visible_frames)}"
        )
    for index, frame in enumerate(visible_frames):
        frame["_phase"] = (
            "settled" if index >= max(0, len(visible_frames) - 3) else "motion"
        )
    return (
        visible_frames,
        scenario.get("settledLayout", {}).get("fidelity", {}).get("appKit", {}),
        scenario.get("captureBounds", {}),
        errors,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--trajectory", default="fast-horizontal")
    parser.add_argument("--lifecycle-receipt")
    parser.add_argument("--scenario", default="main-entry")
    parser.add_argument("--out")
    args = parser.parse_args()

    metrics = load_contrast_module()
    receipt_path = Path(args.receipt).resolve()
    receipt = json.loads(receipt_path.read_text())
    errors: list[str] = []
    lifecycle_path: Path | None = None
    entry_mode = args.lifecycle_receipt is not None
    if entry_mode:
        lifecycle_path = Path(args.lifecycle_receipt).resolve()
        lifecycle_receipt = json.loads(lifecycle_path.read_text())
        frames, appkit, capture_bounds, lifecycle_errors = lifecycle_entry_frames(
            lifecycle_receipt,
            args.scenario,
        )
        errors.extend(lifecycle_errors)
    else:
        trial = next(
            (
                row
                for row in receipt.get("trials", [])
                if row.get("trajectory") == args.trajectory
            ),
            None,
        )
        if trial is None:
            errors.append(f"required trajectory {args.trajectory!r} is missing")
            frames = []
        else:
            frames = trial.get("filmstrip", {}).get("frames", [])
        appkit = receipt.get("layout", {}).get("fidelity", {}).get("appKit", {})
        capture_bounds = {}
    motion_frames = [
        frame
        for frame in frames
        if frame.get("_phase") == "motion"
        or frame.get("phase") == "motion"
        or (
            frame.get("_phase") is None
            and float(frame.get("fraction", 0)) < 1
        )
    ]
    settled_frames = [
        frame
        for frame in frames
        if frame.get("_phase") == "settled"
        or frame.get("phase") == "settled"
        or (
            frame.get("_phase") is None
            and float(frame.get("fraction", 0)) > 1
        )
    ]
    minimum_motion_frames = 1 if entry_mode else 15
    if len(motion_frames) < minimum_motion_frames or len(settled_frames) < 3:
        errors.append(
            f"expected at least {minimum_motion_frames} motion + 3 settled frames, found "
            f"{len(motion_frames)} + {len(settled_frames)}"
        )

    host_width = float(appkit.get("footerContainerFrame", {}).get("width", 0))
    backdrop = appkit.get("mainBackdropFrame")
    foreground_nodes = appkit.get("nodes", [])
    geometry_receipt = {"layout": {"fidelity": {"appKit": appkit}}}
    capsule_nodes = metrics.node_capsules(geometry_receipt)
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
        frame_appkit = appkit
        frame_foreground_nodes = foreground_nodes
        frame_capsule_nodes = capsule_nodes
        frame_backdrop = backdrop
        if entry_mode:
            actual_bounds = window_bounds_tuple(frame.get("windowBounds"))
            if actual_bounds is None:
                errors.append(f"entry frame missing exact window bounds: {image_path}")
                continue
            try:
                frame_appkit = transform_appkit_geometry_for_display_frame(
                    appkit,
                    actual_bounds,
                    capture_bounds,
                    image.size,
                )
            except ValueError as error:
                errors.append(f"{image_path}: {error}")
                continue
            frame_foreground_nodes = frame_appkit.get("nodes", [])
            frame_capsule_nodes = metrics.node_capsules(
                {"layout": {"fidelity": {"appKit": frame_appkit}}}
            )
            frame_backdrop = frame_appkit.get("mainBackdropFrame")
            host_width = float(
                frame_appkit.get("footerContainerFrame", {}).get("width", 0)
            )
        scale = image.width / host_width if host_width > 0 else 0
        if scale <= 0:
            continue
        capsules = []
        unmeasured_capsules: list[dict] = []
        for node in frame_capsule_nodes:
            try:
                capsules.append(
                    metrics.capsule_metrics(
                        image, node, scale, frame_foreground_nodes
                    )
                )
            except ValueError as error:
                unmeasured_capsules.append(
                    {"id": node.get("id"), "reason": str(error)}
                )
        if unmeasured_capsules and frame.get("_phase") == "settled":
            errors.append(
                f"settled entry frame has unmeasured capsules: {unmeasured_capsules}"
            )
        _, stage_y, _, stage_height = metrics.frame_pixels(
            frame_backdrop, scale, image.height
        )
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
            local_rgb = metrics.local_stage_rgb(
                image, capsule, frame_backdrop, scale
            )
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
                "sequence": frame.get("sequence"),
                "windowBounds": frame.get("windowBounds"),
                "windowAlpha": frame.get("windowAlpha"),
                "phase": frame.get("_phase")
                or ("motion" if float(frame.get("fraction", 0)) < 1 else "settled"),
                "path": str(image_path),
                "sha256": frame.get("sha256"),
                "stageMedianRgb": stage_rgb,
                "capsules": capsules,
                "unmeasuredCapsules": unmeasured_capsules,
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
    (
        adaptive,
        settled_relations,
        maximum_neighbor_relation_difference,
        adaptive_errors,
    ) = adaptive_relation_summary(
        frame_rows,
        capsule_ids,
        metrics,
    )
    errors.extend(adaptive_errors)
    boundary_pass_all_frames = boundary_pass_every_frame(frame_rows)
    # A main-entry display stream intentionally includes the sub-opaque fade
    # before the controls are interactive. Absolute perimeter contrast tends
    # to zero with the window alpha, so it is not a meaningful discoverability
    # gate during those frames. Color coherence remains gated on every visible
    # frame through the adaptive residual. Absolute perimeter thresholds are
    # gated on all three fully settled frames and every raw frame value remains
    # in the receipt for audit.
    boundary_gate_rows = (
        [row for row in frame_rows if row["phase"] == "settled"]
        if entry_mode
        else frame_rows
    )
    boundary_gate_pass = boundary_pass_every_frame(boundary_gate_rows)
    result = {
        "schemaVersion": 1,
        "receipt": str(receipt_path),
        "lifecycleReceipt": str(lifecycle_path) if lifecycle_path else None,
        "trajectory": args.scenario if entry_mode else args.trajectory,
        "frameCount": len(frame_rows),
        "motionFrameCount": sum(row["phase"] == "motion" for row in frame_rows),
        "settledFrameCount": sum(row["phase"] == "settled" for row in frame_rows),
        "frames": frame_rows,
        "summary": {
            "adaptiveCapsules": adaptive,
            "maximumNeighboringSettledRelationDeltaE00":
                maximum_neighbor_relation_difference,
            "settledCapsuleRelationDeltaE00": settled_relations,
            "boundaryPassEveryFrame": boundary_pass_all_frames,
            "boundaryPassEverySettledFrame": boundary_pass_every_frame(
                [row for row in frame_rows if row["phase"] == "settled"]
            ),
            "boundaryGateScope": (
                "settled-opaque-entry-frames"
                if entry_mode
                else "every-motion-and-settled-frame"
            ),
        },
        "errors": errors,
        "pass": (
            not errors
            and len(motion_frames) >= minimum_motion_frames
            and len(settled_frames) >= 3
            and len(adaptive) == len(capsule_ids)
            and all(row["pass"] for row in adaptive.values())
            and maximum_neighbor_relation_difference <= 6.0
            and boundary_gate_pass
        ),
    }
    serialized = json.dumps(result, indent=2) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
