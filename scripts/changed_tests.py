#!/usr/bin/env python3
"""Selecting and running the tests affected by an edit (`make test-changed`).

From `git diff` against `merge-base(BASE, HEAD)` plus untracked files and the path-to-test selection
rules it decides the `cargo nextest` slices,
`make test-py`/`test-js`/`web-typecheck`/`web-check`/`android-test` and runs them in order. It does not parse Rust — only
a text search for `meshtown::<module>` as a whole word (`users_of`), without an AST.
Run: `python3 scripts/changed_tests.py --base main [--dry-run]`.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

CORE = "core"
# Entries of `core/` that are not library or binary modules and have their own rules.
NON_MODULE_TOPS = {"tests", "examples", "web_gui", "Cargo.toml"}
STEP_COMMENT_RE = re.compile(r"//.*")
MOD_DECL_RE = re.compile(
    r'(?:^|\n)\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;')
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
    bin: dict[str, str]
    examples: dict[str, str]

def _read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError:
        return ""

def bin_modules(main_rs: str) -> set[str]:
    """Names from the `mod x;` lines in main.rs (including with `#[cfg]` and `pub`), without the service `tests`."""
    mods = {m.group(1) for m in MOD_DECL_RE.finditer(main_rs)}
    mods.discard("tests")
    return mods

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

def _load_bin_module(proto: Path, name: str) -> str:
    files = [proto / CORE / f"{name}.rs", proto / CORE / f"{name}_tests.rs"]
    files = [f for f in files if f.is_file()]
    module_dir = proto / CORE / name
    if module_dir.is_dir():
        files += sorted(module_dir.rglob("*.rs"))
    return "\n".join(_read_text(f) for f in files)

def _load_examples(examples_dir: Path) -> dict[str, str]:
    """Example file `examples/x.rs` → key `x`; example folder `examples/x/main.rs` → key `x`,
    text — all `*.rs` of the folder in a row (like `_load_bin_module`). A folder without `main.rs` is skipped."""
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
    main_rs = _read_text(proto / CORE / "main.rs")
    bin_dict = {"main": main_rs}
    for name in sorted(bin_modules(main_rs)):
        bin_dict[name] = _load_bin_module(proto, name)
    return Sources(
        main_rs=main_rs,
        tests=_load_tests(proto / CORE / "tests"),
        bin=bin_dict,
        examples=_load_examples(proto / CORE / "examples"),
    )

def _group_bodies(text: str) -> list[str]:
    """The content of each `meshtown::{ ... }` (balanced braces, may be multiline)."""
    bodies = []
    for m in re.finditer(r"meshtown::\{", text):
        depth = 1
        i = m.end()
        while i < len(text) and depth > 0:
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
            i += 1
        bodies.append(text[m.end() : i - 1])
    return bodies

def users_of(module: str, sources: dict[str, str]) -> set[str]:
    """Keys of `sources` where `meshtown::<module>` (directly or in a `{…}` group) is a whole word, not in `//`."""
    direct = re.compile(r"\bmeshtown::" + re.escape(module) + r"\b")
    word = re.compile(r"\b" + re.escape(module) + r"\b")
    result = set()
    for key, text in sources.items():
        clean = _strip_comments(text)
        if direct.search(clean) or any(word.search(body) for body in _group_bodies(clean)):
            result.add(key)
    return result

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
    bin_modules: set[str] = field(default_factory=set)
    integration: set[str] = field(default_factory=set)
    reasons: list[str] = field(default_factory=list)

def _src_top(path: str, restrict_to: set[str] | None) -> str | None:
    prefix = f"{CORE}/"
    if not path.startswith(prefix):
        return None
    top = path[len(prefix) :].split("/", 1)[0]
    if top in NON_MODULE_TOPS:
        return None
    for suffix in ("_tests.rs", ".rs"):
        if top.endswith(suffix):
            top = top[: -len(suffix)]
            break
    if not top or (restrict_to is not None and top not in restrict_to):
        return None
    return top

def _apply_lib_module(top: str, src: Sources, sel: Selection) -> None:
    sel.lib_modules.add(top)
    sel.integration |= users_of(top, src.tests)
    for user in users_of(top, src.bin):
        if user == "main":
            sel.whole_bin = True
        else:
            sel.bin_modules.add(user)
    if users_of(top, src.examples) or top == "web":
        sel.web_check = True

def _select_one(path: str, src: Sources, bin_names: set[str], sel: Selection) -> str:
    if path in (f"{CORE}/main.rs", f"{CORE}/main_tests.rs"):
        sel.whole_bin = True
        return "whole_bin"
    if path == f"{CORE}/lib.rs":
        sel.full = True
        return "full"
    top = _src_top(path, bin_names)
    if top is not None:
        sel.bin_modules.add(top)
        return f"bin_modules += {top}"
    top = _src_top(path, None)
    if top is not None:
        _apply_lib_module(top, src, sel)
        return f"lib_modules += {top}"
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
        sel.lib_modules |= {"web", "node::identity"}
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
    if path.startswith(f"{CORE}/web_gui/e2e/"):
        sel.web_check = sel.web_lint = True
        return "web_check + web_lint"
    sel.full = sel.py = sel.js = True
    return "full + py + js (catch-all)"

def select(paths: list[str], src: Sources) -> Selection:
    bin_names = bin_modules(src.main_rs)
    sel = Selection()
    for path in paths:
        what = _select_one(path, src, bin_names, sel)
        sel.reasons.append(f"{path} → {what}")
    return sel

def _rust_parts(sel: Selection) -> list[str]:
    parts = [f"(kind(lib) & test(/^{m}::/))" for m in sel.lib_modules]
    if sel.whole_bin:
        parts.append("kind(bin)")
    elif sel.bin_modules:
        parts += [f"(kind(bin) & test(/^{m}::/))" for m in sel.bin_modules]
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
    if sel.whole_bin or sel.bin_modules:
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
