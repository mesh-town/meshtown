"""Tests for run_all() from scripts/changed_tests.py: a red command does not cancel the following ones."""

import unittest
from unittest import mock

from changed_tests_fixtures import changed_tests


class RunAll(unittest.TestCase):
    """run_all() runs commands like `make -k`: all to the end, the code is the first non-zero."""

    def _run(self, codes: list[int]) -> tuple[int, list[list[str]]]:
        cmds = [["cmd", str(i)] for i in range(len(codes))]
        seen: list[list[str]] = []
        results = iter(codes)

        def fake(cmd: list[str]) -> int:
            seen.append(cmd)
            return next(results)

        with mock.patch.object(changed_tests, "_execute", fake):
            return changed_tests.run_all(cmds), seen

    def test_red_first_command_does_not_skip_the_rest(self):
        code, seen = self._run([2, 0, 0])

        self.assertEqual(code, 2)
        self.assertEqual(len(seen), 3)

    def test_first_nonzero_code_wins(self):
        code, seen = self._run([0, 3, 7])

        self.assertEqual(code, 3)
        self.assertEqual(len(seen), 3)

    def test_all_green_is_zero(self):
        code, seen = self._run([0, 0])

        self.assertEqual(code, 0)
        self.assertEqual(len(seen), 2)

    def test_no_commands_is_zero(self):
        self.assertEqual(self._run([]), (0, []))


if __name__ == "__main__":
    unittest.main()
