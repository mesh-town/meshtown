"""Shared helpers for test_changed_tests_*.py — not test_*, so that discover does not treat it as a test.

changed_tests.py is written in parallel by another executor; here the module is loaded via
sys.path rather than imported as a package, so that the test builds independently of whether the
code is ready at run time.
"""

import contextlib
import os
import subprocess
import sys
from pathlib import Path

_SCRIPTS_DIR = Path(__file__).resolve().parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

import changed_tests  # noqa: E402,F401  (the module is written by a parallel executor)


def _sources(main_rs="", tests=None, bin_files=None, examples=None):
    """Sources with empty dictionaries by default — Sources is assembled by hand, not from disk."""
    return changed_tests.Sources(
        main_rs=main_rs,
        tests=dict(tests or {}),
        bin=dict(bin_files or {}),
        examples=dict(examples or {}),
    )


class _GitRepo:
    """A temporary git repository (the project root is the repository root), for changed_files()/main()."""

    def __init__(self, root: Path):
        self.root = root
        self._run("init", "-q")
        self._run("checkout", "-q", "-b", "main")
        self._run("config", "user.email", "test@example.com")
        self._run("config", "user.name", "Test")

    def _run(self, *args):
        subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)

    def write(self, rel_path: str, content: str) -> None:
        path = self.root / rel_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    def commit_all(self, message: str) -> None:
        self._run("add", "-A")
        self._run("commit", "-q", "-m", message)

    def checkout_branch(self, name: str) -> None:
        self._run("checkout", "-q", "-b", name)

    def cd_root(self):
        return _chdir(self.root)


@contextlib.contextmanager
def _chdir(path: Path):
    old = Path.cwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(old)
