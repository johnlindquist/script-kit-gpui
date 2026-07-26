#!/usr/bin/env python3
"""Measure capsule/main material relation on every real drag filmstrip frame."""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import math
import statistics
from pathlib import Path

from PIL import Image

# The acceptance metric grades what the user SEES: raw compositor pixels on
# every lifecycle-visible entry frame. Alpha recovery is a secondary,
# NON-GATING diagnostic answering "was the internal material itself stable?"
# (Oracle plan `floating-capsule-entry-material`, step 1 — the old
# MIN_ENTRY_MATERIAL_STABILITY_ALPHA floor excluded the very frames the
# complaint was about, certifying the defect by construction.)
MIN_INTRINSIC_RECOVERY_ALPHA = 0.15
MIN_VISIBLE_ENTRY_ALPHA = 0.85
MIN_VISIBLE_ENTRY_MOTION_FRAMES = 5
MAX_DISPLAYED_ENTRY_DELTA_E00 = 5.0
MAX_CAPSULE_STAGE_RELATION_DRIFT_DELTA_E00 = 5.0


def classify_entry_frame(window_alpha: float, entry_visible: bool) -> str:
    """Alpha-zero semantics, exactly (Oracle step 1):

    - ``alpha == 0`` and nothing visible: the frame exists before the window
      contributes pixels. Not a failure; no color is calculated; the recovery
      function is never called (division by zero has no defined color).
    - ``alpha == 0`` with a visible region: hard failure
      (``visibleRegionAtZeroWindowAlpha``) — the user is looking at pure
      wallpaper where UI should be.
    - ``0 < alpha < MIN_VISIBLE_ENTRY_ALPHA`` with a visible region: hard
      failure of the visible-alpha policy, but the raw displayed color is
      STILL measured — never silently excluded.
    - otherwise: an ordinary measurable visible frame.
    """
    if window_alpha == 0:
        return "precontributing" if not entry_visible else "visibleZeroAlpha"
    if not entry_visible:
        return "precontributing"
    if window_alpha < MIN_VISIBLE_ENTRY_ALPHA:
        return "belowFloor"
    return "measurable"


def alpha_policy_summary(
    visible_alphas: list[tuple[int | None, float]],
    below_floor_sequences: list[int | None],
    zero_alpha_visible_sequences: list[int | None],
    unmeasurable_visible_frames: list[dict],
) -> dict:
    """The explicit visible-entry alpha policy receipt.

    ``visible_alphas`` is ``(sequence, windowAlpha)`` for every
    lifecycle-visible motion frame in sequence order (including below-floor
    and zero-alpha frames — nothing visible is ever dropped from the policy).
    """
    return {
        "requiredMinimumVisibleEntryAlpha": MIN_VISIBLE_ENTRY_ALPHA,
        "firstVisibleEntryAlpha": (
            visible_alphas[0][1] if visible_alphas else None
        ),
        "minimumVisibleEntryAlpha": (
            min(alpha for _, alpha in visible_alphas) if visible_alphas else None
        ),
        "visibleFramesBelowAlphaFloor": below_floor_sequences,
        "visibleZeroAlphaFrames": zero_alpha_visible_sequences,
        "unmeasurableVisibleFrames": unmeasurable_visible_frames,
        "unmeasurableVisibleFrameCount": len(unmeasurable_visible_frames),
        "pass": (
            not below_floor_sequences
            and not zero_alpha_visible_sequences
            and not unmeasurable_visible_frames
        ),
    }


def load_contrast_module():
    source = Path(__file__).with_name("glass-contrast-metrics.py")
    spec = importlib.util.spec_from_file_location("glass_contrast_metrics", source)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def displayed_color_summary(
    frame_rows: list[dict],
    capsule_ids: list[str],
    metrics,
    *,
    minimum_samples: int,
    maximum_gate: float,
    p95_gate: float | None = None,
    relation_drift_gate: float | None = None,
) -> tuple[dict[str, dict], dict[str, tuple], float, float, float, list[str]]:
    """PRIMARY, GATING summary over RAW displayed pixels.

    Every motion row with ``displayedColorEligible`` participates — eligibility
    is lifecycle visibility, NEVER a window-alpha floor. The global maximum
    therefore includes the very first visible frame.
    """
    errors: list[str] = []
    capsules_summary: dict[str, dict] = {}
    settled_reference_labs: list[tuple[float, float, float]] = []
    global_maximum_residual = 0.0
    global_maximum_relation_drift = 0.0
    for capsule_id in capsule_ids:
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
            metrics.rgb_to_lab(tuple(row["displayedMaterialMedianRgb"]))
            for row in settled_rows
        ]
        settled_reference = tuple(
            statistics.median(item[index] for item in settled_labs)
            for index in range(3)
        )
        settled_reference_labs.append(settled_reference)
        settled_relation = statistics.median(
            row["stageDeltaE00"] for row in settled_rows
        )
        residuals: list[float] = []
        relation_drifts: list[float] = []
        for row in frame_rows:
            capsule = next(
                (item for item in row["capsules"] if item["id"] == capsule_id),
                None,
            )
            if capsule is None:
                continue
            actual_lab = metrics.rgb_to_lab(
                tuple(capsule["displayedMaterialMedianRgb"])
            )
            residual = metrics.delta_e_2000(actual_lab, settled_reference)
            capsule["settledReferenceLab"] = settled_reference
            capsule["displayedSettledReferenceDeltaE00"] = residual
            relation_drift = abs(capsule["stageDeltaE00"] - settled_relation)
            capsule["stageRelationDriftDeltaE00"] = relation_drift
            if row["phase"] == "motion" and row.get("displayedColorEligible", True):
                residuals.append(residual)
                relation_drifts.append(relation_drift)
        maximum_residual = max(residuals, default=math.inf)
        maximum_relation_drift = max(relation_drifts, default=math.inf)
        if residuals:
            global_maximum_residual = max(global_maximum_residual, maximum_residual)
            global_maximum_relation_drift = max(
                global_maximum_relation_drift, maximum_relation_drift
            )
        capsule_pass = (
            len(residuals) >= minimum_samples
            and maximum_residual <= maximum_gate
        )
        if p95_gate is not None:
            capsule_pass = capsule_pass and (
                metrics.percentile(residuals, 0.95) <= p95_gate
            )
        if relation_drift_gate is not None:
            capsule_pass = capsule_pass and (
                maximum_relation_drift <= relation_drift_gate
            )
        capsules_summary[capsule_id] = {
            "settledReferenceLab": settled_reference,
            "settledStageRelationDeltaE00": settled_relation,
            "motionSampleCount": len(residuals),
            "motionP95DeltaE00": metrics.percentile(residuals, 0.95),
            "motionMaximumDeltaE00": maximum_residual,
            "maximumStageRelationDriftDeltaE00": maximum_relation_drift,
            "pass": capsule_pass,
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
        capsules_summary,
        settled_references,
        maximum_neighbor_relation_difference,
        global_maximum_residual,
        global_maximum_relation_drift,
        errors,
    )


def intrinsic_material_diagnostic(
    frame_rows: list[dict],
    capsule_ids: list[str],
    metrics,
) -> dict[str, dict]:
    """Secondary, NON-GATING diagnostic on alpha-recovered internal material.

    Answers "was the material itself stable behind the fade?" for frames whose
    window alpha permits meaningful recovery (>= MIN_INTRINSIC_RECOVERY_ALPHA).
    Deliberately carries NO ``pass`` field: acceptance is decided only by the
    displayed-color summary. Missing recovery data is a gap in the diagnostic,
    never an acceptance error.
    """
    diagnostic: dict[str, dict] = {}
    for capsule_id in capsule_ids:
        settled_rows = [
            next(
                (capsule for capsule in row["capsules"] if capsule["id"] == capsule_id),
                None,
            )
            for row in frame_rows
            if row["phase"] == "settled"
        ]
        settled_rows = [
            row
            for row in settled_rows
            if row is not None and row.get("intrinsicMaterialMedianRgb") is not None
        ][-3:]
        if len(settled_rows) != 3:
            diagnostic[capsule_id] = {
                "settledReferenceLab": None,
                "motionSampleCount": 0,
                "motionP95DeltaE00": None,
                "motionMaximumDeltaE00": None,
                "note": "insufficient settled intrinsic samples",
            }
            continue
        settled_labs = [
            metrics.rgb_to_lab(tuple(row["intrinsicMaterialMedianRgb"]))
            for row in settled_rows
        ]
        settled_reference = tuple(
            statistics.median(item[index] for item in settled_labs)
            for index in range(3)
        )
        residuals: list[float] = []
        for row in frame_rows:
            capsule = next(
                (item for item in row["capsules"] if item["id"] == capsule_id),
                None,
            )
            if capsule is None or capsule.get("intrinsicMaterialMedianRgb") is None:
                continue
            actual_lab = metrics.rgb_to_lab(
                tuple(capsule["intrinsicMaterialMedianRgb"])
            )
            residual = metrics.delta_e_2000(actual_lab, settled_reference)
            capsule["intrinsicSettledReferenceDeltaE00"] = residual
            if row["phase"] == "motion" and row.get(
                "intrinsicDiagnosticEligible", False
            ):
                residuals.append(residual)
        diagnostic[capsule_id] = {
            "settledReferenceLab": settled_reference,
            "motionSampleCount": len(residuals),
            "motionP95DeltaE00": (
                metrics.percentile(residuals, 0.95) if residuals else None
            ),
            "motionMaximumDeltaE00": max(residuals, default=None),
        }
    return diagnostic


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
    # Keep EVERY filmstrip frame. Visibility is an annotation the alpha policy
    # and per-frame classification consume — never a silent exclusion filter.
    motion_frames: list[dict] = []
    visible_count = 0
    for frame in frames:
        metric = metric_rows.get(frame.get("sequence"), {})
        row = dict(frame)
        row["_lifecycleMetrics"] = metric
        row["_stageVisible"] = bool(metric.get("stageVisible"))
        row["_footerVisible"] = bool(metric.get("footerVisible"))
        row["_entryVisible"] = row["_stageVisible"] or row["_footerVisible"]
        row["_phase"] = "motion"
        if row["_entryVisible"]:
            visible_count += 1
        motion_frames.append(row)
    if visible_count < 1:
        errors.append(
            f"{scenario_name}: expected visible entry material frames, found {visible_count}"
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
        motion_frames + settled_frames,
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
    # Lifecycle mode is layout-self-contained: geometry comes from the
    # lifecycle receipt's settledLayout, so --receipt is optional there and,
    # when supplied, is recorded as legacy provenance rather than treated as
    # the geometry owner (Oracle plan glass-smoke-harness-max-info, WP2).
    parser.add_argument("--receipt")
    parser.add_argument("--trajectory", default="fast-horizontal")
    parser.add_argument("--lifecycle-receipt")
    parser.add_argument("--scenario", default="main-entry")
    parser.add_argument("--out")
    args = parser.parse_args()

    entry_mode = args.lifecycle_receipt is not None
    if not entry_mode and not args.receipt:
        parser.error("--receipt is required outside lifecycle mode")

    metrics = load_contrast_module()
    errors: list[str] = []
    lifecycle_path: Path | None = None
    legacy_provenance_receipt: str | None = None
    if entry_mode:
        lifecycle_path = Path(args.lifecycle_receipt).resolve()
        lifecycle_receipt = json.loads(lifecycle_path.read_text())
        receipt_path = (
            Path(args.receipt).resolve() if args.receipt else lifecycle_path
        )
        if args.receipt:
            legacy_provenance_receipt = str(receipt_path)
            legacy_receipt = json.loads(receipt_path.read_text())
            legacy_sha = legacy_receipt.get("binarySha256")
            lifecycle_sha = lifecycle_receipt.get("binarySha256")
            if legacy_sha and lifecycle_sha and legacy_sha != lifecycle_sha:
                errors.append(
                    "legacy provenance receipt binarySha256 "
                    f"{legacy_sha} does not match lifecycle receipt "
                    f"binarySha256 {lifecycle_sha}"
                )
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
        if trial is None:
            errors.append(f"required trajectory {args.trajectory!r} is missing")
            frames = []
        else:
            frames = trial.get("filmstrip", {}).get("frames", [])
        appkit = receipt.get("layout", {}).get("fidelity", {}).get("appKit", {})
        capture_bounds = {}
        reference_path = None
        reference_image = None
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
    precontributing_entry_frames: list[dict] = []
    unmeasurable_visible_frames: list[dict] = []
    visible_alphas: list[tuple[int | None, float]] = []
    below_floor_sequences: list[int | None] = []
    zero_alpha_visible_sequences: list[int | None] = []
    for frame in frames:
        image_path = Path(frame.get("path", "")).resolve()
        if not image_path.exists():
            errors.append(f"filmstrip frame missing: {image_path}")
            continue
        image = Image.open(image_path).convert("RGB")
        display_image = image
        intrinsic_image: Image.Image | None = image
        frame_appkit = appkit
        frame_foreground_nodes = foreground_nodes
        frame_capsule_nodes = capsule_nodes
        frame_backdrop = backdrop
        phase = frame.get("_phase") or (
            "motion" if float(frame.get("fraction", 0)) < 1 else "settled"
        )
        window_alpha = float(
            frame["windowAlpha"]
            if frame.get("windowAlpha") is not None
            else (0.0 if entry_mode and phase == "motion" else 1.0)
        )
        entry_visible = bool(frame.get("_entryVisible", True))
        classification = (
            classify_entry_frame(window_alpha, entry_visible)
            if entry_mode and phase == "motion"
            else "measurable"
        )
        if entry_mode and phase == "motion" and entry_visible:
            visible_alphas.append((frame.get("sequence"), window_alpha))
        if classification == "precontributing":
            # The window does not contribute pixels yet. Not a failure; no
            # color exists to grade, and the alpha-recovery function is never
            # called (alpha 0 has no defined intrinsic color).
            precontributing_entry_frames.append(
                {
                    "sequence": frame.get("sequence"),
                    "path": str(image_path),
                    "sha256": frame.get("sha256"),
                    "windowAlpha": window_alpha,
                    "reason": "window not yet contributing pixels",
                }
            )
            continue
        if classification == "visibleZeroAlpha":
            # Hard policy failure: the lifecycle sees a region while the
            # window multiplies every pixel by zero — the user is looking at
            # wallpaper where UI should be. No window color exists to measure.
            zero_alpha_visible_sequences.append(frame.get("sequence"))
            continue
        if classification == "belowFloor":
            # Hard policy failure recorded in the alpha policy — but the raw
            # displayed color is STILL measured below. Never silently excluded.
            below_floor_sequences.append(frame.get("sequence"))
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
                if phase == "motion":
                    # A lifecycle-VISIBLE frame whose geometry cannot be
                    # recovered is a hard failure. It must never be relabeled
                    # as pre-visible — that relabeling was the old metric's
                    # blind spot.
                    unmeasurable_visible_frames.append(
                        {
                            "sequence": frame.get("sequence"),
                            "path": str(image_path),
                            "sha256": frame.get("sha256"),
                            "windowAlpha": window_alpha,
                            "reason": f"unmeasurableVisibleEntryFrame: {error}",
                        }
                    )
                else:
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
            if window_alpha >= MIN_INTRINSIC_RECOVERY_ALPHA:
                try:
                    intrinsic_image = recover_content_before_window_alpha(
                        image,
                        reference_image,
                        window_alpha,
                    )
                except ValueError as error:
                    errors.append(f"{image_path}: {error}")
                    continue
            else:
                # Below the recovery floor, division by alpha amplifies noise
                # past usefulness. The frame still counts for the DISPLAYED
                # acceptance metric; only the intrinsic diagnostic skips it.
                intrinsic_image = None
        scale = image.width / host_width if host_width > 0 else 0
        if scale <= 0:
            continue
        capsules = []
        unmeasured_capsules: list[dict] = []
        for node in frame_capsule_nodes:
            try:
                displayed_capsule = metrics.capsule_metrics(
                    display_image, node, scale, frame_foreground_nodes
                )
            except ValueError as error:
                unmeasured_capsules.append(
                    {"id": node.get("id"), "reason": str(error)}
                )
                continue
            displayed_capsule["displayedMaterialMedianRgb"] = (
                displayed_capsule.pop("materialMedianRgb")
            )
            if intrinsic_image is not None:
                try:
                    intrinsic_capsule = metrics.capsule_metrics(
                        intrinsic_image, node, scale, frame_foreground_nodes
                    )
                    displayed_capsule["intrinsicMaterialMedianRgb"] = (
                        intrinsic_capsule["materialMedianRgb"]
                    )
                except ValueError:
                    displayed_capsule["intrinsicMaterialMedianRgb"] = None
            else:
                displayed_capsule["intrinsicMaterialMedianRgb"] = None
            capsules.append(displayed_capsule)
        if unmeasured_capsules and phase == "settled":
            errors.append(
                f"settled entry frame has unmeasured capsules: {unmeasured_capsules}"
            )
        if entry_mode and phase == "motion" and unmeasured_capsules:
            unmeasurable_visible_frames.append(
                {
                    "sequence": frame.get("sequence"),
                    "path": str(image_path),
                    "sha256": frame.get("sha256"),
                    "windowAlpha": window_alpha,
                    "reason": (
                        "unmeasurableVisibleEntryFrame: capsules "
                        f"{unmeasured_capsules}"
                    ),
                }
            )
        _, stage_y, _, stage_height = metrics.frame_pixels(
            frame_backdrop, scale, image.height
        )
        stage_pixels = [
            display_image.getpixel((x, y))
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
                display_image, capsule, frame_backdrop, scale
            )
            stage_lab = metrics.rgb_to_lab(local_rgb)
            material_lab = metrics.rgb_to_lab(
                tuple(capsule["displayedMaterialMedianRgb"])
            )
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
                "windowAlpha": window_alpha,
                "classification": classification,
                "entryVisible": entry_visible,
                "displayedColorEligible": True,
                "intrinsicDiagnosticEligible": intrinsic_image is not None,
                "phase": phase,
                "path": str(image_path),
                "sha256": frame.get("sha256"),
                "displayedStageMedianRgb": stage_rgb,
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
    if entry_mode:
        (
            displayed_capsules_summary,
            settled_references,
            maximum_neighbor_relation_difference,
            maximum_displayed_entry_delta,
            maximum_relation_drift,
            displayed_errors,
        ) = displayed_color_summary(
            frame_rows,
            capsule_ids,
            metrics,
            minimum_samples=MIN_VISIBLE_ENTRY_MOTION_FRAMES,
            maximum_gate=MAX_DISPLAYED_ENTRY_DELTA_E00,
            relation_drift_gate=MAX_CAPSULE_STAGE_RELATION_DRIFT_DELTA_E00,
        )
    else:
        (
            displayed_capsules_summary,
            settled_references,
            maximum_neighbor_relation_difference,
            maximum_displayed_entry_delta,
            maximum_relation_drift,
            displayed_errors,
        ) = displayed_color_summary(
            frame_rows,
            capsule_ids,
            metrics,
            minimum_samples=1,
            maximum_gate=8.0,
            p95_gate=5.0,
        )
    errors.extend(displayed_errors)
    intrinsic_diagnostic = intrinsic_material_diagnostic(
        frame_rows, capsule_ids, metrics
    )
    alpha_policy = (
        alpha_policy_summary(
            visible_alphas,
            below_floor_sequences,
            zero_alpha_visible_sequences,
            unmeasurable_visible_frames,
        )
        if entry_mode
        else None
    )
    measured_motion_count = sum(row["phase"] == "motion" for row in frame_rows)
    measured_settled_count = sum(row["phase"] == "settled" for row in frame_rows)
    minimum_motion_frames = (
        MIN_VISIBLE_ENTRY_MOTION_FRAMES if entry_mode else 15
    )
    if (
        measured_motion_count < minimum_motion_frames
        or measured_settled_count < 3
    ):
        errors.append(
            f"expected at least {minimum_motion_frames} measured motion + 3 settled "
            f"frames, found {measured_motion_count} + {measured_settled_count}"
        )
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
    shared_pass = (
        not errors
        and measured_motion_count >= minimum_motion_frames
        and measured_settled_count >= 3
        and len(displayed_capsules_summary) == len(capsule_ids)
        and all(row["pass"] for row in displayed_capsules_summary.values())
        and maximum_neighbor_relation_difference <= 6.0
        and boundary_gate_pass
    )
    if entry_mode:
        overall_pass = (
            shared_pass
            and alpha_policy is not None
            and alpha_policy["pass"]
            and measured_settled_count == 3
            and maximum_displayed_entry_delta <= MAX_DISPLAYED_ENTRY_DELTA_E00
            and maximum_relation_drift
            <= MAX_CAPSULE_STAGE_RELATION_DRIFT_DELTA_E00
        )
    else:
        overall_pass = shared_pass
    layout_source = None
    if entry_mode:
        layout_source = {
            "kind": "lifecycle-settled-layout",
            "scenario": args.scenario,
            "lifecycleReceipt": str(lifecycle_path),
            "lifecycleReceiptSha256": hashlib.sha256(
                lifecycle_path.read_bytes()
            ).hexdigest(),
            "legacyProvenanceReceipt": legacy_provenance_receipt,
        }
    result = {
        "schemaVersion": 2,
        "receipt": str(receipt_path),
        "lifecycleReceipt": str(lifecycle_path) if lifecycle_path else None,
        "layoutSource": layout_source,
        "backgroundReference": str(reference_path) if reference_path else None,
        "trajectory": args.scenario if entry_mode else args.trajectory,
        "frameCount": len(frame_rows),
        "motionFrameCount": measured_motion_count,
        "settledFrameCount": measured_settled_count,
        "frames": frame_rows,
        "precontributingEntryFrames": precontributing_entry_frames,
        "summary": {
            "materialStabilityCapsules": displayed_capsules_summary,
            "maximumNeighboringSettledMaterialDeltaE00":
                maximum_neighbor_relation_difference,
            "maximumDisplayedEntryDeltaE00": (
                maximum_displayed_entry_delta if entry_mode else None
            ),
            "maximumCapsuleStageRelationDriftDeltaE00": (
                maximum_relation_drift if entry_mode else None
            ),
            "settledCapsuleMaterialLab": settled_references,
            "intrinsicMaterialDiagnosticCapsules": intrinsic_diagnostic,
            "alphaPolicy": alpha_policy,
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
                "raw-displayed-pixels-every-visible-entry-frame"
                if entry_mode
                else "captured-motion-pixels"
            ),
        },
        "errors": errors,
        "pass": overall_pass,
    }
    serialized = json.dumps(result, indent=2) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
