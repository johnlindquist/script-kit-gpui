#!/usr/bin/env python3
"""Synthetic locks for per-capsule adaptive motion and boundary gates."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

from PIL import Image


def load_module(filename: str, module_name: str):
    source = Path(__file__).with_name(filename)
    spec = importlib.util.spec_from_file_location(module_name, source)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


motion = load_module("glass-motion-color-metrics.py", "glass_motion_color_metrics")
contrast = load_module("glass-contrast-metrics.py", "glass_contrast_metrics")


def frame(
    phase: str,
    material_a=(90, 100, 110),
    material_b=(100, 110, 120),
    *,
    window_alpha: float = 1.0,
    intrinsic_a=None,
    intrinsic_b=None,
    intrinsic_eligible: bool = True,
    sequence=None,
):
    capsules = [
        {
            "id": "a",
            "stageMedianRgb": (70, 80, 90),
            "displayedMaterialMedianRgb": material_a,
            "intrinsicMaterialMedianRgb": (
                intrinsic_a if intrinsic_a is not None else material_a
            ),
        },
        {
            "id": "b",
            "stageMedianRgb": (90, 100, 110),
            "displayedMaterialMedianRgb": material_b,
            "intrinsicMaterialMedianRgb": (
                intrinsic_b if intrinsic_b is not None else material_b
            ),
        },
    ]
    for capsule in capsules:
        capsule["stageDeltaE00"] = contrast.delta_e_2000(
            contrast.rgb_to_lab(capsule["displayedMaterialMedianRgb"]),
            contrast.rgb_to_lab(capsule["stageMedianRgb"]),
        )
    return {
        "phase": phase,
        "sequence": sequence,
        "windowAlpha": window_alpha,
        "displayedColorEligible": True,
        "intrinsicDiagnosticEligible": intrinsic_eligible,
        "capsules": capsules,
        "minimumMedianBoundaryLuminanceDifference": 0.050,
        "minimumP10BoundaryLuminanceDifference": 0.020,
        "minimumFractionAtLeast015": 0.90,
    }


ENTRY_GATES = dict(
    minimum_samples=motion.MIN_VISIBLE_ENTRY_MOTION_FRAMES,
    maximum_gate=motion.MAX_DISPLAYED_ENTRY_DELTA_E00,
    relation_drift_gate=motion.MAX_CAPSULE_STAGE_RELATION_DRIFT_DELTA_E00,
)
DRAG_GATES = dict(minimum_samples=1, maximum_gate=8.0, p95_gate=5.0)


class MaterialStabilityTests(unittest.TestCase):
    def test_entry_material_recovery_removes_shared_window_alpha(self):
        background = Image.new("RGB", (2, 1), (200, 100, 40))
        content = Image.new("RGB", (2, 1), (40, 60, 80))
        alpha = 0.5
        composited = Image.new("RGB", content.size)
        composited.putdata(
            [
                tuple(
                    round(alpha * foreground[channel] + (1 - alpha) * backdrop[channel])
                    for channel in range(3)
                )
                for foreground, backdrop in zip(
                    content.getdata(),
                    background.getdata(),
                )
            ]
        )
        recovered = motion.recover_content_before_window_alpha(
            composited,
            background,
            alpha,
        )
        self.assertEqual(list(recovered.getdata()), list(content.getdata()))

    def test_stable_per_capsule_material_and_every_frame_boundary_pass(self):
        rows = [frame("motion") for _ in range(15)] + [
            frame("settled") for _ in range(3)
        ]
        stability, _, neighboring, _, _, errors = motion.displayed_color_summary(
            rows, ["a", "b"], contrast, **DRAG_GATES
        )
        self.assertEqual(errors, [])
        self.assertTrue(all(result["pass"] for result in stability.values()))
        self.assertLessEqual(neighboring, 6.0)
        self.assertTrue(motion.boundary_pass_every_frame(rows))

    def test_one_frame_hue_spike_and_one_bad_boundary_cannot_be_hidden(self):
        rows = [frame("motion") for _ in range(14)]
        rows.append(frame("motion", material_a=(255, 0, 255)))
        rows.extend(frame("settled") for _ in range(3))
        stability, _, _, _, _, errors = motion.displayed_color_summary(
            rows, ["a", "b"], contrast, **DRAG_GATES
        )
        self.assertEqual(errors, [])
        self.assertFalse(stability["a"]["pass"])
        rows[0]["minimumP10BoundaryLuminanceDifference"] = 0.014
        self.assertFalse(motion.boundary_pass_every_frame(rows))

    # ---- Negative controls for the removed alpha blind spot (Oracle step 1).
    # The old metric excluded sub-opaque visible frames from the gating summary
    # (materialStabilityEligible = windowAlpha >= 0.85), which certified the
    # exact defect under measurement. These tests make that regression loud.

    def test_visible_subopaque_wallpaper_shift_fails_despite_stable_intrinsic(self):
        # Displayed pixels at alpha 0.20 are dragged toward the wallpaper even
        # though the recovered INTRINSIC material is perfectly stable. The
        # acceptance verdict must come from what the user saw: FAIL.
        wallpaper_shifted = frame(
            "motion",
            material_a=(178, 100, 54),
            intrinsic_a=(90, 100, 110),
            window_alpha=0.20,
            sequence=5,
        )
        rows = (
            [frame("motion", sequence=index) for index in range(5)]
            + [wallpaper_shifted]
            + [frame("settled") for _ in range(3)]
        )
        summary, _, _, global_max, _, errors = motion.displayed_color_summary(
            rows, ["a", "b"], contrast, **ENTRY_GATES
        )
        self.assertEqual(errors, [])
        self.assertFalse(summary["a"]["pass"])
        self.assertGreater(
            summary["a"]["motionMaximumDeltaE00"],
            motion.MAX_DISPLAYED_ENTRY_DELTA_E00,
        )
        # The global maximum includes the sub-opaque frame — it can never be
        # smoothed away by the stable neighbors.
        self.assertGreater(global_max, motion.MAX_DISPLAYED_ENTRY_DELTA_E00)
        # The intrinsic diagnostic sees the stable internal material, and is
        # non-gating by construction: it carries NO pass verdict at all.
        diagnostic = motion.intrinsic_material_diagnostic(
            rows, ["a", "b"], contrast
        )
        self.assertLess(diagnostic["a"]["motionMaximumDeltaE00"], 1.0)
        self.assertNotIn("pass", diagnostic["a"])
        self.assertNotIn("pass", diagnostic["b"])

    def test_subopaque_visible_frames_count_as_displayed_samples(self):
        # Eligibility is lifecycle visibility, never a window-alpha floor.
        # Reintroducing `windowAlpha >= 0.85` sample eligibility drops the
        # low-alpha sample and shrinks the maximum, failing both assertions.
        rows = (
            [frame("motion", sequence=index) for index in range(5)]
            + [
                frame(
                    "motion",
                    material_a=(178, 100, 54),
                    window_alpha=0.20,
                    sequence=5,
                )
            ]
            + [frame("settled") for _ in range(3)]
        )
        summary, _, _, global_max, _, errors = motion.displayed_color_summary(
            rows, ["a", "b"], contrast, **ENTRY_GATES
        )
        self.assertEqual(errors, [])
        self.assertEqual(summary["a"]["motionSampleCount"], 6)
        self.assertGreater(global_max, motion.MAX_DISPLAYED_ENTRY_DELTA_E00)

    def test_relation_drift_gate_fails_a_capsule_that_melts_into_the_stage(self):
        # Material color may match the settled reference while the capsule's
        # relation to its LOCAL stage collapses (capsule melting into the
        # backdrop mid-fade). The drift gate catches that independently.
        melted = frame("motion", sequence=5)
        for capsule in melted["capsules"]:
            capsule["stageDeltaE00"] = capsule["stageDeltaE00"] + 9.0
        rows = (
            [frame("motion", sequence=index) for index in range(5)]
            + [melted]
            + [frame("settled") for _ in range(3)]
        )
        summary, _, _, _, max_drift, errors = motion.displayed_color_summary(
            rows, ["a", "b"], contrast, **ENTRY_GATES
        )
        self.assertEqual(errors, [])
        self.assertFalse(summary["a"]["pass"])
        self.assertGreater(
            max_drift, motion.MAX_CAPSULE_STAGE_RELATION_DRIFT_DELTA_E00
        )

    def test_classify_entry_frame_alpha_zero_semantics(self):
        # alpha 0 + invisible: pre-contributing, not a failure, no color math.
        self.assertEqual(motion.classify_entry_frame(0.0, False), "precontributing")
        # alpha 0 + visible region: wallpaper where UI should be — hard fail.
        self.assertEqual(
            motion.classify_entry_frame(0.0, True), "visibleZeroAlpha"
        )
        # visible but below the floor: policy fail, still measured.
        self.assertEqual(motion.classify_entry_frame(0.20, True), "belowFloor")
        self.assertEqual(
            motion.classify_entry_frame(
                motion.MIN_VISIBLE_ENTRY_ALPHA - 0.01, True
            ),
            "belowFloor",
        )
        self.assertEqual(
            motion.classify_entry_frame(motion.MIN_VISIBLE_ENTRY_ALPHA, True),
            "measurable",
        )
        self.assertEqual(motion.classify_entry_frame(0.90, False), "precontributing")

    def test_alpha_recovery_is_undefined_at_zero_alpha(self):
        # The recovery function must reject alpha 0 outright — a zero-alpha
        # frame has no defined intrinsic color, so nothing upstream may ever
        # "recover" one into the diagnostic.
        image = Image.new("RGB", (2, 1), (10, 10, 10))
        with self.assertRaises(ValueError):
            motion.recover_content_before_window_alpha(image, image.copy(), 0.0)

    def test_alpha_policy_hard_fails_are_not_erasable(self):
        clean = motion.alpha_policy_summary(
            [(0, 0.85), (1, 0.90), (2, 1.0)], [], [], []
        )
        self.assertTrue(clean["pass"])
        self.assertEqual(clean["firstVisibleEntryAlpha"], 0.85)
        self.assertEqual(clean["minimumVisibleEntryAlpha"], 0.85)
        below_floor = motion.alpha_policy_summary(
            [(0, 0.20), (1, 1.0)], [0], [], []
        )
        self.assertFalse(below_floor["pass"])
        zero_alpha = motion.alpha_policy_summary([(0, 0.0)], [], [0], [])
        self.assertFalse(zero_alpha["pass"])
        # A lifecycle-visible frame whose geometry could not be measured is a
        # hard failure that survives into the receipt — it cannot disappear
        # into a "pre-visible" bucket.
        unmeasurable = motion.alpha_policy_summary(
            [(0, 0.90)],
            [],
            [],
            [{"sequence": 0, "reason": "unmeasurableVisibleEntryFrame: x"}],
        )
        self.assertFalse(unmeasurable["pass"])
        self.assertEqual(unmeasurable["unmeasurableVisibleFrameCount"], 1)
        self.assertEqual(
            unmeasurable["unmeasurableVisibleFrames"][0]["sequence"], 0
        )

    def test_entry_projection_reproduces_footer_autoresizing_anchors(self):
        appkit = {
            "windowBounds": {"x": 381, "y": 166, "width": 750, "height": 480},
            "mainBackdropFrame": {"x": 0, "y": 40, "width": 750, "height": 440},
            "footerContainerFrame": {"x": 0, "y": 0, "width": 750, "height": 32},
            "nodes": [
                {
                    "id": "script-kit-footer-capsule-actions",
                    "className": "NSGlassEffectView",
                    "hidden": False,
                    "screenshotFrame": {
                        "x": 504,
                        "y": 2,
                        "width": 118,
                        "height": 28,
                    },
                    "layer": {"cornerRadius": 6},
                }
            ],
        }
        projected = motion.transform_appkit_geometry_for_display_frame(
            appkit,
            (381, 166, 750, 480),
            {"x": 381, "y": 166, "width": 750, "height": 501},
            (1500, 1002),
        )
        frame = projected["nodes"][0]["screenshotFrame"]
        pixels = contrast.frame_pixels(frame, 2, 1002)
        self.assertEqual(pixels, (1008, 900, 236, 56))
        # CGWindow reports a larger, up-left presentation envelope during the
        # frame animation. The capsule retains its size and moves with the
        # right edge rather than being geometrically scaled.
        expanded = motion.transform_appkit_geometry_for_display_frame(
            appkit,
            (358, 160, 795, 492),
            {"x": 381, "y": 166, "width": 750, "height": 501},
            (1500, 1002),
        )
        expanded_pixels = contrast.frame_pixels(
            expanded["nodes"][0]["screenshotFrame"], 2, 1002
        )
        self.assertEqual(expanded_pixels, (1052, 912, 236, 56))
        self.assertEqual(
            expanded["presentationWindowBounds"],
            {"x": 358, "y": 160, "width": 795, "height": 492},
        )
        self.assertEqual(expanded["nodes"][0]["layer"]["cornerRadius"], 6)

    def test_expanded_capture_crop_contains_the_full_entry_morph(self):
        appkit = {
            "windowBounds": {"x": 381, "y": 166, "width": 750, "height": 480},
            "mainBackdropFrame": {"x": 0, "y": 40, "width": 750, "height": 440},
            "footerContainerFrame": {"x": 0, "y": 0, "width": 750, "height": 32},
            "nodes": [
                {
                    "id": "script-kit-footer-capsule-ai",
                    "className": "NSGlassEffectView",
                    "hidden": False,
                    "screenshotFrame": {
                        "x": 641,
                        "y": 2,
                        "width": 108,
                        "height": 28,
                    },
                    "layer": {"cornerRadius": 6},
                }
            ],
        }
        capture = {"x": 351, "y": 145, "width": 810, "height": 542}
        projected = motion.transform_appkit_geometry_for_display_frame(
            appkit,
            (358, 160, 795, 492),
            capture,
            (1620, 1084),
        )
        pixels = contrast.frame_pixels(
            projected["nodes"][0]["screenshotFrame"],
            2,
            1084,
        )
        self.assertGreaterEqual(pixels[0], 0)
        self.assertGreaterEqual(pixels[1], 0)
        self.assertLessEqual(pixels[0] + pixels[2], 1620)
        self.assertLessEqual(pixels[1] + pixels[3], 1084)

    def test_rendered_window_envelope_comes_from_explicit_background(self):
        reference = Image.new("RGB", (200, 140), (240, 240, 240))
        rendered = reference.copy()
        for y in range(10, 110):
            for x in range(20, 180):
                rendered.putpixel((x, y), (80, 80, 80))
        # Footer controls are intentionally disjoint and must not extend the
        # continuous stage envelope.
        for left, right in ((30, 70), (100, 130), (140, 170)):
            for y in range(120, 130):
                for x in range(left, right):
                    rendered.putpixel((x, y), (30, 30, 30))
        appkit = {
            "windowBounds": {"x": 0, "y": 0, "width": 100, "height": 60},
            "mainBackdropFrame": {"x": 0, "y": 20, "width": 100, "height": 40},
        }
        bounds, evidence = motion.rendered_window_bounds_from_reference(
            rendered,
            reference,
            appkit,
            {"x": 0, "y": 0, "width": 100, "height": 70},
        )
        self.assertEqual(bounds, (10, 5, 80, 70))
        self.assertEqual(
            evidence["stageFramePixels"],
            {"x": 20, "y": 10, "width": 160, "height": 100},
        )

    def test_lifecycle_uses_explicit_settled_captures_not_stream_tail(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = []
            for index in range(4):
                path = Path(directory) / f"frame-{index}.png"
                Image.new("RGB", (20, 20), (index, index, index)).save(path)
                paths.append(path)
            receipt = {
                "scenarios": [
                    {
                        "name": "main-entry",
                        "filmstrip": {
                            "receipt": {
                                "frames": [
                                    {
                                        "sequence": 0,
                                        "path": str(paths[0]),
                                        "windowBounds": [[0, 0], [10, 10]],
                                    },
                                    {
                                        "sequence": 1,
                                        "path": str(paths[0]),
                                        "windowBounds": [[0, 0], [10, 10]],
                                    },
                                ]
                            },
                            "metrics": {
                                "frames": [
                                    {
                                        "sequence": 0,
                                        "stageVisible": False,
                                        "footerVisible": False,
                                    },
                                    {
                                        "sequence": 1,
                                        "stageVisible": True,
                                        "footerVisible": True,
                                    },
                                ]
                            },
                        },
                        "settledCapturesPass": True,
                        "settledCaptures": [
                            {
                                "sequence": f"settled-{index}",
                                "path": str(paths[index + 1]),
                                "windowBounds": [[0, 0], [10, 10]],
                            }
                            for index in range(3)
                        ],
                        "settledLayout": {"fidelity": {"appKit": {}}},
                        "captureBounds": {},
                    }
                ]
            }
            frames, _, _, errors = motion.lifecycle_entry_frames(
                receipt,
                "main-entry",
            )
            self.assertEqual(errors, [])
            self.assertEqual(
                [frame["_phase"] for frame in frames],
                ["motion", "motion", "settled", "settled", "settled"],
            )
            # Lifecycle-invisible frames are RETAINED with visibility
            # annotations — visibility is data for the alpha policy, never a
            # silent exclusion filter.
            self.assertEqual(
                [frame.get("_entryVisible") for frame in frames],
                [False, True, None, None, None],
            )


if __name__ == "__main__":
    unittest.main()
