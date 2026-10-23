"""Tests of flaky_log.py on fragments of nextest logs."""
import unittest

import flaky_log

N = "net_hyper::contract_tests::transport::streams::"


def fail(name: str, n: int = 3) -> str:
    return f"  TRY {n} FAIL [  11.512s] (2342/2975) meshtown {N}{name}\n"


def summary(failed: int) -> str:
    return f"────────────\n     Summary [  71.885s] 2975 tests run: {2975 - failed} passed (4 slow), {failed} failed, 8 skipped\n"


def log(names: list[str], tail: str = "error: test run failed\nmake[1]: *** [test] Error 100\n") -> str:
    body = "".join(fail(n, 1) for n in names) + "".join(fail(n) for n in names)
    return body + summary(len(names)) + "        SLOW [  16.268s] (2323/2975) meshtown x::y\n" + "".join(fail(n) for n in names) + tail


class ClassifyTests(unittest.TestCase):
    def test_one_flaky_test(self):
        self.assertEqual(flaky_log.classify(log(["hundred"])), (True, [N + "hundred"]))

    def test_three_distinct_names(self):
        ok, names = flaky_log.classify(log(["c", "a", "b"]))
        self.assertTrue(ok)
        self.assertEqual(names, [N + "a", N + "b", N + "c"])

    def test_four_distinct_names_is_breakage(self):
        ok, why = flaky_log.classify(log(["a", "b", "c", "d"]))
        self.assertFalse(ok)
        self.assertIn("more than 3", why)

    def test_test_list_failed(self):
        text = "error: creating test list failed\n\nmake[1]: *** [test] Error 104\n"
        self.assertEqual(flaky_log.classify(text), (False, "test list"))

    def test_compile_error(self):
        text = "error: could not compile `meshtown`\nmake[1]: *** [test] Error 100\n"
        self.assertEqual(flaky_log.classify(text), (False, "compilation"))

    def test_js_failure_next_to_flaky_rust(self):
        text = log(["hundred"]) + "make[1]: *** [test-js] Error 1\n"
        self.assertEqual(flaky_log.classify(text), (False, "test-js"))

    def test_outer_make_line_is_not_a_failure_reason(self):
        text = log(["hundred"]) + "make: *** [test-changed] Error 100\n"
        self.assertTrue(flaky_log.classify(text)[0])

    def test_count_mismatch_is_not_flaky(self):
        text = log(["a"]).replace("1 failed", "2 failed")
        self.assertFalse(flaky_log.classify(text)[0])

    def test_no_failures(self):
        self.assertFalse(flaky_log.classify("all good\n")[0])


class MainTests(unittest.TestCase):
    def run_main(self, text):
        import contextlib, io, os, tempfile
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "nextest.log")
            with open(path, "w", encoding="utf-8") as f:
                f.write(text)
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                code = flaky_log.main(["flaky_log.py", path])
        return out.getvalue(), code

    def test_main_flaky_line(self):
        self.assertEqual(self.run_main(log(["hundred"])), ("flaky: hundred\n", 0))

    def test_main_not_flaky_line(self):
        text = "error: creating test list failed\n\nmake[1]: *** [test] Error 104\n"
        self.assertEqual(self.run_main(text), ("not flaky: test list\n", 1))


if __name__ == "__main__":
    unittest.main()
