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


def capsule_metrics(image: Image.Image, node: dict, scale: float) -> dict:
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

    material_pixels: list[tuple[int, ...]] = []
    strip = max(2, round(3 * scale))
    for py in list(range(y + strip, y + strip * 2)) + list(
        range(y + height - strip * 2, y + height - strip)
    ):
        for px in range(x + radius, x + width - radius):
            if 0 <= px < image.width and 0 <= py < image.height:
                material_pixels.append(image.getpixel((px, py)))

    return {
        "id": node["id"],
        "framePixels": {"x": x, "y": y, "width": width, "height": height},
        "sampleCount": len(differences),
        "medianBoundaryLuminanceDifference": statistics.median(differences),
        "p10BoundaryLuminanceDifference": percentile(differences, 0.10),
        "fractionAtLeast015": sum(value >= 0.015 for value in differences) / len(differences),
        "materialMedianRgb": median_rgb(material_pixels),
    }


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
    capsules = [capsule_metrics(image, node, scale) for node in node_capsules(receipt)]

    backdrop = receipt["layout"]["fidelity"]["appKit"]["mainBackdropFrame"]
    _, stage_y, _, stage_height = frame_pixels(backdrop, scale, image.height)
    stage_pixels = [
        image.getpixel((x, y))
        for y in range(stage_y + max(8, stage_height - round(40 * scale)), stage_y + stage_height - 8)
        for x in range(round(20 * scale), image.width - round(20 * scale), max(1, round(2 * scale)))
    ]
    stage_rgb = median_rgb(stage_pixels)
    stage_lab = rgb_to_lab(stage_rgb)
    for capsule in capsules:
        material_lab = rgb_to_lab(tuple(capsule["materialMedianRgb"]))
        capsule["stageDeltaE76"] = math.sqrt(
            sum((material_lab[index] - stage_lab[index]) ** 2 for index in range(3))
        )
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
            "maximumStageDeltaE76": max(
                (capsule["stageDeltaE76"] for capsule in capsules), default=math.inf
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
        and summary["maximumStageDeltaE76"] <= 12.0
        and summary["maximumStageAbsoluteLStarDifference"] <= 12.0
    )
    serialized = json.dumps(result, indent=2) + "\n"
    if args.out:
        Path(args.out).write_text(serialized)
    print(serialized, end="")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
