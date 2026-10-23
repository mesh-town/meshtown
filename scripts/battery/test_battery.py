"""Tests of `battery.py` (`make battery`): the summary parser for logs, the report table,
item selection by --only, run_item/main without real runs (subprocess.run
is replaced). These tests do not run the real `make test`/`web-check` etc."""

import contextlib
import io
import tempfile
import unittest
from dataclasses import replace
from datetime import datetime
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import battery


class ExtractSummary(unittest.TestCase):
    def test_nextest_tests_run_line_wins_first(self):
        log = "Starting 486 tests\n     Summary [  11.970s] 486 tests run: 486 passed, 0 skipped\n"
        self.assertEqual(
            battery.extract_summary(log),
            "Summary [  11.970s] 486 tests run: 486 passed, 0 skipped",
        )

    def test_unittest_ran_n_tests_line(self):
        log = "..........\nRan 44 tests in 1.11s\n\nOK\n"
        self.assertEqual(battery.extract_summary(log), "Ran 44 tests in 1.11s")

    def test_summary_line(self):
        log = "13-narrow: PASSED\n18-final: PASSED\nSUMMARY: all items passed.\n"
        self.assertEqual(battery.extract_summary(log), "SUMMARY: all items passed.")

    def test_n_passed_fallback_takes_last_match(self):
        log = (
            "test result: ok. 288 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
        )
        self.assertEqual(
            battery.extract_summary(log),
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
        )

    def test_no_pattern_falls_back_to_last_nonempty_line(self):
        log = "Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.34s\n\n"
        self.assertEqual(
            battery.extract_summary(log),
            "Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.34s",
        )

    def test_empty_log(self):
        self.assertEqual(battery.extract_summary(""), "(empty output)")

    def test_priority_order_tests_run_beats_passed(self):
        # "486 tests run" must win over the more general "N passed" on the same line.
        log = "Summary 486 tests run: 486 passed, 0 skipped\n"
        self.assertIn("tests run", battery.extract_summary(log))


class ExtractSummaryWarning(unittest.TestCase):
    def test_warning_appended_to_summary(self):
        log = (
            "[14-static-limits] warning, size above the ceiling — total 1 of 2\n"
            "  1 passed (2.0s)\n"
        )
        out = battery.extract_summary(log)
        self.assertIn("passed", out)
        self.assertIn("warning", out)
        self.assertIn(" · ", out)

    def test_without_warning_unchanged(self):
        self.assertEqual(battery.extract_summary("  1 passed (2.0s)\n"), "1 passed (2.0s)")


class SelectItems(unittest.TestCase):
    def test_default_returns_all_items_in_order(self):
        self.assertEqual(battery._select_items(None), list(battery.ITEMS))

    def test_web_check_lint_follows_test_py_in_items(self):
        names = [i.name for i in battery.ITEMS]

        self.assertEqual(names.index("web-check-lint"), names.index("test-py") + 1)

    def test_playwright_tail_summary_is_the_passed_line(self):
        log = "  1 skipped\n  74 passed (52.1s)\n"

        self.assertEqual(battery.extract_summary(log), "74 passed (52.1s)")

    def test_only_filters_and_preserves_requested_order(self):
        selected = battery._select_items("test-py,test")
        self.assertEqual([i.name for i in selected], ["test-py", "test"])

    def test_unknown_name_raises_system_exit_2(self):
        with self.assertRaises(SystemExit) as ctx:
            battery._select_items("nope")
        self.assertEqual(ctx.exception.code, 2)


class RunItem(unittest.TestCase):
    def test_success_writes_log_and_ok_true(self):
        item = battery.Item("fake-ok", "fake ok", ("true",))
        fake_proc = SimpleNamespace(stdout="all good\n", returncode=0)
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(battery, "LOG_DIR", Path(tmp)):
                with mock.patch("battery.subprocess.run", return_value=fake_proc):
                    result = battery.run_item(item)
            self.assertTrue(result.ok)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.log_path.read_text(encoding="utf-8"), "all good\n")

    def test_failure_is_captured_not_raised(self):
        item = battery.Item("fake-fail", "fake fail", ("false",))
        fake_proc = SimpleNamespace(stdout="boom\n", returncode=1)
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(battery, "LOG_DIR", Path(tmp)):
                with mock.patch("battery.subprocess.run", return_value=fake_proc):
                    result = battery.run_item(item)
            self.assertFalse(result.ok)
            self.assertEqual(result.returncode, 1)

    def test_missing_binary_is_reported_as_failure_not_exception(self):
        item = battery.Item("missing", "missing binary", ("no-such-binary-xyz",))
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(battery, "LOG_DIR", Path(tmp)):
                with mock.patch(
                    "battery.subprocess.run", side_effect=OSError("not found")
                ):
                    result = battery.run_item(item)
            self.assertFalse(result.ok)
            self.assertEqual(result.returncode, 127)
            self.assertIn("couldn't start", result.output)


class ItemArgv(unittest.TestCase):
    def test_make_items_get_explicit_empty_only(self):
        item = battery.Item("web-check", "make web-check", ("make", "web-check"))
        self.assertEqual(battery.item_argv(item), ["make", "web-check", "ONLY="])

    def test_non_make_items_are_untouched(self):
        item = battery.Item("sh", "sh", ("sh", "-c", "true"))
        self.assertEqual(battery.item_argv(item), ["sh", "-c", "true"])

    def test_run_item_passes_only_override_to_subprocess(self):
        item = battery.Item("web-check", "make web-check", ("make", "web-check"))
        fake_proc = SimpleNamespace(stdout="ok\n", returncode=0)
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(battery, "LOG_DIR", Path(tmp)):
                with mock.patch("battery.subprocess.run", return_value=fake_proc) as run:
                    battery.run_item(item)
        self.assertEqual(run.call_args.args[0], ["make", "web-check", "ONLY="])


class MainEntryPoint(unittest.TestCase):
    def _result(self, name, ok):
        item = battery.Item(name, f"make {name}", ("make", name))
        return battery.ItemResult(
            item=item,
            ok=ok,
            duration_sec=1.0,
            summary="ok" if ok else "FAILED",
            log_path=battery.LOG_DIR / f"{name}.log",
            returncode=0 if ok else 1,
            output="",
        )

    def test_all_green_returns_0_and_writes_report(self):
        results = [self._result("test", True), self._result("clippy", True)]
        with tempfile.TemporaryDirectory() as tmp:
            out_path = Path(tmp) / "report.md"
            with mock.patch("battery.run_item", side_effect=results):
                with mock.patch("battery._git_info", return_value=("abc123", "main")):
                    stdout = io.StringIO()
                    with contextlib.redirect_stdout(stdout):
                        code = battery.main(["--out", str(out_path), "--only", "test,clippy", "--serial"])
            self.assertEqual(code, 0)
            self.assertTrue(out_path.exists())
            self.assertIn("abc123", out_path.read_text(encoding="utf-8"))
            self.assertIn("| item |", stdout.getvalue())

    def test_one_red_returns_1(self):
        results = [self._result("test", True), self._result("clippy", False)]
        with tempfile.TemporaryDirectory() as tmp:
            out_path = Path(tmp) / "report.md"
            with mock.patch("battery.run_item", side_effect=results):
                with mock.patch("battery._git_info", return_value=("abc123", "main")):
                    with contextlib.redirect_stdout(io.StringIO()):
                        code = battery.main(["--out", str(out_path), "--only", "test,clippy", "--serial"])
            self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
