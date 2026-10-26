"""Tests of check_dist_urls.py: each case builds a small tree in a temporary directory."""
import contextlib
import io
import tempfile
import unittest
from pathlib import Path

import check_dist_urls as chk

ROOT = Path(__file__).resolve().parents[2]


def _run(root: Path) -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        code = chk.main(["--root", str(root)])
    return code, out.getvalue()


class CheckDistUrlsTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.addCleanup(self._tmp.cleanup)

    def put(self, name: str, text: str):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def assertHit(self, name: str, text: str):
        self.put(name, "first\n" + text + "\n")
        code, out = _run(self.root)
        self.assertEqual(code, 1)
        self.assertIn(f"{name}:2: ", out)

    def test_finds_latest_api(self):
        self.assertHit("a.ts", 'const U = "https://api.github.com/repos/mesh-town/meshtown/releases/latest";')

    def test_finds_download_url(self):
        self.assertHit("f.rb", "url https://github.com/mesh-town/meshtown/releases/download/v1/x")

    def test_finds_repo_flag(self):
        self.assertHit("w.yml", "run: gh release view --repo mesh-town/meshtown")

    def test_skips_dist_and_other_forms(self):
        self.put("ok.md", "\n".join([
            "https://api.github.com/repos/mesh-town/meshtown-dist/releases/latest",
            "gh release view --repo mesh-town/meshtown-dist",
            "brew install mesh-town/meshtown/meshtown",
            "https://github.com/mesh-town/meshtown",
        ]))
        self.assertEqual(_run(self.root), (0, ""))

    def test_skips_vendor_dirs_without_git(self):
        bad = "--repo mesh-town/meshtown"
        self.put("node_modules/x/a.js", bad)
        self.put("target/debug/b.txt", bad)
        self.assertEqual(_run(self.root), (0, ""))

    def test_real_tree_is_clean(self):
        self.assertEqual(chk.find_hits(ROOT), [])


if __name__ == "__main__":
    unittest.main()
