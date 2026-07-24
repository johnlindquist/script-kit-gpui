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

MIN_ANALYZABLE_ENTRY_ALPHA = 0.15
MIN_ENTRY_MATERIAL_STABILITY_ALPHA = 0.85


def load_contrast_module():
    source = Path(__file__).with_name("glass-contrast-metrics.py")
    spec = importlib.util.spec_from_file_location("glass_contrast_metrics", source)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def material_stability_summary(
    frame_rows: list[dict],
    capsule_ids: list[str],
    metrics,
) -> tuple[dict[str, dict], dict[str, float], float, list[str]]:
    errors: list[str] = []
    stability: dict[str, dict] = {}
    settled_reference_labs: list[tuple[float, float, float]] = []
    for capsule_id in capsule_ids:
        motion_sample_count = sum(
            row["phase"] == "motion"
            and row.get("materialStabilityEligible", True)
            and any(
                capsule["id"] == capsule_id for capsule in row["capsules"]
            )
            for row in frame_rows
        )
        raw_motion_sample_count = sum(
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
            errors.append(f"{capsule_id}: expected three settled material samples")
            settled_reference_labs.append((math.inf, math.inf, math.inf))
            continue
        settled_labs = [
            metrics.rgb_to_lab(tuple(row["materialMedianRgb"]))
            for row in settled_rows
        ]
        settled_reference = tuple(
            statistics.median(item[index] for item in settled_labs)
            for index in range(3)
        )
        settled_reference_labs.append(settled_reference)
        residuals = []
        for row in frame_rows:
            capsule = next(
                (item for item in row["capsules"] if item["id"] == capsule_id),
                None,
            )
            if capsule is None:
                continue
            actual_lab = metrics.rgb_to_lab(tuple(capsule["materialMedianRgb"]))
            residual = metrics.delta_e_2000(actual_lab, settled_reference)
            capsule["settledReferenceLab"] = settled_reference
            capsule["settledReferenceDeltaE00"] = residual
            if (
                row["phase"] == "motion"
                and row.get("materialStabilityEligible", True)
            ):
                residuals.append(residual)
        stability[capsule_id] = {
            "settledReferenceLab": settled_reference,
            "rawMotionSampleCount": raw_motion_sample_count,
            "motionSampleCount": motion_sample_count,
            "motionP95DeltaE00": metrics.percentile(residuals, 0.95),
            "motionMaximumDeltaE00": max(residuals, default=math.inf),
            "pass": (
                motion_sample_count >= 1
                and metrics.percentile(residuals, 0.95) <= 5.0
                and max(residuals, default=math.inf) <= 8.0
            ),
        }
    settled_references = dict(zip(capsule_ids, settled_reference_labs))
    neighboring_relation_differences = [
        metrics.delta_e_2000(left, right)
        for left, right in zip(
            settled_reference_labs,
            settled_reference_labs[1:],
        )
    ]
    maximum_neighbor_relation_difference = max(
        neighboring_relation_differences, default=0
    )
    return (
        stability,
        settled_references,
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


def recover_content_before_window_alpha(
    image: Image.Image,
    reference: Image.Image,
    alpha: float,
) -> Image.Image:
    """Undo the owning NSWindow alpha for material-color measurements.

    Entry intentionally fades the one physical window from transparent. Raw
    ScreenCaptureKit pixels therefore move toward the desktop fixture even
    when the backdrop and every capsule keep a stable internal material. Undo
    that *shared* compositor alpha before comparing motion frames with the
    settled material; otherwise the observer reports the intended whole-window
    fade as capsule-only hue drift.
    """
    if image.size != reference.size:
        raise ValueError("entry frame and explicit background reference sizes differ")
    if not math.isfinite(alpha) or alpha <= 0 or alpha > 1:
        raise ValueError(f"entry frame has invalid window alpha {alpha!r}")
    if alpha >= 0.999:
        return image

    inverse_alpha = 1.0 / alpha
    reference_weight = 1.0 - alpha
    recovered = [
        tuple(
            max(
                0,
                min(
                    255,
                    round(
                        (current[channel] - reference_weight * background[channel])
                        * inverse_alpha
                    ),
                ),
            )
            for channel in range(3)
        )
        for current, background in zip(image.getdata(), reference.getdata())
    ]
    result = Image.new("RGB", image.size)
    result.putdata(recovered)
    return result


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


def rendered_window_bounds_from_reference(
    image: Image.Image,
    reference: Image.Image,
    appkit: dict,
    capture_bounds: dict,
) -> tuple[tuple[float, float, float, float], dict]:
    """Recover the rendered window envelope from the exact background frame.

    SCStream delivery and CGWindowListCopyWindowInfo are not compositor-atomic,
    so a separately queried window frame can describe the neighboring display
    frame during an animation. The explicit post-exit reference makes the
    actual rendered stage independently observable in each captured PNG.
    """
    if image.size != reference.size:
        raise ValueError("entry frame and explicit background reference sizes differ")
    base_window = appkit.get("windowBounds", {})
    backdrop = appkit.get("mainBackdropFrame", {})
    base_height = float(base_window.get("height", 0))
    backdrop_height = float(backdrop.get("height", 0))
    capture_x = float(capture_bounds.get("x", 0))
    capture_y = float(capture_bounds.get("y", 0))
    capture_width = float(capture_bounds.get("width", 0))
    capture_height = float(capture_bounds.get("height", 0))
    image_width, image_height = image.size
    if min(
        base_height,
        backdrop_height,
        capture_width,
        capture_height,
        image_width,
        image_height,
    ) <= 0:
        raise ValueError("rendered-stage recovery has non-positive geometry")
    pixel_scale_x = image_width / capture_width
    pixel_scale_y = image_height / capture_height
    if abs(pixel_scale_x - pixel_scale_y) > 0.01:
        raise ValueError("rendered-stage recovery has non-uniform pixel scale")
    pixel_scale = (pixel_scale_x + pixel_scale_y) / 2.0
    image_pixels = image.load()
    reference_pixels = reference.load()
    sample_step = max(1, min(image_width, image_height) // 160)

    def darkened(x: int, y: int) -> bool:
        current = image_pixels[x, y]
        background = reference_pixels[x, y]
        current_luminance = (
            0.2126 * current[0] + 0.7152 * current[1] + 0.0722 * current[2]
        )
        background_luminance = (
            0.2126 * background[0]
            + 0.7152 * background[1]
            + 0.0722 * background[2]
        )
        return background_luminance - current_luminance > 25.0

    sampled_y = list(range(0, image_height, sample_step))
    column_candidates = [
        x
        for x in range(image_width)
        if (
            sum(darkened(x, y) for y in sampled_y) / len(sampled_y)
            >= 0.70
        )
    ]
    if not column_candidates:
        raise ValueError("rendered stage horizontal envelope was not observable")
    left = column_candidates[0]
    right_exclusive = column_candidates[-1] + 1

    sampled_x = list(range(0, image_width, sample_step))
    row_candidates = [
        y
        for y in range(image_height)
        if (
            sum(darkened(x, y) for x in sampled_x) / len(sampled_x)
            >= 0.80
        )
    ]
    if not row_candidates:
        raise ValueError("rendered stage vertical envelope was not observable")
    top = row_candidates[0]
    bottom_exclusive = row_candidates[-1] + 1
    stage_width_pixels = right_exclusive - left
    stage_height_pixels = bottom_exclusive - top
    if (
        stage_width_pixels < image_width * 0.50
        or stage_height_pixels < image_height * 0.50
    ):
        raise ValueError("rendered stage envelope is implausibly small")

    footer_gap = base_height - backdrop_height
    actual_bounds = (
        capture_x + left / pixel_scale,
        capture_y + top / pixel_scale,
        stage_width_pixels / pixel_scale,
        stage_height_pixels / pixel_scale + footer_gap,
    )
    evidence = {
        "method": "explicit-background-reference-darkening-envelope",
        "sampleStepPixels": sample_step,
        "darkeningThreshold": 25.0,
        "minimumColumnFraction": 0.70,
        "minimumRowFraction": 0.80,
        "stageFramePixels": {
            "x": left,
            "y": top,
            "width": stage_width_pixels,
            "height": stage_height_pixels,
        },
        "windowBounds": {
            "x": actual_bounds[0],
            "y": actual_bounds[1],
            "width": actual_bounds[2],
            "height": actual_bounds[3],
        },
    }
    return actual_bounds, evidence


def transform_appkit_geometry_for_display_frame(
    appkit: dict,
    actual_window_bounds: tuple[float, float, float, float],
    capture_bounds: dict,
    image_size: tuple[int, int],
) -> dict:
    """Project settled AppKit geometry into one fixed display-stream crop.

    AppKit fidelity frames use bottom-left window coordinates. Display-stream
    PNGs use top-left crop pixels. During a native NSWindow frame animation,
    CGWindow reports the transient presentation envelope. Footer children keep
    their model sizes: the left capsule is left-anchored, the action cluster is
    right-anchored, and the stage stretches with the window. Reproduce those
    autoresizing rules rather than scaling every child through the envelope.
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

    width_delta = actual_width - base_width
    right_capsule_origins = [
        float(node["screenshotFrame"]["x"])
        for node in appkit.get("nodes", [])
        if (
            node.get("className") == "NSGlassEffectView"
            and str(node.get("id", "")).startswith("script-kit-footer-capsule-")
            and isinstance(node.get("screenshotFrame"), dict)
        )
    ]
    right_anchor_x = min(right_capsule_origins, default=base_width)

    def project(
        frame: object,
        *,
        anchor_right: bool = False,
        stretch_width: bool = False,
        stretch_height: bool = False,
    ) -> dict | None:
        if not isinstance(frame, dict):
            return None
        x = float(frame.get("x", 0))
        y = float(frame.get("y", 0))
        width = float(frame.get("width", 0))
        height = float(frame.get("height", 0))
        projected_width = width + width_delta if stretch_width else width
        projected_height = (
            height + (actual_height - base_height) if stretch_height else height
        )
        projected_x = x + width_delta if anchor_right else x
        pixel_x = (actual_x - capture_x + projected_x) * pixel_scale
        pixel_y = (
            actual_y
            - capture_y
            + (actual_height - (y + projected_height))
        ) * pixel_scale
        pixel_width = projected_width * pixel_scale
        pixel_height = projected_height * pixel_scale
        # Convert the projected top-left pixel rect back to the bottom-left
        # logical coordinate shape consumed by glass-contrast-metrics.py.
        return {
            "x": pixel_x / pixel_scale,
            "y": (image_height - pixel_y - pixel_height) / pixel_scale,
            "width": pixel_width / pixel_scale,
            "height": pixel_height / pixel_scale,
        }

    for node in transformed.get("nodes", []):
        frame = node.get("screenshotFrame")
        frame_x = float(frame.get("x", 0)) if isinstance(frame, dict) else 0
        projected = project(
            frame,
            anchor_right=frame_x >= right_anchor_x,
            stretch_width=str(node.get("id", "")) in {
                "script-kit-footer-effect",
                "script-kit-footer-divider",
            },
        )
        if projected is not None:
            node["screenshotFrame"] = projected
    projected_backdrop = project(
        transformed.get("mainBackdropFrame"),
        stretch_width=True,
        stretch_height=True,
    )
    if projected_backdrop is not None:
        transformed["mainBackdropFrame"] = projected_backdrop
    transformed["footerContainerFrame"] = {
        "x": 0,
        "y": 0,
        "width": capture_width,
        "height": capture_height,
    }
    transformed["projectedPixelScale"] = pixel_scale
    transformed["presentationWindowBounds"] = {
        "x": actual_x,
        "y": actual_y,
        "width": actual_width,
        "height": actual_height,
    }
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
        row["_phase"] = "motion"
        visible_frames.append(row)
    if len(visible_frames) < 1:
        errors.append(
            f"{scenario_name}: expected visible entry material frames, found {len(visible_frames)}"
        )
    settled_frames = []
    for frame in scenario.get("settledCaptures", []):
        row = dict(frame)
        row["_phase"] = "settled"
        settled_frames.append(row)
    if scenario.get("settledCapturesPass") is not True or len(settled_frames) != 3:
        errors.append(
            f"{scenario_name}: expected exactly three valid explicit settled captures"
        )
    return (
        visible_frames + settled_frames,
        scenario.get("settledLayout", {}).get("fidelity", {}).get("appKit", {}),
        scenario.get("captureBounds", {}),
        errors,
    )


def lifecycle_background_reference(
    lifecycle_receipt: dict,
) -> tuple[Path | None, list[str]]:
    scenario = next(
        (
            row
            for row in lifecycle_receipt.get("scenarios", [])
            if row.get("name") == "main-exit"
        ),
        None,
    )
    if scenario is None:
        return None, ["main-exit explicit background reference scenario is missing"]
    reference = scenario.get("hiddenReference", {})
    path = Path(reference.get("path", "")).resolve()
    if (
        scenario.get("hiddenReferencePass") is not True
        or reference.get("captureSource")
        != "explicit-post-exit-display-screenshot"
        or not path.exists()
    ):
        return None, ["valid explicit post-exit background reference is missing"]
    return path, []


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
        reference_path, reference_errors = lifecycle_background_reference(
            lifecycle_receipt
        )
        errors.extend(reference_errors)
        reference_image = (
            Image.open(reference_path).convert("RGB")
            if reference_path is not None
            else None
        )
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
        reference_path = None
        reference_image = None
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
    previsible_entry_frames: list[dict] = []
    for frame in frames:
        image_path = Path(frame.get("path", "")).resolve()
        if not image_path.exists():
            errors.append(f"filmstrip frame missing: {image_path}")
            continue
        image = Image.open(image_path).convert("RGB")
        analysis_image = image
        frame_appkit = appkit
        frame_foreground_nodes = foreground_nodes
        frame_capsule_nodes = capsule_nodes
        frame_backdrop = backdrop
        if entry_mode:
            reported_bounds = window_bounds_tuple(frame.get("windowBounds"))
            if reported_bounds is None:
                errors.append(f"entry frame missing exact window bounds: {image_path}")
                continue
            if reference_image is None:
                continue
            try:
                actual_bounds, rendered_bounds_evidence = (
                    rendered_window_bounds_from_reference(
                        image,
                        reference_image,
                        appkit,
                        capture_bounds,
                    )
                )
                frame_appkit = transform_appkit_geometry_for_display_frame(
                    appkit,
                    actual_bounds,
                    capture_bounds,
                    image.size,
                )
            except ValueError as error:
                window_alpha = float(frame.get("windowAlpha") or 0)
                if (
                    window_alpha <= MIN_ANALYZABLE_ENTRY_ALPHA
                    and "rendered stage" in str(error)
                ):
                    previsible_entry_frames.append(
                        {
                            "sequence": frame.get("sequence"),
                            "path": str(image_path),
                            "sha256": frame.get("sha256"),
                            "windowAlpha": window_alpha,
                            "reason": str(error),
                        }
                    )
                    continue
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
            window_alpha = float(frame.get("windowAlpha") or 0)
            try:
                analysis_image = recover_content_before_window_alpha(
                    image,
                    reference_image,
                    window_alpha,
                )
            except ValueError as error:
                errors.append(f"{image_path}: {error}")
                continue
        scale = image.width / host_width if host_width > 0 else 0
        if scale <= 0:
            continue
        capsules = []
        unmeasured_capsules: list[dict] = []
        for node in frame_capsule_nodes:
            try:
                capsules.append(
                    metrics.capsule_metrics(
                        analysis_image, node, scale, frame_foreground_nodes
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
            analysis_image.getpixel((x, y))
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
                analysis_image, capsule, frame_backdrop, scale
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
                "renderedWindowBounds": (
                    rendered_bounds_evidence if entry_mode else None
                ),
                "windowAlpha": frame.get("windowAlpha"),
                "windowAlphaRemovedForMaterialMetrics": entry_mode,
                "materialStabilityEligible": (
                    not entry_mode
                    or float(frame.get("windowAlpha") or 0)
                    >= MIN_ENTRY_MATERIAL_STABILITY_ALPHA
                ),
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
        material_stability,
        settled_references,
        maximum_neighbor_relation_difference,
        stability_errors,
    ) = material_stability_summary(
        frame_rows,
        capsule_ids,
        metrics,
    )
    errors.extend(stability_errors)
    boundary_pass_all_frames = boundary_pass_every_frame(frame_rows)
    # A main-entry display stream intentionally includes the sub-opaque fade
    # before the controls are interactive. Absolute perimeter contrast tends
    # to zero with the window alpha, so it is not a meaningful discoverability
    # gate during those frames. Color coherence remains gated on every visible
    # frame through the settled-material residual. Absolute perimeter thresholds are
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
        "backgroundReference": str(reference_path) if reference_path else None,
        "trajectory": args.scenario if entry_mode else args.trajectory,
        "frameCount": len(frame_rows),
        "motionFrameCount": sum(row["phase"] == "motion" for row in frame_rows),
        "settledFrameCount": sum(row["phase"] == "settled" for row in frame_rows),
        "frames": frame_rows,
        "previsibleEntryFrames": previsible_entry_frames,
        "summary": {
            "materialStabilityCapsules": material_stability,
            "maximumNeighboringSettledMaterialDeltaE00":
                maximum_neighbor_relation_difference,
            "settledCapsuleMaterialLab": settled_references,
            "boundaryPassEveryFrame": boundary_pass_all_frames,
            "boundaryPassEverySettledFrame": boundary_pass_every_frame(
                [row for row in frame_rows if row["phase"] == "settled"]
            ),
            "boundaryGateScope": (
                "settled-opaque-entry-frames"
                if entry_mode
                else "every-motion-and-settled-frame"
            ),
            "materialMetricScope": (
                f"owning-window-alpha-normalized-at-or-above-{MIN_ENTRY_MATERIAL_STABILITY_ALPHA:.2f}"
                if entry_mode
                else "captured-motion-pixels"
            ),
        },
        "errors": errors,
        "pass": (
            not errors
            and len(motion_frames) >= minimum_motion_frames
            and len(settled_frames) >= 3
            and len(material_stability) == len(capsule_ids)
            and all(row["pass"] for row in material_stability.values())
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
