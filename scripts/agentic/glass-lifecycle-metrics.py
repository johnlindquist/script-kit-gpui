#!/usr/bin/env python3
"""Pixel and geometry analysis for exact-window glass lifecycle filmstrips."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

from PIL import Image, ImageFilter, ImageStat


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


def monotonic(values: list[float], direction: str, tolerance: float = 0.025) -> bool:
    if len(values) < 2:
        return False
    if direction == "down":
        return all(right <= left + tolerance for left, right in zip(values, values[1:]))
    return all(right + tolerance >= left for left, right in zip(values, values[1:]))


def analyze(receipt: dict, scenario: str) -> dict:
    errors: list[str] = []
    rows = []
    for frame in receipt.get("frames", []):
        path = Path(frame.get("path", ""))
        if not path.exists():
            errors.append(f"missing frame {path}")
            continue
        image = Image.open(path).convert("RGBA")
        profile = alpha_profile(image)
        lower_start = round(image.height * 0.62)
        gutter_rows = [
            index
            for index, occupancy in enumerate(profile[lower_start:], lower_start)
            if occupancy <= 0.18
        ]
        rows.append(
            {
                "sequence": frame.get("sequence"),
                "displayTimeNs": frame.get("displayTimeNs"),
                "windowBounds": frame.get("windowBounds"),
                "windowAlpha": frame.get("windowAlpha"),
                "meanAlpha": sum(profile) / len(profile),
                "gutterRowCount": len(gutter_rows),
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
    if not geometry_stable:
        errors.append("exact native window geometry changed during lifecycle capture")
    visible_rows = [row for row in rows if row["meanAlpha"] >= 0.10]
    gutter_pass = bool(visible_rows) and all(row["gutterRowCount"] >= 1 for row in visible_rows)
    if scenario.startswith("main-") and not gutter_pass:
        errors.append("transparent footer gutter was not preserved in every visible frame")
    alpha_values = [row["meanAlpha"] for row in rows]
    if scenario.endswith("exit") or "exit-reopen" in scenario:
        alpha_progression_pass = len(set(round(value, 3) for value in alpha_values)) >= 2
    else:
        alpha_progression_pass = len(alpha_values) >= 4
    if not alpha_progression_pass:
        errors.append("filmstrip does not contain a measurable alpha/state progression")
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
        "gutterPass": gutter_pass,
        "alphaProgressionPass": alpha_progression_pass,
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
