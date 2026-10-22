"""Tests of the two battery phases: layout by epochs, cargo output parsing, lanes, a red build and
relink, --only, --serial. cargo and make are not run: `battery.run_item` is replaced."""

import contextlib
import io
import json
import tempfile
import threading
import unittest
from datetime import datetime
from pathlib import Path
from unittest import mock

import battery
import battery_phases as phases


def fake_result(item, ok=True):
    return battery.ItemResult(
        item, ok, 1.0, "ok" if ok else "FAILED", battery.LOG_DIR / f"{item.name}.log",
        0 if ok else 1, "",
    )


class Recorder:
    """Replacement for run_item: remembers the launch order; makes the listed names red."""

    def __init__(self, red=()):
        self.red = set(red)
        self.items = []
        self.lock = threading.Lock()

    def __call__(self, item):
        with self.lock:
            self.items.append(item)
        return fake_result(item, item.name not in self.red)

    def names(self):
        return [i.name for i in self.items]


class Layout(unittest.TestCase):
    def test_every_item_is_in_an_epoch_or_phase_one(self):
        in_epochs = {n for lanes in phases.EPOCHS.values() for o in lanes.values() for n in o}
        names = {i.name for i in battery.ITEMS}
        self.assertEqual(in_epochs | {"clippy"}, names)

    def test_epoch_b_runs_before_epoch_a(self):
        self.assertEqual(list(phases.EPOCHS), ["B", "A"])

    def test_needed_steps_follow_phase_one_order(self):
        steps = phases.needed_steps(["web-check", "test", "clippy"])
        self.assertEqual(steps, ["web-deps", "cargo-metadata", "build-test", "build-web", "clippy"])

    def test_test_py_needs_no_steps(self):
        self.assertEqual(phases.needed_steps(["test-py"]), [])

    def test_lint_needs_only_web_deps(self):
        self.assertEqual(phases.needed_steps(["test-py", "web-check-lint"]), ["web-deps"])

    def test_web_typecheck_is_in_light_lane_of_epoch_b_and_needs_only_web_deps(self):
        self.assertIn("web-typecheck", phases.EPOCHS["B"]["light"])
        self.assertEqual(phases.needed_steps(["web-typecheck"]), ["web-deps"])
        self.assertEqual(phases.epoch_plan(["web-typecheck"])[0][:2], ("B", None))

    def test_default_build_is_last_cargo_step(self):
        steps = phases.needed_steps([i.name for i in battery.ITEMS])
        self.assertEqual(steps[-2:], ["build-web", "clippy"])
        self.assertLess(steps.index("build-test-no-default"), steps.index("build-test"))

    def test_build_web_has_default_features(self):
        self.assertNotIn("--no-default-features", phases.build_argv("build-web", Path("/m"))[-1])

    def test_relink_repeats_build_command(self):
        m = Path("/m")
        self.assertEqual(phases.build_argv("relink-default", m), phases.build_argv("build-web", m))
        self.assertEqual(
            phases.build_argv("relink-no-default", m), phases.build_argv("build-test-no-default", m)
        )

    def test_epoch_plan_relinks_only_for_dylib_items(self):
        plan = phases.epoch_plan(["test-py", "web-check-lint"])
        self.assertEqual([(e, r) for e, r, _ in plan], [("B", None)])
        plan = phases.epoch_plan(["test", "test-no-default", "test-py"])
        self.assertEqual(
            [(e, r) for e, r, _ in plan], [("B", "relink-no-default"), ("A", "relink-default")]
        )


class ParseExecutables(unittest.TestCase):
    def test_takes_executable_by_target_name(self):
        text = "\n".join([
            json.dumps({"reason": "compiler-artifact", "target": {"name": "meshtown"},
                        "executable": "/t/debug/meshtown"}),
            json.dumps({"reason": "compiler-artifact", "target": {"name": "libx"},
                        "executable": None}),
            json.dumps({"reason": "build-finished", "success": True}),
            "not json",
        ])
        self.assertEqual(phases.parse_executables(text), {"meshtown": "/t/debug/meshtown"})

    def test_after_step_writes_bins_and_reports_missing(self):
        def artifact(name):
            return json.dumps(
                {"reason": "compiler-artifact", "target": {"name": name}, "executable": "/" + name}
            )

        with tempfile.TemporaryDirectory() as tmp:
            meta = Path(tmp)
            (meta / "build-web.json").write_text(artifact("web_demo"))
            self.assertIn("meshtown", phases.after_step("build-web", meta))
            (meta / "build-web.json").write_text("\n".join(map(artifact, ("web_demo", "meshtown"))))
            self.assertIsNone(phases.after_step("build-web", meta))
            bins = json.loads((meta / "web-bins.json").read_text())
            self.assertEqual(bins["meshtown"], "/meshtown")

    def test_other_steps_have_no_after_hook(self):
        self.assertIsNone(phases.after_step("build-test", Path("/nonexistent")))


class Argv(unittest.TestCase):
    def test_nextest_run_uses_binaries_metadata(self):
        argv, extra = phases.run_argv("test", Path("/m"))
        self.assertIn("--binaries-metadata /m/test.binaries.json", argv[-1])
        self.assertIn("--cargo-metadata /m/cargo-metadata.json", argv[-1])
        self.assertEqual(extra, {})

    def test_node_run_gets_bin_path_and_tag(self):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "web-bins.json").write_text(json.dumps({"meshtown": "/b/mp"}))
            argv, extra = phases.run_argv("web-check-node", Path(tmp))
        self.assertEqual(argv, ("make", "web-check-node-nobuild", "E2E_NODE_BIN=/b/mp"))
        self.assertEqual(extra, {"E2E_RUN_TAG": "battery-node"})

    def test_plain_make_items_are_unchanged(self):
        self.assertIsNone(phases.run_argv("test-py", Path("/m")))

    def test_build_steps_redirect_json_to_meta(self):
        no_default = phases.build_argv("build-test-no-default", Path("/m"))[-1]
        self.assertIn("> /m/test.binaries.json", phases.build_argv("build-test", Path("/m"))[-1])
        self.assertIn("--no-default-features", no_default)
        self.assertIn("--features net-hyper", no_default)

    def test_no_default_item_names_node_features(self):
        # both branches of the test-no-default item script (nextest and cargo test) name the node features
        node_set = "--no-default-features --features net-hyper"
        self.assertEqual(battery._NO_DEFAULT_SCRIPT.count(node_set), 2)


class Lanes(unittest.TestCase):
    def test_lanes_run_concurrently_and_inside_lane_in_order(self):
        started, order = threading.Barrier(2, timeout=5), []

        def run_one(name):
            if name in ("a1", "b1"):
                started.wait()  # both lanes must be in progress at the same time, otherwise a timeout
            order.append(name)
            return name

        out = phases.run_lanes({"a": ["a1", "a2"], "b": ["b1"], "c": []}, run_one)
        self.assertEqual(out, ["a1", "a2", "b1"])
        self.assertLess(order.index("a1"), order.index("a2"))


class TwoPhase(unittest.TestCase):
    def _main(self, rec, extra):
        base = battery.REPO_ROOT / ".tmp" / "tests"  # logs inside the tree: the report prints paths from the root
        base.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=base) as tmp, \
                mock.patch.object(battery, "LOG_DIR", Path(tmp)), \
                mock.patch("battery.run_item", rec), \
                mock.patch("battery.phases.after_step", return_value=None), \
                mock.patch("battery._prebuilt_item", side_effect=lambda i, m: i), \
                mock.patch("battery._git_info", return_value=("abc", "main")), \
                contextlib.redirect_stdout(io.StringIO()):
            code = battery.main(["--out", str(Path(tmp) / "r.md"), *extra])
            return code, (Path(tmp) / "r.md").read_text(encoding="utf-8")

    def test_phases_and_epochs_run_in_order(self):
        rec = Recorder()
        code, report = self._main(rec, ["--only", "test,test-no-default,web-check,test-py"])
        n = rec.names()
        self.assertEqual(code, 0)
        self.assertEqual(
            n[:5], ["web-deps", "cargo-metadata", "build-test-no-default", "build-test", "build-web"]
        )
        self.assertEqual(n[5], "relink-no-default")
        self.assertEqual(set(n[6:8]), {"test-no-default", "test-py"})
        self.assertEqual(n[8], "relink-default")
        self.assertEqual(set(n[9:]), {"test", "web-check"})
        self.assertIn("Mode: two phases (epochs: B [", report)
        self.assertIn("`relink-default`", report)

    def test_red_build_skips_dependents_only(self):
        rec = Recorder(red={"build-test"})
        code, report = self._main(rec, ["--only", "test,test-py"])
        self.assertEqual(code, 1)
        self.assertNotIn("test", rec.names())
        self.assertIn("test-py", rec.names())
        self.assertIn("skipped: build is red", report)

    def test_red_relink_skips_only_its_epoch_dylib_items(self):
        rec = Recorder(red={"relink-no-default"})
        code, _ = self._main(rec, ["--only", "test-no-default,test-py,test"])
        self.assertEqual(code, 1)
        self.assertNotIn("test-no-default", rec.names())
        self.assertIn("test-py", rec.names())
        self.assertIn("test", rec.names())

    def test_only_test_py_and_lint_needs_just_web_deps(self):
        rec = Recorder()
        self._main(rec, ["--only", "test-py,web-check-lint"])
        self.assertEqual(sorted(rec.names()), ["test-py", "web-check-lint", "web-deps"])

    def test_first_table_keeps_columns_and_selection_order(self):
        rec = Recorder()
        _, report = self._main(rec, ["--only", "test-py,clippy"])
        self.assertIn("| item | status | time | summary | log |", report)
        self.assertLess(report.index("`make test-py`"), report.index("`make clippy-full`"))

    def test_first_table_has_nine_rows_for_full_battery(self):
        rec = Recorder()
        _, report = self._main(rec, [])
        table = report.split("| item | status | time | summary | log |")[1].split("\n\n")[0]
        self.assertEqual(len([ln for ln in table.splitlines() if ln.startswith("| `")]), 9)

    def test_serial_keeps_old_order_and_no_phase_one(self):
        rec = Recorder()
        code, report = self._main(rec, ["--serial"])
        self.assertEqual(rec.names(), [i.name for i in battery.ITEMS])
        self.assertEqual(code, 0)
        self.assertIn("Mode: serial", report)
        self.assertNotIn("Build (phase 1)", report)

    def test_report_without_mode_is_unchanged(self):
        report = battery.build_report([fake_result(battery.ITEMS[0])], "c", "b", datetime(2026, 1, 1))
        self.assertNotIn("Mode", report)


class PrebuiltItem(unittest.TestCase):
    def test_sets_bin_path_and_tag(self):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "web-bins.json").write_text(json.dumps({"web_demo": "/b/wd"}))
            item = next(i for i in battery.ITEMS if i.name == "web-check")
            built = battery._prebuilt_item(item, Path(tmp))
        self.assertEqual(built.env["E2E_RUN_TAG"], "battery-demo")
        self.assertIn("E2E_DEMO_BIN=/b/wd", built.argv)


if __name__ == "__main__":
    unittest.main()
