"""Tests of the S99meshtown init script against a stub rc.func that records what the script sets up.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/keenetic/feed -t platforms/linux/build/keenetic/feed
"""

import os
import stat
import subprocess
import unittest
from pathlib import Path

import build_feed
from feed_fixtures import tmpdir

STUB_RC = """\
echo "ENABLED=$ENABLED PROCS=$PROCS ARGS=$ARGS NO_COLOR=$NO_COLOR" > "$OUT/vars"
$PRECMD
"""
STUB_MESHTOWN = '#!/bin/sh\necho "meshtown $*" >> "$OUT/calls"\n'


def run_script(test):
    """Run S99meshtown with /opt redirected into a temp root: (vars line, calls, config dir mode)."""
    tmp = tmpdir(test)
    root = tmp / "root"
    (root / "opt/bin").mkdir(parents=True)
    (root / "opt/etc/init.d").mkdir(parents=True)
    (root / "opt/etc/init.d/rc.func").write_text(STUB_RC)
    (root / "opt/bin/meshtown").write_text(STUB_MESHTOWN)
    (root / "opt/bin/meshtown").chmod(0o755)
    script = (build_feed.INIT_D).read_text().replace("/opt/", f"{root}/opt/")
    (tmp / "S99").write_text(script)
    env = {**os.environ, "OUT": str(tmp)}
    done = subprocess.run(["sh", str(tmp / "S99"), "start"], capture_output=True, text=True, env=env, timeout=30)
    assert done.returncode == 0, done.stderr
    calls = (tmp / "calls").read_text().splitlines() if (tmp / "calls").exists() else []
    conf_dir = root / "opt/etc/meshtown"
    mode = stat.S_IMODE(conf_dir.stat().st_mode) if conf_dir.exists() else None
    return (tmp / "vars").read_text(), calls, mode, root


class InitScript(unittest.TestCase):
    def test_sets_the_rc_func_variables_of_the_node(self):
        # given / when
        variables, _, _, root = run_script(self)
        # then
        self.assertEqual(variables.strip(),
                         f"ENABLED=yes PROCS=meshtown ARGS=run --config {root}/opt/etc/meshtown/config.json NO_COLOR=1")

    def test_before_the_start_it_makes_a_private_state_directory_and_seeds_the_lan_bridge(self):
        # given / when
        _, calls, mode, root = run_script(self)
        # then
        self.assertEqual(mode, 0o700)
        self.assertEqual(calls, [f"meshtown ifaces --config {root}/opt/etc/meshtown/config.json --if-unset br0"])

    def test_is_valid_posix_shell(self):
        # given / when
        done = subprocess.run(["sh", "-n", str(build_feed.INIT_D)], capture_output=True, text=True)
        # then
        self.assertEqual(done.returncode, 0, done.stderr)


if __name__ == "__main__":
    unittest.main()
