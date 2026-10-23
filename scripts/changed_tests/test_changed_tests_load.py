"""Tests for _load_examples() from scripts/changed_tests/changed_tests.py.

Shared helpers are in changed_tests_fixtures.py (see its docstring).
"""

import tempfile
import unittest
from pathlib import Path

from changed_tests_fixtures import changed_tests


class LoadExamplesTest(unittest.TestCase):
    """_load_examples() — an example file `examples/x.rs` and an example folder `examples/x/main.rs`."""

    def test_example_directory_with_main_rs_is_read_as_one_key(self):
        with tempfile.TemporaryDirectory() as tmp:
            examples = Path(tmp) / "examples"
            demo = examples / "web_demo"
            demo.mkdir(parents=True)
            (demo / "main.rs").write_text("mod node;\n")
            (demo / "node.rs").write_text("use meshtown_core::members::words;\n")

            loaded = changed_tests._load_examples(examples)

            self.assertIn("web_demo", loaded)
            self.assertIn("meshtown_core::members::words", loaded["web_demo"])

    def test_example_file_is_read_by_its_stem(self):
        with tempfile.TemporaryDirectory() as tmp:
            examples = Path(tmp) / "examples"
            examples.mkdir()
            text = "fn main() {}\n"
            (examples / "solo.rs").write_text(text)

            loaded = changed_tests._load_examples(examples)

            self.assertEqual(loaded["solo"], text)

    def test_directory_without_main_rs_is_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            examples = Path(tmp) / "examples"
            notes = examples / "notes"
            notes.mkdir(parents=True)
            (notes / "readme.rs").write_text("// not an example\n")

            loaded = changed_tests._load_examples(examples)

            self.assertNotIn("notes", loaded)


class LoadSourcesTest(unittest.TestCase):
    """load_sources() — reads the binary entry point, the tests and the examples from `core/`."""

    def test_core_dir_is_read_for_main_tests_and_examples(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = {
                "core/cli/main.rs": "fn main() { meshtown_core::cli::run(); }\n",
                "core/tests/a.rs": '#[path = "common/c.rs"]\nmod c;\n',
                "core/tests/common/c.rs": "fn from_common() {}\n",
                "core/examples/web_demo/main.rs": "fn main() {}\n",
            }
            for rel, text in files.items():
                path = root / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text)

            loaded = changed_tests.load_sources(root)

            self.assertIn("cli::run", loaded.main_rs)
            self.assertIn("from_common", loaded.tests["a"])
            self.assertIn("web_demo", loaded.examples)


if __name__ == "__main__":
    unittest.main()
