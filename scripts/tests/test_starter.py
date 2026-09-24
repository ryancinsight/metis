"""The starter page's structure and its agreement with the Rust crate."""
import html.parser
import json
import pathlib
import re
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import starter
from browser_protocol import BrowserRuntimeError, WebDriverClient

CRATE = starter.STARTER
FRONTEND = CRATE / "frontend"
CANONICAL_MARK = starter.ROOT / "examples" / "browser" / "assets" / "metis-mark.svg"


def _manifest() -> dict:
    return json.loads(starter.MANIFEST.read_text(encoding="utf-8"))


class _Page(html.parser.HTMLParser):
    """Collects the page's scripts, CSP and element ids."""

    def __init__(self) -> None:
        super().__init__()
        self.scripts = []
        self.policy = None
        self.ids = set()

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        if "id" in attributes:
            self.ids.add(attributes["id"])
        if tag == "script":
            self.scripts.append(attributes)
        if tag == "meta" and attributes.get("http-equiv") == "Content-Security-Policy":
            self.policy = attributes["content"]


def _page() -> _Page:
    page = _Page()
    page.feed((FRONTEND / "index.html").read_text(encoding="utf-8"))
    return page


class StarterPageTests(unittest.TestCase):
    def test_the_manifest_builds_this_crate_and_page(self):
        frontend = _manifest()["frontend"]
        package = re.search(r'^name = "([\w-]+)"$', (CRATE / "Cargo.toml").read_text(encoding="utf-8"),
                            re.MULTILINE).group(1)
        self.assertEqual(frontend["package"], package)
        self.assertTrue((CRATE / frontend["directory"] / "index.html").is_file())

    def test_the_mark_matches_the_canonical_mark(self):
        self.assertEqual((FRONTEND / "assets" / "metis-mark.svg").read_bytes(), CANONICAL_MARK.read_bytes())

    def test_the_page_runs_only_the_module_loader(self):
        page = _page()
        self.assertEqual(page.scripts, [{"type": "module", "src": "main.js"}])
        loader = (FRONTEND / "main.js").read_text(encoding="utf-8")
        module = _manifest()["frontend"]["package"].replace("-", "_")
        self.assertIn(f'from "./{module}.js"', loader)
        export = re.search(r"\.(\w+)\(\);", loader).group(1)
        rust = (CRATE / "src" / "browser.rs").read_text(encoding="utf-8")
        self.assertIn(f'pub extern "C" fn {export}()', rust)

    def test_the_policy_admits_webassembly_and_nothing_inline(self):
        directives = dict(
            (part.split()[0], part.split()[1:])
            for part in _page().policy.split(";") if part.strip()
        )
        self.assertEqual(directives["script-src"], ["'self'", "'wasm-unsafe-eval'"])
        self.assertEqual(directives["default-src"], ["'self'"])
        self.assertEqual(directives["form-action"], ["'none'"])
        self.assertNotIn("'unsafe-inline'", " ".join(sum(directives.values(), [])))

    def test_rust_binds_the_ids_the_page_declares(self):
        rust = (CRATE / "src" / "browser.rs").read_text(encoding="utf-8")
        bound = set(re.findall(r'const \w+: &str = "([\w-]+)";', rust))
        self.assertEqual(bound, {"greet-form", "greet-input", "greet-msg"})
        self.assertLessEqual(bound, _page().ids)

    def test_the_checked_greeting_is_the_rust_wording(self):
        rust = (CRATE / "src" / "greet.rs").read_text(encoding="utf-8")
        template = re.search(r'format!\("([^"]*)"\)', rust).group(1)
        self.assertEqual(starter.expected_greeting("Ada"), template.replace("{name}", "Ada"))


class ChromiumArgumentTests(unittest.TestCase):
    def test_switches_reach_chromium_options_and_other_browsers_refuse_them(self):
        requests = []
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client._request = lambda method, path, payload=None: requests.append(payload) or {
            "sessionId": "session", "capabilities": {},
        }
        client.create_session("MicrosoftEdge", headless=True,
                              chromium_arguments=[starter.COLOR_SCHEMES["light"]])
        self.assertEqual(
            requests[0]["capabilities"]["alwaysMatch"]["ms:edgeOptions"]["args"],
            ["--blink-settings=preferredColorScheme=1", "--headless=new"],
        )
        with self.assertRaisesRegex(BrowserRuntimeError, "only to Chrome and Edge"):
            client.create_session("firefox", chromium_arguments=["--x"])


if __name__ == "__main__":
    unittest.main()
