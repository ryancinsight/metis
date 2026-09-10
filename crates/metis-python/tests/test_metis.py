"""Value-semantic tests for the built Metis PyO3 wheel."""

import math
import sys
from concurrent.futures import ThreadPoolExecutor

import pytest

import metis


def test_calculation_matches_rust_analytical_contract() -> None:
    result = metis.calculate_infusion_rate(
        metis.PatientWeight(60.0),
        metis.DrugConcentration(2.0),
        metis.TargetDose(0.2),
    )

    assert result.rate_ml_hr == pytest.approx(0.36, rel=1e-14)
    assert result.drug_rate_mg_hr == pytest.approx(0.72, rel=1e-14)
    assert result.is_pediatric is False


def test_calculation_is_input_sensitive_and_selects_pediatric_limit() -> None:
    adult = metis.calculate_infusion_rate(
        metis.PatientWeight(60.0),
        metis.DrugConcentration(2.0),
        metis.TargetDose(0.2),
    )
    pediatric = metis.calculate_infusion_rate(
        metis.PatientWeight(25.0),
        metis.DrugConcentration(1.5),
        metis.TargetDose(0.5),
    )

    assert pediatric.rate_ml_hr != adult.rate_ml_hr
    assert pediatric.is_pediatric is True


@pytest.mark.parametrize(
    "constructor, value",
    [
        (metis.PatientWeight, math.nan),
        (metis.PatientWeight, 0.0),
        (metis.DrugConcentration, math.inf),
        (metis.DrugConcentration, 0.0),
        (metis.TargetDose, math.nan),
        (metis.TargetDose, 0.0),
    ],
)
def test_constructors_reject_invalid_values(constructor, value: float) -> None:
    with pytest.raises(ValueError):
        constructor(value)


def test_custom_safety_envelope_rejects_rate() -> None:
    with pytest.raises(ValueError, match="ERR_RATE_EXCEEDS_SAFETY_ENVELOPE"):
        metis.calculate_infusion_rate(
            metis.PatientWeight(60.0),
            metis.DrugConcentration(2.0),
            metis.TargetDose(0.2),
            metis.SafetyEnvelope(0.1, 0.1, 35.0),
        )


def test_default_envelope_matches_explicit_defaults() -> None:
    values = (
        metis.PatientWeight(60.0),
        metis.DrugConcentration(2.0),
        metis.TargetDose(0.2),
    )
    implicit = metis.calculate_infusion_rate(*values)
    explicit = metis.calculate_infusion_rate(*values, metis.SafetyEnvelope())

    assert implicit.rate_ml_hr == explicit.rate_ml_hr
    assert implicit.drug_rate_mg_hr == explicit.drug_rate_mg_hr
    assert implicit.is_pediatric == explicit.is_pediatric


def test_rust_canvas_renders_clipped_image_with_exact_rgba_output() -> None:
    image = metis.RasterImage(
        2,
        1,
        bytes((229, 62, 62, 255, 49, 130, 206, 255)),
    )
    canvas = metis.Canvas(3, 2)
    canvas.clear(255, 255, 255, 255)
    canvas.draw_image(image, metis.Rect(0, 0, 2, 1), metis.Rect(-1, 0, 4, 2))

    row = bytes(
        (
            229,
            62,
            62,
            255,
            49,
            130,
            206,
            255,
            49,
            130,
            206,
            255,
        )
    )
    assert canvas.to_rgba() == row + row


def test_rust_canvas_preserves_alpha_and_rejects_invalid_image_geometry() -> None:
    image = metis.RasterImage(1, 1, bytes((0, 0, 0, 128)))
    canvas = metis.Canvas(1, 1)
    canvas.clear(255, 255, 255, 255)
    canvas.draw_image(image, metis.Rect(0, 0, 1, 1), metis.Rect(0, 0, 1, 1))
    assert canvas.to_rgba() == bytes((127, 127, 127, 255))

    with pytest.raises(ValueError, match="ERR_RENDER_FAILURE"):
        canvas.draw_image(image, metis.Rect(1, 0, 1, 1), metis.Rect(0, 0, 1, 1))

    with pytest.raises(ValueError, match="ERR_RENDER_FAILURE"):
        metis.RasterImage(2, 1, bytes((0, 0, 0, 255)))


@pytest.mark.parametrize(
    "constructor, arguments, code",
    [
        (metis.Canvas, (0, 1), "ERR_SURFACE_ALLOCATION_ERROR"),
        (metis.RasterImage, (0, 1, b""), "ERR_SURFACE_ALLOCATION_ERROR"),
    ],
)
def test_presentation_limits_are_reported_as_stable_errors(
    constructor, arguments, code: str
) -> None:
    with pytest.raises(ValueError, match=code):
        constructor(*arguments)


def test_application_preserves_fifo_and_framebuffer_generation() -> None:
    application = metis.Application(2, 1)
    generation = application.generation
    application.clear(generation, 10, 20, 30, 255)
    assert application.to_rgba(generation) == bytes((10, 20, 30, 255)) * 2
    application.key_down(generation, 41)
    application.pointer_down(generation, 7, 9)
    assert application.poll_event(generation) == {"kind": "key_down", "key": 41}
    assert application.poll_event(generation) == {
        "kind": "pointer_down",
        "x": 7,
        "y": 9,
    }
    assert application.poll_event(generation) is None


def test_application_close_reopen_rejects_stale_generation() -> None:
    application = metis.Application(1, 1)
    old_generation = application.generation
    application.close(old_generation)
    with pytest.raises(ValueError, match="Application is closed"):
        application.to_rgba(old_generation)
    new_generation = application.reopen(1, 1)
    assert new_generation == old_generation + 1
    with pytest.raises(ValueError, match="generation is stale"):
        application.clear(old_generation, 1, 2, 3, 255)
    application.clear(new_generation, 1, 2, 3, 255)
    assert application.to_rgba(new_generation) == bytes((1, 2, 3, 255))


def test_application_queue_is_bounded() -> None:
    application = metis.Application(1, 1)
    generation = application.generation
    for key in range(1024):
        application.key_down(generation, key)
    with pytest.raises(ValueError, match="ERR_RENDER_FAILURE"):
        application.key_down(generation, 1024)


def test_application_serializes_concurrent_frame_access() -> None:
    application = metis.Application(2, 2)
    generation = application.generation

    def read_frame() -> bytes:
        return application.to_rgba(generation)

    with ThreadPoolExecutor(max_workers=4) as pool:
        frames = list(pool.map(lambda _: read_frame(), range(16)))
    assert all(frame == bytes(16) for frame in frames)


def test_application_serializes_concurrent_mutation() -> None:
    application = metis.Application(1, 1)
    generation = application.generation

    def enqueue(key: int) -> None:
        application.key_down(generation, key)

    with ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(enqueue, range(128)))

    events = [application.poll_event(generation) for _ in range(128)]
    keys = sorted(event["key"] for event in events if event is not None)
    assert keys == list(range(128))


def test_free_threaded_runtime_keeps_the_gil_disabled() -> None:
    gil_probe = getattr(sys, "_is_gil_enabled", None)
    if gil_probe is None or gil_probe():
        pytest.skip("free-threaded CPython is required for this runtime probe")
    assert gil_probe() is False
