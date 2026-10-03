"""Keep every fuzz target backed by a source file and a committed seed corpus."""
import pathlib
import tomllib
import unittest


FUZZ = pathlib.Path(__file__).resolve().parents[2] / "fuzz"
# One seed is a starting point for mutation, not a fixture: a large one slows
# every campaign run that replays it.
SEED_LIMIT_BYTES = 64 * 1024


class FuzzHarnessTests(unittest.TestCase):
    """The campaign replays fuzz/seeds/<target> for each target it lists."""

    @classmethod
    def setUpClass(cls):
        manifest = tomllib.loads((FUZZ / "Cargo.toml").read_text(encoding="utf-8"))
        cls.targets = {binary["name"]: binary["path"] for binary in manifest["bin"]}

    def test_every_target_names_an_existing_source_file(self):
        for name, path in self.targets.items():
            with self.subTest(target=name):
                self.assertEqual(path, f"fuzz_targets/{name}.rs")
                self.assertTrue((FUZZ / path).is_file())

    def test_every_source_file_is_a_declared_target(self):
        sources = {path.stem for path in (FUZZ / "fuzz_targets").glob("*.rs")}
        self.assertEqual(sources, set(self.targets))

    def test_every_target_has_committed_seeds_within_the_size_limit(self):
        for name in self.targets:
            with self.subTest(target=name):
                seeds = [path for path in (FUZZ / "seeds" / name).glob("*") if path.is_file()]
                self.assertTrue(seeds, "the campaign needs at least one seed")
                for seed in seeds:
                    self.assertLessEqual(seed.stat().st_size, SEED_LIMIT_BYTES, seed.name)

    def test_no_seed_directory_lacks_a_target(self):
        directories = {path.name for path in (FUZZ / "seeds").iterdir() if path.is_dir()}
        self.assertEqual(directories, set(self.targets))


if __name__ == "__main__":
    unittest.main()
