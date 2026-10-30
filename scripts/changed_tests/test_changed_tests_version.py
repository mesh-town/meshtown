"""Version-only edits of Cargo.toml / Cargo.lock select nothing (cargo_version.py + main())."""

import contextlib
import io
import tempfile
import unittest
from pathlib import Path

import cargo_version
from changed_tests_fixtures import _GitRepo, changed_tests

MANIFEST = '[package]\nname = "a"\nversion = "{v}"\n\n[dependencies]\nserde = "{d}"\n'
LOCK = (
    '[[package]]\nname = "a"\nversion = "{v}"\n\n'
    '[[package]]\nname = "serde"\nversion = "{d}"\nsource = "registry+x"\n'
)
WORKSPACE = '[workspace.package]\nversion = "{v}"\nedition = "2021"\n'


def _texts(template, v1, d1, v2, d2):
    return template.format(v=v1, d=d1), template.format(v=v2, d=d2)


class VersionOnly(unittest.TestCase):
    def test_package_version_bump_is_version_only(self):
        for path, template in (("Cargo.toml", MANIFEST), ("Cargo.lock", LOCK)):
            old, new = _texts(template, "1.0.1", "1", "1.0.2", "1")
            self.assertTrue(cargo_version.version_only(path, old, new), path)

    def test_workspace_package_version_bump_is_version_only(self):
        self.assertTrue(
            cargo_version.version_only("Cargo.toml", WORKSPACE.format(v="1"), WORKSPACE.format(v="2"))
        )

    def test_dependency_change_is_not_version_only(self):
        for path, template in (("Cargo.toml", MANIFEST), ("Cargo.lock", LOCK)):
            old, new = _texts(template, "1.0.1", "1", "1.0.2", "2")
            self.assertFalse(cargo_version.version_only(path, old, new), path)

    def test_missing_side_is_not_version_only(self):
        self.assertFalse(cargo_version.version_only("Cargo.toml", None, "x"))
        self.assertFalse(cargo_version.version_only("Cargo.toml", "x", None))


def _run_main(repo):
    out = io.StringIO()
    with repo.cd_root(), contextlib.redirect_stdout(out):
        changed_tests.main(["--base", "main", "--dry-run"])
    return out.getvalue()


def _repo_with_bump(tmp, dep_new="1", extra=None):
    repo = _GitRepo(Path(tmp))
    old_m, new_m = _texts(MANIFEST, "1.0.1", "1", "1.0.2", dep_new)
    old_l, new_l = _texts(LOCK, "1.0.1", "1", "1.0.2", dep_new)
    repo.write("Cargo.toml", old_m)
    repo.write("Cargo.lock", old_l)
    repo.write("core/cli/x.rs", "// v0\n")
    repo.commit_all("baseline")
    repo.checkout_branch("feature")
    repo.write("Cargo.toml", new_m)
    repo.write("Cargo.lock", new_l)
    if extra:
        repo.write(*extra)
    repo.commit_all("bump")
    return repo


class VersionBumpInMain(unittest.TestCase):
    def test_only_version_selects_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = _run_main(_repo_with_bump(tmp))
        self.assertIn("nothing to run", out)

    def test_version_plus_dependency_selects_full(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = _run_main(_repo_with_bump(tmp, dep_new="2"))
        self.assertIn("Cargo.toml → full + py + js (catch-all)", out)

    def test_version_plus_cli_edit_selects_only_cli(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = _run_main(_repo_with_bump(tmp, extra=("core/cli/x.rs", "// v1\n")))
        self.assertNotIn("Cargo.", out)
        self.assertIn("core/cli/x.rs", out)


if __name__ == "__main__":
    unittest.main()
