"""Typed interface for the native ``metis._metis`` extension."""

from typing import Optional


class Application:
    def __init__(self, width: int, height: int) -> None: ...

    @property
    def generation(self) -> int: ...

    @property
    def width(self) -> int: ...

    @property
    def height(self) -> int: ...

    def clear(
        self, generation: int, red: int, green: int, blue: int, alpha: int
    ) -> None: ...

    def to_rgba(self, generation: int) -> bytes: ...

    def key_down(self, generation: int, key: int) -> None: ...

    def pointer_down(self, generation: int, x: int, y: int) -> None: ...

    def char_input(self, generation: int, value: str) -> None: ...

    def quit(self, generation: int) -> None: ...

    def poll_event(self, generation: int) -> Optional[dict[str, object]]: ...

    def close(self, generation: int) -> None: ...

    def reopen(self, width: int, height: int) -> int: ...


class Rect:
    def __init__(self, x: int, y: int, width: int, height: int) -> None: ...

    @property
    def x(self) -> int: ...

    @property
    def y(self) -> int: ...

    @property
    def width(self) -> int: ...

    @property
    def height(self) -> int: ...


class RasterImage:
    def __init__(self, width: int, height: int, rgba: bytes) -> None: ...

    @property
    def width(self) -> int: ...

    @property
    def height(self) -> int: ...


class Canvas:
    def __init__(self, width: int, height: int) -> None: ...

    @property
    def width(self) -> int: ...

    @property
    def height(self) -> int: ...

    def clear(self, red: int, green: int, blue: int, alpha: int) -> None: ...

    def draw_image(
        self, image: RasterImage, source: Rect, destination: Rect
    ) -> None: ...

    def to_rgba(self) -> bytes: ...


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
