#!/usr/bin/env python3
"""Acceptance battery in one run:
`test`, `cargo (nextest/test) --no-default-features`, `clippy`, `test-js`, `web-typecheck`, `test-py`,
`web-check-lint`, `web-check` (Playwright), `web-check-node` (Playwright); continues after a red
item, writes a markdown report as a table and the full log of each item to `.tmp/runs/battery/<item>.log`.

Two modes. By default — two phases: phase 1 builds everything in turn (steps and layout are in
`battery_phases.py`; step logs live there too, `<step>.log`, service files in `meta/`), phase 2 runs
the built binaries in lanes in parallel (one thread per lane) in two consecutive epochs.
Why epochs: with `-C prefer-dynamic` cargo puts the dylib under the unhashed name
`deps/libmeshtown_core.dylib`, one for both feature sets, and each build relinks it for its own
set — binaries of the other set crash in dyld. So only one set is alive at any moment:
epoch B (`--no-default-features --features net-hyper`), then epoch A (default, with the tray; last, so that debug/ keeps
the binary with the tray); before each one comes a relink step (a repeat of the build command, seconds). `--serial` is the old mode:
`make test` and the rest one after another, in the order of ITEMS.

Usage:
    make battery [OUT=<path>] [ONLY=test,clippy] [SERIAL=1]
    python3 scripts/battery/battery.py [--out path] [--only test,clippy] [--serial]

The list of items is ITEMS below, edited in place (order = run order in --serial).
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field, replace
from datetime import datetime
from pathlib import Path

import battery_phases as phases
from battery_report import REPO_ROOT, build_report, format_table

LOG_DIR = REPO_ROOT / ".tmp" / "runs" / "battery"
DEFAULT_OUT = LOG_DIR / "report.md"


@dataclass(frozen=True)
class Item:
    name: str  # slug: log file name and the --only key
    label: str  # caption in the report table
    argv: tuple[str, ...]  # command, no shell substitutions except an explicit sh -c
    env: dict[str, str] | None = None  # environment overrides (merged on top of os.environ)


def _target_dir() -> str:
    """The same default as in the Makefile: BUILD_ROOT?=$(CURDIR)/.tmp/build,
    TARGET_DIR?=$(BUILD_ROOT)/host — respects an already set TARGET_DIR/CARGO_TARGET_DIR."""
    return (
        os.environ.get("CARGO_TARGET_DIR")
        or os.environ.get("TARGET_DIR")
        or str(REPO_ROOT / ".tmp" / "build" / "host")
    )


# The Makefile `test` has no counterpart without `--no-default-features` — `cargo nextest run
# --no-default-features` is not defined as a `make` target; this repeats the same fallback logic
# (nextest if present, otherwise cargo test) as the `test` target in the Makefile.
_NO_DEFAULT_SCRIPT = (
    "if cargo nextest --version >/dev/null 2>&1; then "
    "cargo nextest run --no-default-features --features net-hyper; "
    "else cargo test --no-default-features --features net-hyper; fi"
)

ITEMS: tuple[Item, ...] = (
    Item("test", "make test", ("make", "test")),
    Item(
        "test-no-default",
        "cargo (nextest/test) --no-default-features",
        ("sh", "-c", _NO_DEFAULT_SCRIPT),
        {"CARGO_TARGET_DIR": _target_dir()},
    ),
    Item("clippy", "make clippy-full", ("make", "clippy-full")),
    Item("test-js", "make test-js", ("make", "test-js")),
    Item("web-typecheck", "make web-typecheck", ("make", "web-typecheck")),
    Item("test-py", "make test-py", ("make", "test-py")),
    Item("web-check-lint", "make web-check-lint", ("make", "web-check-lint")),
    Item("web-check", "make web-check", ("make", "web-check")),
    Item("web-check-node", "make web-check-node", ("make", "web-check-node")),
)

# Summary extraction priority is in this order, the LAST matching log line is taken;
# if none matches, the last non-empty line as a whole (see extract_summary).
SUMMARY_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(r".*\d+\s+tests?\s+run.*"),
    re.compile(r".*\bRan\s+\d+\s+tests?\b.*"),
    re.compile(r".*SUMMARY:.*"),
    re.compile(r".*\d+\s+passed.*"),
)


WARNING_LINE = re.compile(r"^\s*\[[^\]]+\]\s+warning\b.*")


def _base_summary(lines: list[str]) -> str:
    for pattern in SUMMARY_PATTERNS:
        matches = [ln.strip() for ln in lines if pattern.match(ln)]
        if matches:
            return matches[-1]
    for ln in reversed(lines):
        if ln.strip():
            return ln.strip()
    return "(empty output)"


def extract_summary(log_text: str) -> str:
    """Item summary; the first line `[<case>] warning …` is appended via " · "
    (for example, a static ceiling warning)."""
    lines = log_text.splitlines()
    base = _base_summary(lines)
    warnings = [ln.strip() for ln in lines if WARNING_LINE.match(ln)]
    return f"{base} · {warnings[0]}" if warnings else base


@dataclass
class ItemResult:
    item: Item
    ok: bool
    duration_sec: float
    summary: str
    log_path: Path
    returncode: int
    output: str = field(repr=False, default="")


def item_argv(item: Item) -> list[str]:
    """The item command. The sub-make gets an explicit empty `ONLY=`: when run from `make battery
    ONLY=web-check`, GNU make itself passes command variables down to children (MAKEFLAGS), and
    `ONLY` — a battery item name — would end up in `E2E_GREP` of the `web-check` target as a filter on Playwright
    cases ("No tests found"). A command-line argument overrides the inherited value."""
    argv = list(item.argv)
    return argv + ["ONLY="] if argv[:1] == ["make"] else argv


def run_item(item: Item) -> ItemResult:
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    log_path = LOG_DIR / f"{item.name}.log"
    env = dict(os.environ)
    if item.env:
        env.update(item.env)

    start = time.monotonic()
    try:
        proc = subprocess.run(
            item_argv(item),
            cwd=REPO_ROOT,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            errors="replace",
        )
        output = proc.stdout or ""
        returncode = proc.returncode
    except OSError as exc:
        output = f"battery: couldn't start {item.argv}: {exc}\n"
        returncode = 127
    duration = time.monotonic() - start

    log_path.write_text(output, encoding="utf-8")
    return ItemResult(
        item=item,
        ok=(returncode == 0),
        duration_sec=duration,
        summary=extract_summary(output),
        log_path=log_path,
        returncode=returncode,
        output=output,
    )




def _git_info() -> tuple[str, str]:
    def _run(args: list[str]) -> str:
        try:
            out = subprocess.run(
                ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True
            )
            return out.stdout.strip()
        except OSError:
            return ""

    commit = _run(["rev-parse", "--short", "HEAD"]) or "?"
    branch = _run(["branch", "--show-current"]) or "(detached)"
    return commit, branch


def _select_items(only: str | None) -> list[Item]:
    if not only:
        return list(ITEMS)
    by_name = {i.name: i for i in ITEMS}
    names = [n.strip() for n in only.split(",") if n.strip()]
    unknown = [n for n in names if n not in by_name]
    if unknown:
        valid = ", ".join(by_name)
        print(
            f"battery: unknown items {unknown}, available: {valid}",
            file=sys.stderr,
        )
        raise SystemExit(2)
    return [by_name[n] for n in names]


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default=str(DEFAULT_OUT), help="markdown report path")
    parser.add_argument("--only", default=None, help="comma-separated items (names from ITEMS)")
    parser.add_argument("--serial", action="store_true", help="old mode: everything in turn, no phases")
    return parser.parse_args(argv)


def _failed(item: Item, summary: str, returncode: int = 125) -> ItemResult:
    log_path = LOG_DIR / f"{item.name}.log"
    return ItemResult(item, False, 0.0, summary, log_path, returncode, summary + "\n")


def _step_item(name: str, meta: Path) -> Item:
    env = {"CARGO_TARGET_DIR": _target_dir()} if name in phases.CARGO_NAMES else None
    return Item(name, name, phases.build_argv(name, meta), env)


def _prebuilt_item(item: Item, meta: Path) -> Item:
    """A phase 2 item with the command and environment for the built binaries (after phase 1)."""
    built = phases.run_argv(item.name, meta)
    if built is None:
        return item
    argv, extra = built
    env = dict(item.env or {})
    if item.name in phases.CARGO_NAMES:
        env["CARGO_TARGET_DIR"] = _target_dir()
    env.update(extra)
    return replace(item, argv=argv, env=env)


def _run_step(item: Item, meta: Path) -> ItemResult:
    result = run_item(item)
    error = phases.after_step(item.name, meta) if result.ok else None
    if error is None:
        return result
    return replace(result, ok=False, returncode=1, output=result.output + error + "\n", summary=error)


def _phase_one(items: list[Item], meta: Path) -> tuple[list[ItemResult], dict[str, ItemResult]]:
    """Build steps in turn; clippy is a report item, it runs among them and is not part of the steps table."""
    by_name = {i.name: i for i in items}
    steps: list[ItemResult] = []
    clippy: dict[str, ItemResult] = {}
    for name in phases.needed_steps(list(by_name)):
        if name == "clippy":
            clippy[name] = run_item(by_name[name])
            continue
        steps.append(_run_step(_step_item(name, meta), meta))
    return steps, clippy


def _run_epoch_item(item: Item, red: set[str], relink: str | None, meta: Path) -> ItemResult:
    needs = set(phases.NEEDS.get(item.name, ()))
    if item.name in phases.LIVE_DYLIB and relink in red:
        needs.add(relink)
    if red & needs:
        return _failed(item, "skipped: build is red")
    try:
        return run_item(_prebuilt_item(item, meta))
    except (OSError, KeyError, ValueError) as exc:
        return _failed(item, f"no built binary: {exc!r}")


def _phase_two(
    items: list[Item], red: set[str], meta: Path
) -> tuple[list[ItemResult], list[ItemResult]]:
    """Epochs in turn: relink the dylib of the needed feature set, then lanes in parallel.
    Returns the item results and the relink rows for the build table."""
    by_name = {i.name: i for i in items if i.name not in phases.PHASE1}
    done: list[ItemResult] = []
    relinks: list[ItemResult] = []
    for _epoch, relink, lanes in phases.epoch_plan(list(by_name)):
        if relink:
            relinks.append(_run_step(_step_item(relink, meta), meta))
            if not relinks[-1].ok:
                red = red | {relink}
        done += phases.run_lanes(
            lanes, lambda n: _run_epoch_item(by_name[n], red, relink, meta)
        )
    return done, relinks


def run_two_phase(items: list[Item]) -> tuple[list[ItemResult], list[ItemResult], str]:
    meta = LOG_DIR / "meta"
    meta.mkdir(parents=True, exist_ok=True)
    t0 = time.monotonic()
    steps, clippy = _phase_one(items, meta)
    t1 = time.monotonic()
    red = {s.item.name for s in steps if not s.ok}
    results, relinks = _phase_two(items, red, meta)
    done = {r.item.name: r for r in results} | clippy
    t2 = time.monotonic()
    mode = (
        f"two phases (epochs: {phases.epochs_text()}); wall = {t2 - t0:.0f} s; "
        f"phase 1 = {t1 - t0:.0f} s, phase 2 = {t2 - t1:.0f} s"
    )
    return [done[i.name] for i in items], steps + relinks, mode


def run_serial(items: list[Item]) -> tuple[list[ItemResult], list[ItemResult], str]:
    t0 = time.monotonic()
    results = [run_item(item) for item in items]
    return results, [], f"serial; wall = {time.monotonic() - t0:.0f} s"


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    items = _select_items(args.only)

    results, steps, mode = (run_serial if args.serial else run_two_phase)(items)

    commit, branch = _git_info()
    report = build_report(results, commit, branch, datetime.now(), mode, steps)

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(report, encoding="utf-8")

    # Compact stdout — an agent reads the output, extra lines cost tokens.
    print(format_table(results))
    print(f"\nMode: {mode}\nReport: {out_path}")

    return 1 if any(not r.ok for r in results + steps) else 0


if __name__ == "__main__":
    sys.exit(main())
