#!/usr/bin/env python3
"""Checking the boundary between the core (`core/`) and the platform crates (`platforms/<OS>/`) (`make boundary`).

Three kinds of checks:
  * manifests (`cargo metadata --no-deps`): the core has no path dependency into `platforms/`; a platform crate
    depends by path only on `meshtown-core`;
  * `include_str!`, `include_bytes!`, `include!` and `#[path = ...]` in `*.rs` do not leave their own tree
    (`core/` or `platforms/<OS>/`);
  * non-Rust files of a platform (scripts, Makefiles, manifests, docs): a `..` climb or a path from the workspace
    root variable must not leave `platforms/<OS>/` except to a few known places of the workspace.
Run: `python3 scripts/check_boundary.py [ROOT]`. Exit code: 0 clean (no output), 1 violations (stdout, one per
line: `<file>: <rule>: <text>`), 2 could not check (stderr).
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

ROOT_FILES = {"Cargo.toml", "Cargo.lock"}
ROOT_DIRS = {".tmp", "target", "dist"}
ASCENT_RE = re.compile(r"(?<![.\w])(\.\.(?:/\.\.)*)(?![.\w])(/[\w./-]*)?")
ASSIGN_RE = re.compile(r"^\s*(?:export\s+)?([A-Za-z_]\w*)\s*(?::=|\?=|=)(.*)$")
INCLUDE_RE = re.compile(
    r'\b(?:include_str|include_bytes|include)!\s*\(\s*(?:concat!\s*\(\s*)?(env!\s*\(\s*"[^"]*"\s*\)\s*,\s*)?"([^"]*)"'
)
PATH_ATTR_RE = re.compile(r'#\[path\s*=\s*"([^"]*)"\]')


class Violation(NamedTuple):
    file: str
    rule: str
    text: str


class CannotCheck(Exception):
    pass


def within(path: Path, base: Path) -> bool:
    return path == base or base in path.parents


def zone_of(root: Path, rel: str) -> Path:
    parts = Path(rel).parts
    return root / parts[0] if parts[0] == "core" else root / parts[0] / parts[1]


def allowed_target(root: Path, target: Path, own: Path, manifest: bool) -> bool:
    if not within(target, root):
        return False
    parts = target.relative_to(root).parts
    if within(target, own) or not parts:
        return True
    return (
        (len(parts) == 1 and parts[0] in ROOT_FILES)
        or parts[0] in ROOT_DIRS
        or parts == ("release", "lib.sh")
        or parts[:2] == ("release", "dist")
        or (manifest and parts[0] == "core")
    )


def load_metadata(root: Path) -> dict:
    cmd = ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"]
    proc = subprocess.run(cmd, cwd=root, capture_output=True, text=True)
    if proc.returncode != 0:
        raise CannotCheck("cargo metadata failed:\n" + proc.stderr.rstrip())
    return json.loads(proc.stdout)


def dep_violations(pkg: dict, root: Path) -> list:
    manifest = Path(pkg["manifest_path"]).resolve()
    if manifest == root / "core" / "Cargo.toml":
        return core_dep_violations(pkg, root)
    if manifest.parent.parent == root / "platforms":
        return platform_dep_violations(pkg, root)
    return []


def core_dep_violations(pkg: dict, root: Path) -> list:
    found = []
    for dep in pkg["dependencies"]:
        target = Path(dep["path"]).resolve() if dep.get("path") else None
        if target and within(target, root / "platforms"):
            found.append(Violation("core/Cargo.toml", "core-depends-on-platform",
                                   dep["name"] + " path=" + str(target.relative_to(root))))
    return found


def platform_dep_violations(pkg: dict, root: Path) -> list:
    found = []
    manifest = Path(pkg["manifest_path"]).resolve()
    for dep in pkg["dependencies"]:
        target = Path(dep["path"]).resolve() if dep.get("path") else None
        if target and not (dep["name"] == "meshtown-core" and target == root / "core"):
            found.append(Violation(str(manifest.relative_to(root)), "platform-foreign-dep",
                                   dep["name"] + " path=" + os.path.relpath(target, root)))
    return found


def manifest_violations(meta: dict, root: Path) -> list:
    found = []
    for pkg in meta["packages"]:
        found.extend(dep_violations(pkg, root))
    return found


def tracked_files(root: Path) -> list:
    cmd = ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "core", "platforms"]
    proc = subprocess.run(cmd, cwd=root, capture_output=True)
    if proc.returncode != 0:
        raise CannotCheck("not a git tree: " + proc.stderr.decode(errors="replace").strip())
    return sorted(set(p for p in proc.stdout.decode().split("\0") if p))


def read_text(path: Path):
    try:
        return path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return None


def rs_literal_violations(root: Path, rel: str, text: str) -> list:
    zone = zone_of(root, rel)
    kind = "core" if zone.name == "core" else "platform"
    found = []
    for lineno, line in enumerate(text.splitlines(), 1):
        for env, lit in [(m.group(1), m.group(2)) for m in INCLUDE_RE.finditer(line)] + \
                        [(None, m.group(1)) for m in PATH_ATTR_RE.finditer(line)]:
            base = zone if env else (root / rel).parent
            target = Path(os.path.normpath(str(base) + "/" + lit.lstrip("/") if env else str(base / lit)))
            if not within(target, zone):
                found.append(Violation(rel, kind + "-include-escape", "line %d: %s" % (lineno, lit)))
    return found


def root_variables(root: Path, rel: str, text: str) -> set:
    names = set()
    filedir = (root / rel).parent
    for line in text.splitlines():
        m = ASSIGN_RE.match(line)
        chain = ASCENT_RE.search(m.group(2)) if m else None
        if chain and ascent_target(filedir, chain) == root:
            names.add(m.group(1))
    return names


def ascent_target(filedir: Path, match) -> Path:
    climb = "/".join([".."] * (match.group(1).count("..")))
    base = os.path.normpath(str(filedir) + "/" + climb)
    return Path(os.path.normpath(base + (match.group(2) or "")))


def script_violations(root: Path, rel: str, text: str) -> list:
    own = zone_of(root, rel)
    filedir = (root / rel).parent
    manifest = rel == str(own.relative_to(root)) + "/Cargo.toml"
    names = root_variables(root, rel, text)
    var_re = None
    if names:
        alt = "|".join(re.escape(n) for n in sorted(names))
        var_re = re.compile(r"\$(?:\{(?:%s)\}|\((?:%s)\)|(?:%s)(?!\w))(/[\w./-]*)?" % (alt, alt, alt))
    found = []
    for lineno, line in enumerate(text.splitlines(), 1):
        targets = [(m.group(0), ascent_target(filedir, m)) for m in ASCENT_RE.finditer(line)]
        if var_re:
            targets += [(m.group(0), Path(os.path.normpath(str(root) + (m.group(1) or "")))) for m in var_re.finditer(line)]
        for shown, target in targets:
            if not allowed_target(root, target, own, manifest):
                found.append(Violation(rel, "platform-escape", "line %d: %s" % (lineno, shown)))
    return found


def file_violations(root: Path, rel: str) -> list:
    text = read_text(root / rel)
    if text is None:
        return []
    if rel.endswith(".rs"):
        return rs_literal_violations(root, rel, text)
    if rel.startswith("platforms/"):
        return script_violations(root, rel, text)
    return []


def text_violations(root: Path, files: list) -> list:
    found = []
    for rel in files:
        found.extend(file_violations(root, rel))
    return found


def check_environment(root: Path) -> None:
    for tool in ("cargo", "git"):
        if shutil.which(tool) is None:
            raise CannotCheck(tool + " not found in PATH")
    for need in ("Cargo.toml", "core", "platforms"):
        if not (root / need).exists():
            raise CannotCheck("no %s in %s" % (need, root))


def run(root: Path) -> list:
    check_environment(root)
    files = tracked_files(root)
    return manifest_violations(load_metadata(root), root) + text_violations(root, files)


def main(argv: list) -> int:
    default = Path(__file__).resolve().parent.parent
    root = Path(argv[1]).resolve() if len(argv) > 1 else default
    try:
        found = run(root)
    except CannotCheck as exc:
        print("check_boundary: " + str(exc), file=sys.stderr)
        return 2
    for v in found:
        print("%s: %s: %s" % v)
    if found:
        print("%d violation(s)" % len(found), file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
