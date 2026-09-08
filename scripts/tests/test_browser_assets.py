"""Check the browser shell's static trust-boundary assets."""
from __future__ import annotations

import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


class BrowserAssetContractTests(unittest.TestCase):
    """The page loads only same-origin external assets under the host policy."""

    def test_document_uses_external_assets_and_strict_csp(self):
        document = (ROOT / "examples" / "browser" / "index.html").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("<style", document.lower())
        self.assertNotIn("<script type=\"module\">", document.lower())
        self.assertIn('<link rel="stylesheet" href="./styles.css">', document)
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

    def test_bootstrap_keeps_navigation_same_origin(self):
        bootstrap = (ROOT / "examples" / "browser" / "bootstrap.js").read_text(
            encoding="utf-8"
        )
        self.assertIn('import init from "./metis_web.js"', bootstrap)
        self.assertIn("wasm.metis_start()", bootstrap)
        self.assertIn("wasm.metis_stop()", bootstrap)
        self.assertIn("destination.origin !== window.location.origin", bootstrap)
        self.assertIn("event.preventDefault()", bootstrap)

    def test_file_drop_surface_is_semantic_and_bounded(self):
        controls = (ROOT / "crates" / "metis-web" / "src" / "controls.rs").read_text(
            encoding="utf-8"
        )
        styles = (ROOT / "examples" / "browser" / "styles.css").read_text(
            encoding="utf-8"
        )
        for fragment in (
            'id="drop-status" role="status"',
            'id="drop-zone" role="group" tabindex="0"',
            'aria-label="DICOM file drop zone"',
            'data-drop-state="idle"',
        ):
            self.assertIn(fragment, controls)
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
        ):
            self.assertIn(fragment, controls)
        for event_name in (
            '"input"',
            '"compositionstart"',
            '"compositionupdate"',
            '"compositionend"',
            '"compositioncancel"',
            '"select"',
        ):
            self.assertIn(event_name, listeners)
        for selector in (
            '.metis-text',
            '#text-specimen[data-text-state="composing"]',
            '#text-specimen[data-composing="true"]',
            '#text-specimen:focus-visible',
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
            "body { margin: 0; min-width: 320px; }",
            "width: 100%; max-width: 960px",
            "grid-template-columns: minmax(0, 1fr) minmax(0, 1fr)",
            "min-width: 0",
            "overflow-wrap: anywhere",
            "@media (max-width: 700px)",
            ".metis-host-controls { padding-inline: 1rem; }",
            "#metis-app { grid-template-columns: 1fr; padding: 1rem; }",
        ):
            self.assertIn(fragment, styles)


if __name__ == "__main__":
    unittest.main()
