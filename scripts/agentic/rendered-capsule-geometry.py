#!/usr/bin/env python3
"""Derive presentation-layer capsule bounds from composited display pixels.

The ScreenCaptureKit helper captures a fixed display region before a transient
window exists. This analyzer uses the final pre-owner frame as the exact
background reference, then measures each owner-bound frame from changed pixels.
CGWindow model bounds remain diagnostic only; the returned `windowBounds` are
composited-pixel measurements consumed by the locked entry-envelope evaluator.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from PIL import Image, ImageChops


def changed_mask(image: Image.Image, reference: Image.Image, tolerance: int = 1) -> Image.Image:
    difference = ImageChops.difference(image.convert("RGB"), reference.convert("RGB"))
    red, green, blue = difference.split()
    maximum = ImageChops.lighter(ImageChops.lighter(red, green), blue)
    return maximum.point(lambda value: 255 if value > tolerance else 0, mode="L")


def measured_box(mask: Image.Image, minimum_axis_pixels: int = 2) -> tuple[int, int, int, int] | None:
    """Return an inclusive changed-pixel box, rejecting isolated pixel noise."""
    width, height = mask.size
    pixels = mask.load()
    columns = [
        x for x in range(width)
        if sum(1 for y in range(height) if pixels[x, y] != 0) >= minimum_axis_pixels
    ]
    rows = [
        y for y in range(height)
        if sum(1 for x in range(width) if pixels[x, y] != 0) >= minimum_axis_pixels
    ]
    if not columns or not rows:
        return None
    return min(columns), min(rows), max(columns), max(rows)


def presentation_bounds(
    box: tuple[int, int, int, int],
    capture_bounds: dict[str, float],
    scale: float,
) -> list[list[float]]:
    left, top, right, bottom = box
    return [
        [
            float(capture_bounds["x"]) + left / scale,
            float(capture_bounds["y"]) + top / scale,
        ],
        [
            (right - left) / scale,
            (bottom - top) / scale,
        ],
    ]


def strongest_edge(scores: list[float], center: int, radius: int) -> tuple[int, float]:
    start = max(1, center - radius)
    end = min(len(scores) - 2, center + radius)
    index = max(range(start, end + 1), key=lambda candidate: scores[candidate])
    return index, scores[index]


def edge_box(
    image: Image.Image,
    capture_bounds: dict[str, float],
    anchor_bounds: dict[str, float],
    scale: float,
) -> tuple[tuple[int, int, int, int] | None, dict[str, float]]:
    """Measure capsule edges from composited luminance gradients.

    The settled native bounds locate narrow search bands only; the selected edge
    coordinates come from pixels. This avoids both the global focus/material
    change outside an Actions popup and CGWindow's model-frame values.
    """
    gray = image.convert("L")
    width, height = gray.size
    source = gray.load()
    x_top, x_bottom = round(height * 0.15), round(height * 0.85)
    y_left, y_right = round(width * 0.20), round(width * 0.80)
    vertical = [0.0] * width
    horizontal = [0.0] * height
    for x in range(1, width):
        vertical[x] = sum(
            abs(int(source[x, y]) - int(source[x - 1, y]))
            for y in range(x_top, x_bottom)
        ) / max(1, x_bottom - x_top)
    for y in range(1, height):
        horizontal[y] = sum(
            abs(int(source[x, y]) - int(source[x, y - 1]))
            for x in range(y_left, y_right)
        ) / max(1, y_right - y_left)

    anchor_left = round((float(anchor_bounds["x"]) - float(capture_bounds["x"])) * scale)
    anchor_top = round((float(anchor_bounds["y"]) - float(capture_bounds["y"])) * scale)
    anchor_width = round(float(anchor_bounds["width"]) * scale)
    anchor_height = round(float(anchor_bounds["height"]) * scale)
    x_radius = max(24, round(anchor_width * 0.06))
    y_radius = max(24, round(anchor_height * 0.06))
    left, left_score = strongest_edge(vertical, anchor_left, x_radius)
    right, right_score = strongest_edge(vertical, anchor_left + anchor_width, x_radius)
    top, top_score = strongest_edge(horizontal, anchor_top, y_radius)
    bottom, bottom_score = strongest_edge(horizontal, anchor_top + anchor_height, y_radius)
    scores = {
        "left": left_score, "right": right_score,
        "top": top_score, "bottom": bottom_score,
    }
    if min(scores.values()) < 10.0 or right <= left or bottom <= top:
        return None, scores
    return (left, top, right, bottom), scores


def analyze(
    receipt: dict[str, Any],
    expected_owner: int,
    anchor_bounds: dict[str, float],
) -> dict[str, Any]:
    errors: list[str] = []
    frames = receipt.get("frames", [])
    capture_bounds = receipt.get("captureBounds")
    scale = float(receipt.get("captureScale", 0))
    if not isinstance(capture_bounds, dict) or scale <= 0:
        return {"schemaVersion": 1, "errors": ["capture bounds or scale missing"], "pass": False}
    if int(receipt.get("windowID", 0)) != expected_owner:
        errors.append("filmstrip resolved owner does not match expected owner")

    pre_owner = [
        frame for frame in frames
        if frame.get("actualWindowID") is None
        or float(frame.get("windowAlpha") or 0) <= 0.001
        or frame.get("windowOnscreen") is False
    ]
    owned = [
        frame for frame in frames
        if int(frame.get("actualWindowID") or 0) == expected_owner
        and float(frame.get("windowAlpha") or 0) > 0.001
        and frame.get("windowOnscreen") is not False
    ]
    foreign = [
        frame for frame in frames
        if frame.get("actualWindowID") is not None
        and int(frame.get("actualWindowID")) != expected_owner
    ]
    if foreign:
        errors.append("filmstrip contains frames bound to a foreign owner")
    if not pre_owner:
        errors.append("no pre-owner background frame was captured")
    if not owned:
        errors.append("no owner-bound rendered frame was captured")

    reference_path = Path(pre_owner[-1].get("path", "")) if pre_owner else None
    if reference_path is None or not reference_path.exists():
        errors.append("pre-owner background frame is missing on disk")
        reference = None
    else:
        reference = Image.open(reference_path).convert("RGB")

    parked_owner_background = any(
        int(frame.get("actualWindowID") or 0) == expected_owner
        and float(frame.get("windowAlpha") or 0) <= 0.001
        for frame in pre_owner
    )
    presentation_frames: list[dict[str, Any]] = []
    if reference is not None:
        for frame in owned:
            path = Path(frame.get("path", ""))
            if not path.exists():
                errors.append(f"owned frame missing: {path}")
                continue
            image = Image.open(path).convert("RGB")
            if image.size != reference.size:
                errors.append(f"owned frame dimensions differ from reference: {path}")
                continue
            if parked_owner_background:
                inclusive = measured_box(changed_mask(image, reference))
                box = None if inclusive is None else (
                    inclusive[0], inclusive[1], inclusive[2] + 1, inclusive[3] + 1
                )
                edge_scores = {"mode": "same-owner-hidden-background-delta"}
            else:
                box, edge_scores = edge_box(
                    image, capture_bounds, anchor_bounds, scale
                )
            if box is None:
                # The first owner-bound damage sample can precede coherent edge
                # compositing. It is retained in the native receipt but cannot
                # become presentation geometry without four visible edges.
                continue
            presentation_frames.append({
                **frame,
                "modelWindowBounds": frame.get("windowBounds"),
                "windowBounds": presentation_bounds(box, capture_bounds, scale),
                "presentationPixelBounds": {
                    "left": box[0], "top": box[1], "right": box[2], "bottom": box[3],
                    "width": box[2] - box[0],
                    "height": box[3] - box[1],
                },
                "edgeScores": edge_scores,
                "geometrySource": "composited-luminance-edge",
            })

    # The measurement seam must preserve a one-device-pixel edge perturbation.
    # This pure coordinate control cannot be hidden by point rounding or by the
    # locked envelope's wider physical tolerance.
    one_pixel_control: dict[str, Any] = {"pass": False, "reason": "no measured frame"}
    if presentation_frames:
        original = presentation_frames[0]["presentationPixelBounds"]
        original_width = int(original["right"]) - int(original["left"])
        perturbed_width = (int(original["right"]) + 1) - int(original["left"])
        one_pixel_control = {
            "originalPixelBounds": original,
            "perturbedRightEdge": int(original["right"]) + 1,
            "expectedWidthDeltaPixels": 1,
            "observedWidthDeltaPixels": perturbed_width - original_width,
            "pass": perturbed_width - original_width == 1,
        }
    if not one_pixel_control.get("pass"):
        errors.append("one-pixel segmentation negative control failed")

    distinct_widths = len({
        round(float(frame["windowBounds"][1][0]), 4) for frame in presentation_frames
    })
    if len(presentation_frames) < 6 or distinct_widths < 4:
        errors.append(
            f"presentation geometry under-resolved: {len(presentation_frames)} frames, "
            f"{distinct_widths} distinct widths"
        )

    return {
        "schemaVersion": 1,
        "method": (
            "same-owner hidden-background RGB delta"
            if parked_owner_background
            else "pre-armed same-stream capture; composited luminance edges inside settled-anchor search bands"
        ),
        "expectedOwnerID": expected_owner,
        "resolvedOwnerID": receipt.get("windowID"),
        "referenceFrame": None if reference_path is None else str(reference_path),
        "preOwnerFrameCount": len(pre_owner),
        "ownedFrameCount": len(owned),
        "presentationFrameCount": len(presentation_frames),
        "distinctPresentationWidths": distinct_widths,
        "frames": presentation_frames,
        "negativeControls": {"onePixelEdgePerturbation": one_pixel_control},
        "errors": errors,
        "pass": not errors,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--expected-owner", required=True, type=int)
    parser.add_argument("--anchor-bounds", required=True, nargs=4, type=float)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    receipt = json.loads(Path(args.receipt).read_text())
    result = analyze(receipt, args.expected_owner, {
        "x": args.anchor_bounds[0],
        "y": args.anchor_bounds[1],
        "width": args.anchor_bounds[2],
        "height": args.anchor_bounds[3],
    })
    Path(args.out).write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({
        "out": args.out,
        "pass": result["pass"],
        "frames": result.get("presentationFrameCount", 0),
        "distinctWidths": result.get("distinctPresentationWidths", 0),
    }))
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
