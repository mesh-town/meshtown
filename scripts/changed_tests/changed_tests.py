#!/usr/bin/env python3
"""Selecting and running the tests affected by an edit (`make test-changed`).

From `git diff` against `merge-base(BASE, HEAD)` plus untracked files and the path-to-test selection
rules it decides the `cargo nextest` slices,
`make test-py`/`test-js`/`web-typecheck`/`web-check`/`android-test` and runs them in order. It does not parse Rust — only
a text search for `meshtown_core::<module>` as a whole word (`users_of`), without an AST.
Run: `python3 scripts/changed_tests/changed_tests.py --base main [--dry-run]`.
"""

from __future__ import annotations

import argparse
import os
import platform
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

CORE = "core"
# Entries of `core/` that are not library modules and have their own rules.
NON_MODULE_TOPS = {"tests", "examples", "Cargo.toml"}
# Subdirectories of `core/web_gui/` that are not Rust modules (web UI sources and browser tests).
WEB_GUI_DATA_DIRS = {"ui", "e2e"}
BIN_MAIN = f"{CORE}/cli/main.rs"
STEP_COMMENT_RE = re.compile(r"//.*")
COMMON_PATH_RE = re.compile(r'#\[path\s*=\s*"common/([^"]+\.rs)"\]')

def _strip_comments(text: str) -> str:
    return STEP_COMMENT_RE.sub("", text)

def _run_git(args: list[str], cwd: str | None) -> str:
    try:
        proc = subprocess.run(
            ["git", *args], cwd=cwd, capture_output=True, text=True
        )
    except FileNotFoundError as exc:
        print(f"changed_tests: git unavailable: {exc}", file=sys.stderr)
        raise SystemExit(2)
    if proc.returncode != 0:
        print(f"changed_tests: git {' '.join(args)} failed: {proc.stderr.strip()}", file=sys.stderr)
        raise SystemExit(2)
    return proc.stdout

def changed_files(base: str) -> list[str]:
    """Paths from the repository root: diff(merge-base(base, HEAD)) ∪ untracked."""
    root = _run_git(["rev-parse", "--show-toplevel"], cwd=None).strip()
    merge_base = _run_git(["merge-base", base, "HEAD"], cwd=root).strip()
    diff_out = _run_git(["diff", "--name-only", merge_base], cwd=root)
    untracked_out = _run_git(["ls-files", "--others", "--exclude-standard"], cwd=root)
    paths = {p for p in diff_out.splitlines() if p}
    paths |= {p for p in untracked_out.splitlines() if p}
    return sorted(paths)

@dataclass
class Sources:
    main_rs: str
    tests: dict[str, str]
    examples: dict[str, str]

def _read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError:
        return ""

def _load_tests(tests_dir: Path) -> dict[str, str]:
    tests: dict[str, str] = {}
    if not tests_dir.is_dir():
        return tests
    for f in sorted(tests_dir.glob("*.rs")):
        text = _read_text(f)
        merged = [text]
        for m in COMMON_PATH_RE.finditer(text):
            merged.append(_read_text(tests_dir / "common" / m.group(1)))
        tests[f.stem] = "\n".join(merged)
    return tests

def _load_examples(examples_dir: Path) -> dict[str, str]:
    """Example file `examples/x.rs` → key `x`; example folder `examples/x/main.rs` → key `x`,
    text — all `*.rs` of the folder in a row (all files of the example). A folder without `main.rs` is skipped."""
    if not examples_dir.is_dir():
        return {}
    examples: dict[str, str] = {}
    for f in sorted(examples_dir.glob("*.rs")):
        examples[f.stem] = _read_text(f)
    for d in sorted(examples_dir.iterdir()):
        if not d.is_dir() or not (d / "main.rs").is_file():
            continue
        examples[d.name] = "\n".join(_read_text(f) for f in sorted(d.rglob("*.rs")))
    return examples

def load_sources(proto: Path) -> Sources:
    return Sources(
        main_rs=_read_text(proto / BIN_MAIN),
        tests=_load_tests(proto / CORE / "tests"),
        examples=_load_examples(proto / CORE / "examples"),
    )

def _group_bodies(text: str) -> list[tuple[str, str]]:
    """`(prefix, content)` of each `meshtown_core::[a::b::]{ ... }` (balanced braces, may be multiline);
    the prefix is `a::b` or an empty string."""
    bodies = []
    for m in re.finditer(r"meshtown_core::((?:\w+::)*)\{", text):
        depth = 1
        i = m.end()
        while i < len(text) and depth > 0:
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
            i += 1
        bodies.append((m.group(1).rstrip(":"), text[m.end() : i - 1]))
    return bodies

def _rest_after(prefix: str, module: str) -> str | None:
    """The part of `module` below the group prefix, or `None` if the module is not under it."""
    if not prefix:
        return module
    return module[len(prefix) + 2 :] if module.startswith(prefix + "::") else None

def _uses_module(module: str, clean: str) -> bool:
    segs = module.split("::")
    direct = re.compile(r"\bmeshtown_core::" + "::".join(map(re.escape, segs)) + r"\b")
    parents = [
        re.compile(r"\bmeshtown_core::" + "::".join(map(re.escape, segs[:k])) + r"\b(?!::[a-z_{])")
        for k in range(1, len(segs))
    ]
    if direct.search(clean) or any(p.search(clean) for p in parents):
        return True
    for prefix, body in _group_bodies(clean):
        if _group_uses(prefix, module, body):
            return True
    return False

def _group_uses(prefix: str, module: str, body: str) -> bool:
    rest = _rest_after(prefix, module)
    if rest is None:
        return False
    if re.search(r"\b" + re.escape(rest.split("::")[0]) + r"\b", body):
        return True
    # A group under a parent of the module that names `self` or a type (`Name`) uses the parent's items.
    return rest != module and re.search(r"\bself\b|\b[A-Z]\w*", body) is not None

def users_of(module: str, sources: dict[str, str]) -> set[str]:
    """Keys of `sources` where `meshtown_core::<module>` (`a::b`; directly, via a parent module import or in
    a `{…}` group) is a whole word, not in `//`."""
    return {key for key, text in sources.items() if _uses_module(module, _strip_comments(text))}

def common_users(common_file: str, tests: dict[str, str]) -> set[str]:
    needle = f"common/{common_file}.rs"
    return {name for name, text in tests.items() if needle in text}

@dataclass
class Selection:
    full: bool = False
    whole_bin: bool = False
    web_check: bool = False
    web_lint: bool = False
    py: bool = False
    js: bool = False
    web_typecheck: bool = False
    bundle_check: bool = False
    android: bool = False
    lib_modules: set[str] = field(default_factory=set)
    integration: set[str] = field(default_factory=set)
    reasons: list[str] = field(default_factory=list)

def _module_of(path: str) -> str | None:
    """`core/<group>/<module>/…` → `<group>::<module>`; `core/<group>/mod.rs` → `<group>`; `core/<name>.rs`
    → `<name>`. `None` — not a Rust module (data, tests directory, web UI sources)."""
    prefix = f"{CORE}/"
    if not path.startswith(prefix):
        return None
    parts = path[len(prefix) :].split("/")
    if parts[0] in NON_MODULE_TOPS or (parts[0] == "web_gui" and parts[1:2] and parts[1] in WEB_GUI_DATA_DIRS):
        return None
    if len(parts) > 2:
        return f"{parts[0]}::{parts[1]}"
    stem = _rs_stem(parts[-1])
    if stem is None:
        return None
    if len(parts) == 1:
        return stem
    return parts[0] if stem == "mod" else f"{parts[0]}::{stem}"

def _rs_stem(name: str) -> str | None:
    """`x.rs` and `x_tests.rs` → `x`; not a Rust file → `None`."""
    if name.endswith("_tests.rs"):
        return name[: -len("_tests.rs")] or None
    return name[: -len(".rs")] if name.endswith(".rs") else None

def _apply_lib_module(module: str, src: Sources, sel: Selection) -> None:
    sel.lib_modules.add(module)
    sel.integration |= users_of(module, src.tests)
    if users_of(module, {"main": src.main_rs}):
        sel.whole_bin = True
    if users_of(module, src.examples) or module.split("::")[0] == "web_gui":
        sel.web_check = True

PLATFORM_OSES = ("macos", "windows", "linux")

def _host_os() -> str:
    override = os.environ.get("CHANGED_TESTS_HOST_OS")
    if override:
        return override
    return {"Darwin": "macos", "Linux": "linux", "Windows": "windows"}.get(platform.system(), "")

def _platform_crate_os(path: str) -> str | None:
    """The OS of a platform crate whose manifest or gui/ tree holds `path`, else None."""
    for name in PLATFORM_OSES:
        if path == f"platforms/{name}/Cargo.toml" or path.startswith(f"platforms/{name}/gui/"):
            return name
    return None

def _select_platform_crate(name: str, sel: Selection) -> str:
    if name == _host_os():
        sel.full = True
        return "full (host platform crate)"
    return "no-op (foreign platform crate)"

def _select_command_line_tests(path: str, src: Sources, sel: Selection) -> None:
    """An edit under `core/cli/` selects every `cli_*` integration test: they run the real binary and name no
    library module, so the usual text search does not find them."""
    if path.startswith(f"{CORE}/cli/"):
        sel.integration |= {name for name in src.tests if name.startswith("cli_")}

def _select_one(path: str, src: Sources, sel: Selection) -> str:
    _select_command_line_tests(path, src, sel)
    if path == BIN_MAIN:
        sel.whole_bin = True
        return "whole_bin"
    if path == f"{CORE}/lib.rs":
        sel.full = True
        return "full"
    module = _module_of(path)
    if module is not None:
        _apply_lib_module(module, src, sel)
        return f"lib_modules += {module}"
    if path.startswith(f"{CORE}/tests/common/") and path.endswith(".rs"):
        common_file = path[len(f"{CORE}/tests/common/") : -len(".rs")]
        sel.integration |= common_users(common_file, src.tests)
        return f"common_users({common_file})"
    if path.startswith(f"{CORE}/tests/") and path.endswith(".rs"):
        name = path[len(f"{CORE}/tests/") : -len(".rs")]
        if name in src.tests:
            sel.integration.add(name)
        return f"integration += {name}"
    if path.startswith(f"{CORE}/web_gui/ui/"):
        sel.web_check = True
        sel.lib_modules |= {"web_gui", "node::identity"}
        if path.startswith(f"{CORE}/web_gui/ui/vue/") or path.startswith(f"{CORE}/web_gui/ui/bundle/"):
            sel.js = sel.bundle_check = True
        if path.startswith(f"{CORE}/web_gui/ui/vue/"):
            sel.web_typecheck = True
        return "web"
    if path.startswith(f"{CORE}/examples/"):
        sel.web_check = True
        return "web_check"
    if path.startswith("scripts/") and path.endswith(".py"):
        sel.py = True
        return "py"
    if path.endswith(".md"):
        return "no-op (md)"
    if path.startswith("platforms/android/"):
        sel.android = True
        return "android"
    crate_os = _platform_crate_os(path)
    if crate_os is not None:
        return _select_platform_crate(crate_os, sel)
    if path.startswith(f"{CORE}/web_gui/e2e/"):
        sel.web_check = sel.web_lint = True
        return "web_check + web_lint"
    sel.full = sel.py = sel.js = True
    return "full + py + js (catch-all)"

def select(paths: list[str], src: Sources) -> Selection:
    sel = Selection()
    for path in paths:
        what = _select_one(path, src, sel)
        sel.reasons.append(f"{path} → {what}")
    return sel

def _rust_parts(sel: Selection) -> list[str]:
    parts = [f"(kind(lib) & test(/^{m}::/))" for m in sel.lib_modules]
    if sel.whole_bin:
        parts.append("kind(bin)")
    parts += [f"binary({x})" for x in sel.integration]
    parts.sort()
    return parts

def _rust_command(sel: Selection) -> list[str] | None:
    parts = _rust_parts(sel)
    if not parts:
        return None
    argv = ["cargo", "nextest", "run", "--no-tests=warn"]
    if sel.lib_modules:
        argv.append("--lib")
    if sel.whole_bin:
        argv += ["--bin", "meshtown"]
    for x in sorted(sel.integration):
        argv += ["--test", x]
    if parts != ["kind(bin)"]:  # --bin alone already gives the same — -E is redundant here
        argv += ["-E", " | ".join(parts)]
    return argv

def commands(sel: Selection) -> list[list[str]]:
    cmds: list[list[str]] = []
    if sel.full:
        cmds.append(["make", "test"])
    else:
        rust = _rust_command(sel)
        if rust is not None:
            cmds.append(rust)
    if sel.py:
        cmds.append(["make", "test-py"])
    if sel.js:
        cmds.append(["make", "test-js"])
    if sel.web_typecheck:
        cmds.append(["make", "web-typecheck"])
    if sel.bundle_check:
        cmds.append(["make", "web-bundle-check"])
    if sel.web_check:
        cmds.append(["make", "web-check"])
    if sel.web_lint:
        cmds.append(["make", "web-check-lint"])
    if sel.android:
        cmds.append(["make", "android-test"])
    return cmds

def _execute(cmd: list[str]) -> int:
    return subprocess.run(cmd).returncode

def run_all(cmds: list[list[str]]) -> int:
    """Runs all commands to the end (a red one does not cancel the following, like `make -k`); the code is the first non-zero."""
    first = 0
    for cmd in cmds:
        code = _execute(cmd)
        first = first or code
    return first

def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="changed_tests.py")
    parser.add_argument("--base", default="main")
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args(argv)

def main(argv: list[str] | None = None) -> int:
    args = _parse_args(sys.argv[1:] if argv is None else argv)
    src = load_sources(Path("."))
    sel = select(changed_files(args.base), src)
    for reason in sel.reasons:
        print(reason)
    cmds = commands(sel)
    if not cmds:
        print("nothing to run")
        return 0
    for cmd in cmds:
        print(" ".join(cmd))
    if args.dry_run:
        return 0
    return run_all(cmds)

if __name__ == "__main__":
    sys.exit(main())
