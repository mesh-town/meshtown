"""Tests for commands() from scripts/changed_tests.py.

Shared helpers are in changed_tests_fixtures.py (see its docstring).
"""

import unittest

from changed_tests_fixtures import _sources, changed_tests


class Commands(unittest.TestCase):
    """commands() — building the command list from a ready Selection, without side effects."""

    def test_lib_module_with_integration_test_is_one_nextest_line(self):
        sel = changed_tests.Selection(
            full=False,
            whole_bin=False,
            web_check=False,
            web_lint=False,
            py=False,
            js=False,
            lib_modules={"words"},
            bin_modules=set(),
            integration={"identity_restart"},
            reasons=[],
        )

        cmds = changed_tests.commands(sel)

        self.assertEqual(
            cmds,
            [
                [
                    "cargo",
                    "nextest",
                    "run",
                    "--no-tests=warn",
                    "--lib",
                    "--test",
                    "identity_restart",
                    "-E",
                    "(kind(lib) & test(/^words::/)) | binary(identity_restart)",
                ]
            ],
        )

    def test_whole_bin_only_runs_bin_target_without_lib_filter(self):
        sel = changed_tests.Selection(
            full=False,
            whole_bin=True,
            web_check=False,
            web_lint=False,
            py=False,
            js=False,
            lib_modules=set(),
            bin_modules=set(),
            integration=set(),
            reasons=[],
        )

        cmds = changed_tests.commands(sel)

        self.assertEqual(
            cmds,
            [["cargo", "nextest", "run", "--no-tests=warn", "--bin", "meshtown"]],
        )

    def test_full_runs_make_without_nextest(self):
        sel = changed_tests.Selection(
            full=True,
            whole_bin=False,
            web_check=False,
            web_lint=False,
            py=False,
            js=False,
            lib_modules=set(),
            bin_modules=set(),
            integration=set(),
            reasons=[],
        )

        cmds = changed_tests.commands(sel)

        self.assertEqual(cmds, [["make", "test"]])

    def test_empty_selection_runs_nothing(self):
        sel = changed_tests.Selection(
            full=False,
            whole_bin=False,
            web_check=False,
            web_lint=False,
            py=False,
            js=False,
            lib_modules=set(),
            bin_modules=set(),
            integration=set(),
            reasons=[],
        )

        self.assertEqual(changed_tests.commands(sel), [])

    def test_commands_do_not_depend_on_the_order_of_changed_paths(self):
        src = _sources()
        paths = ["src/words.rs", "e2e/specs/basic.spec.ts"]

        sel_forward = changed_tests.select(paths, src)
        sel_reversed = changed_tests.select(list(reversed(paths)), src)

        self.assertEqual(sel_forward.lib_modules, sel_reversed.lib_modules)
        self.assertEqual(sel_forward.bin_modules, sel_reversed.bin_modules)
        self.assertEqual(sel_forward.integration, sel_reversed.integration)
        self.assertEqual(
            changed_tests.commands(sel_forward), changed_tests.commands(sel_reversed)
        )

    def test_web_typecheck_follows_test_js(self):
        sel = changed_tests.Selection(js=True, web_typecheck=True)

        cmds = changed_tests.commands(sel)

        self.assertEqual(
            cmds, [["make", "test-js"], ["make", "web-typecheck"]]
        )

    def test_web_lint_adds_web_check_lint_last(self):
        sel = changed_tests.Selection(
            web_check=True,
            web_lint=True,
            py=True,
        )

        cmds = changed_tests.commands(sel)

        self.assertEqual(cmds[-1], ["make", "web-check-lint"])
        self.assertEqual(cmds[-2], ["make", "web-check"])


if __name__ == "__main__":
    unittest.main()
