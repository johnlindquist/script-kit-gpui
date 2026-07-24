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


def classify_main_frame(
    image: Image.Image,
    reference: Image.Image,
    *,
    channel_tolerance: int = 16,
    changed_threshold: float = 0.01,
    component_threshold: float = 0.08,
    adjacency_rows: int = 12,
    minimum_gap_rows: int = 4,
) -> dict:
    """Find the full-width transparent run separating stage and footer.

    The window scales during entry, so a fixed y-coordinate is not evidence.
    Instead, compare every captured row with a hidden-background reference and
    require a transparent run with substantial changed pixels immediately
    above *and* below it. This fails closed when the footer disappears while
    the main stage is still visible.
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


def analyze(receipt: dict, scenario: str) -> dict:
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
    bounds = [
        json.dumps(row["windowBounds"], sort_keys=True)
        for row in rows
        if row["windowBounds"] is not None
    ]
    geometry_stable = len(set(bounds)) <= 1 and len(bounds) == len(rows)
    frame_classifications: list[dict] = []
    gutter_pass = True
    if scenario.startswith("main-") and loaded_frames:
        reference_image = (
            loaded_frames[0][1]
            if scenario == "main-entry"
            else loaded_frames[-1][1]
        ).convert("RGB")
        for index, ((_, image), row) in enumerate(zip(loaded_frames, rows)):
            classification = classify_main_frame(image, reference_image)
            classification["sequence"] = row["sequence"]
            classification["referenceFrame"] = (
                0 if scenario == "main-entry" else len(loaded_frames) - 1
            )
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
    return {
        "schemaVersion": 1,
        "scenario": scenario,
        "frameCount": len(rows),
        "frames": rows,
        "geometryStable": geometry_stable,
        "geometryStateCount": len(set(bounds)),
        "gutterPass": gutter_pass,
        "gutterReference": {
            "method":
                "display-stream per-row comparison against the hidden background; "
                "a <=1% changed run must be bounded by >=8% changed stage/footer rows",
            "referenceFrame": "first" if scenario == "main-entry" else "last",
            "channelTolerance": 16,
            "changedRowThreshold": 0.01,
            "componentThreshold": 0.08,
            "adjacencyRows": 12,
            "minimumGapRows": 4,
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
        "errors": errors,
        "pass": not errors,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--scenario", required=True)
    parser.add_argument("--out")
    args = parser.parse_args()
    receipt = json.loads(Path(args.receipt).read_text())
    result = analyze(receipt, args.scenario)
    serialized = json.dumps(result, indent=2) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
