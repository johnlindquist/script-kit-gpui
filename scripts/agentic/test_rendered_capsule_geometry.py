#!/usr/bin/env python3
"""Behavior tests for presentation-geometry blur deconvolution."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).with_name("rendered-capsule-geometry.py")
SPEC = importlib.util.spec_from_file_location("rendered_capsule_geometry", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
GEOMETRY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GEOMETRY)


def frame(*, top: int = 40, bottom: int = 996, model_y: float = 166.0, model_height: float = 480.0):
    return {
        "modelWindowBounds": [[381.0, model_y], [750.0, model_height]],
        "presentationPixelBounds": {
            "left": 60,
            "top": top,
            "right": 1560,
            "bottom": bottom,
            "width": 1500,
            "height": bottom - top,
        },
        "windowBounds": [[381.0, 166.0], [750.0, (bottom - top) / 2]],
    }


class MainVerticalBlurDeconvolutionTests(unittest.TestCase):
    def test_normalizes_only_outward_bottom_blur_against_settled_pixels(self):
        frames = [frame(bottom=1000), frame(bottom=998), frame(bottom=996)]

        receipt = GEOMETRY.deconvolve_main_vertical_blur(
            frames,
            {"x": 351.0, "y": 146.0},
            2.0,
        )

        self.assertTrue(receipt["pass"])
        self.assertEqual(receipt["maxBottomBlurExcessPixels"], 4)
        self.assertEqual(
            [sample["presentationPixelBounds"]["bottom"] for sample in frames],
            [996, 996, 996],
        )
        self.assertEqual(frames[0]["rawPresentationPixelBounds"]["bottom"], 1000)

    def test_rejects_native_vertical_axis_movement(self):
        frames = [frame(bottom=1000), frame(model_height=479.0)]

        receipt = GEOMETRY.deconvolve_main_vertical_blur(
            frames,
            {"x": 351.0, "y": 146.0},
            2.0,
        )

        self.assertFalse(receipt["pass"])
        self.assertIn("native model vertical axis changed", receipt["errors"][0])

    def test_rejects_composited_top_motion_or_core_contraction(self):
        top_motion = [frame(top=39, bottom=1000), frame()]
        contraction = [frame(bottom=995), frame()]

        top_receipt = GEOMETRY.deconvolve_main_vertical_blur(
            top_motion,
            {"x": 351.0, "y": 146.0},
            2.0,
        )
        contraction_receipt = GEOMETRY.deconvolve_main_vertical_blur(
            contraction,
            {"x": 351.0, "y": 146.0},
            2.0,
        )

        self.assertFalse(top_receipt["pass"])
        self.assertFalse(contraction_receipt["pass"])
        self.assertTrue(any("top edge moved" in error for error in top_receipt["errors"]))
        self.assertTrue(any("contracted inside" in error for error in contraction_receipt["errors"]))


if __name__ == "__main__":
    unittest.main()
