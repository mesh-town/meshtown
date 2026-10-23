"""Tests of check_boundary.py: each case builds a small workspace in a temporary directory and runs the checker on it."""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

CHECKER = Path(__file__).resolve().parent / "check_boundary.py"

BUILD_SH = """#!/bin/sh
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
grep version "$ROOT/Cargo.toml"
. "$SCRIPT_DIR/../../../scripts/release/lib.sh"
"""


def _manifest(name: str, lib_path: str, deps: dict) -> str:
    lines = ["[package]", f'name = "{name}"', 'version = "0.1.0"', 'edition = "2021"', "",
             "[lib]", f'path = "{lib_path}"', "", "[dependencies]"]
    return "\n".join(lines + [f'{dep} = {{ path = "{path}" }}' for dep, path in deps.items()]) + "\n"


class _Tree:
    """A synthetic workspace: a core, one macOS platform, a release library; a git index with everything added."""

    def __init__(self, root: Path):
        self.root = root
        self.members = ["core", "platforms/macos"]
        self.write("core/Cargo.toml", _manifest("meshtown-core", "lib.rs", {}))
        self.write("core/lib.rs", "")
        self.write("platforms/macos/Cargo.toml", _manifest("meshtown-macos", "gui/lib.rs", {"meshtown-core": "../../core"}))
        self.write("platforms/macos/gui/lib.rs", "")
        self.write("platforms/macos/build/build.sh", BUILD_SH)
        self.write("scripts/release/lib.sh", "PROTO=1\n")
        self.write_root_manifest()
        subprocess.run(["git", "init", "-q"], cwd=root, check=True)
        self.git_add()

    def write(self, rel: str, text: str) -> None:
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def append(self, rel: str, text: str) -> None:
        with open(self.root / rel, "a") as f:
            f.write(text)

    def write_root_manifest(self) -> None:
        members = ", ".join(f'"{m}"' for m in self.members)
        self.write("Cargo.toml", f"[workspace]\nresolver = \"2\"\nmembers = [{members}]\n")

    def add_platform(self, os_name: str) -> None:
        self.members.append(f"platforms/{os_name}")
        self.write(f"platforms/{os_name}/Cargo.toml", _manifest(f"meshtown-{os_name}", "lib.rs", {}))
        self.write(f"platforms/{os_name}/lib.rs", "")
        self.write_root_manifest()

    def git_add(self) -> None:
        subprocess.run(["git", "add", "-A"], cwd=self.root, check=True)


def _run(root: Path, path_env=None) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    if path_env is not None:
        env["PATH"] = str(path_env)
    return subprocess.run([sys.executable, str(CHECKER), str(root)], capture_output=True, text=True, env=env)


@unittest.skipUnless(shutil.which("cargo") and shutil.which("git"), "needs cargo and git")
class _TreeCase(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name).resolve()
        (self.tmp / "ws").mkdir()
        self.tree = _Tree(self.tmp / "ws")

    def run_checker(self, path_env=None) -> subprocess.CompletedProcess:
        return _run(self.tree.root, path_env)

    def assert_violation(self, rule: str, file: str):
        result = self.run_checker()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(f"{file}: {rule}", result.stdout)


class CleanTree(_TreeCase):
    def test_clean_tree_exits_0(self):
        # given: a workspace where the platform uses only the core and scripts/release/lib.sh
        # when: the checker runs
        result = self.run_checker()
        # then: it passes silently
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout, "")


class CoreReachesPlatform(_TreeCase):
    def test_core_path_dep_on_platform_exits_1(self):
        # given: the core depends on a platform package by path (and the platform no longer depends on the core)
        self.tree.write("core/Cargo.toml", _manifest("meshtown-core", "lib.rs", {"meshtown-macos": "../platforms/macos"}))
        self.tree.write("platforms/macos/Cargo.toml", _manifest("meshtown-macos", "gui/lib.rs", {}))
        self.tree.git_add()
        # then: the manifest of the core is reported
        self.assert_violation("core-depends-on-platform", "core/Cargo.toml")

    def test_core_include_outside_core_exits_1(self):
        # given: a core source embeds a file from outside core/
        self.tree.write("core/lib.rs", 'const S: &str = include_str!("../scripts/release/lib.sh");\n')
        self.tree.git_add()
        # then: the source is reported
        self.assert_violation("core-include-escape", "core/lib.rs")


class PlatformReachesOut(_TreeCase):
    def test_platform_include_into_core_exits_1(self):
        # given: a platform source embeds a file of the core
        self.tree.write("platforms/macos/gui/lib.rs", 'const S: &str = include_str!("../../../core/x");\n')
        self.tree.git_add()
        # then: the source is reported
        self.assert_violation("platform-include-escape", "platforms/macos/gui/lib.rs")

    def test_platform_script_uses_core_via_root_exits_1(self):
        # given: a platform script reads a core file through the workspace root variable
        self.tree.append("platforms/macos/build/build.sh", 'cat "$ROOT/core/lib.rs"\n')
        self.tree.git_add()
        # then: the script is reported
        self.assert_violation("platform-escape", "platforms/macos/build/build.sh")

    def test_platform_dep_on_other_platform_exits_1(self):
        # given: a second platform and a path dependency of macOS on it
        self.tree.add_platform("linux")
        deps = {"meshtown-core": "../../core", "meshtown-linux": "../linux"}
        self.tree.write("platforms/macos/Cargo.toml", _manifest("meshtown-macos", "gui/lib.rs", deps))
        self.tree.git_add()
        # then: the manifest of macOS is reported
        self.assert_violation("platform-foreign-dep", "platforms/macos/Cargo.toml")

    def test_untracked_file_is_checked(self):
        # given: a file added after `git add`, so it is untracked, with a path out of the platform directory
        self.tree.write("platforms/macos/build/x.sh", ". ../../../scripts/a.sh\n")
        # then: it is reported all the same
        self.assert_violation("platform-escape", "platforms/macos/build/x.sh")


class CannotCheck(_TreeCase):
    def test_no_cargo_in_path_exits_2(self):
        # given: a PATH made of an empty directory
        empty = self.tmp / "empty-bin"
        empty.mkdir()
        # when: the checker runs
        result = self.run_checker(path_env=empty)
        # then: it refuses to judge and names the missing tool
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("cargo", result.stderr)


if __name__ == "__main__":
    unittest.main()
