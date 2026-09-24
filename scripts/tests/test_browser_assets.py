"""Check the browser shell's static trust-boundary assets."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import pathlib
import struct
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))


class BrowserAssetContractTests(unittest.TestCase):
    """The page loads only same-origin external assets under the host policy."""

    def test_document_uses_external_assets_and_strict_csp(self):
        document = (ROOT / "examples" / "browser" / "index.html").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("<style", document.lower())
        self.assertNotIn("<script type=\"module\">", document.lower())
        self.assertIn('<link rel="stylesheet" href="./styles.css">', document)
        self.assertIn('<link rel="icon" type="image/svg+xml" href="./assets/metis-mark.svg">', document)
        self.assertIn('<link rel="alternate icon" type="image/png" href="./assets/metis-mark.png">', document)
        self.assertIn('<script type="module" src="./bootstrap.js"></script>', document)
        policy = document.split('http-equiv="Content-Security-Policy" content="', 1)[1].split(
            '"', 1
        )[0]
        canonical = (ROOT / "crates" / "metis-core" / "src" / "content_security_policy.txt").read_text(
            encoding="utf-8"
        )
        self.assertEqual(policy, canonical)
        for directive in (
            "default-src 'self'",
            "script-src 'self' 'wasm-unsafe-eval'",
            "script-src 'self'",
            "style-src 'self'",
            "connect-src 'self'",
            "form-action 'self'",
            "object-src 'none'",
            "frame-ancestors 'none'",
        ):
            self.assertIn(directive, policy)
        self.assertNotIn("unsafe-inline", policy)
        self.assertNotIn("*", policy)

    def test_starter_marks_are_local_bounded_assets(self):
        png = ROOT / "examples" / "browser" / "assets" / "metis-mark.png"
        self.assertTrue(png.is_file())
        self.assertEqual(png.read_bytes()[:8], bytes.fromhex("89504e470d0a1a0a"))
        self.assertLessEqual(png.stat().st_size, 1024 * 1024)
        ico = ROOT / "examples" / "browser" / "assets" / "metis-mark.ico"
        data = ico.read_bytes()
        self.assertGreaterEqual(len(data), 6)
        self.assertEqual(data[:4], b"\x00\x00\x01\x00")
        self.assertEqual(int.from_bytes(data[4:6], "little"), 7)
        self.assertLessEqual(len(data), 1024 * 1024)
        svg = ROOT / "examples" / "browser" / "assets" / "metis-mark.svg"
        svg_text = svg.read_text(encoding="utf-8")
        self.assertLessEqual(svg.stat().st_size, 256 * 1024)
        self.assertTrue(svg_text.startswith('<svg xmlns="http://www.w3.org/2000/svg"'))
        for forbidden in ("<!", "<script", "href=", "url(", "javascript:", "https://"):
            self.assertNotIn(forbidden, svg_text.lower())
        controls = (ROOT / "crates" / "metis-web" / "src" / "controls.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn('class="metis-mark"', controls)
        self.assertIn('srcset="./assets/metis-mark.svg"', controls)
        self.assertIn('src="./assets/metis-mark.png"', controls)

    def test_playback_fixture_is_a_bounded_pcm_wave(self):
        wave = ROOT / "examples" / "browser" / "assets" / "metis-tone.wav"
        data = wave.read_bytes()
        self.assertEqual(data[:4], b"RIFF")
        self.assertEqual(data[8:12], b"WAVE")
        self.assertLessEqual(len(data), 16 * 1024)

    def test_probe_font_is_a_bounded_project_owned_woff2(self):
        font = ROOT / "examples" / "browser" / "assets" / "metis-probe.woff2"
        data = font.read_bytes()
        self.assertEqual(data[:4], b"wOF2")
        self.assertLessEqual(len(data), 64 * 1024)
        # The WOFF2 header is self-describing: flavour, total length, table count.
        flavour, length, tables = struct.unpack(">IIH", data[4:14])
        self.assertEqual(flavour, 0x00010000)
        self.assertEqual(length, len(data))
        self.assertGreaterEqual(tables, 9)

    def test_build_and_distribution_declare_the_starter_mark(self):
        manifest = json.loads((ROOT / "metis.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["icon"], "examples/browser/assets/metis-mark.ico")
        self.assertIn(
            {
                "source": "examples/browser/assets/metis-mark.png",
                "destination": "assets/metis-mark.png",
            },
            manifest["resources"],
        )
        self.assertIn(
            {
                "source": "examples/browser/assets/metis-mark.ico",
                "destination": "assets/metis-mark.ico",
            },
            manifest["resources"],
        )
        self.assertIn(
            {
                "source": "examples/browser/assets/metis-mark.svg",
                "destination": "assets/metis-mark.svg",
            },
            manifest["resources"],
        )
        self.assertIn(
            {
                "source": "examples/browser/assets/metis-tone.wav",
                "destination": "assets/metis-tone.wav",
            },
            manifest["resources"],
        )
        self.assertIn(
            {
                "source": "examples/browser/assets/metis-probe.woff2",
                "destination": "assets/metis-probe.woff2",
            },
            manifest["resources"],
        )
        browser_script = (ROOT / "scripts" / "browser.py").read_text(encoding="utf-8")
        self.assertIn("SOURCE.rglob(\"*\")", browser_script)
        self.assertIn('OUTPUT / "assets" / "metis-mark.png"', browser_script)
        self.assertIn('OUTPUT / "assets" / "metis-mark.ico"', browser_script)
        self.assertIn('OUTPUT / "assets" / "metis-mark.svg"', browser_script)

    def test_consumer_gallery_is_explicit_and_bounded(self):
        spec = importlib.util.spec_from_file_location("metis_browser_packager", SCRIPTS / "browser.py")
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        browser = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(browser)
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            package = root / "package"
            package.mkdir()
            (package / "consumer.js").write_text("export default {};\n", encoding="utf-8")
            (package / "consumer_bg.wasm").write_bytes(b"\0asm\x01\0\0\0")
            gallery = root / "gallery"
            gallery.mkdir()
            source_index = ROOT / "examples" / "browser" / "index.html"
            (gallery / "gallery.html").write_bytes(source_index.read_bytes())
            (gallery / "gallery.js").write_text("export {};\n", encoding="utf-8")
            (gallery / "gallery.css").write_text("body {}\n", encoding="utf-8")
            browser.OUTPUT = root / "output"
            browser.package_consumer(package, gallery)
            self.assertEqual(
                sorted(path.name for path in (root / "output" / "consumer").iterdir()),
                ["consumer.js", "consumer_bg.wasm"],
            )
            self.assertTrue((root / "output" / "gallery.html").is_file())
            self.assertTrue((root / "output" / "gallery.js").is_file())
            self.assertTrue((root / "output" / "gallery.css").is_file())
            (package / "unexpected.txt").write_text("ignored", encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "exactly one JavaScript"):
                browser.package_consumer(package, gallery)
            (package / "unexpected.txt").unlink()
            (gallery / "unexpected.txt").write_text("ignored", encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "exactly gallery.html"):
                browser.package_consumer(package, gallery)

    def test_default_source_has_no_consumer_gallery(self):
        source = ROOT / "examples" / "browser"
        self.assertFalse((source / "gallery.html").exists())
        self.assertFalse((source / "gallery.js").exists())
        self.assertFalse((source / "gallery.css").exists())

    def test_bootstrap_keeps_navigation_same_origin(self):
        bootstrap = (ROOT / "examples" / "browser" / "bootstrap.js").read_text(
            encoding="utf-8"
        )
        self.assertIn('import init from "./metis_web.js"', bootstrap)
        self.assertIn("wasm.metis_start()", bootstrap)
        self.assertIn("wasm.metis_stop()", bootstrap)
        self.assertIn("destination.origin !== window.location.origin", bootstrap)
        self.assertIn("event.preventDefault()", bootstrap)

    def test_http_boundary_demo_uses_the_canonical_policy(self):
        document = (ROOT / "examples" / "browser" / "http-health.html").read_text(
            encoding="utf-8"
        )
        policy = document.split('http-equiv="Content-Security-Policy" content="', 1)[1].split(
            '"', 1
        )[0]
        canonical = (ROOT / "crates" / "metis-core" / "src" / "content_security_policy.txt").read_text(
            encoding="utf-8"
        )
        self.assertEqual(policy, canonical)
        self.assertIn('<link rel="stylesheet" href="./styles.css">', document)
        self.assertIn('<script type="module" src="./http-health.js"></script>', document)
        self.assertNotIn("<script type=\"module\">", document.lower())
        self.assertIn('id="metis-status" role="status"', document)
        self.assertIn('id="metis-health" type="button"', document)
        for fragment in (
            'id="fragment-input" type="text"',
            'id="metis-fragment" type="button" disabled',
            'id="metis-reset" type="button"',
            'id="metis-events"',
            'id="metis-negative"',
            'id="metis-lifecycle"',
            "DICOM loading and viewer state remain in RITK.",
        ):
            self.assertIn(fragment, document)

        script = (ROOT / "examples" / "browser" / "http-health.js").read_text(
            encoding="utf-8"
        )
        self.assertIn('new URL("http://127.0.0.1:8766/health")', script)
        self.assertIn('new URL("http://127.0.0.1:8766/v1/session")', script)
        self.assertIn('new URL("http://127.0.0.1:8766/v1/fragments")', script)
        self.assertIn('body !== "metis-http-ready\\n"', script)
        for fragment in (
            "function encodeHandshake()",
            'function encodeAction(generation, input, target = "metis-events")',
            "function encodeInvocation(token, action)",
            "const maxHttpBodyBytes = 16 * 1024",
            "async function readBoundedBody(result)",
            "result.body.getReader()",
            'Content-Type": "application/metis"',
            "function applyPatchSet(patchSet)",
            "async function dispatchFragmentToTarget(input, lease, target",
            "name.length <= 64",
            "/^[a-z0-9:_-]+$/.test(name)",
            "function isCurrentLease(lease)",
            "function beginRequest()",
            "requireCurrentLease(lease)",
            "signal: lease.controller.signal",
            "stale fragment generation",
            "target is not allowlisted",
            "target rejected",
            "malformed probe returned",
            "unauthorized probe returned",
            'events.textContent = "—";',
            "await runNegativeProbes(lease)",
            'fragmentButton.addEventListener("click", runFragment)',
            'resetButton.addEventListener("click", resetMount)',
        ):
            self.assertIn(fragment, script)
        self.assertNotIn("async function openSession()", script)
        self.assertNotIn("await runFragment();", script)

    def test_file_drop_surface_is_semantic_and_bounded(self):
        controls = (ROOT / "crates" / "metis-web" / "src" / "controls.rs").read_text(
            encoding="utf-8"
        )
        styles = (ROOT / "examples" / "browser" / "styles.css").read_text(
            encoding="utf-8"
        )
        for fragment in (
            'id="drop-status" role="status"',
            'id="drop-byte-status" role="status" aria-live="polite"',
            'id="drop-zone" role="group" tabindex="0"',
            'aria-label="File drop zone"',
            'data-drop-state="idle"',
            'data-byte-state="idle"',
        ):
            self.assertIn(fragment, controls)
        self.assertIn('<label for="file-input">Choose files</label>', controls)
        self.assertIn('<input id="file-input" type="file" multiple ', controls)
        self.assertNotIn("accept=\".dcm,application/dicom\"", controls)
        for selector in (
            '#drop-zone[data-drop-state="hovering"]',
            '#drop-zone[data-drop-state="accepted"]',
            '#drop-zone[data-drop-state="rejected"]',
            "#drop-zone:focus-visible",
        ):
            self.assertIn(selector, styles)

    def test_text_surface_is_semantic_and_tracks_composition(self):
        controls = (ROOT / "crates" / "metis-web" / "src" / "controls.rs").read_text(
            encoding="utf-8"
        )
        listeners = (ROOT / "crates" / "metis-web" / "src" / "browser" / "text.rs").read_text(
            encoding="utf-8"
        )
        styles = (ROOT / "examples" / "browser" / "styles.css").read_text(
            encoding="utf-8"
        )
        for fragment in (
            'id="text-specimen"',
            'aria-describedby="text-status composition-status selection-status"',
            'id="text-status" role="status" aria-live="polite"',
            'id="composition-status" role="status" aria-live="polite"',
            'id="selection-status" role="status"',
            'data-selection-direction="none"',
            'id="clipboard-read" type="button"',
            'id="clipboard-write" type="button"',
            'id="clipboard-status" role="status" aria-live="polite"',
            'data-clipboard-state="ready"',
        ):
            self.assertIn(fragment, controls)
        for event_name in (
            '"input"',
            '"compositionstart"',
            '"compositionupdate"',
            '"compositionend"',
            '"compositioncancel"',
            '"select"',
            '"keyup"',
            'keyboard_metadata',
            'input_type',
            'apply_input',
            'apply_navigation',
        ):
            self.assertIn(event_name, listeners)
        clipboard = (ROOT / "crates" / "metis-web" / "src" / "browser" / "clipboard.rs").read_text(
            encoding="utf-8"
        )
        for fragment in (
            'document.clipboard()',
            'provider.read_text()',
            'provider.write_text(&value)',
            'generation_is_current',
            'spawn_local_with_handle',
            'ClipboardStatus::Unavailable',
        ):
            self.assertIn(fragment, clipboard)
        for selector in (
            '.metis-text',
            '#text-specimen[data-text-state="composing"]',
            '#text-specimen[data-composing="true"]',
            '#text-specimen:focus-visible',
        ):
            self.assertIn(selector, styles)

    def test_result_explorer_is_bounded_and_keyboard_accessible(self):
        controls = (ROOT / "crates" / "metis-web" / "src" / "controls.rs").read_text(
            encoding="utf-8"
        )
        listeners = (ROOT / "crates" / "metis-web" / "src" / "browser" / "explorer.rs").read_text(
            encoding="utf-8"
        )
        styles = (ROOT / "examples" / "browser" / "styles.css").read_text(
            encoding="utf-8"
        )
        for fragment in (
            'aria-labelledby="explorer-heading"',
            'id="explorer-status" role="status" aria-live="polite"',
            'id="explorer-filter" type="search" maxlength="128"',
            'id="explorer-sort" name="explorer-sort"',
            'id="explorer-table"',
            'scope="col"',
            'id="explorer-entry-0"',
            'id="explorer-entry-7"',
            'id="explorer-previous" type="button"',
            'id="explorer-next" type="button"',
        ):
            self.assertIn(fragment, controls)
        for event_name in ('"input"', '"change"', '"click"'):
            self.assertIn(event_name, listeners)
        for selector in (
            '.metis-explorer',
            '#explorer-table',
            '.explorer-entry:focus-visible',
            '.explorer-entry-empty { display: none; }',
            '.explorer-row-selected',
            '.metis-explorer-pagination',
        ):
            self.assertIn(selector, styles)

    def test_layout_contract_constrains_grid_items_and_narrow_viewport(self):
        styles = (ROOT / "examples" / "browser" / "styles.css").read_text(
            encoding="utf-8"
        )
        for fragment in (
            ":root {",
            "box-sizing: border-box",
            "*, *::before, *::after { box-sizing: inherit; }",
            "body { margin: 0; min-width: 320px; background: var(--metis-page); color: var(--metis-text); }",
            "--metis-hit-target: 2.75rem;",
            "width: 100%; max-width: 960px",
            "grid-template-columns: minmax(0, 1fr) minmax(0, 1fr)",
            "min-width: 0",
            "overflow-wrap: anywhere",
            "@media (max-width: 700px)",
            ".metis-host-controls { padding-inline: 1rem; }",
            "#metis-app { grid-template-columns: 1fr; padding: 1rem; }",
            "--metis-page:",
            "--metis-surface:",
            "body[data-metis-theme=\"light\"]",
            "body[data-metis-theme=\"dark\"]",
            "body[data-metis-theme=\"high-contrast\"]",
            "data-metis-theme",
            "#application-toolbar { display: flex; flex-wrap: wrap;",
            "#command-menu[data-command-menu-open=\"false\"] { display: none; }",
            "#command-menu { grid-template-columns: 1fr; }",
            ".metis-option { display: flex; align-items: center; gap: 0.55rem; min-height: var(--metis-hit-target); }",
            'input[type="range"] { min-height: var(--metis-hit-target); padding: 0; accent-color: var(--metis-accent); }',
        ):
            self.assertIn(fragment, styles)

    def test_runtime_layout_manifest_covers_scale_one_viewports(self):
        styles_path = ROOT / "examples" / "browser" / "styles.css"
        manifest = json.loads(
            (ROOT / "docs" / "manual" / "images" / "browser-layout-metrics.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            manifest["source_sha256"],
            hashlib.sha256(styles_path.read_bytes()).hexdigest(),
        )
        expected = {
            "360x640": (360, 640, "312.8px"),
            "800x600": (800, 600, "348.4px 348.4px"),
            "1440x900": (1440, 900, "436px 436px"),
        }
        for name, (width, height, columns) in expected.items():
            capture = manifest["captures"][name]
            self.assertEqual(capture["viewport"], {"height": height, "scale": 1, "width": width})
            self.assertEqual(capture["grid"]["columns"], columns)
            self.assertLessEqual(capture["document"]["maxRight"], width)
            self.assertLessEqual(capture["document"]["scrollWidth"], width)
            targets = [
                target
                for target in capture["hitTargets"]
                if target["tag"] == "label" or target["id"] == "result-scale"
            ]
            self.assertEqual(len(targets), 4)
            self.assertTrue(all(target["rect"]["height"] >= 44 for target in targets))
            image = ROOT / "docs" / "manual" / "images" / f"browser-layout-{name}.jpg"
            self.assertEqual(image.read_bytes()[:2], bytes.fromhex("ffd8"))

    def test_accessibility_presentation_contract(self):
        controls = (ROOT / "crates" / "metis-web" / "src" / "controls.rs").read_text(
            encoding="utf-8"
        )
        view = (ROOT / "crates" / "metis-web" / "src" / "view.rs").read_text(
            encoding="utf-8"
        )
        styles = (ROOT / "examples" / "browser" / "styles.css").read_text(
            encoding="utf-8"
        )
        for fragment in (
            'aria-haspopup="dialog"',
            'aria-controls="session-dialog"',
            'id="application-navigation" aria-label="Application navigation"',
            'id="application-toolbar" role="toolbar" aria-label="Application commands"',
            'id="command-menu-toggle" type="button" aria-haspopup="menu" aria-expanded="false" aria-controls="command-menu"',
            'id="command-menu" role="menu" aria-label="Application commands" aria-hidden="true"',
            'id="command-focus-patient" role="menuitem" type="button" aria-keyshortcuts="Alt+Shift+P"',
            'id="command-theme-dark" role="menuitem" type="button" aria-keyshortcuts="Alt+Shift+D"',
            'id="command-theme-system" role="menuitem" type="button" aria-keyshortcuts="Alt+Shift+S"',
            'id="command-status" role="status" aria-live="polite"',
            'aria-labelledby="pointer-heading"',
            'aria-labelledby="drop-heading"',
            'aria-labelledby="text-heading"',
            'id="metis-status" role="status" aria-live="polite" aria-atomic="true" aria-busy="false"',
            'id="metis-form" class="metis-form" aria-describedby="metis-status" aria-busy="false"',
            'id="result-state" role="status" aria-live="polite" aria-atomic="true" aria-busy="false"',
            'id="metis-events" role="status"',
            'id="drop-status" role="status"',
            'id="composition-status" role="status" aria-live="polite"',
            'id="pointer-surface" role="group" tabindex="0"',
            "one pointer, or use two pointers to pinch",
            'id="drop-zone" role="group" tabindex="0"',
            'id="theme-mode" name="theme-mode"',
        ):
            self.assertIn(fragment, controls)
        for fragment in (
            'let request_busy = matches!(state.state, FormState::Pending);',
            'set_attribute("aria-busy", busy_value)',
            'matches!(explorer.status(), ExplorerStatus::Loading)',
        ):
            self.assertIn(fragment, view)
        focus_order = (
            "command-menu-toggle",
            "open-session-dialog",
            "command-focus-patient",
            "command-theme-dark",
            "command-theme-system",
            "patient-id",
            "weight-kg",
            "concentration-mg-ml",
            "target-dose",
            "submit-calculation",
            "show-events",
            "dose-volume",
            "dose-mass",
            "result-scale",
            "result-detail-select",
            "theme-mode",
            "pointer-surface",
            "drop-zone",
            "text-specimen",
        )
        offsets = [controls.index(f'id="{control_id}"') for control_id in focus_order]
        self.assertEqual(offsets, sorted(offsets))
        self.assertNotIn('tabindex="1"', controls)
        for fragment in (
            "@media (prefers-reduced-motion: reduce)",
            "animation-duration: 0.001ms",
            "transition-duration: 0.001ms",
            "@media (forced-colors: active)",
            "forced-color-adjust: auto",
            "background: Canvas",
            "color: CanvasText",
            "background: ButtonFace",
            "outline-color: Highlight",
        ):
            self.assertIn(fragment, styles)


if __name__ == "__main__":
    unittest.main()
