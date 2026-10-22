"""Tests for changed_files()/main() from scripts/changed_tests.py.

Shared helpers are in changed_tests_fixtures.py (see its docstring).
"""

import contextlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from changed_tests_fixtures import _GitRepo, changed_tests


class ChangedFiles(unittest.TestCase):
    """changed_files() — diff from merge-base plus untracked files, paths from the root."""

    def test_committed_uncommitted_and_untracked_outside_src_are_all_included(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = _GitRepo(Path(tmp))
            repo.write("src/committed.rs", "v0")
            repo.write("src/modified.rs", "v0")
            repo.commit_all("baseline")
            repo.checkout_branch("feature")
            repo.write("src/committed.rs", "v1")
            repo.commit_all("change on branch")
            repo.write("src/modified.rs", "v1")  # not committed
            repo.write("outside/new_file.txt", "new")  # untracked outside core/

            with repo.cd_root():
                changed = changed_tests.changed_files("main")

            self.assertEqual(
                changed,
                ["outside/new_file.txt", "src/committed.rs", "src/modified.rs"],
                "a commit on the branch + uncommitted + untracked outside core/ — all three",
            )

    def test_unknown_base_exits_with_code_2(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = _GitRepo(Path(tmp))
            repo.write("a.rs", "v0")
            repo.commit_all("baseline")

            with repo.cd_root():
                with self.assertRaises(SystemExit) as ctx:
                    changed_tests.changed_files("does-not-exist-branch")

            self.assertEqual(ctx.exception.code, 2)


class MainDryRun(unittest.TestCase):
    """main() with --dry-run prints the commands and code 0, running nothing."""

    def test_dry_run_prints_commands_returns_zero_and_does_not_execute(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = _GitRepo(Path(tmp))
            repo.write("core/lib.rs", "// v0\n")
            repo.commit_all("baseline")
            repo.checkout_branch("feature")
            repo.write("core/lib.rs", "// v1\n")
            repo.commit_all("touch lib")

            with repo.cd_root():
                with mock.patch.object(changed_tests, "_execute") as execute:
                    out = io.StringIO()
                    with contextlib.redirect_stdout(out):
                        code = changed_tests.main(["--base", "main", "--dry-run"])

            self.assertEqual(code, 0)
            execute.assert_not_called()
            printed = out.getvalue()
            self.assertIn("core/lib.rs → full\n", printed)
            self.assertIn("make", printed)
            self.assertIn("test", printed)


if __name__ == "__main__":
    unittest.main()
