"""Version-only edits of Cargo.toml / Cargo.lock select nothing.

Every merge bumps the package version in the manifests and in the lock file; that alone must not trigger the
whole battery. A manifest or lock file counts as version-only when, after blanking the `version` of the
workspace packages, its old and new texts are equal.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path
from typing import Callable

VERSION_RE = re.compile(r'^version\s*=\s*"[^"]*"\s*$')
SECTION_RE = re.compile(r"^\s*\[")
PACKAGE_SECTIONS = ("[package]", "[workspace.package]")
BLANK = 'version = "<workspace>"'


def _blank_manifest(text: str) -> list[str]:
    out, in_package = [], False
    for line in text.splitlines():
        if SECTION_RE.match(line):
            in_package = line.strip() in PACKAGE_SECTIONS
        out.append(BLANK if in_package and VERSION_RE.match(line) else line)
    return out


def _lock_blocks(text: str) -> list[list[str]]:
    blocks: list[list[str]] = [[]]
    for line in text.splitlines():
        if line.strip() == "[[package]]":
            blocks.append([])
        blocks[-1].append(line)
    return blocks


def _blank_lock_block(block: list[str]) -> list[str]:
    is_workspace = block[:1] == ["[[package]]"] and not any(l.startswith("source") for l in block)
    if not is_workspace:
        return block
    return [BLANK if VERSION_RE.match(l) else l for l in block]


def _blank_lock(text: str) -> list[str]:
    return [line for block in _lock_blocks(text) for line in _blank_lock_block(block)]


def _blank(path: str, text: str) -> list[str]:
    return _blank_lock(text) if path.endswith("Cargo.lock") else _blank_manifest(text)


def is_cargo_file(path: str) -> bool:
    return path.rsplit("/", 1)[-1] in ("Cargo.toml", "Cargo.lock")


def version_only(path: str, old: str | None, new: str | None) -> bool:
    """True when `path` differs between `old` and `new` only in workspace package versions."""
    if old is None or new is None:
        return False
    return _blank(path, old) == _blank(path, new)


def drop_version_only(
    paths: list[str], old_text: Callable[[str], str | None], root: Path
) -> list[str]:
    """`paths` without the Cargo files whose change is a version bump only."""
    kept = []
    for path in paths:
        new = (root / path).read_text() if (root / path).is_file() else None
        if not (is_cargo_file(path) and version_only(path, old_text(path), new)):
            kept.append(path)
    return kept


def _git_out(args: list[str]) -> str | None:
    proc = subprocess.run(["git", *args], capture_output=True, text=True)
    return proc.stdout if proc.returncode == 0 else None


def base_text_reader(base: str) -> Callable[[str], str | None]:
    """Reader of file content at merge-base(base, HEAD); None for files that did not exist there."""
    merge_base = (_git_out(["merge-base", base, "HEAD"]) or "").strip()
    root = (_git_out(["rev-parse", "--show-toplevel"]) or ".").strip()

    def read(path: str) -> str | None:
        return _git_out(["-C", root, "show", f"{merge_base}:{path}"]) if merge_base else None

    return read
