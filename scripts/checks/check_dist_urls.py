#!/usr/bin/env python3
"""Nobody downloads releases or asks for the latest version from the source repository.

Release downloads and version lookups go to the distribution repository (mesh-town/meshtown-dist).
Scans the files tracked by git in --root (the whole tree when there is no git) for the old addresses.
Prints `file:line: text` for every hit and exits 1; exits 0 silently when clean.
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path

PATTERNS = [re.compile(p) for p in (
    r"api\.github\.com/repos/mesh-town/meshtown/",
    r"github\.com/mesh-town/meshtown/(releases|raw)\b",
    r"""--repo[ =]["']?mesh-town/meshtown\b(?!-)""",
)]
SELF = {"check_dist_urls.py", "test_check_dist_urls.py"}
SKIP_DIRS = {".git", "node_modules", "target"}


def _tracked(root: Path) -> list[Path] | None:
    try:
        out = subprocess.run(["git", "-C", str(root), "ls-files", "-z"], capture_output=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    return [root / n for n in out.decode("utf-8", "replace").split("\0") if n]


def _walk(root: Path) -> list[Path]:
    return [p for p in root.rglob("*") if p.is_file() and not SKIP_DIRS.intersection(p.relative_to(root).parts)]


def _files(root: Path) -> list[Path]:
    # A temporary tree inside another repository has no index of its own: ls-files then lists nothing.
    listed = _tracked(root) if (root / ".git").exists() else None
    return listed if listed is not None else _walk(root)


def _lines(path: Path) -> list[str]:
    try:
        data = path.read_bytes()
    except OSError:
        return []
    if b"\0" in data[:8192]:
        return []
    return data.decode("utf-8", "replace").splitlines()


def find_hits(root: Path) -> list[tuple[str, int, str]]:
    hits = []
    for path in sorted(_files(root)):
        if path.name in SELF or not path.is_file():
            continue
        rel = path.relative_to(root).as_posix()
        for no, line in enumerate(_lines(path), 1):
            if any(p.search(line) for p in PATTERNS):
                hits.append((rel, no, line.strip()[:200]))
    return hits


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = ap.parse_args(argv)
    hits = find_hits(args.root.resolve())
    for rel, no, text in hits:
        print(f"{rel}:{no}: {text}")
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
