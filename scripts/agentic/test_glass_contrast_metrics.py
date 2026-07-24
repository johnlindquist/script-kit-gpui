#!/usr/bin/env python3
"""Synthetic locks for rounded capsule masking and descendant exclusion."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

from PIL import Image, ImageDraw


def load_metrics():
    source = Path(__file__).with_name("glass-contrast-metrics.py")
    spec = importlib.util.spec_from_file_location("glass_contrast_metrics", source)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


metrics = load_metrics()


class RoundedMaskTests(unittest.TestCase):
    def test_corners_and_foreground_descendants_do_not_change_material_median(self):
        image = Image.new("RGB", (140, 80), (20, 20, 20))
        draw = ImageDraw.Draw(image)
        draw.rounded_rectangle((20, 20, 119, 59), radius=12, fill=(100, 110, 120))
        foreground_frames = [
            (32, 30, 41, 39),
            (45, 30, 54, 39),
            (58, 30, 67, 39),
            (71, 30, 80, 39),
            (84, 30, 93, 39),
            (97, 30, 106, 39),
        ]
        for frame in foreground_frames:
            draw.rectangle(frame, fill=(255, 0, 255))
        image.putpixel((21, 21), (0, 255, 0))
        capsule = {
            "id": "script-kit-footer-capsule-test",
            "screenshotFrame": {"x": 20, "y": 20, "width": 100, "height": 40},
            "layer": {"cornerRadius": 12},
        }
        nodes = [
            capsule,
            {
                "id": "script-kit-footer-capsule-content-test",
                "parentId": capsule["id"],
                "className": "NSView",
                "screenshotFrame": capsule["screenshotFrame"],
            },
            {
                "id": "script-kit-footer-label-test",
                "parentId": "script-kit-footer-capsule-content-test",
                "className": "NSTextField",
                "screenshotFrame": {"x": 32, "y": 40, "width": 10, "height": 10},
            },
            {
                "id": "script-kit-footer-test-icon",
                "parentId": "script-kit-footer-capsule-content-test",
                "className": "NSImageView",
                "screenshotFrame": {"x": 45, "y": 40, "width": 10, "height": 10},
            },
            {
                "id": "script-kit-footer-keycap-test",
                "parentId": "script-kit-footer-capsule-content-test",
                "className": "NSView",
                "screenshotFrame": {"x": 58, "y": 40, "width": 10, "height": 10},
            },
            {
                "id": "script-kit-footer-shortcut-glyph-test",
                "parentId": "script-kit-footer-keycap-test",
                "className": "NSTextField",
                "screenshotFrame": {"x": 71, "y": 40, "width": 10, "height": 10},
            },
            {
                "id": "script-kit-footer-status-dot",
                "parentId": "script-kit-footer-capsule-content-test",
                "className": "NSView",
                "screenshotFrame": {"x": 84, "y": 40, "width": 10, "height": 10},
            },
            {
                "id": "script-kit-footer-state-layer-test",
                "parentId": "script-kit-footer-capsule-content-test",
                "className": "NSView",
                "layer": {"backgroundColor": {"alpha": 1}},
                "screenshotFrame": {"x": 97, "y": 40, "width": 10, "height": 10},
            },
        ]
        result = metrics.capsule_metrics(image, capsule, 1.0, nodes)
        self.assertEqual(result["materialMedianRgb"], (100, 110, 120))
        self.assertEqual(result["mask"]["shape"], "rounded-rect")
        self.assertEqual(result["mask"]["erosionDevicePixels"], 3)
        self.assertEqual(result["mask"]["foregroundDescendantCount"], 6)
        self.assertTrue(result["mask"]["activeStateOverlay"])

    def test_descendant_walker_includes_nested_keycaps_icons_and_state_layers(self):
        nodes = [
            {"id": "root"},
            {"id": "content", "parentId": "root"},
            {"id": "keycap", "parentId": "content"},
            {"id": "glyph", "parentId": "keycap"},
            {"id": "other", "parentId": "elsewhere"},
        ]
        self.assertEqual(
            metrics.descendant_ids(nodes, "root"),
            {"content", "keycap", "glyph"},
        )

    def test_foreground_at_perimeter_is_excluded_from_boundary_samples(self):
        clean = Image.new("RGB", (140, 80), (20, 20, 20))
        draw = ImageDraw.Draw(clean)
        draw.rounded_rectangle((20, 20, 119, 59), radius=12, fill=(100, 110, 120))
        contaminated = clean.copy()
        contaminated_draw = ImageDraw.Draw(contaminated)
        contaminated_draw.rectangle((48, 20, 72, 28), fill=(255, 0, 255))
        capsule = {
            "id": "script-kit-footer-capsule-test",
            "screenshotFrame": {"x": 20, "y": 20, "width": 100, "height": 40},
            "layer": {"cornerRadius": 12},
        }
        foreground = {
            "id": "script-kit-footer-label-test",
            "parentId": capsule["id"],
            "className": "NSTextField",
            # AppKit screenshot frames are bottom-left coordinates.
            "screenshotFrame": {"x": 48, "y": 51, "width": 25, "height": 9},
        }
        clean_result = metrics.capsule_metrics(clean, capsule, 1.0, [capsule])
        contaminated_result = metrics.capsule_metrics(
            contaminated, capsule, 1.0, [capsule, foreground]
        )
        self.assertEqual(
            contaminated_result["medianBoundaryLuminanceDifference"],
            clean_result["medianBoundaryLuminanceDifference"],
        )
        self.assertEqual(
            contaminated_result["p10BoundaryLuminanceDifference"],
            clean_result["p10BoundaryLuminanceDifference"],
        )
        self.assertGreater(
            contaminated_result["mask"]["excludedBoundaryPairCount"],
            0,
        )


if __name__ == "__main__":
    unittest.main()
