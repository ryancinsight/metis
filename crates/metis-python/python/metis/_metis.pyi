"""Typed interface for the native ``metis._metis`` extension."""

from typing import Optional


class PatientWeight:
    def __init__(self, kilograms: float) -> None: ...

    @property
    def kilograms(self) -> float: ...


class DrugConcentration:
    def __init__(self, milligrams_per_milliliter: float) -> None: ...

    @property
    def milligrams_per_milliliter(self) -> float: ...


class TargetDose:
    def __init__(self, micrograms_per_kilogram_per_minute: float) -> None: ...

    @property
    def micrograms_per_kilogram_per_minute(self) -> float: ...


class SafetyEnvelope:
    def __init__(
        self,
        adult_rate_ml_hr: float = 300.0,
        pediatric_rate_ml_hr: float = 50.0,
        pediatric_weight_threshold_kg: float = 35.0,
    ) -> None: ...


class InfusionResult:
    @property
    def rate_ml_hr(self) -> float: ...

    @property
    def drug_rate_mg_hr(self) -> float: ...

    @property
    def is_pediatric(self) -> bool: ...


def calculate_infusion_rate(
    weight: PatientWeight,
    concentration: DrugConcentration,
    dose: TargetDose,
    envelope: Optional[SafetyEnvelope] = None,
) -> InfusionResult: ...
