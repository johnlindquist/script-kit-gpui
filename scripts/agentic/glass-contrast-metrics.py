#!/usr/bin/env python3
"""Measure floating-capsule boundary and material relation from a probe receipt."""

from __future__ import annotations

import argparse
import json
import math
import statistics
from pathlib import Path

from PIL import Image


def linear_channel(value: int) -> float:
    channel = value / 255.0
    return channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4


def luminance(pixel: tuple[int, ...]) -> float:
    red, green, blue = pixel[:3]
    return (
        0.2126 * linear_channel(red)
        + 0.7152 * linear_channel(green)
        + 0.0722 * linear_channel(blue)
    )


def rgb_to_lab(pixel: tuple[int, int, int]) -> tuple[float, float, float]:
    red, green, blue = (linear_channel(channel) for channel in pixel)
    x = (red * 0.4124 + green * 0.3576 + blue * 0.1805) / 0.95047
    y = red * 0.2126 + green * 0.7152 + blue * 0.0722
    z = (red * 0.0193 + green * 0.1192 + blue * 0.9505) / 1.08883

    def f(value: float) -> float:
        return value ** (1.0 / 3.0) if value > 0.008856 else 7.787 * value + 16.0 / 116.0

    fx, fy, fz = f(x), f(y), f(z)
    return 116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)


def delta_e_2000(
    lab1: tuple[float, float, float], lab2: tuple[float, float, float]
) -> float:
    """CIEDE2000 color difference (Sharma, Wu, and Dalal 2005)."""
    l1, a1, b1 = lab1
    l2, a2, b2 = lab2
    c1 = math.hypot(a1, b1)
    c2 = math.hypot(a2, b2)
    c_bar = (c1 + c2) / 2.0
    g = 0.5 * (1.0 - math.sqrt(c_bar**7 / (c_bar**7 + 25.0**7)))
    a1p, a2p = (1.0 + g) * a1, (1.0 + g) * a2
    c1p, c2p = math.hypot(a1p, b1), math.hypot(a2p, b2)

    def hue(ap: float, b: float) -> float:
        value = math.degrees(math.atan2(b, ap))
        return value + 360.0 if value < 0.0 else value

    h1p, h2p = hue(a1p, b1), hue(a2p, b2)
    delta_lp = l2 - l1
    delta_cp = c2p - c1p
    delta_h = h2p - h1p
    if c1p * c2p == 0.0:
        delta_hp = 0.0
    elif abs(delta_h) <= 180.0:
        delta_hp = delta_h
    elif delta_h > 180.0:
        delta_hp = delta_h - 360.0
    else:
        delta_hp = delta_h + 360.0
    delta_big_hp = 2.0 * math.sqrt(c1p * c2p) * math.sin(math.radians(delta_hp / 2.0))
    l_bar = (l1 + l2) / 2.0
    c_bar_p = (c1p + c2p) / 2.0
    if c1p * c2p == 0.0:
        h_bar = h1p + h2p
    elif abs(h1p - h2p) <= 180.0:
        h_bar = (h1p + h2p) / 2.0
    elif h1p + h2p < 360.0:
        h_bar = (h1p + h2p + 360.0) / 2.0
    else:
        h_bar = (h1p + h2p - 360.0) / 2.0
    t = (
        1.0
        - 0.17 * math.cos(math.radians(h_bar - 30.0))
        + 0.24 * math.cos(math.radians(2.0 * h_bar))
        + 0.32 * math.cos(math.radians(3.0 * h_bar + 6.0))
        - 0.20 * math.cos(math.radians(4.0 * h_bar - 63.0))
    )
    delta_theta = 30.0 * math.exp(-(((h_bar - 275.0) / 25.0) ** 2))
    rc = 2.0 * math.sqrt(c_bar_p**7 / (c_bar_p**7 + 25.0**7))
    sl = 1.0 + 0.015 * (l_bar - 50.0) ** 2 / math.sqrt(20.0 + (l_bar - 50.0) ** 2)
    sc = 1.0 + 0.045 * c_bar_p
    sh = 1.0 + 0.015 * c_bar_p * t
    rt = -math.sin(math.radians(2.0 * delta_theta)) * rc
    return math.sqrt(
        (delta_lp / sl) ** 2
        + (delta_cp / sc) ** 2
        + (delta_big_hp / sh) ** 2
        + rt * (delta_cp / sc) * (delta_big_hp / sh)
    )


def median_rgb(pixels: list[tuple[int, ...]]) -> tuple[int, int, int]:
    return tuple(round(statistics.median(pixel[index] for pixel in pixels)) for index in range(3))


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(len(ordered) * fraction) - 1))
    return ordered[index]


def node_capsules(receipt: dict) -> list[dict]:
    nodes = receipt.get("layout", {}).get("fidelity", {}).get("appKit", {}).get("nodes", [])
    return [
        node
        for node in nodes
        if node.get("className") == "NSGlassEffectView"
        and (
            str(node.get("id", "")).startswith("script-kit-footer-capsule-")
            or node.get("id") == "script-kit-footer-left-info-capsule"
        )
        and not node.get("hidden", False)
        and node.get("screenshotFrame")
    ]


def frame_pixels(frame: dict, scale: float, image_height: int) -> tuple[int, int, int, int]:
    x = round(float(frame["x"]) * scale)
    width = round(float(frame["width"]) * scale)
    height = round(float(frame["height"]) * scale)
    y = image_height - round((float(frame["y"]) + float(frame["height"])) * scale)
    return x, y, width, height


def rounded_rect_contains(
    px: float,
    py: float,
    rect: tuple[int, int, int, int],
    radius: float,
) -> bool:
    x, y, width, height = rect
    if width <= 0 or height <= 0:
        return False
    radius = max(0.0, min(radius, width / 2.0, height / 2.0))
    if x + radius <= px <= x + width - radius or y + radius <= py <= y + height - radius:
        return x <= px < x + width and y <= py < y + height
    cx = x + radius if px < x + width / 2.0 else x + width - radius
    cy = y + radius if py < y + height / 2.0 else y + height - radius
    return (px - cx) ** 2 + (py - cy) ** 2 <= radius**2


def descendant_ids(nodes: list[dict], root_id: str) -> set[str]:
    children: dict[str, list[str]] = {}
    for node in nodes:
        parent = node.get("parentId")
        node_id = node.get("id")
        if parent and node_id:
            children.setdefault(str(parent), []).append(str(node_id))
    result: set[str] = set()
    pending = list(children.get(root_id, []))
    while pending:
        node_id = pending.pop()
        if node_id in result:
            continue
        result.add(node_id)
        pending.extend(children.get(node_id, []))
    return result


def foreground_exclusion_rects(
    node: dict,
    nodes: list[dict],
    scale: float,
    image_height: int,
) -> tuple[list[tuple[int, int, int, int]], bool]:
    descendants = descendant_ids(nodes, str(node.get("id", "")))
    excluded: list[tuple[int, int, int, int]] = []
    active_full_state_overlay = False
    for foreground in nodes:
        foreground_id = str(foreground.get("id", ""))
        if (
            foreground_id not in descendants
            or foreground.get("hidden", False)
            or float(foreground.get("alpha", 1)) <= 0
        ):
            continue
        foreground_class = str(foreground.get("className", ""))
        layer = foreground.get("layer", {})
        background_alpha = float(layer.get("backgroundColor", {}).get("alpha", 0))
        is_state_overlay = "state-layer" in foreground_id and background_alpha > 0
        is_foreground = (
            foreground_class in {"NSTextField", "NSImageView"}
            or "keycap" in foreground_id
            or foreground_id.endswith("-icon")
            or foreground_id.endswith("-dot")
        )
        if not is_foreground and not is_state_overlay:
            continue
        frame = foreground.get("screenshotFrame")
        if not frame:
            continue
        fx, fy, fw, fh = frame_pixels(frame, scale, image_height)
        if is_state_overlay:
            active_full_state_overlay = True
        excluded.append((fx - 1, fy - 1, fw + 2, fh + 2))
    return excluded, active_full_state_overlay


def capsule_metrics(
    image: Image.Image, node: dict, scale: float, foreground_nodes: list[dict]
) -> dict:
    x, y, width, height = frame_pixels(node["screenshotFrame"], scale, image.height)
    inset = 1
    outside = 1
    radius = max(2, round(float(node.get("layer", {}).get("cornerRadius", 6)) * scale))
    differences: list[float] = []

    def add_pair(inner: tuple[int, int], outer: tuple[int, int]) -> None:
        if (
            0 <= inner[0] < image.width
            and 0 <= inner[1] < image.height
            and 0 <= outer[0] < image.width
            and 0 <= outer[1] < image.height
        ):
            differences.append(
                abs(luminance(image.getpixel(inner)) - luminance(image.getpixel(outer)))
            )

    for px in range(x + radius, x + width - radius):
        add_pair((px, y + inset), (px, y - outside))
        add_pair((px, y + height - 1 - inset), (px, y + height - 1 + outside))
    for py in range(y + radius, y + height - radius):
        add_pair((x + inset, py), (x - outside, py))
        add_pair((x + width - 1 - inset, py), (x + width - 1 + outside, py))
    corner_centers = [
        (x + radius, y + radius, math.pi, math.pi * 1.5),
        (x + width - radius, y + radius, math.pi * 1.5, math.pi * 2),
        (x + width - radius, y + height - radius, 0.0, math.pi * 0.5),
        (x + radius, y + height - radius, math.pi * 0.5, math.pi),
    ]
    for cx, cy, start, end in corner_centers:
        steps = max(8, round(radius * (end - start)))
        for step in range(steps + 1):
            angle = start + (end - start) * step / steps
            add_pair(
                (
                    round(cx + (radius - inset) * math.cos(angle)),
                    round(cy + (radius - inset) * math.sin(angle)),
                ),
                (
                    round(cx + (radius + outside) * math.cos(angle)),
                    round(cy + (radius + outside) * math.sin(angle)),
                ),
            )

    # The material contract is measured on the capsule body, not its text or
    # state chrome. Erode by three device-independent pixels and subtract every
    # Erode by exactly three device pixels. The rounded interior mask excludes
    # corner pixels as well as every rendered foreground descendant.
    erode = 3
    excluded, active_state_overlay = foreground_exclusion_rects(
        node, foreground_nodes, scale, image.height
    )
    inner_rect = (x + erode, y + erode, width - erode * 2, height - erode * 2)
    inner_radius = max(0, radius - erode)
    material_pixels: list[tuple[int, ...]] = []
    for py in range(y, y + height):
        for px in range(x, x + width):
            if not rounded_rect_contains(px + 0.5, py + 0.5, inner_rect, inner_radius):
                continue
            if any(ex <= px < ex + ew and ey <= py < ey + eh for ex, ey, ew, eh in excluded):
                continue
            material_pixels.append(image.getpixel((px, py)))
    if not material_pixels:
        raise ValueError(f"capsule {node.get('id')} has no material pixels after masking")

    return {
        "id": node["id"],
        "framePixels": {"x": x, "y": y, "width": width, "height": height},
        "sampleCount": len(differences),
        "mask": {
            "shape": "rounded-rect",
            "cornerRadiusDevicePixels": radius,
            "erosionDevicePixels": erode,
            "foregroundDescendantCount": len(excluded),
            "activeStateOverlay": active_state_overlay,
        },
        "medianBoundaryLuminanceDifference": statistics.median(differences),
        "p10BoundaryLuminanceDifference": percentile(differences, 0.10),
        "fractionAtLeast015": sum(value >= 0.015 for value in differences) / len(differences),
        "materialMedianRgb": median_rgb(material_pixels),
    }


def local_stage_rgb(
    image: Image.Image, capsule: dict, backdrop: dict, scale: float
) -> tuple[int, int, int]:
    capsule_frame = capsule["framePixels"]
    _, stage_y, _, stage_height = frame_pixels(backdrop, scale, image.height)
    horizontal_inset = max(3, round(3 * scale))
    left = max(0, capsule_frame["x"] + horizontal_inset)
    right = min(
        image.width,
        capsule_frame["x"] + capsule_frame["width"] - horizontal_inset,
    )
    top = stage_y + max(8, stage_height - round(32 * scale))
    bottom = stage_y + stage_height - max(4, round(4 * scale))
    pixels = [
        image.getpixel((x, y))
        for y in range(top, bottom)
        for x in range(left, right, max(1, round(2 * scale)))
    ]
    if not pixels:
        raise ValueError(f"capsule {capsule.get('id')} has no local stage reference pixels")
    return median_rgb(pixels)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--out")
    args = parser.parse_args()

    receipt_path = Path(args.receipt).resolve()
    receipt = json.loads(receipt_path.read_text())
    capture = next(
        capture
        for capture in receipt.get("stationary", {}).get("captures", [])
        if capture.get("name") == "stationary-default-2x"
    )
    image_path = Path(capture["compositedPath"]).resolve()
    image = Image.open(image_path).convert("RGB")
    host_width = float(receipt["layout"]["fidelity"]["appKit"]["footerContainerFrame"]["width"])
    scale = image.width / host_width
    foreground_nodes = receipt.get("layout", {}).get("fidelity", {}).get("appKit", {}).get(
        "nodes", []
    )
    capsules = [
        capsule_metrics(image, node, scale, foreground_nodes)
        for node in node_capsules(receipt)
    ]

    backdrop = receipt["layout"]["fidelity"]["appKit"]["mainBackdropFrame"]
    _, stage_y, _, stage_height = frame_pixels(backdrop, scale, image.height)
    stage_pixels = [
        image.getpixel((x, y))
        for y in range(stage_y + max(8, stage_height - round(40 * scale)), stage_y + stage_height - 8)
        for x in range(round(20 * scale), image.width - round(20 * scale), max(1, round(2 * scale)))
    ]
    stage_rgb = median_rgb(stage_pixels)
    for capsule in capsules:
        local_rgb = local_stage_rgb(image, capsule, backdrop, scale)
        stage_lab = rgb_to_lab(local_rgb)
        material_lab = rgb_to_lab(tuple(capsule["materialMedianRgb"]))
        capsule["stageMedianRgb"] = local_rgb
        capsule["stageDeltaE00"] = delta_e_2000(material_lab, stage_lab)
        capsule["stageAbsoluteLStarDifference"] = abs(material_lab[0] - stage_lab[0])

    all_boundary = [capsule["medianBoundaryLuminanceDifference"] for capsule in capsules]
    all_p10 = [capsule["p10BoundaryLuminanceDifference"] for capsule in capsules]
    all_fraction = [capsule["fractionAtLeast015"] for capsule in capsules]
    result = {
        "schemaVersion": 1,
        "receipt": str(receipt_path),
        "image": str(image_path),
        "scale": scale,
        "stageMedianRgb": stage_rgb,
        "capsules": capsules,
        "summary": {
            "minimumMedianBoundaryLuminanceDifference": min(all_boundary, default=0),
            "minimumP10BoundaryLuminanceDifference": min(all_p10, default=0),
            "minimumFractionAtLeast015": min(all_fraction, default=0),
            "maximumStageDeltaE00": max(
                (capsule["stageDeltaE00"] for capsule in capsules), default=math.inf
            ),
            "maximumStageAbsoluteLStarDifference": max(
                (capsule["stageAbsoluteLStarDifference"] for capsule in capsules),
                default=math.inf,
            ),
        },
    }
    summary = result["summary"]
    result["pass"] = (
        len(capsules) >= 2
        and summary["minimumMedianBoundaryLuminanceDifference"] >= 0.040
        and summary["minimumP10BoundaryLuminanceDifference"] >= 0.015
        and summary["minimumFractionAtLeast015"] >= 0.80
        and summary["maximumStageDeltaE00"] <= 10.0
        and summary["maximumStageAbsoluteLStarDifference"] <= 12.0
    )
    serialized = json.dumps(result, indent=2) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
