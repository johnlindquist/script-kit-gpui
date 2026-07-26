#!/usr/bin/env python3
"""WP10 (glass-smoke-harness-max-info): information economics must be
gameproof — fields without registry units add nothing, missing units lower
counts, stale metric caches never revalidate, artifact paths never escape
the study root, and a final comparison ledger can never contain PENDING or
lose an entry.

Run from the repo root:
  python3 -m unittest scripts.agentic.test_glass_smoke_economics
"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

_spec = importlib.util.spec_from_file_location(
    "glass_smoke_economics",
    Path(__file__).with_name("glass-smoke-economics.py"),
)
economics = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(economics)


class RegistryUnitTests(unittest.TestCase):
    def setUp(self) -> None:
        self.registry = economics.load_registry()

    def test_registry_loads_with_stable_unit_ids(self) -> None:
        ids = economics.registry_unit_ids(self.registry)
        self.assertIn("main-entry.displayed-color", ids)
        self.assertIn("main-exit.displayed-color-curve", ids)
        self.assertIn("run.interference-classification", ids)
        self.assertEqual(len(ids), 18)

    def test_unknown_fields_cannot_improve_economics(self) -> None:
        counted = economics.count_information_units(
            {
                "informationUnits": {
                    "captured": [
                        "main-entry.displayed-color",
                        "made-up.shiny-new-field",
                    ],
                    "promoted": ["made-up.shiny-new-field"],
                }
            },
            self.registry,
        )
        self.assertEqual(counted["capturedCount"], 1)
        self.assertEqual(counted["promotedCount"], 0)
        self.assertTrue(
            any("not in registry" in error for error in counted["errors"])
        )

    def test_missing_units_lower_the_count_never_assumed(self) -> None:
        counted = economics.count_information_units(
            {"informationUnits": {"captured": [], "promoted": []}},
            self.registry,
        )
        self.assertEqual(counted["capturedCount"], 0)
        self.assertEqual(len(counted["missing"]), 18)

    def test_promotion_requires_capture(self) -> None:
        counted = economics.count_information_units(
            {
                "informationUnits": {
                    "captured": [],
                    "promoted": ["main-entry.displayed-color"],
                }
            },
            self.registry,
        )
        self.assertEqual(counted["promotedCount"], 0)
        self.assertTrue(
            any("never captured" in error for error in counted["errors"])
        )


class MetricCacheKeyTests(unittest.TestCase):
    def test_changed_frame_input_changes_the_key(self) -> None:
        base = dict(
            input_receipt_hashes=["r1"],
            frame_inventory_hashes=["f1", "f2"],
            metric_source_bundle_sha256="m1",
            scenario="main-entry",
            options={"phase": "entry"},
        )
        key = economics.metric_cache_key(**base)
        self.assertEqual(key, economics.metric_cache_key(**base))
        self.assertNotEqual(
            key,
            economics.metric_cache_key(
                **{**base, "frame_inventory_hashes": ["f1", "f2-tampered"]}
            ),
        )
        self.assertNotEqual(
            key,
            economics.metric_cache_key(
                **{**base, "metric_source_bundle_sha256": "m2"}
            ),
        )
        self.assertNotEqual(
            key,
            economics.metric_cache_key(
                **{**base, "options": {"phase": "exit"}}
            ),
        )


class ArtifactIndexTests(unittest.TestCase):
    def index(self, artifacts):
        return {"schemaVersion": 1, "artifacts": artifacts}

    def test_paths_escaping_the_study_root_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            errors = economics.validate_artifact_index(
                self.index(
                    [
                        {
                            "role": "lifecycle-receipt",
                            "relativePath": "../outside.json",
                            "sha256": "aa",
                            "parents": [],
                        }
                    ]
                ),
                Path(tmp),
            )
            self.assertTrue(
                any("escapes the study root" in error for error in errors)
            )

    def test_duplicate_logical_artifact_with_different_hashes_fails(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            errors = economics.validate_artifact_index(
                self.index(
                    [
                        {
                            "role": "lifecycle-receipt",
                            "relativePath": "a/receipt.json",
                            "sha256": "aa",
                            "parents": [],
                        },
                        {
                            "role": "lifecycle-receipt",
                            "relativePath": "a/receipt.json",
                            "sha256": "bb",
                            "parents": [],
                        },
                    ]
                ),
                Path(tmp),
            )
            self.assertTrue(
                any("different hashes" in error for error in errors)
            )

    def test_unknown_parent_hash_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            errors = economics.validate_artifact_index(
                self.index(
                    [
                        {
                            "role": "scenario-metrics",
                            "relativePath": "a/metrics.json",
                            "sha256": "aa",
                            "parents": ["missing-parent"],
                        }
                    ]
                ),
                Path(tmp),
            )
            self.assertTrue(
                any("unknown parent hash" in error for error in errors)
            )

    def test_clean_index_validates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            errors = economics.validate_artifact_index(
                self.index(
                    [
                        {
                            "role": "capture-receipt",
                            "relativePath": "a/capture-receipt.json",
                            "sha256": "cap",
                            "parents": [],
                        },
                        {
                            "role": "scenario-metrics",
                            "relativePath": "a/metrics.json",
                            "sha256": "met",
                            "parents": ["cap"],
                        },
                    ]
                ),
                Path(tmp),
            )
            self.assertEqual(errors, [])


class ComparisonLedgerTests(unittest.TestCase):
    REQUIRED = ["cross-run-frame-reuse-as-replication", "embedded-actions"]

    def ledger(self):
        return [
            {
                "id": "cross-run-frame-reuse-as-replication",
                "decision": "REJECTED",
                "reason": "would turn one observation into false replication",
                "evidence": [],
            },
            {
                "id": "embedded-actions",
                "decision": "CONDITIONAL_REJECTED",
                "reason": "benchmark pending equivalence proof",
                "evidence": [],
            },
        ]

    def test_terminal_ledger_validates(self) -> None:
        self.assertEqual(
            economics.validate_comparison_ledger(self.ledger(), self.REQUIRED),
            [],
        )

    def test_pending_is_forbidden(self) -> None:
        ledger = self.ledger()
        ledger[1]["decision"] = "PENDING"
        errors = economics.validate_comparison_ledger(ledger, self.REQUIRED)
        self.assertTrue(any("PENDING is forbidden" in error for error in errors))

    def test_removing_an_entry_fails_validation(self) -> None:
        errors = economics.validate_comparison_ledger(
            self.ledger()[:1], self.REQUIRED
        )
        self.assertTrue(
            any(
                "embedded-actions: expected exactly one, observed 0" in error
                for error in errors
            )
        )

    def test_unknown_decisions_fail(self) -> None:
        ledger = self.ledger()
        ledger[0]["decision"] = "MAYBE"
        errors = economics.validate_comparison_ledger(ledger, self.REQUIRED)
        self.assertTrue(any("MAYBE" in error for error in errors))


class EconomicsComparisonTests(unittest.TestCase):
    BASELINE = {
        "source": ".artifacts/glass-entry-abba/smoke3-2026-07-25",
        "qualification": "NON_QUALIFYING_LOAD",
        "sessionWallMs": 65067.0,
        "capturedMetricFamilyObservations": 12,
        "promotedMetricFamilyObservations": 6,
    }

    def test_baseline_is_labeled_nonqualifying_but_still_compared(self) -> None:
        comparison = economics.economics_comparison(
            self.BASELINE,
            {
                "wallMs": 60000.0,
                "capturedUnits": 36,
                "promotedUnits": 30,
                "displayExclusiveMs": 40000.0,
                "equivalentRepeatedPairProjectedWallMs": 120000.0,
            },
        )
        historical = comparison["historicalBaseline"]
        self.assertFalse(historical["acceptanceQualifying"])
        self.assertEqual(historical["qualification"], "NON_QUALIFYING_LOAD")
        self.assertAlmostEqual(
            historical["wallMsPerPromotedUnit"], 65067.0 / 6
        )
        current = comparison["current"]
        self.assertAlmostEqual(current["wallMsPerPromotedUnit"], 2000.0)
        self.assertAlmostEqual(
            current["displayExclusiveMsPerPromotedUnit"], 40000.0 / 30
        )
        savings = comparison["savings"]
        self.assertAlmostEqual(savings["absoluteReductionMs"], 60000.0)
        self.assertAlmostEqual(savings["percentageReduction"], 50.0)

    def test_zero_promoted_units_never_divides(self) -> None:
        comparison = economics.economics_comparison(
            self.BASELINE,
            {"wallMs": 60000.0, "capturedUnits": 0, "promotedUnits": 0},
        )
        self.assertIsNone(comparison["current"]["wallMsPerPromotedUnit"])
        self.assertIsNone(comparison["current"]["wallMsPerCapturedUnit"])


if __name__ == "__main__":
    unittest.main()
