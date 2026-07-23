#!/usr/bin/env python3
"""Synthetic locks for per-capsule adaptive motion and boundary gates."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


def load_module(filename: str, module_name: str):
    source = Path(__file__).with_name(filename)
    spec = importlib.util.spec_from_file_location(module_name, source)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


motion = load_module("glass-motion-color-metrics.py", "glass_motion_color_metrics")
contrast = load_module("glass-contrast-metrics.py", "glass_contrast_metrics")


def frame(phase: str, material_a=(90, 100, 110), material_b=(110, 120, 130)):
    capsules = [
        {
            "id": "a",
            "stageMedianRgb": (70, 80, 90),
            "materialMedianRgb": material_a,
        },
        {
            "id": "b",
            "stageMedianRgb": (90, 100, 110),
            "materialMedianRgb": material_b,
        },
    ]
    for capsule in capsules:
        capsule["stageDeltaE00"] = contrast.delta_e_2000(
            contrast.rgb_to_lab(capsule["materialMedianRgb"]),
            contrast.rgb_to_lab(capsule["stageMedianRgb"]),
        )
    return {
        "phase": phase,
        "capsules": capsules,
        "minimumMedianBoundaryLuminanceDifference": 0.050,
        "minimumP10BoundaryLuminanceDifference": 0.020,
        "minimumFractionAtLeast015": 0.90,
    }


class AdaptiveMotionTests(unittest.TestCase):
    def test_stable_per_capsule_relation_and_every_frame_boundary_pass(self):
        rows = [frame("motion") for _ in range(15)] + [
            frame("settled") for _ in range(3)
        ]
        adaptive, _, neighboring, errors = motion.adaptive_relation_summary(
            rows, ["a", "b"], contrast
        )
        self.assertEqual(errors, [])
        self.assertTrue(all(result["pass"] for result in adaptive.values()))
        self.assertLessEqual(neighboring, 6.0)
        self.assertTrue(motion.boundary_pass_every_frame(rows))

    def test_one_frame_hue_spike_and_one_bad_boundary_cannot_be_hidden(self):
        rows = [frame("motion") for _ in range(14)]
        rows.append(frame("motion", material_a=(255, 0, 255)))
        rows.extend(frame("settled") for _ in range(3))
        adaptive, _, _, errors = motion.adaptive_relation_summary(
            rows, ["a", "b"], contrast
        )
        self.assertEqual(errors, [])
        self.assertFalse(adaptive["a"]["pass"])
        rows[0]["minimumP10BoundaryLuminanceDifference"] = 0.014
        self.assertFalse(motion.boundary_pass_every_frame(rows))


if __name__ == "__main__":
    unittest.main()
