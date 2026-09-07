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
        for directive in (
            "default-src 'self'",
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


if __name__ == "__main__":
    unittest.main()
