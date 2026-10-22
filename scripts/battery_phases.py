"""Two battery phases (`battery.py`): what to build in turn (phase 1) and which commands run
the built binaries in lanes in parallel (phase 2). A leaf module: it knows nothing about `Item` or
the report, only the layout tables and the construction of argv/environment.

How nextest is launched and how binary paths are chosen are collected in two places — `build_argv` (the phase 1
step command) and `run_argv` (the phase 2 item command). Replacing reuse-build with a plain `cargo nextest run`
means fixing only `_nextest_run` and the two `_list_*`."""

from __future__ import annotations

import json
import shlex
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Callable, TypeVar

T = TypeVar("T")
R = TypeVar("R")

# Phase 2 is two consecutive "epochs"; within an epoch the lanes run simultaneously (cargo is not called),
# within a lane — in turn. Why epochs: with `-C prefer-dynamic` cargo puts the dylib under the
# unhashed name `deps/libmeshtown.dylib`, one for both feature sets, and each build
# relinks it for its own features; binaries of the other set crash in dyld. Only one set is alive at any moment:
# epoch B is `--no-default-features --features net-hyper`, epoch A (last, so that debug/ keeps
# the binary with the tray) is default. The layout is tuned by measurement.
EPOCHS: dict[str, dict[str, tuple[str, ...]]] = {
    "B": {"rust": ("test-no-default",), "light": ("web-check-lint", "test-js", "web-typecheck", "test-py")},
    "A": {"rust": ("test",), "node": ("web-check-node",), "demo": ("web-check",)},
}

# The dylib relink step before an epoch (a repeat of the phase 1 step command) -> the step whose command it repeats.
RELINK: dict[str, tuple[str, str]] = {
    "B": ("relink-no-default", "build-test-no-default"),
    "A": ("relink-default", "build-web"),
}

# Phase 1, strictly in turn. `clippy` is a report item (a make target), the rest are build steps.
# no-default is built before default: the default set stays alive last.
PHASE1: tuple[str, ...] = (
    "web-deps",
    "cargo-metadata",
    "build-test-no-default",
    "build-test",
    "build-web",
    "clippy",
)

# Which phase 1 steps an item needs; no entry — the item needs no build.
NEEDS: dict[str, tuple[str, ...]] = {
    "test": ("cargo-metadata", "build-test"),
    "test-no-default": ("cargo-metadata", "build-test-no-default"),
    "test-js": ("web-deps",),
    "web-check-lint": ("web-deps",),
    "web-typecheck": ("web-deps",),
    "web-check": ("web-deps", "build-web"),
    "web-check-node": ("web-deps", "build-web"),
}

# Items that need the live dylib of their feature set (a relink comes before their epoch).
LIVE_DYLIB = ("test", "test-no-default", "web-check", "web-check-node")


def _q(path: Path) -> str:
    return shlex.quote(str(path))


def _sh_proto(script: str) -> tuple[str, ...]:
    return ("sh", "-c", script)


def _list_binaries(meta: Path, name: str, flags: str) -> tuple[str, ...]:
    out = meta / f"{name}.binaries.json"
    return _sh_proto(
        f"cargo nextest list {flags}--list-type binaries-only --message-format json > {_q(out)}"
    )


def _nextest_run(meta: Path, name: str) -> tuple[str, ...]:
    return _sh_proto(
        f"cargo nextest run --binaries-metadata {_q(meta / (name + '.binaries.json'))} "
        f"--cargo-metadata {_q(meta / 'cargo-metadata.json')}"
    )


_BUILD_ARGV: dict[str, Callable[[Path], tuple[str, ...]]] = {
    "web-deps": lambda meta: ("make", "web-deps"),
    "cargo-metadata": lambda meta: _sh_proto(
        f"cargo metadata --format-version 1 > {_q(meta / 'cargo-metadata.json')}"
    ),
    "build-web": lambda meta: _sh_proto(
        "cargo build --example web_demo --bin meshtown "
        f"--message-format json-render-diagnostics > {_q(meta / 'build-web.json')}"
    ),
    "build-test-no-default": lambda meta: _list_binaries(
        meta, "test-no-default", "--no-default-features --features net-hyper "
    ),
    "build-test": lambda meta: _list_binaries(meta, "test", ""),
}


_BUILD_ARGV["relink-no-default"] = _BUILD_ARGV["build-test-no-default"]
_BUILD_ARGV["relink-default"] = _BUILD_ARGV["build-web"]


def build_argv(step: str, meta: Path) -> tuple[str, ...]:
    """The phase 1 step command."""
    return _BUILD_ARGV[step](meta)


def _bin_path(meta: Path, target: str) -> str:
    return json.loads((meta / "web-bins.json").read_text(encoding="utf-8"))[target]


def _make_nobuild(make_target: str, var: str, target: str, tag: str):
    def build(meta: Path) -> tuple[tuple[str, ...], dict[str, str]]:
        argv = ("make", make_target, f"{var}={_bin_path(meta, target)}")
        return argv, {"E2E_RUN_TAG": tag}

    return build


_RUN_ARGV: dict[str, Callable[[Path], tuple[tuple[str, ...], dict[str, str]]]] = {
    "test": lambda meta: (_nextest_run(meta, "test"), {}),
    "test-no-default": lambda meta: (_nextest_run(meta, "test-no-default"), {}),
    "web-check": _make_nobuild("web-check-nobuild", "E2E_DEMO_BIN", "web_demo", "battery-demo"),
    "web-check-node": _make_nobuild(
        "web-check-node-nobuild", "E2E_NODE_BIN", "meshtown", "battery-node"
    ),
}


CARGO_NAMES = frozenset(
    (
        "cargo-metadata", "build-web", "build-test", "build-test-no-default",
        "relink-default", "relink-no-default", "test", "test-no-default",
    )
)


def run_argv(name: str, meta: Path) -> tuple[tuple[str, ...], dict[str, str]] | None:
    """The command and environment additions of a phase 2 item for the built binaries; `None` — the item runs
    with its old command (make test-js and the like)."""
    builder = _RUN_ARGV.get(name)
    return builder(meta) if builder else None


def parse_executables(text: str) -> dict[str, str]:
    """From the `cargo build --message-format json` stream: target name -> `executable` path."""
    found: dict[str, str] = {}
    for line in text.splitlines():
        found.update(_artifact(line))
    return found


def _artifact(line: str) -> dict[str, str]:
    try:
        msg = json.loads(line)
    except ValueError:
        return {}
    if not isinstance(msg, dict) or msg.get("reason") != "compiler-artifact":
        return {}
    exe = msg.get("executable")
    return {msg["target"]["name"]: exe} if exe else {}


def after_step(step: str, meta: Path) -> str | None:
    """Handling the result of a green step; an error text means the step counts as red."""
    if step != "build-web":
        return None
    try:
        text = (meta / "build-web.json").read_text(encoding="utf-8")
    except OSError as exc:
        return f"no build output: {exc}"
    bins = parse_executables(text)
    missing = [t for t in ("web_demo", "meshtown") if t not in bins]
    if missing:
        return f"no executable for {missing} in cargo output"
    (meta / "web-bins.json").write_text(json.dumps(bins, indent=1), encoding="utf-8")
    return None


def _run_lane(items: list[T], run_one: Callable[[T], R]) -> list[R]:
    return [run_one(item) for item in items]


def run_lanes(lanes: dict[str, list[T]], run_one: Callable[[T], R]) -> list[R]:
    """Lanes simultaneously (one thread per lane), within a lane in turn. Results are in
    the order of lanes and items."""
    lanes = {k: v for k, v in lanes.items() if v}
    if not lanes:
        return []
    with ThreadPoolExecutor(max_workers=len(lanes)) as pool:
        futures = [pool.submit(_run_lane, items, run_one) for items in lanes.values()]
        return [r for f in futures for r in f.result()]


def needed_steps(names: list[str]) -> list[str]:
    """Phase 1 steps for the selected items, in PHASE1 order (clippy only if selected)."""
    wanted = {s for n in names for s in NEEDS.get(n, ())}
    wanted |= {n for n in names if n in PHASE1}
    return [s for s in PHASE1 if s in wanted]


def epoch_plan(names: list[str]) -> list[tuple[str, str | None, dict[str, list[str]]]]:
    """Epochs in order: (epoch, relink step or None, lanes with selected items); empty epochs
    are not included."""
    plan = []
    for epoch, lanes in EPOCHS.items():
        picked = {lane: [n for n in order if n in names] for lane, order in lanes.items()}
        flat = [n for v in picked.values() for n in v]
        if flat:
            relink = RELINK[epoch][0] if any(n in LIVE_DYLIB for n in flat) else None
            plan.append((epoch, relink, picked))
    return plan


def epochs_text() -> str:
    """Layout for the report's "Mode" line: `B [rust: a, light: b+c] → A [...]`."""
    def lane(k: str, v: tuple[str, ...]) -> str:
        return f"{k}: {'+'.join(v)}"

    return " → ".join(
        f"{e} [{', '.join(lane(k, v) for k, v in lanes.items())}]" for e, lanes in EPOCHS.items()
    )
