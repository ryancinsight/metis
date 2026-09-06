"""Independent codec fixtures and end-to-end visual evidence failure tests."""
import csv
import hashlib
import io
import json
import os
import pathlib
import struct
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import visual


def svg_fixture(width, height, pixels):
    """One explicit rectangle per pixel, independent of production run encoding."""
    lines = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" shape-rendering="crispEdges">',
             '<title>Metis form software framebuffer</title>']
    for index in range(width * height):
        blue, green, red, alpha = pixels[index * 4:index * 4 + 4]
        x, y = index % width, index // width
        lines.append(f'<path fill="#{red:02x}{green:02x}{blue:02x}" fill-opacity="{alpha / 255}" d="M{x} {y}h1v1H{x}z"/>')
    return ("\n".join((*lines, "</svg>")) + "\n").encode()


def bmp_fixture(width, height, pixels):
    image_size = width * height * 4
    header = struct.pack("<2sIII IiiHHII16s", b"BM", 54 + image_size, 0, 54,
                         40, width, height, 1, 32, 0, image_size, bytes(16))
    stride = width * 4
    return header + b"".join(pixels[row * stride:(row + 1) * stride] for row in reversed(range(height)))


def solid_svg(color="#000000", first_length=800):
    """800x600 fixture made of independently specified opaque scanline rectangles."""
    rows = [f'<path fill="{color}" fill-opacity="1" d="M0 0h{first_length}v1H0z"/>']
    if first_length != 800:
        rows.append(f'<path fill="#000000" fill-opacity="1" d="M{first_length} 0h{800 - first_length}v1H{first_length}z"/>')
    rows.extend(f'<path fill="#000000" fill-opacity="1" d="M0 {y}h800v1H0z"/>' for y in range(1, 600))
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600" shape-rendering="crispEdges">\n'
            '<title>Metis form software framebuffer</title>\n' + "\n".join(rows) + '\n</svg>\n').encode()


def semantics_fixture(name):
    state = {"form": "idle", "form-success": "success", "form-edited": "idle",
             "form-rejected": "rejected", "form-corrected": "success",
             "form-disconnected": "disconnected", "form-recovered": "success"}[name]
    observation = {"state": state}
    if state == "success":
        observation.update(rate_ml_hr="0.36", drug_rate_mg_hr="0.72", audit_sequence_id="2")
    elif state != "idle":
        observation["error_code"] = "12289" if state == "rejected" else "16386"
    labels = {key: key for key in ("status-badge", "label-patient", "label-weight", "label-conc",
                                   "label-dose", "output-rate", "output-status", "output-signature")}
    return {"meta": {"schema": "1", "scenario": name, "target": "software", "width": "800",
                     "height": "600", "scale": "1", "font": "metis-platform-bitmap"},
            "input": {"patient_id": 'patient, "quoted"', "weight_kg": "60", "concentration_mg_ml": "2", "target_dose_mcg_kg_min": "0.2"},
            "observed": observation.copy(), "expected": observation.copy(), "label": labels,
            "text": {str(i): text for i, text in enumerate(labels.values())},
            "geometry": {str(i): f"0 {i * 16} 1" for i in range(len(labels))},
            "action": {"0": 'set patient, "quoted"\nsubmit'}}


def encode_semantics(values):
    stream = io.StringIO(newline="")
    writer = csv.writer(stream)
    writer.writerow(["category", "key", "value"])
    for category, fields in values.items():
        writer.writerows((category, key, value) for key, value in fields.items())
    return stream.getvalue().encode()


class CodecTests(unittest.TestCase):
    def setUp(self):
        self.pixels = bytes((0, 0, 255, 255, 0, 255, 0, 128, 255, 0, 0, 255, 0, 0, 0, 255))
        self.image = (2, 2, self.pixels)

    def test_independent_bitmap_and_svg_values(self):
        self.assertEqual(visual.decode_bitmap(bmp_fixture(2, 2, self.pixels)), self.image)
        self.assertEqual(visual.decode_svg(svg_fixture(2, 2, self.pixels)), self.image)
        self.assertEqual(visual.decode_svg(visual.encode_svg(self.image)), self.image)

    def test_bitmap_header_and_truncation_are_rejected_before_pixels(self):
        valid = bmp_fixture(2, 2, self.pixels)
        altered = bytearray(valid)
        struct.pack_into("<i", altered, 18, 801)
        for malformed in (b"", valid[:53], valid[:-1], valid + b"x", bytes(altered), b"xx" + valid[2:]):
            with self.subTest(length=len(malformed)), self.assertRaises(visual.VisualError):
                visual.decode_bitmap(malformed)

    def test_svg_rejects_overlap_gaps_external_content_and_bounds(self):
        valid = svg_fixture(2, 2, self.pixels)
        malformed = (valid[:-7], valid.replace(b'h1v1H0z', b'h3v1H0z', 1),
                     valid.replace(b'M1 0h1v1H1z', b'M0 0h1v1H0z'),
                     valid.replace(b'width="2"', b'width="801"', 1),
                     valid.replace(b'fill-opacity="1.0"', b'fill-opacity="NaN"', 1),
                     valid.replace(b'<title>', b'<!DOCTYPE svg><title>', 1),
                     b'<?probe unsafe?>' + valid, b'x' * (visual.MAX_BYTES + 1))
        for content in malformed:
            with self.subTest(prefix=content[:50]), self.assertRaises(visual.VisualError):
                visual.decode_svg(content)

    def test_svg_rejects_encoded_doctypes_before_xml_autodetection(self):
        valid = svg_fixture(2, 2, self.pixels).decode("ascii")
        declared = '<!DOCTYPE svg [<!ENTITY title "Metis form software framebuffer">]>' + valid.replace(
            "<title>Metis form software framebuffer</title>", "<title>&title;</title>")
        for encoding in ("utf-16", "utf-16-le", "utf-16-be", "utf-32-le", "utf-32-be"):
            for text in (valid, declared):
                with self.subTest(encoding=encoding, declared=text is declared), self.assertRaises(visual.VisualError):
                    visual.decode_svg(text.encode(encoding))

    def test_difference_has_exact_count_bounds_and_colors(self):
        actual = bytearray(self.pixels)
        actual[4:8] = bytes((0, 0, 0, 255))
        actual[8:12] = bytes((0, 0, 0, 255))
        stats, image = visual.difference(self.image, (2, 2, bytes(actual)))
        self.assertEqual(stats, {"changed_pixels": 2, "bounds": [0, 0, 2, 2]})
        self.assertEqual(image[2], bytes((0, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 0, 255)))
        unchanged, _ = visual.difference(self.image, self.image)
        self.assertEqual(unchanged, {"changed_pixels": 0, "bounds": None})
        with self.assertRaises(visual.VisualError):
            visual.difference(self.image, (2, 2, b""))

    def test_semantics_quotes_duplicate_fields_missing_labels_and_geometry(self):
        values = semantics_fixture("form")
        encoded = encode_semantics(values)
        self.assertEqual(visual.read_semantics(encoded, "form"), values)
        with self.assertRaises(visual.VisualError):
            visual.read_semantics(encoded + b"observed,state,idle\r\n", "form")
        for category, key, replacement in (("geometry", "0", "799 0 1"), ("meta", "width", "801"),
                                            ("input", "weight_kg", "NaN"), ("label", "output-rate", None)):
            changed = semantics_fixture("form")
            if replacement is None:
                del changed[category][key]
            else:
                changed[category][key] = replacement
            with self.subTest(category=category), self.assertRaises(visual.VisualError):
                visual.read_semantics(encode_semantics(changed), "form")

    def test_oracle_preserves_numeric_bound_and_rejects_fake_integer(self):
        values = semantics_fixture("form-success")
        values["observed"]["rate_ml_hr"] = repr(0.36 + sys.float_info.epsilon * 0.36)
        self.assertEqual(visual._oracle(values), [])
        values["observed"]["rate_ml_hr"] = "0.40"
        self.assertEqual(visual._oracle(values)[0]["path"], "observed.rate_ml_hr")
        for field, invalid in (("audit_sequence_id", "garbage"), ("audit_sequence_id", "0"),
                               ("audit_sequence_id", str(2**64))):
            values = semantics_fixture("form-success")
            values["observed"][field] = values["expected"][field] = invalid
            with self.assertRaises(visual.VisualError):
                visual._oracle(values)
        values = semantics_fixture("form-rejected")
        values["observed"]["error_code"] = values["expected"]["error_code"] = "65536"
        with self.assertRaises(visual.VisualError):
            visual._oracle(values)


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name).resolve()
        self.output = self.root / "output"
        (self.root / "docs/manual/images").mkdir(parents=True)
        self.sources = {}
        for relative in ("examples/presentation.rs", "examples/presentation/capture.rs",
                         "crates/metis-platform/src/font.rs", "crates/metis-frontend/src/presentation.rs"):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f"// Fixture source: {relative}\n", encoding="utf-8")
            self.sources[str(path.resolve())] = hashlib.sha256(path.read_bytes()).hexdigest()
        self.lock = b"# Independent comparison fixture lock\n"
        (self.root / "Cargo.lock").write_bytes(self.lock)
        self.provenance = {"sources": self.sources, "host": "independent-fixture",
                           "lock_sha256": hashlib.sha256(self.lock).hexdigest()}

    def produce(self):
        self.provenance["run_nonce"] = visual.begin_run(self.output)
        black = bytes((0, 0, 0, 255)) * (800 * 600)
        for name in visual.CAPTURES:
            (self.output / f"{name}.svg").write_bytes(solid_svg())
            (self.output / f"{name}.bmp").write_bytes(bmp_fixture(800, 600, black))
            (self.output / f"{name}.csv").write_bytes(encode_semantics(semantics_fixture(name)))
        for name in visual.PROBES:
            (self.output / f"{name}.svg").write_bytes(solid_svg("#ff0000", 1))
            (self.output / f"{name}.bmp").write_bytes(bmp_fixture(800, 600, bytes((0, 0, 255, 255)) + black[4:]))

    def report_failure(self, update=False):
        with self.assertRaises(visual.VisualError) as failure:
            visual.compare(self.root, self.output, self.provenance, update)
        self.assertEqual(failure.exception.report_path, self.output / "visual/latest/report.json")
        return json.loads(failure.exception.report_path.read_text(encoding="utf-8"))

    def test_baseline_acceptance_and_collection_of_every_failure(self):
        self.produce()
        first = visual.compare(self.root, self.output, self.provenance, update=True)
        self.assertEqual(first["status"], "passed")
        self.assertEqual(set(first["captures"]), set(visual.CAPTURES))
        self.assertEqual(first["probes"]["probe-label"]["pixels"], {"changed_pixels": 1, "bounds": [0, 0, 1, 1]})
        baseline_path = self.root / "docs/manual/images/captures.json"
        baseline = baseline_path.read_bytes()
        self.produce()
        second = visual.compare(self.root, self.output, self.provenance)
        self.assertEqual(second["status"], "passed")
        self.assertEqual(second["fixture_sha256"], first["fixture_sha256"])
        self.assertEqual(baseline_path.read_bytes(), baseline)
        # Recompare the same capture run after corruption: no begin_run call may
        # be needed to invalidate the second comparison's old success manifest.
        (self.output / "form.svg").unlink()
        (self.output / "form-success.bmp").write_bytes(b"BM")
        wrong_pixels = bytearray((self.output / "form-edited.bmp").read_bytes())
        wrong_pixels[54] = 255
        (self.output / "form-edited.bmp").write_bytes(wrong_pixels)
        values = semantics_fixture("form-rejected")
        values["observed"]["error_code"] = "16386"
        (self.output / "form-rejected.csv").write_bytes(encode_semantics(values))
        values = semantics_fixture("form-corrected")
        values["geometry"]["0"] = "799 0 1"
        (self.output / "form-corrected.csv").write_bytes(encode_semantics(values))
        (self.output / "form-disconnected.svg").write_bytes(b"x" * (visual.MAX_BYTES + 1))
        values = semantics_fixture("form-recovered")
        values["action"]["0"] = "different input trace"
        (self.output / "form-recovered.csv").write_bytes(encode_semantics(values))
        failed = self.report_failure()
        self.assertEqual([item["status"] for item in failed["captures"].values()], ["failed"] * 7)
        self.assertIn("BMP and SVG pixels disagree", failed["captures"]["form-edited"]["errors"])
        self.assertEqual(failed["captures"]["form-rejected"]["semantic_diff"][0]["path"], "observed.error_code")
        self.assertTrue(any(item["path"] == "action.0" for item in failed["captures"]["form-recovered"]["semantic_diff"]))
        self.assertEqual(baseline_path.read_bytes(), baseline)
        self.assertFalse((self.output / "visual/latest/manifest.json").exists())

    def test_bad_oracles_or_provenance_never_update_baselines(self):
        self.produce()
        values = semantics_fixture("form-success")
        values["observed"]["rate_ml_hr"] = "0"
        (self.output / "form-success.csv").write_bytes(encode_semantics(values))
        self.provenance["run_nonce"] = "0" * 32
        self.provenance["sources"][next(iter(self.sources))] = "0" * 64
        failed = self.report_failure(update=True)
        self.assertIn("Missing current begin_run marker", failed["errors"])
        self.assertEqual(failed["captures"]["form-success"]["semantic_diff"][0]["actual"], "0")
        self.assertFalse((self.root / "docs/manual/images/captures.json").exists())

    def test_fixture_digest_is_portable_and_tracks_nested_source(self):
        digest = visual._fixture(self.root, self.provenance)
        with tempfile.TemporaryDirectory() as other_directory:
            other_root = pathlib.Path(other_directory).resolve()
            (other_root / "Cargo.lock").write_bytes(self.lock)
            copied_sources = {}
            for source, source_digest in self.sources.items():
                original = pathlib.Path(source)
                copied = other_root / original.relative_to(self.root)
                copied.parent.mkdir(parents=True, exist_ok=True)
                copied.write_bytes(original.read_bytes())
                copied_sources[str(copied)] = source_digest
            other_provenance = {**self.provenance, "sources": copied_sources}
            self.assertEqual(visual._fixture(other_root, other_provenance), digest)
        path = self.root / "examples/presentation/capture.rs"
        path.write_text("// Changed trace\n", encoding="utf-8")
        with self.assertRaisesRegex(visual.VisualError, "Stale source"):
            visual._fixture(self.root, self.provenance)
        self.provenance["sources"][str(path.resolve())] = hashlib.sha256(path.read_bytes()).hexdigest()
        self.assertNotEqual(visual._fixture(self.root, self.provenance), digest)
        del self.provenance["sources"][str(path.resolve())]
        with self.assertRaisesRegex(visual.VisualError, "Missing fixture source"):
            visual._fixture(self.root, self.provenance)

    def test_source_provenance_rejects_stale_lock_and_missing_host(self):
        for changed, message in (({"lock_sha256": "a" * 64}, "Stale lock provenance"),
                                 ({"host": ""}, "Missing source provenance")):
            with self.subTest(changed=changed), self.assertRaisesRegex(visual.VisualError, message):
                visual._fixture(self.root, {**self.provenance, **changed})

    def test_json_rejects_duplicate_fields_and_nonfinite_constants(self):
        path = self.root / "provenance.json"
        for content in ('{"schema":1,"schema":2}', '{"sources":{"path":"a","path":"b"}}',
                        '{"value":NaN}', '{"value":Infinity}', '[incomplete'):
            path.write_text(content, encoding="utf-8")
            with self.subTest(content=content), self.assertRaises(visual.VisualError):
                visual._load_json(path)
        path.write_text('{"schema":1}', encoding="utf-8")
        self.assertEqual(visual._load_json(path), {"schema": 1})

    def test_owned_paths_reject_resolved_escape(self):
        with self.assertRaises(visual.VisualError):
            visual._owned(self.root / ".." / "outside.svg", self.root)
        self.assertEqual(visual._owned(self.output / "form.svg", self.root), self.output / "form.svg")

    def test_owned_retention_and_json_writes_reject_actual_hardlinks(self):
        visual.begin_run(self.output)
        original = self.root / "unique.txt"
        original.write_text("unique work", encoding="utf-8")
        link = self.output / "visual/latest/manifest.json"
        os.link(original, link)
        with self.assertRaisesRegex(visual.VisualError, "Hardlinked"):
            visual._owned(link, self.output)
        with self.assertRaisesRegex(visual.VisualError, "Hardlinked"):
            visual._json(link, {"status": "running"})
        with self.assertRaisesRegex(visual.VisualError, "Hardlinked"):
            visual.begin_run(self.output)
        self.assertEqual(original.read_text(encoding="utf-8"), "unique work")
        self.assertEqual(link.stat().st_nlink, 2)

    def test_owned_retention_and_json_writes_reject_directory_redirects(self):
        target = self.output / "user-data"
        (target / "latest").mkdir(parents=True)
        original = target / "latest/report.json"
        original.write_text("unique work", encoding="utf-8")
        link = self.output / "visual"
        if sys.platform == "win32":
            subprocess.run(["cmd", "/d", "/c", "mklink", "/J", str(link), str(target)],
                           capture_output=True, text=True, timeout=5, check=True)
        else:
            link.symlink_to(target, target_is_directory=True)
        try:
            # The redirect remains within output, so a containment-only check is insufficient.
            with self.assertRaisesRegex(visual.VisualError, "redirected"):
                visual.begin_run(self.output)
            with self.assertRaisesRegex(visual.VisualError, "redirected"):
                visual._json(link / "latest/report.json", {"status": "running"})
            self.assertEqual(original.read_text(encoding="utf-8"), "unique work")
        finally:
            if sys.platform == "win32":
                link.rmdir()
            else:
                link.unlink()

    def test_cli_invalid_provenance_clears_previous_success_evidence(self):
        visual.begin_run(self.output)
        latest = self.output / "visual/latest"
        provenance_path = self.output / "provenance.json"
        for content in ("not-json", None):
            if content is None:
                provenance_path.unlink()
            else:
                provenance_path.write_text(content, encoding="utf-8")
            for name in ("report.json", "manifest.json"):
                (latest / name).write_text('{"status":"passed"}', encoding="utf-8")
            result = subprocess.run([sys.executable, str(pathlib.Path(visual.__file__)),
                                     "--root", str(self.root), "--output", str(self.output),
                                     "--provenance", str(provenance_path)],
                                    capture_output=True, text=True, timeout=60, check=False)
            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            self.assertFalse((latest / "manifest.json").exists())
            failed = json.loads((latest / "report.json").read_text(encoding="utf-8"))
            self.assertEqual(failed["status"], "failed")
            self.assertEqual(set(failed["captures"]), set(visual.CAPTURES))
            self.assertTrue(any("provenance.json" in error for error in failed["errors"]))

    def test_report_write_failure_cannot_publish_success_manifest(self):
        visual.begin_run(self.output)
        latest = self.output / "visual/latest"
        original = self.root / "unique-report.txt"
        original.write_text("unique work", encoding="utf-8")
        os.link(original, latest / "report.json")
        with self.assertRaises(visual.VisualError) as failure:
            visual._record(latest, {"schema": 1, "status": "passed"}, publish=True)
        self.assertEqual(failure.exception.report_path, latest / "report.json")
        self.assertFalse((latest / "manifest.json").exists())
        self.assertEqual(original.read_text(encoding="utf-8"), "unique work")
        (latest / "manifest.json").write_text('{"status":"passed"}', encoding="utf-8")
        with self.assertRaises(visual.VisualError):
            visual.compare(self.root, self.output, self.provenance)
        self.assertFalse((latest / "manifest.json").exists())
        self.assertEqual(original.read_text(encoding="utf-8"), "unique work")

    def test_begin_run_rotates_only_known_files_and_erases_stale_captures(self):
        nonce = visual.begin_run(self.output)
        latest, previous = self.output / "visual/latest", self.output / "visual/previous"
        (latest / "report.json").write_text('"first"', encoding="utf-8")
        (latest / "user-notes.txt").write_text("unique work", encoding="utf-8")
        (self.output / "form.svg").write_text("old capture", encoding="utf-8")
        self.assertNotEqual(visual.begin_run(self.output), nonce)
        self.assertEqual((previous / "report.json").read_text(), '"first"')
        self.assertEqual((latest / "user-notes.txt").read_text(), "unique work")
        self.assertFalse((self.output / "form.svg").exists())
        (latest / "report.json").write_text('"second"', encoding="utf-8")
        visual.begin_run(self.output)
        self.assertEqual((previous / "report.json").read_text(), '"second"')


if __name__ == "__main__":
    unittest.main()
