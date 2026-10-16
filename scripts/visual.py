"""Bounded software capture validation, baseline comparison and review evidence."""
import csv
import argparse
import hashlib
import io
import json
import math
import pathlib
import re
import stat
import sys
import uuid

import visual_tools.codec
import visual_tools.contract
import visual_tools.difference

CAPTURES = ("form", "form-menu", "form-menu-dark", "form-focus", "form-success", "form-edited",
            "form-rejected", "form-corrected", "form-disconnected", "form-recovered")
PROBES = ("probe-label", "probe-geometry", "probe-color")
ASSETS = ("image-placement",)
_ARTIFACTS = ("report.json", "manifest.json", "run.json") + tuple(
    f"{name}-{kind}.png" for name in (*CAPTURES, *PROBES)
    for kind in ("expected", "actual", "difference")) + tuple(
    f"{name}-semantics.json" for name in CAPTURES) + tuple(
    f"{name}-{kind}.png" for name in ASSETS
    for kind in ("expected", "actual", "difference"))


def _redirects(path):
    try:
        attributes = path.lstat()
    except FileNotFoundError:
        return False
    return stat.S_ISLNK(attributes.st_mode) or (
        sys.platform == "win32" and bool(attributes.st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT))


def _plain(path):
    visual_tools.contract.require(not any(_redirects(parent) for parent in (path, *path.parents)),
             f"Linked or redirected visual output path: {path}")
    visual_tools.contract.require(not path.is_file() or path.stat().st_nlink == 1, f"Hardlinked visual output path: {path}")


def _owned(path, output):
    _plain(path)
    visual_tools.contract.require(path.resolve().is_relative_to(output), f"Unsafe visual output path: {path}")
    return path


def begin_run(output):
    """Rotate only declared files, preserving unknown user files and one prior run."""
    output = pathlib.Path(output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    visual = _owned(output / "visual", output)
    latest, previous = (_owned(visual / name, output) for name in ("latest", "previous"))
    for directory in (visual, latest, previous):
        directory.mkdir(exist_ok=True)
    for name in _ARTIFACTS:
        old, current = (_owned(directory / name, output) for directory in (previous, latest))
        old.unlink(missing_ok=True)
        if current.exists():
            current.replace(old)
    for name in (*CAPTURES, *PROBES, *ASSETS):
        for suffix in (".png", ".bmp", ".csv"):
            _owned(output / (name + suffix), output).unlink(missing_ok=True)
    nonce = uuid.uuid4().hex
    _json(latest / "run.json", {"schema": 1, "nonce": nonce})
    return nonce


def _read(path, limit=visual_tools.contract.MAX_BYTES):
    visual_tools.contract.require(path.is_file() and not path.is_symlink(), f"Missing or linked artifact: {path}")
    with path.open("rb") as stream:
        content = stream.read(limit + 1)
    visual_tools.contract.require(len(content) <= limit, f"Artifact exceeds {limit} bytes: {path}")
    return content


def source_digest(path):
    """Hash a repository text source using its canonical LF representation."""
    content = path.read_bytes()
    return hashlib.sha256(content.replace(b"\r\n", b"\n").replace(b"\r", b"\n")).hexdigest()


def read_semantics(content, name):
    """Validate the CSV observation contract without accepting duplicate fields."""
    visual_tools.contract.require(len(content) <= 64 * 1024, "Semantic capture exceeds 64 KiB")
    try:
        rows = list(csv.reader(io.StringIO(content.decode("utf-8")), strict=True))
    except (UnicodeError, csv.Error) as error:
        raise visual_tools.contract.VisualError("Malformed semantic CSV") from error
    visual_tools.contract.require(rows and rows[0] == ["category", "key", "value"] and len(rows) <= 512,
             "Semantic header or row budget differs")
    values = {key: {} for key in ("meta", "input", "observed", "expected", "label", "geometry", "text", "action")}
    for row in rows[1:]:
        visual_tools.contract.require(len(row) == 3, "Semantic row does not have three fields")
        category, key, value = row
        visual_tools.contract.require(category in values and key and key not in values[category]
                 and len(key) <= 128 and len(value) <= 4096, "Duplicate or unsupported semantic field")
        values[category][key] = value
    visual_tools.contract.require(values["meta"] == {"schema": "1", "scenario": name, "target": "software",
             "width": "800", "height": "600", "scale": "1.000x", "font": "atkinson-hyperlegible"},
             "Capture target or schema differs")
    _scale_milli(values["meta"]["scale"])
    visual_tools.contract.require(set(values["input"]) == {"patient_id", "weight_kg", "concentration_mg_ml", "target_dose_mcg_kg_min"},
             "Capture inputs are incomplete")
    for field in ("weight_kg", "concentration_mg_ml", "target_dose_mcg_kg_min"):
        _number(values["input"][field])
    visual_tools.contract.require(set(values["label"]) == {"status-badge", "label-patient", "label-weight", "label-conc",
             "label-dose", "output-rate", "output-status", "output-signature"}
             and values["action"] and values["text"], "Missing labels, actions or text")
    visual_tools.contract.require(set(values["label"].values()).issubset(values["text"].values()), "Labels differ from display-list text")
    visual_tools.contract.require(set(values["text"]) == set(values["geometry"]), "Text and geometry identities differ")
    for category in ("action", "text"):
        visual_tools.contract.require(set(values[category]) == {str(index) for index in range(len(values[category]))},
                 f"{category} indices are not contiguous")
    for geometry in values["geometry"].values():
        # Origin, size in pixels per em, and the run's measured width and line
        # height in whole pixels, as the capture's layout reported them.
        match = re.fullmatch(r"(\d{1,6}) (\d{1,6}) ([1-9][0-9]{0,3}(?:\.[0-9]{1,6})?) (\d{1,6}) (\d{1,6})",
                             geometry)
        visual_tools.contract.require(match is not None, "Malformed text geometry")
        x, y, width, height = (int(match.group(index)) for index in (1, 2, 4, 5))
        visual_tools.contract.require(width > 0 and height > 0, "Malformed text geometry")
        visual_tools.contract.require(x + width <= 800 and y + height <= 600, "Captured text is clipped")
    return values


def _number(value):
    try:
        number = float(value)
    except ValueError as error:
        raise visual_tools.contract.VisualError(f"Malformed number: {value}") from error
    visual_tools.contract.require(math.isfinite(number), "Nonfinite semantic number")
    return number


_DISPLAY_SCALE = re.compile(r"(?:0|[1-9][0-9]*)\.[0-9]{3}x\Z")


def _scale_milli(value):
    """Parse the fixed-point display scale emitted by the Rust capture."""
    visual_tools.contract.require(isinstance(value, str) and _DISPLAY_SCALE.fullmatch(value) is not None,
             "Malformed display scale")
    whole, fraction = value[:-1].split(".")
    visual_tools.contract.require(len(whole) <= 10, "Display scale exceeds the capture range")
    milli = int(whole) * 1_000 + int(fraction)
    visual_tools.contract.require(0 < milli <= 0xFFFF_FFFF, "Display scale exceeds the capture range")
    return milli


def _oracle(values):
    observed, expected = values["observed"], values["expected"]
    fields = {"idle": {"state"}, "success": {"state", "rate_ml_hr", "drug_rate_mg_hr", "audit_sequence_id"},
              "rejected": {"state", "error_code"}, "disconnected": {"state", "error_code"}}
    state = expected.get("state")
    visual_tools.contract.require(state in fields and set(expected) == fields[state] and set(observed) == fields[state],
             "Missing or unsupported state observation")
    for fields_to_check in (observed, expected):
        for field, limit in (("audit_sequence_id", 2**64 - 1), ("error_code", 2**16 - 1)):
            if field in fields_to_check:
                value = fields_to_check[field]
                visual_tools.contract.require(re.fullmatch(r"[0-9]{1,20}", value) is not None and int(value) <= limit
                         and (field != "audit_sequence_id" or int(value) > 0), "Invalid integer observation")
    deviations = []
    gamma = 6 * sys.float_info.epsilon / (1 - 6 * sys.float_info.epsilon)
    for field in expected:
        left, right = expected[field], observed[field]
        if field in ("rate_ml_hr", "drug_rate_mg_hr"):
            wanted, actual = _number(left), _number(right)
            equal = wanted >= 0 and actual >= 0 and abs(wanted - actual) <= abs(wanted) * gamma
        else:
            equal = left == right
        if not equal:
            deviations.append({"path": f"observed.{field}", "expected": left, "actual": right})
    return deviations


def _json(path, content):
    _plain(path)
    # LF on every host, matching the repository's normalized text.
    path.write_text(json.dumps(content, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")


def _load_json(path):
    def unique_fields(pairs):
        result = {}
        for key, value in pairs:
            visual_tools.contract.require(key not in result, f"Duplicate JSON field: {key}")
            result[key] = value
        return result

    try:
        return json.loads(_read(path), object_pairs_hook=unique_fields,
                          parse_constant=lambda value: visual_tools.contract.require(False, f"Invalid JSON constant: {value}"))
    except (ValueError, UnicodeError) as error:
        raise visual_tools.contract.VisualError(f"Malformed JSON: {path}") from error


def _baseline(path):
    baseline = _load_json(path)
    visual_tools.contract.require(isinstance(baseline, dict) and baseline.get("schema") == 2
             and set(baseline) == {"schema", "captures"}
             and isinstance(baseline.get("captures"), dict)
             and set(baseline["captures"]) == set(CAPTURES), "Missing or malformed visual baseline")
    for entry in baseline["captures"].values():
        visual_tools.contract.require(isinstance(entry, dict) and isinstance(entry.get("semantics"), dict)
                 and isinstance(entry.get("image_sha256"), str), "Malformed semantic baseline")
        for fields in entry["semantics"].values():
            visual_tools.contract.require(isinstance(fields, dict) and all(isinstance(k, str) and isinstance(v, str)
                     for k, v in fields.items()), "Malformed baseline fields")
    return baseline


def _fixture(root, provenance):
    visual_tools.contract.require(isinstance(provenance, dict) and isinstance(provenance.get("sources"), dict)
             and isinstance(provenance.get("host"), str) and provenance["host"], "Missing source provenance")
    visual_tools.contract.require(re.fullmatch(r"[0-9a-f]{64}", provenance.get("lock_sha256", "")) is not None,
             "Malformed lock provenance")
    visual_tools.contract.require(source_digest(root / "Cargo.lock") == provenance["lock_sha256"],
             "Stale lock provenance")
    selected = {}
    sources = provenance["sources"]
    for path_text, digest in sources.items():
        path = pathlib.Path(path_text).resolve()
        visual_tools.contract.require(isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest) is not None,
                 "Malformed source digest")
        visual_tools.contract.require(path.is_file() and source_digest(path) == digest,
                 f"Stale source provenance: {path}")
    roots = (root / "examples" / "presentation", root / "crates" / "metis-frontend" / "src",
             root / "crates" / "metis-platform" / "src", root / "crates" / "metis-ui-lang" / "src")
    paths = {root / "examples" / "presentation.rs", root / "examples" / "image.rs"}
    paths.update(path for directory in roots for path in directory.rglob("*.rs"))
    # The embedded typefaces decide every text pixel.
    paths.update((root / "crates" / "metis-platform" / "fonts").glob("*.ttf"))
    for path in paths:
        digest = sources.get(str(path.resolve()))
        visual_tools.contract.require(digest is not None, f"Missing fixture source provenance: {path}")
        selected[path.relative_to(root).as_posix()] = digest
    encoded = json.dumps({"sources": selected, "lock_sha256": provenance["lock_sha256"]}, sort_keys=True).encode()
    return hashlib.sha256(encoded).hexdigest()


def _semantic_diff(expected, actual):
    return [{"path": f"{category}.{key}", "expected": expected.get(category, {}).get(key),
             "actual": actual.get(category, {}).get(key)}
            for category in sorted(set(expected) | set(actual))
            for key in sorted(set(expected.get(category, {})) | set(actual.get(category, {})))
            if expected.get(category, {}).get(key) != actual.get(category, {}).get(key)]


def _record(latest, report, *, publish=False):
    report_path = latest / "report.json"
    try:
        _json(report_path, report)
        if publish:
            _json(latest / "manifest.json", report)
    except (visual_tools.contract.VisualError, OSError) as error:
        raise visual_tools.contract.VisualError(f"Cannot write visual evidence: {error}", report_path) from error


def compare(root, output, provenance, update=False):
    """Collect every state and probe; replace baselines only after all intrinsic checks pass."""
    root, output = pathlib.Path(root).resolve(), pathlib.Path(output).resolve()
    latest = _owned(output / "visual" / "latest", output)
    latest.mkdir(parents=True, exist_ok=True)
    # Comparison can be retried without another capture run; a failed comparison
    # must never leave the preceding invocation's success artifact discoverable.
    _owned(latest / "manifest.json", output).unlink(missing_ok=True)
    report_path = latest / "report.json"
    report = {"schema": 1, "status": "failed", "errors": [], "captures": {}, "probes": {}, "assets": {}}
    _record(latest, report)
    fixture, baseline = None, {}
    try:
        if isinstance(provenance, pathlib.Path):
            provenance_path, provenance = provenance, None
            provenance = _load_json(provenance_path)
        nonce = provenance.get("run_nonce") if isinstance(provenance, dict) else None
        visual_tools.contract.require(isinstance(nonce, str) and re.fullmatch(r"[0-9a-f]{32}", nonce) is not None
                 and _load_json(latest / "run.json") == {"schema": 1, "nonce": nonce},
                 "Missing current begin_run marker")
        fixture = _fixture(root, provenance)
    except (visual_tools.contract.VisualError, OSError, TypeError) as error:
        report["errors"].append(str(error))
    baseline_path = root / "docs" / "manual" / "images" / "captures.json"
    if not update and fixture is not None:
        try:
            baseline = _baseline(baseline_path)
        except (visual_tools.contract.VisualError, OSError, AttributeError) as error:
            report["errors"].append(str(error))
    images, new_baseline = {}, {"schema": 2, "captures": {}}
    for name in CAPTURES:
        result = {"status": "failed", "errors": [], "semantic_diff": []}
        report["captures"][name] = result
        actual, expected, semantic = None, None, None
        try:
            actual_bytes = _read(output / f"{name}.png")
            actual = visual_tools.codec.decode_png(actual_bytes)
            visual_tools.contract.require(actual[:2] == (800, 600), "Capture viewport differs")
            images[name] = actual
            _owned(latest / f"{name}-actual.png", output).write_bytes(actual_bytes)
            visual_tools.contract.require(visual_tools.codec.decode_bitmap(_read(output / f"{name}.bmp")) == actual, "BMP and PNG pixels disagree")
            result["image_sha256"] = hashlib.sha256(actual_bytes).hexdigest()
        except (visual_tools.contract.VisualError, OSError) as error:
            result["errors"].append(str(error))
        try:
            semantic = read_semantics(_read(output / f"{name}.csv", 64 * 1024), name)
            result["semantic_diff"].extend(_oracle(semantic))
            if not update and name in baseline.get("captures", {}):
                result["semantic_diff"].extend(_semantic_diff(baseline["captures"][name]["semantics"], semantic))
            _json(latest / f"{name}-semantics.json", semantic)
            new_baseline["captures"][name] = {"semantics": semantic, "image_sha256": result.get("image_sha256")}
        except (visual_tools.contract.VisualError, OSError, KeyError, TypeError) as error:
            result["errors"].append(str(error))
        try:
            expected_bytes = _read(baseline_path.parent / f"{name}.png")
            expected = visual_tools.codec.decode_png(expected_bytes)
            _owned(latest / f"{name}-expected.png", output).write_bytes(expected_bytes)
            if actual is not None:
                result["pixels"], mask = visual_tools.difference.compare(expected, actual)
                _owned(latest / f"{name}-difference.png", output).write_bytes(visual_tools.codec.encode_png(mask))
                if not update and result["pixels"]["changed_pixels"]:
                    result["errors"].append("Golden PNG pixels differ")
                if (not update and name in baseline.get("captures", {})
                        and baseline["captures"][name]["image_sha256"] != hashlib.sha256(expected_bytes).hexdigest()):
                    result["errors"].append("Golden PNG hash differs from its semantic baseline")
        except (visual_tools.contract.VisualError, OSError) as error:
            if not update:
                result["errors"].append(str(error))
        if not result["errors"] and not result["semantic_diff"]:
            result["status"] = "passed"
    for name in PROBES:
        result = {"status": "failed", "errors": []}
        report["probes"][name] = result
        try:
            raw = _read(output / f"{name}.png")
            actual = visual_tools.codec.decode_png(raw)
            visual_tools.contract.require(visual_tools.codec.decode_bitmap(_read(output / f"{name}.bmp")) == actual, "Probe BMP and PNG pixels disagree")
            visual_tools.contract.require("form" in images, "Initial frame unavailable for mutation probe")
            result["pixels"], mask = visual_tools.difference.compare(images["form"], actual)
            for kind, content in (("expected", _read(output / "form.png")), ("actual", raw), ("difference", visual_tools.codec.encode_png(mask))):
                _owned(latest / f"{name}-{kind}.png", output).write_bytes(content)
            visual_tools.contract.require(result["pixels"]["changed_pixels"] > 0, "Mutation probe changed no pixels")
            result["status"] = "passed"
        except (visual_tools.contract.VisualError, OSError) as error:
            result["errors"].append(str(error))
    for name in ASSETS:
        result = {"status": "failed", "errors": []}
        report["assets"][name] = result
        actual_bytes, expected_bytes = None, None
        actual, expected = None, None
        try:
            actual_bytes = _read(output / f"{name}.png")
            actual = visual_tools.codec.decode_png(actual_bytes)
            _owned(latest / f"{name}-actual.png", output).write_bytes(actual_bytes)
        except (visual_tools.contract.VisualError, OSError) as error:
            result["errors"].append(str(error))
        try:
            expected_bytes = _read(root / "docs" / "manual" / "images" / f"{name}.png")
            expected = visual_tools.codec.decode_png(expected_bytes)
            _owned(latest / f"{name}-expected.png", output).write_bytes(expected_bytes)
        except (visual_tools.contract.VisualError, OSError) as error:
            if not update:
                result["errors"].append(str(error))
        if actual is not None and expected is not None:
            result["pixels"], mask = visual_tools.difference.compare(expected, actual)
            _owned(latest / f"{name}-difference.png", output).write_bytes(visual_tools.codec.encode_png(mask))
            if not update and result["pixels"]["changed_pixels"]:
                result["errors"].append("Golden PNG pixels differ")
        if not result["errors"]:
            result["status"] = "passed"
    failed = report["errors"] or any(item["status"] != "passed"
                                     for group in ("captures", "probes", "assets")
                                     for item in report[group].values())
    report["fixture_sha256"], report["provenance"] = fixture, provenance
    if not failed and update:
        try:
            # Check every reviewed destination before the first write; linked paths
            # must not turn an explicit baseline refresh into an external write.
            destinations = [_owned(baseline_path, root)] + [
                _owned(baseline_path.parent / f"{name}.png", root) for name in CAPTURES]
            for name in CAPTURES:
                (baseline_path.parent / f"{name}.png").write_bytes(_read(output / f"{name}.png"))
            for name in ASSETS:
                (baseline_path.parent / f"{name}.png").write_bytes(_read(output / f"{name}.png"))
            _json(destinations[0], new_baseline)
        except (visual_tools.contract.VisualError, OSError) as error:
            failed = True
            report["errors"].append(str(error))
    if not failed:
        report["status"] = "passed"
    _record(latest, report, publish=not failed)
    if failed:
        raise visual_tools.contract.VisualError("Software visual verification failed", report_path)
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--provenance", type=pathlib.Path, required=True)
    parser.add_argument("--update", action="store_true")
    arguments = parser.parse_args()
    try:
        compare(arguments.root, arguments.output, arguments.provenance, arguments.update)
    except (visual_tools.contract.VisualError, OSError) as error:
        print(f"{error}; report: {getattr(error, 'report_path', None)}", file=sys.stderr)
        sys.exit(1)
    print(f"Software visual verification passed: {arguments.output / 'visual' / 'latest' / 'report.json'}")
