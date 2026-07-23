#!/usr/bin/env python3
"""Deterministic unit checks for the glass CIEDE2000 implementation."""

import importlib.util
from pathlib import Path


module_path = Path(__file__).with_name("glass-contrast-metrics.py")
spec = importlib.util.spec_from_file_location("glass_contrast_metrics", module_path)
assert spec and spec.loader
metrics = importlib.util.module_from_spec(spec)
spec.loader.exec_module(metrics)


def main() -> None:
    # Published Sharma/Wu/Dalal CIEDE2000 reference pairs.
    pairs = [
        ((50.0, 2.6772, -79.7751), (50.0, 0.0, -82.7485), 2.0425),
        ((50.0, 3.1571, -77.2803), (50.0, 0.0, -82.7485), 2.8615),
        ((50.0, 2.8361, -74.0200), (50.0, 0.0, -82.7485), 3.4412),
        ((50.0, -1.3802, -84.2814), (50.0, 0.0, -82.7485), 1.0000),
        ((50.0, 0.0, 0.0), (50.0, -1.0, 2.0), 2.3669),
    ]
    for left, right, expected in pairs:
        actual = metrics.delta_e_2000(left, right)
        assert abs(actual - expected) < 0.0002, (left, right, actual, expected)
        reverse = metrics.delta_e_2000(right, left)
        assert abs(reverse - expected) < 0.0002, (right, left, reverse, expected)
    print(f"ok: {len(pairs)} CIEDE2000 reference pairs")


if __name__ == "__main__":
    main()
