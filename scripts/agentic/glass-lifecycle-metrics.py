#!/usr/bin/env python3
"""Pixel and geometry analysis for exact-window glass lifecycle filmstrips."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

from PIL import Image, ImageChops, ImageFilter, ImageStat


def alpha_profile(image: Image.Image) -> list[float]:
    alpha = image.convert("RGBA").getchannel("A")
    width = alpha.width
    return [
        sum(alpha.crop((0, y, width, y + 1)).getdata()) / (255.0 * width)
        for y in range(alpha.height)
    ]


def center_edge_energy(image: Image.Image) -> float:
    gray = image.convert("L")
    left, top = round(gray.width * 0.12), round(gray.height * 0.16)
    right, bottom = round(gray.width * 0.88), round(gray.height * 0.78)
    edges = gray.crop((left, top, right, bottom)).filter(ImageFilter.FIND_EDGES)
    return float(ImageStat.Stat(edges).mean[0])


def changed_row_occupancies(
    image: Image.Image, reference: Image.Image, channel_tolerance: int = 1
) -> list[float]:
    """Fraction of pixels changed from the hidden-background reference per row."""
    difference = ImageChops.difference(image.convert("RGB"), reference.convert("RGB"))
    red, green, blue = difference.split()
    maximum = ImageChops.lighter(ImageChops.lighter(red, green), blue)
    changed = maximum.point(
        lambda value: 255 if value > channel_tolerance else 0,
        mode="L",
    )
    row_means = changed.resize((1, changed.height), Image.Resampling.BOX)
    return [value / 255.0 for value in row_means.getdata()]


def contiguous_runs(indices: list[int]) -> list[tuple[int, int]]:
    if not indices:
        return []
    runs: list[tuple[int, int]] = []
    start = previous = indices[0]
    for index in indices[1:]:
        if index == previous + 1:
            previous = index
            continue
        runs.append((start, previous))
        start = previous = index
    runs.append((start, previous))
    return runs


def exit_geometry_rows(
    rows: list[dict],
    expected_exit_frame: tuple[float, float, float, float],
) -> list[dict]:
    """Discard pre-exit entry settling, then retain every subsequent frame.

    CGWindow bounds use a top-left display origin while NSWindow receipts use
    a bottom-left display origin, so x/width/height are the shared exact
    coordinates. Once the captured owner reaches the ticket's original frame,
    every later captured frame remains in the geometry proof; a later resize
    therefore still fails rather than being filtered away.
    """
    expected_x, _, expected_width, expected_height = expected_exit_frame
    first_exact = next(
        (
            index
            for index, row in enumerate(rows)
            if row.get("windowBounds") is not None
            and abs(float(row["windowBounds"][0][0]) - expected_x) <= 0.25
            and abs(float(row["windowBounds"][1][0]) - expected_width) <= 0.25
            and abs(float(row["windowBounds"][1][1]) - expected_height) <= 0.25
        ),
        None,
    )
    return rows[first_exact:] if first_exact is not None else []


def frame_crop_box(
    window_bounds: object,
    capture_bounds: tuple[float, float, float, float],
    capture_scale: float,
    image_size: tuple[int, int],
) -> tuple[int, int, int, int] | None:
    if (
        not isinstance(window_bounds, list)
        or len(window_bounds) != 2
        or not all(isinstance(row, list) and len(row) == 2 for row in window_bounds)
    ):
        return None
    capture_x, capture_y, _, _ = capture_bounds
    window_x = float(window_bounds[0][0])
    window_y = float(window_bounds[0][1])
    window_width = float(window_bounds[1][0])
    window_height = float(window_bounds[1][1])
    left = max(0, round((window_x - capture_x) * capture_scale))
    top = max(0, round((window_y - capture_y) * capture_scale))
    right = min(image_size[0], round(left + window_width * capture_scale))
    bottom = min(image_size[1], round(top + window_height * capture_scale))
    if right <= left or bottom <= top:
        return None
    return (left, top, right, bottom)


def classify_main_frame(
    image: Image.Image,
    reference: Image.Image,
    *,
    channel_tolerance: int = 16,
    changed_threshold: float = 0.0,
    component_threshold: float = 0.08,
    adjacency_rows: int = 12,
    minimum_gap_rows: int = 8,
) -> dict:
    """Find the full-width transparent run separating stage and footer.

    The window scales during entry, so a fixed y-coordinate is not evidence.
    Instead, compare every captured row with a hidden-background reference and
    require a transparent run with substantial changed pixels immediately
    above *and* below it. The native footer capsules are inset inside their
    transparent host, so the rendered desktop run can be wider than the exact
    structural gutter. AppKit geometry proves the gutter is exactly 8 points;
    this pixel check proves that at least those 8 points remain transparent and
    that visible stage/footer material stays disconnected. This fails closed
    when the footer disappears while the main stage is still visible.
    """
    occupancies = changed_row_occupancies(
        image,
        reference,
        channel_tolerance=channel_tolerance,
    )
    height = len(occupancies)
    lower_start = round(height * 0.50)
    transparent_rows = [
        index
        for index in range(lower_start, height)
        if occupancies[index] <= changed_threshold
    ]
    candidates: list[dict] = []
    for start, end in contiguous_runs(transparent_rows):
        above = occupancies[max(0, start - adjacency_rows) : start]
        below = occupancies[end + 1 : min(height, end + 1 + adjacency_rows)]
        above_max = max(above, default=0.0)
        below_max = max(below, default=0.0)
        if (
            end - start + 1 >= minimum_gap_rows
            and above_max >= component_threshold
            and below_max >= component_threshold
        ):
            candidates.append(
                {
                    "start": start,
                    "end": end,
                    "height": end - start + 1,
                    "maximumChangedFraction": max(
                        occupancies[start : end + 1], default=0.0
                    ),
                    "aboveChangedFraction": above_max,
                    "belowChangedFraction": below_max,
                }
            )
    gutter = max(candidates, key=lambda candidate: candidate["height"], default=None)
    stage_region = occupancies[: round(height * 0.88)]
    stage_occupancy = max(stage_region, default=0.0)
    stage_visible = stage_occupancy >= component_threshold
    footer_region = occupancies[round(height * 0.72) :]
    footer_occupancy = max(footer_region, default=0.0)
    footer_visible = footer_occupancy >= component_threshold
    disconnected = gutter is not None
    active = stage_visible or footer_visible
    broad_bridge_pass = not active or (
        stage_visible and footer_visible and disconnected
    )
    return {
        "changedRowOccupancies": occupancies,
        "maximumChangedFraction": max(occupancies, default=0.0),
        "stageChangedFraction": stage_occupancy,
        "footerChangedFraction": footer_occupancy,
        "stageVisible": stage_visible,
        "footerVisible": footer_visible,
        "gutterRun": gutter,
        "stageFooterDisconnected": disconnected,
        "broadBridgePass": broad_bridge_pass,
        "footerMissingWhileStageVisible": stage_visible and not disconnected,
        "footerOrphanedAfterStageExit": footer_visible and not stage_visible,
    }


def analyze(
    receipt: dict,
    scenario: str,
    body_bounds: tuple[float, float, float, float] | None = None,
    visible_host_time_ns: int | None = None,
    expected_exit_frame: tuple[float, float, float, float] | None = None,
    capture_bounds: tuple[float, float, float, float] | None = None,
    reference_image_path: Path | None = None,
) -> dict:
    errors: list[str] = []
    rows = []
    loaded_frames: list[tuple[dict, Image.Image]] = []
    for frame in receipt.get("frames", []):
        path = Path(frame.get("path", ""))
        if not path.exists():
            errors.append(f"missing frame {path}")
            continue
        image = Image.open(path).convert("RGBA")
        loaded_frames.append((frame, image))
        profile = alpha_profile(image)
        lower_start = round(image.height * 0.62)
        rows.append(
            {
                "sequence": frame.get("sequence"),
                "displayTimeNs": frame.get("displayTimeNs"),
                "windowBounds": frame.get("windowBounds"),
                "windowAlpha": frame.get("windowAlpha"),
                "windowOnscreen": frame.get("windowOnscreen"),
                "meanAlpha": sum(profile) / len(profile),
                "minimumLowerAlphaOccupancy": min(profile[lower_start:], default=1),
                "centerEdgeEnergy": center_edge_energy(image),
            }
        )
    if len(rows) < 4:
        errors.append(f"only {len(rows)}/4 analyzable frames")
    geometry_rows = rows
    if "exit" in scenario or "close-before-settle" in scenario:
        if expected_exit_frame is not None:
            geometry_rows = exit_geometry_rows(rows, expected_exit_frame)
            if not geometry_rows:
                errors.append(
                    "filmstrip never reached the native exit ticket's original frame"
                )
        else:
            first_fading = next(
                (
                    index
                    for index, row in enumerate(rows)
                    if row["windowAlpha"] is not None
                    and float(row["windowAlpha"]) < 0.999
                ),
                None,
            )
            if first_fading is not None:
                end = next(
                    (
                        index
                        for index in range(first_fading + 1, len(rows))
                        if rows[index]["windowAlpha"] is not None
                        and float(rows[index]["windowAlpha"]) >= 0.999
                    ),
                    len(rows),
                )
                geometry_rows = rows[first_fading:end]
            else:
                geometry_rows = [
                    row for row in rows if row["windowBounds"] is not None
                ]
    bounds = [
        json.dumps(row["windowBounds"], sort_keys=True)
        for row in geometry_rows
        if row["windowBounds"] is not None
    ]
    geometry_stable = bool(bounds) and len(set(bounds)) == 1
    if scenario != "main-entry" and scenario != "notes-entry" and not geometry_stable:
        errors.append("exit geometry changed by more than the fixed-frame contract")
    frame_classifications: list[dict] = []
    gutter_pass = True
    reference_source: str | None = None
    if scenario.startswith("main-") and loaded_frames:
        same_stream_reference = next(
            (
                (frame, image)
                for frame, image in reversed(loaded_frames)
                if scenario == "main-exit"
                and frame.get("windowBounds") is None
                and frame.get("windowAlpha") is None
            ),
            None,
        )
        if same_stream_reference is not None:
            reference_frame, reference_image = same_stream_reference
            reference_image = reference_image.convert("RGB")
            reference_source = (
                f"{reference_frame.get('path')}#same-stream-owner-absent"
            )
        elif reference_image_path is not None and reference_image_path.exists():
            reference_image = Image.open(reference_image_path).convert("RGB")
            reference_source = str(reference_image_path)
        else:
            reference_image = (
                loaded_frames[0][1]
                if scenario == "main-entry"
                else loaded_frames[-1][1]
            ).convert("RGB")
            reference_source = (
                "first-filmstrip-frame"
                if scenario == "main-entry"
                else "last-filmstrip-frame"
            )
        if reference_image.size != loaded_frames[0][1].size:
            errors.append(
                "background reference dimensions do not match the filmstrip frames"
            )
            reference_image = loaded_frames[-1][1].convert("RGB")
            reference_source += " (dimension-mismatch fallback)"
        for index, ((_, image), row) in enumerate(zip(loaded_frames, rows)):
            analysis_image = image
            analysis_reference = reference_image
            analysis_crop = None
            if capture_bounds is not None:
                analysis_crop = frame_crop_box(
                    row["windowBounds"],
                    capture_bounds,
                    float(receipt.get("captureScale", 1)),
                    image.size,
                )
                if analysis_crop is not None:
                    analysis_image = image.crop(analysis_crop)
                    analysis_reference = reference_image.crop(analysis_crop)
            classification = classify_main_frame(
                analysis_image,
                analysis_reference,
                channel_tolerance=1,
                changed_threshold=0.0,
                minimum_gap_rows=round(8 * float(receipt.get("captureScale", 1))),
            )
            classification["analysisCropPixels"] = analysis_crop
            classification["sequence"] = row["sequence"]
            classification["referenceSource"] = reference_source
            frame_classifications.append(classification)
            rows[index].update(
                {
                    key: value
                    for key, value in classification.items()
                    if key != "changedRowOccupancies"
                }
            )
        active = [
            classification
            for classification in frame_classifications
            if classification["stageVisible"] or classification["footerVisible"]
        ]
        gutter_pass = bool(active) and all(
            classification["stageVisible"]
            and classification["footerVisible"]
            and classification["stageFooterDisconnected"]
            and classification["broadBridgePass"]
            for classification in active
        )
    if scenario.startswith("main-") and not gutter_pass:
        failing_sequences = [
            str(classification["sequence"])
            for classification in frame_classifications
            if classification["footerMissingWhileStageVisible"]
            or classification["footerOrphanedAfterStageExit"]
        ]
        errors.append(
            "transparent footer gutter was not preserved while the main stage "
            f"was visible (frames {', '.join(failing_sequences) or 'none classified'})"
        )
    distinct_visual_states = len({
        str(frame.get("sha256", ""))
        for frame in receipt.get("frames", [])
        if frame.get("sha256")
    })
    alpha_progression_pass = distinct_visual_states >= 2
    if not alpha_progression_pass:
        errors.append("filmstrip does not contain two measurable visual states")
    edge_values = [row["centerEdgeEnergy"] for row in rows]
    body_pixel_transition = (
        max(edge_values[-max(1, len(edge_values) // 3):], default=0)
        > min(edge_values[:max(1, len(edge_values) // 3)], default=math.inf) + 0.20
    )
    body_mask_pass = scenario != "notes-entry"
    body_mask_receipt = None
    if scenario == "notes-entry" and body_bounds is not None and loaded_frames:
        scale = float(receipt.get("captureScale", 1))
        x, y, width, height = body_bounds
        crop_box = (
            round(x * scale),
            round(y * scale),
            round((x + width) * scale),
            round((y + height) * scale),
        )
        body_rows = []
        for frame, image in loaded_frames:
            body = image.crop(crop_box)
            energy = float(
                ImageStat.Stat(
                    body.convert("L").filter(ImageFilter.FIND_EDGES)
                ).mean[0]
            )
            body_rows.append(
                {
                    "sequence": frame.get("sequence"),
                    "displayTimeNs": frame.get("displayTimeNs"),
                    "edgeEnergy": energy,
                    "hiddenAtCapture": visible_host_time_ns is not None
                    and int(frame.get("displayTimeNs", 0)) < visible_host_time_ns,
                }
            )
        hidden_energy = [
            row["edgeEnergy"] for row in body_rows if row["hiddenAtCapture"]
        ]
        visible_energy = [
            row["edgeEnergy"] for row in body_rows if not row["hiddenAtCapture"]
        ]
        pre_reveal_chrome_energy = []
        title_bottom = max(1, crop_box[1])
        for frame, image in loaded_frames:
            if (
                visible_host_time_ns is not None
                and int(frame.get("displayTimeNs", 0)) < visible_host_time_ns
            ):
                chrome = image.crop((0, 0, image.width, title_bottom))
                pre_reveal_chrome_energy.append(
                    float(
                        ImageStat.Stat(
                            chrome.convert("L").filter(ImageFilter.FIND_EDGES)
                        ).mean[0]
                    )
                )
        max_hidden = max(hidden_energy) if hidden_energy else None
        min_visible = min(visible_energy) if visible_energy else None
        max_visible = max(visible_energy) if visible_energy else None
        visible_transition_rows = [
            row
            for row in body_rows
            if not row["hiddenAtCapture"]
            and max_hidden is not None
            and row["edgeEnergy"] > max_hidden + 0.20
        ]
        first_visible_transition_ns = (
            int(visible_transition_rows[0]["displayTimeNs"])
            if visible_transition_rows
            else None
        )
        refresh_rate_hz = max(1.0, float(receipt.get("refreshRateHz", 60.0)))
        visible_transition_limit_ns = int(
            (4.0 * 1_000_000_000.0 / refresh_rate_hz) + 20_000_000.0
        )
        visible_transition_latency_ns = (
            first_visible_transition_ns - visible_host_time_ns
            if first_visible_transition_ns is not None
            and visible_host_time_ns is not None
            else None
        )
        chrome_visible = min(pre_reveal_chrome_energy, default=0) > 0.20
        body_mask_pass = (
            len(hidden_energy) >= 2
            and len(visible_energy) >= 1
            and max_hidden is not None
            and max_visible is not None
            and max_visible > max_hidden + 0.20
            and visible_transition_latency_ns is not None
            and 0 <= visible_transition_latency_ns <= visible_transition_limit_ns
            and chrome_visible
        )
        body_mask_receipt = {
            "boundsPoints": {
                "x": x,
                "y": y,
                "width": width,
                "height": height,
            },
            "boundsPixels": crop_box,
            "visibleHostTimeNs": visible_host_time_ns,
            "frames": body_rows,
            "hiddenFrameCount": len(hidden_energy),
            "visibleFrameCount": len(visible_energy),
            "maximumHiddenBodyEdgeEnergy": max_hidden,
            "minimumVisibleBodyEdgeEnergy": min_visible,
            "maximumVisibleBodyEdgeEnergy": max_visible,
            "firstVisibleTransitionHostTimeNs": first_visible_transition_ns,
            "visibleTransitionLatencyNs": visible_transition_latency_ns,
            "visibleTransitionLimitNs": visible_transition_limit_ns,
            "preRevealChromeEdgeEnergy": pre_reveal_chrome_energy,
            "preRevealChromeVisible": chrome_visible,
        }
        if not body_mask_pass:
            errors.append(
                "Notes body mask did not prove hidden body text with visible chrome"
            )
    return {
        "schemaVersion": 1,
        "scenario": scenario,
        "frameCount": len(rows),
        "frames": rows,
        "geometryStable": geometry_stable,
        "geometryStateCount": len(set(bounds)),
        "expectedExitFrame": expected_exit_frame,
        "captureBounds": capture_bounds,
        "gutterPass": gutter_pass,
        "gutterReference": {
            "method":
                "display-stream per-row comparison against a same-stream "
                "owner-absent background when available; "
                "an at-least-8pt fully unchanged run must be bounded by >=8% "
                "changed stage/footer rows; exact 8pt gutter geometry is proven "
                "separately by the AppKit layout receipt",
            "referenceSource": reference_source,
            "explicitPostExitReference": (
                str(reference_image_path)
                if reference_image_path is not None
                else None
            ),
            "channelTolerance": 1,
            "changedRowThreshold": 0.0,
            "componentThreshold": 0.08,
            "adjacencyRows": 12,
            "minimumGapRows": round(8 * float(receipt.get("captureScale", 1))),
            "stageVisibleFrameCount": sum(
                classification["stageVisible"]
                for classification in frame_classifications
            ),
            "minimumBoundedGapHeightPixels": min(
                (
                    classification["gutterRun"]["height"]
                    for classification in frame_classifications
                    if classification["stageVisible"]
                    and classification["gutterRun"] is not None
                ),
                default=None,
            ),
            "footerMissingWhileStageVisible": any(
                classification["footerMissingWhileStageVisible"]
                for classification in frame_classifications
            ),
            "footerOrphanedAfterStageExit": any(
                classification["footerOrphanedAfterStageExit"]
                for classification in frame_classifications
            ),
            "stageFooterDisconnected": gutter_pass,
            "broadBridgePass": gutter_pass,
        }
        if scenario.startswith("main-")
        else None,
        "alphaProgressionPass": alpha_progression_pass,
        "distinctVisualStates": distinct_visual_states,
        "bodyPixelTransition": body_pixel_transition,
        "bodyMaskPass": body_mask_pass,
        "bodyMask": body_mask_receipt,
        "errors": errors,
        "pass": not errors,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--scenario", required=True)
    parser.add_argument("--out")
    parser.add_argument("--body-bounds", nargs=4, type=float)
    parser.add_argument("--visible-host-time-ns", type=int)
    parser.add_argument("--expected-exit-frame", nargs=4, type=float)
    parser.add_argument("--capture-bounds", nargs=4, type=float)
    parser.add_argument("--reference-image")
    args = parser.parse_args()
    receipt = json.loads(Path(args.receipt).read_text())
    result = analyze(
        receipt,
        args.scenario,
        tuple(args.body_bounds) if args.body_bounds else None,
        args.visible_host_time_ns,
        tuple(args.expected_exit_frame) if args.expected_exit_frame else None,
        tuple(args.capture_bounds) if args.capture_bounds else None,
        Path(args.reference_image) if args.reference_image else None,
    )
    serialized = json.dumps(result, indent=2) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
