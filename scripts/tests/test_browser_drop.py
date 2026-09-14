"""File-backed browser proof input and gallery delivery contracts."""
from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from browser_drop import (
    MAX_FILE_BYTES,
    MAX_BATCH_BYTES,
    MAX_FILES,
    OBSERVE_TRANSFER,
    resolve_browser_target,
    study_files,
)
from browser_protocol import BrowserRuntimeError
from browser_trace import BrowserEngine
from browser import SOURCE, validate_index_policy


class FileDropTests(unittest.TestCase):
    def test_browser_target_resolution_covers_the_matrix(self):
        self.assertEqual(resolve_browser_target(None, "chrome"), (BrowserEngine.CHROMIUM, "chrome"))
        self.assertEqual(resolve_browser_target("chromium", "MicrosoftEdge"), (BrowserEngine.CHROMIUM, "MicrosoftEdge"))
        self.assertEqual(resolve_browser_target("firefox", None), (BrowserEngine.FIREFOX, "firefox"))
        self.assertEqual(resolve_browser_target("webkit", "safari"), (BrowserEngine.WEBKIT, "safari"))
        with self.assertRaisesRegex(BrowserRuntimeError, "incompatible"):
            resolve_browser_target("firefox", "chrome")

    def test_file_selection_preserves_exact_bytes_and_ignores_other_names(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "b.bin").write_bytes(b"second")
            (root / "a.bin").write_bytes(b"first")
            (root / "LICENSE").write_text("license", encoding="utf-8")
            files, total = study_files(root, "*.bin")
            self.assertEqual([path.name for path in files], ["a.bin", "b.bin"])
            self.assertEqual(total, 11)

    def test_empty_folder_nested_folder_and_traversal_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            with self.assertRaisesRegex(BrowserRuntimeError, "empty"):
                study_files(root)
            (root / "nested").mkdir()
            with self.assertRaisesRegex(BrowserRuntimeError, "files only"):
                study_files(root)
            with self.assertRaisesRegex(BrowserRuntimeError, "immediate"):
                study_files(root, "../*")

    def test_file_and_batch_boundaries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            for index in range(MAX_BATCH_BYTES // MAX_FILE_BYTES):
                with (root / str(index)).open("wb") as file:
                    file.truncate(MAX_FILE_BYTES)
            self.assertEqual(study_files(root)[1], MAX_BATCH_BYTES)
            (root / "extra").write_bytes(b"x")
            with self.assertRaisesRegex(BrowserRuntimeError, "bounds"):
                study_files(root)
            with (root / "large").open("wb") as file:
                file.truncate(MAX_FILE_BYTES + 1)
            with self.assertRaisesRegex(BrowserRuntimeError, "bounds"):
                study_files(root, "large")

    def test_file_count_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            for index in range(MAX_FILES):
                (root / str(index)).write_bytes(b"x")
            files, total = study_files(root)
            self.assertEqual((len(files), total), (MAX_FILES, MAX_FILES))
            (root / "extra").write_bytes(b"x")
            with self.assertRaisesRegex(BrowserRuntimeError, "bounds"):
                study_files(root)

    def test_gallery_uses_real_host_and_external_consumer(self):
        validate_index_policy(SOURCE / "gallery.html")
        html = (SOURCE / "gallery.html").read_text(encoding="utf-8")
        script = (SOURCE / "gallery.js").read_text(encoding="utf-8")
        style = (SOURCE / "gallery.css").read_text(encoding="utf-8")
        self.assertIn('id="metis-app"', html)
        self.assertIn('src="./gallery.js"', html)
        self.assertEqual(html.count("<canvas "), 3)
        self.assertIn('import("./consumer/ritk_snap.js")', script)
        self.assertIn("start_web_orthogonal_canvases(", script)
        self.assertIn("stop_web_canvas()", script)
        self.assertNotIn("DataTransfer", script)
        self.assertNotIn("dispatchEvent", script)
        self.assertNotIn("fetch(", script)
        self.assertIn("#metis-app > :not(.metis-drop)", style)

    def test_transfer_observer_covers_drop_and_standard_chooser(self):
        self.assertIn("input.addEventListener('change'", OBSERVE_TRANSFER)
        self.assertIn("event.dataTransfer", OBSERVE_TRANSFER)
        self.assertIn("window.metisInputFiles", OBSERVE_TRANSFER)
        self.assertNotIn("dispatchEvent", OBSERVE_TRANSFER)


if __name__ == "__main__":
    unittest.main()
