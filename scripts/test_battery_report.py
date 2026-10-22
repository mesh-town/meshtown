"""Tests of `battery_report.py`: the report table, red sections, the log tail, the report header."""

import unittest
from dataclasses import replace
from datetime import datetime

import battery
import battery_report


class FormatTable(unittest.TestCase):
    def _result(self, name="test", ok=True, duration=9.3, summary="486 passed", rc=0):
        item = battery.Item(name, f"make {name}", ("make", name))
        return battery.ItemResult(
            item=item,
            ok=ok,
            duration_sec=duration,
            summary=summary,
            log_path=battery.LOG_DIR / f"{name}.log",
            returncode=rc,
            output="",
        )

    def test_green_row_has_check_and_no_code(self):
        table = battery_report.format_table([self._result()])
        self.assertIn("✅", table)
        self.assertIn("9.30s", table)
        self.assertIn("486 passed", table)
        self.assertNotIn("code", table)

    def test_red_row_shows_exit_code(self):
        table = battery_report.format_table([self._result(ok=False, rc=2)])
        self.assertIn("❌ (code 2)", table)

    def test_pipe_in_summary_is_escaped(self):
        table = battery_report.format_table([self._result(summary="a | b")])
        self.assertIn("a \\| b", table)

    def test_red_sections_only_for_failed_items(self):
        green = self._result(name="clippy", ok=True)
        red = self._result(name="web-check", ok=False, rc=1)
        red = replace(red, output="line1\nline2\nERROR boom\n")
        sections = battery_report.format_red_sections([green, red])
        self.assertIn("web-check", sections)
        self.assertIn("ERROR boom", sections)
        self.assertNotIn("### make clippy", sections)

    def test_no_red_sections_when_all_green(self):
        self.assertEqual(battery_report.format_red_sections([self._result()]), "")

    def test_tail_limits_to_n_lines(self):
        text = "\n".join(f"line{i}" for i in range(50))
        tail = battery_report.tail(text, 30)
        self.assertEqual(len(tail.splitlines()), 30)
        self.assertIn("line49", tail)
        self.assertNotIn("line19", tail)


class BuildReport(unittest.TestCase):
    def test_report_has_header_commit_branch_and_table(self):
        item = battery.Item("test", "make test", ("make", "test"))
        result = battery.ItemResult(
            item=item,
            ok=True,
            duration_sec=9.3,
            summary="486 passed",
            log_path=battery.LOG_DIR / "test.log",
            returncode=0,
            output="",
        )
        report = battery_report.build_report([result], "abc123", "main", datetime(2026, 9, 28, 12, 0))
        self.assertIn("2026-09-28 12:00", report)
        self.assertIn("`abc123`", report)
        self.assertIn("`main`", report)
        self.assertIn("| item |", report)


if __name__ == "__main__":
    unittest.main()
