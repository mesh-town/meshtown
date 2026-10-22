"""publish.sh: one orphan commit on main of a bare repository, authored by Mesh Town."""
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "publish.sh"


def git(*args, cwd=None):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


class PublishTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.tmp = Path(self._tmp.name)
        self.bare = self.tmp / "remote.git"
        git("init", "-q", "--bare", "-b", "main", str(self.bare))
        self.feed = self.tmp / "feed"
        (self.feed / "opkg" / "x86_64").mkdir(parents=True)
        (self.feed / "install.sh").write_text("#!/bin/sh\n")
        (self.feed / ".nojekyll").write_text("")
        (self.feed / "opkg" / "x86_64" / "Packages").write_text("Package: meshtown\n")

    def run_publish(self, feed, version):
        env = dict(os.environ)
        # The caller's identity must not leak into the commit.
        env.update(GIT_AUTHOR_NAME="Someone", GIT_AUTHOR_EMAIL="someone@example.com",
                   GIT_COMMITTER_NAME="Someone", GIT_COMMITTER_EMAIL="someone@example.com")
        return subprocess.run(["bash", str(SCRIPT), str(feed), str(self.bare), version],
                              capture_output=True, text=True, env=env)

    def test_two_runs_leave_one_commit(self):
        self.assertEqual(self.run_publish(self.feed, "0.2.7").returncode, 0)
        (self.feed / "install.sh").write_text("#!/bin/sh\n# changed\n")
        done = self.run_publish(self.feed, "0.2.8")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(git("--git-dir", str(self.bare), "rev-list", "--count", "main"), "1")
        self.assertEqual(git("--git-dir", str(self.bare), "log", "-1", "--format=%s", "main"), "meshtown 0.2.8")
        who = git("--git-dir", str(self.bare), "log", "-1", "--format=%an <%ae>|%cn <%ce>", "main")
        self.assertEqual(who, "Mesh Town <meshtown.app@gmail.com>|Mesh Town <meshtown.app@gmail.com>")
        files = set(git("--git-dir", str(self.bare), "ls-tree", "-r", "--name-only", "main").splitlines())
        self.assertEqual(files, {"install.sh", ".nojekyll", "opkg/x86_64/Packages"})
        shown = git("--git-dir", str(self.bare), "show", "main:install.sh")
        self.assertIn("# changed", shown)

    def test_empty_or_missing_directory_fails_and_pushes_nothing(self):
        self.assertEqual(self.run_publish(self.feed, "0.2.7").returncode, 0)
        before = git("--git-dir", str(self.bare), "rev-parse", "main")
        empty = self.tmp / "empty"
        empty.mkdir()
        for bad in (empty, self.tmp / "missing"):
            done = self.run_publish(bad, "0.2.9")
            self.assertEqual(done.returncode, 1)
        self.assertEqual(git("--git-dir", str(self.bare), "rev-parse", "main"), before)


if __name__ == "__main__":
    unittest.main()
