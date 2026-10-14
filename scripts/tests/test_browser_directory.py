"""Real directory identities and process reports define served-tree admission."""

import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
ACTOR = r'''
import json
import pathlib
import subprocess
import sys
sys.path.insert(0, sys.argv[1])
import browser
records = []
for case in json.loads(sys.argv[3]):
    command = [sys.executable, "-c",
               "import sys;sys.stdout.buffer.write(sys.argv[1].encode('utf-8'));"
               "sys.stderr.buffer.write(sys.argv[2].encode('utf-8'))",
               case["report"], case["stderr"]]
    result = browser.run(command, cwd=pathlib.Path(sys.argv[2]),
                         capture_output=True, text=True, timeout=10)
    try:
        admitted = browser.served_directory(result, pathlib.Path(case["expected"]),
                                             cwd=pathlib.Path(sys.argv[2]))
        value = {"admitted": str(admitted), "samefile": admitted.samefile(case["expected"])}
    except (OSError, ValueError) as error:
        value = {"error_type": type(error).__name__, "error_text": str(error),
                 "errno": getattr(error, "errno", None),
                 "filename": getattr(error, "filename", None)}
    records.append({"command": command, "result": value})
print(json.dumps(records))
'''


class ServedDirectoryContracts(unittest.TestCase):
    def invoke(self, directory, cases):
        result = subprocess.run(
            [sys.executable, "-c", ACTOR, str(SCRIPTS), str(directory), json.dumps(cases)],
            capture_output=True, timeout=30,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8"))
        return json.loads(result.stdout), result.stderr.decode("utf-8")

    def test_real_directory_aliases_preserve_actual_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            expected = root / "患者 λ" / "app"
            expected.mkdir(parents=True)
            aliases = [expected, expected / ".." / "app"]
            if os.name == "nt":
                ordinary = str(expected)
                extended = ("\\\\?\\UNC\\" + ordinary[2:] if ordinary.startswith("\\\\")
                            else "\\\\?\\" + ordinary)
                aliases.append(pathlib.Path(extended))
            cases = [{"report": str(alias) + "\n", "expected": str(expected),
                      "stderr": "compiler warning caméra\n"} for alias in aliases]
            records, diagnostics = self.invoke(root, cases)
            receipts = [json.loads(line.removeprefix("browser served directory: "))
                        for line in diagnostics.splitlines()]
            self.assertEqual(len(receipts), len(aliases))
            self.assertNotEqual(aliases[1], expected)
            for alias, record, receipt in zip(aliases, records, receipts, strict=True):
                self.assertEqual(record["result"], {"admitted": str(alias), "samefile": True})
                self.assertEqual(receipt, {"reported": str(alias), "expected": str(expected),
                                           "is_directory": True, "samefile": True})

    def test_relative_file_and_different_equal_content_directories_reject_with_full_context(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            expected, other = root / "app", root / "other"
            expected.mkdir()
            other.mkdir()
            for directory in (expected, other):
                (directory / "index.html").write_text("<p>same bytes 患者</p>", encoding="utf-8")
            self.assertFalse(expected.samefile(other))
            reports = ["app\n", str(expected / "index.html") + "\n", str(other) + "\n",
                       "", str(expected) + "\n" + str(expected) + "\n"]
            cases = [{"report": report, "expected": str(expected),
                      "stderr": "warning λ\nlast warning 患者"} for report in reports]
            records, diagnostics = self.invoke(root, cases)
            for case, record in zip(cases, records, strict=True):
                self.assertEqual(record["result"]["error_type"], "ValueError")
                self.assertEqual(record["result"]["errno"], None)
                context = {"command": record["command"], "cwd": str(root), "returncode": 0,
                           "expected": str(expected)}
                exact = ("browser build report rejected: " + json.dumps(context, ensure_ascii=False)
                         + "\nstdout:\n" + case["report"] + "\nstderr:\n" + case["stderr"] + "\n")
                self.assertTrue(diagnostics.startswith(exact), diagnostics)
                diagnostics = diagnostics.removeprefix(exact)
            self.assertEqual(diagnostics, "")

    def test_missing_paths_preserve_the_original_native_stat_exception(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            expected = root / "app"
            expected.mkdir()
            missing = root / "missing 患者"
            cases = [{"report": str(missing) + "\n", "expected": str(expected), "stderr": ""},
                     {"report": str(expected) + "\n", "expected": str(missing), "stderr": ""}]
            records, diagnostics = self.invoke(root, cases)
            try:
                missing.stat()
            except OSError as native:
                actual = (type(native).__name__, native.errno, native.filename, str(native))
            for record in records:
                value = record["result"]
                self.assertEqual((value["error_type"], value["errno"], value["filename"], value["error_text"]),
                                 actual)
            self.assertEqual(diagnostics.count("browser build report rejected: "), 2)
            self.assertEqual(diagnostics.count('"returncode": 0'), 2)


if __name__ == "__main__":
    unittest.main()
