"""Shared numeric summaries for process and browser measurements.

The reported 95-percent interval is explicitly a normal approximation. Both
measurement paths use this one implementation and the same sample variance.
"""
from __future__ import annotations

from typing import Iterable, Mapping

NORMAL_95_CRITICAL = 1.96


def summarize_numeric_samples(values: Iterable[int | float]) -> Mapping[str, float | int | None]:
    """Return a bounded sample spread and an explicitly approximate interval."""
    numbers = tuple(float(value) for value in values)
    if not numbers:
        return {"count": 0, "mean": None, "sample_stddev": None, "approximate_95_half_width": None}
    mean = sum(numbers) / len(numbers)
    if len(numbers) < 2:
        deviation = None
        half_width = None
    else:
        deviation = (sum((value - mean) ** 2 for value in numbers) / (len(numbers) - 1)) ** 0.5
        half_width = NORMAL_95_CRITICAL * deviation / (len(numbers) ** 0.5)
    return {
        "count": len(numbers),
        "mean": mean,
        "sample_stddev": deviation,
        "approximate_95_half_width": half_width,
    }

