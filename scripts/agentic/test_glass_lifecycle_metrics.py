#!/usr/bin/env python3
"""Synthetic fail-closed tests for the lifecycle pixel classifier."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

from PIL import Image, ImageDraw


MODULE_PATH = Path(__file__).with_name("glass-lifecycle-metrics.py")
SPEC = importlib.util.spec_from_file_location("glass_lifecycle_metrics", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
METRICS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(METRICS)


def frame(
    *,
    stage_bottom: int = 72,
    footer_top: int = 81,
    include_footer: bool = True,
    bridge: bool = False,
) -> tuple[Image.Image, Image.Image]:
    reference = Image.new("RGB", (160, 120), (24, 30, 38))
    candidate = reference.copy()
    draw = ImageDraw.Draw(candidate)
    draw.rectangle((8, 8, 151, stage_bottom), fill=(118, 126, 140))
    if include_footer:
        draw.rectangle((32, footer_top, 151, 105), fill=(102, 110, 126))
    if bridge:
        draw.rectangle((64, stage_bottom + 1, 95, footer_top - 1), fill=(112, 120, 134))
    return candidate, reference


class GlassLifecycleMetricsTests(unittest.TestCase):
    def test_dynamic_gap_tracks_moving_stage_and_footer(self) -> None:
        for stage_bottom, footer_top in ((68, 77), (72, 81), (76, 85)):
            candidate, reference = frame(
                stage_bottom=stage_bottom,
                footer_top=footer_top,
            )
            result = METRICS.classify_main_frame(candidate, reference)
            self.assertTrue(result["stageVisible"])
            self.assertTrue(result["footerVisible"])
            self.assertTrue(result["stageFooterDisconnected"])
            self.assertTrue(result["broadBridgePass"])
            self.assertEqual(result["gutterRun"]["height"], 8)

    def test_footer_disappearance_fails_while_stage_remains(self) -> None:
        candidate, reference = frame(include_footer=False)
        result = METRICS.classify_main_frame(candidate, reference)
        self.assertTrue(result["stageVisible"])
        self.assertFalse(result["stageFooterDisconnected"])
        self.assertTrue(result["footerMissingWhileStageVisible"])
        self.assertFalse(result["broadBridgePass"])

    def test_partial_bridge_fails_full_width_transparency_gate(self) -> None:
        candidate, reference = frame(bridge=True)
        result = METRICS.classify_main_frame(candidate, reference)
        self.assertTrue(result["stageVisible"])
        self.assertFalse(result["stageFooterDisconnected"])
        self.assertTrue(result["footerMissingWhileStageVisible"])
        self.assertFalse(result["broadBridgePass"])


if __name__ == "__main__":
    unittest.main()
