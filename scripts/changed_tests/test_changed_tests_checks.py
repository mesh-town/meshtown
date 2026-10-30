"""Tests for the platform-crate and linux-check / features-check selection in scripts/changed_tests/changed_tests.py.

Shared helpers are in changed_tests_fixtures.py (see its docstring).
"""

import os
import unittest
from unittest import mock

from changed_tests_fixtures import _sources, changed_tests


class SelectChecks(unittest.TestCase):
    """select() — `linux-check` and `features-check` by path and by cfg text of the changed file."""

    def _sel(self, path, text=""):
        src = _sources()
        src.texts[path] = text
        return changed_tests.select([path], src)

    def test_linux_platform_crate_and_manifests_select_linux_check(self):
        for path in ("platforms/linux/gui/tray.rs", "Cargo.lock", "Cargo.toml", "core/Cargo.toml"):
            with self.subTest(path=path):
                self.assertTrue(self._sel(path).linux_check)

    def test_rs_with_target_os_linux_selects_linux_check(self):
        sel = self._sel("core/transport/net/x.rs", '#[cfg(target_os = "linux")]\nfn f() {}\n')

        self.assertTrue(sel.linux_check)
        self.assertIn("core/transport/net/x.rs → linux-check", sel.reasons)

    def test_plain_rs_and_other_os_do_not_select_linux_check(self):
        self.assertFalse(self._sel("core/invite.rs", "fn f() {}\n").linux_check)
        self.assertFalse(self._sel("core/invite.rs", '#[cfg(target_os = "macos")]\nfn f() {}\n').linux_check)
        self.assertFalse(self._sel("README.md", 'target_os = "linux"').linux_check)

    def test_cfg_feature_and_core_manifest_select_features_check(self):
        self.assertTrue(self._sel("core/invite.rs", '#[cfg(feature = "gui")]\nfn f() {}\n').features_check)
        self.assertTrue(self._sel("core/invite.rs", '#[cfg(not(feature = "gui"))]\nfn f() {}\n').features_check)
        self.assertTrue(self._sel("core/Cargo.toml").features_check)

    def test_plain_rs_does_not_select_features_check(self):
        self.assertFalse(self._sel("core/invite.rs", "fn f() { let feature = 1; }\n").features_check)
        self.assertFalse(self._sel("platforms/linux/gui/x.rs", "fn f() {}\n").features_check)

    def test_checks_go_last_in_commands(self):
        sel = changed_tests.Selection(features_check=True, linux_check=True, py=True)

        self.assertEqual(
            changed_tests.commands(sel),
            [["make", "test-py"], ["make", "features-check"], ["make", "linux-check"]],
        )




class SelectPlatformCrates(unittest.TestCase):
    """select() — platform crates: the host OS runs everything, a foreign OS nothing, build files fall to the catch-all."""

    def test_host_platform_gui_selects_full(self):
        with mock.patch.dict(os.environ, {"CHANGED_TESTS_HOST_OS": "macos"}):
            sel = changed_tests.select(["platforms/macos/gui/backend.rs"], _sources())

        self.assertTrue(sel.full)

    def test_host_platform_manifest_selects_full(self):
        with mock.patch.dict(os.environ, {"CHANGED_TESTS_HOST_OS": "linux"}):
            sel = changed_tests.select(["platforms/linux/Cargo.toml"], _sources())

        self.assertTrue(sel.full)

    def test_foreign_platform_gui_selects_nothing(self):
        with mock.patch.dict(os.environ, {"CHANGED_TESTS_HOST_OS": "macos"}):
            sel = changed_tests.select(["platforms/windows/gui/backend.rs"], _sources())

        self.assertEqual(
            (sel.full, sel.py, sel.js, sel.web_check, sel.android), (False, False, False, False, False)
        )

    def test_platform_build_script_selects_full(self):
        with mock.patch.dict(os.environ, {"CHANGED_TESTS_HOST_OS": "macos"}):
            sel = changed_tests.select(["platforms/linux/build/build.sh"], _sources())

        self.assertEqual((sel.full, sel.py, sel.js), (True, True, True))
