"""Tests of deb.py: dpkg-deb on a built package and the maintainer scripts.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/debian -t platforms/linux/build/debian
"""

import os
import shutil
import unittest

from test_support import HAS_DPKG_DEB
from deb_support import FILES, SCRIPT_NAMES, dpkg_deb, sandbox, setup, tmpdir


@unittest.skipUnless(HAS_DPKG_DEB, "dpkg-deb is not installed")
class RealDpkg(unittest.TestCase):
    def test_info_and_contents_succeed(self):
        # given
        built = setup(self, with_license=True)
        # when
        info = dpkg_deb("--info", built.path)
        contents = dpkg_deb("--contents", built.path)
        # then
        self.assertEqual(info.returncode, 0, info.stderr)
        self.assertEqual(contents.returncode, 0, contents.stderr)
        self.assertIn("Package: meshtown", info.stdout)
        for path in FILES:
            self.assertIn(f"./{path}", contents.stdout)

    def test_reports_the_same_version_and_architecture(self):
        # given
        built = setup(self, arch="arm64", version="2.5.9")
        # when
        version = dpkg_deb("-f", built.path, "Version")
        arch = dpkg_deb("-f", built.path, "Architecture")
        # then
        self.assertEqual((version.returncode, version.stdout.strip()), (0, "2.5.9"))
        self.assertEqual((arch.returncode, arch.stdout.strip()), (0, "arm64"))

    def test_extracts_the_files_with_their_modes(self):
        # given
        built = setup(self, with_license=True)
        target = tmpdir(self) / "root"
        # when
        done = dpkg_deb("--extract", built.path, target)
        # then
        self.assertEqual(done.returncode, 0, done.stderr)
        for path, mode in FILES.items():
            self.assertEqual((target / path).stat().st_mode & 0o777, mode, path)

    def test_control_extracts_with_the_maintainer_scripts(self):
        # given
        built = setup(self)
        target = tmpdir(self) / "DEBIAN"
        # when
        done = dpkg_deb("--control", built.path, target)
        # then
        self.assertEqual(done.returncode, 0, done.stderr)
        for name in SCRIPT_NAMES:
            self.assertTrue(os.access(target / name, os.X_OK), name)


@unittest.skipUnless(shutil.which("sh"), "sh is not installed")
class PostinstScript(unittest.TestCase):
    def test_first_install_reloads_systemd_and_hints_how_to_start(self):
        # given
        box = sandbox(self, systemd=True)
        # when
        result = box.run("postinst", "configure", "")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.verbs(), ["daemon-reload"], "neither enable nor start")
        self.assertIn("systemctl enable --now meshtown", result.out)

    def test_first_install_without_a_previous_version_argument_is_the_same(self):
        # given
        box = sandbox(self, systemd=True)
        # when
        result = box.run("postinst", "configure")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.verbs(), ["daemon-reload"])

    def test_upgrade_reloads_systemd_and_restarts_only_a_running_service(self):
        # given
        box = sandbox(self, systemd=True)
        # when
        result = box.run("postinst", "configure", "0.3.1")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.verbs(), ["daemon-reload", "try-restart"])
        self.assertIn("meshtown", box.journal()[1])

    def test_failing_restart_does_not_fail_the_upgrade(self):
        # given
        box = sandbox(self, systemd=True, fail_verb="try-restart")
        # when
        result = box.run("postinst", "configure", "0.3.1")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.verbs(), ["daemon-reload", "try-restart"])

    def test_hand_made_unit_leaves_systemctl_alone_and_says_so(self):
        # given
        box = sandbox(self, systemd=True, manual_unit=True)
        # when
        upgrade = box.run("postinst", "configure", "0.3.1")
        first = box.run("postinst", "configure", "")
        # then
        self.assertEqual((upgrade.code, first.code), (0, 0))
        self.assertEqual(box.journal(), [])
        self.assertNotEqual(upgrade.out.strip(), "", "a line about the hand-made service")

    def test_no_running_systemd_skips_systemctl_silently(self):
        # given
        box = sandbox(self, systemd=False)
        # when
        upgrade = box.run("postinst", "configure", "0.3.1")
        first = box.run("postinst", "configure", "")
        # then
        self.assertEqual((upgrade.code, first.code), (0, 0), upgrade.err + first.err)
        self.assertEqual(box.journal(), [])

    def test_other_actions_do_nothing(self):
        # given
        box = sandbox(self, systemd=True)
        # when
        result = box.run("postinst", "abort-upgrade", "0.3.1")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.journal(), [])


@unittest.skipUnless(shutil.which("sh"), "sh is not installed")
class PrermScript(unittest.TestCase):
    def test_remove_stops_and_disables_the_service(self):
        # given
        box = sandbox(self, systemd=True)
        # when
        result = box.run("prerm", "remove")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.verbs(), ["stop", "disable"])
        self.assertTrue(all("meshtown" in line for line in box.journal()), box.journal())

    def test_upgrade_does_nothing(self):
        # given
        box = sandbox(self, systemd=True)
        # when
        result = box.run("prerm", "upgrade", "0.3.2")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.journal(), [])

    def test_failing_stop_does_not_stop_the_removal(self):
        # given
        box = sandbox(self, systemd=True, fail_verb="stop")
        # when
        result = box.run("prerm", "remove")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.verbs(), ["stop", "disable"])


@unittest.skipUnless(shutil.which("sh"), "sh is not installed")
class PostrmScript(unittest.TestCase):
    def test_remove_reloads_systemd(self):
        # given
        box = sandbox(self, systemd=True)
        # when
        result = box.run("postrm", "remove")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.verbs(), ["daemon-reload"])

    def test_purge_reloads_systemd_and_keeps_the_node_state(self):
        # given
        box = sandbox(self, systemd=True)
        state = box.root / "etc" / "meshtown"
        state.mkdir(parents=True)
        (state / "config.json").write_text('{"network": "kept"}\n')
        (state / "node.key").write_bytes(b"\x01\x02\x03")
        # when
        result = box.run("postrm", "purge")
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(box.verbs(), ["daemon-reload"])
        self.assertEqual((state / "config.json").read_text(), '{"network": "kept"}\n')
        self.assertEqual((state / "node.key").read_bytes(), b"\x01\x02\x03")
        self.assertEqual(sorted(p.name for p in state.iterdir()), ["config.json", "node.key"])


if __name__ == "__main__":
    unittest.main()
