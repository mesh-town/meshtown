#!/usr/bin/env python3
"""Builds the Mesh Town opkg feed for Keenetic routers with Entware: one .ipk and index per Entware architecture.

build_feed.py --out DIR --version X.Y.Z [--pkg-release N] --base-url URL --bin TRIPLE=PATH [...]
Layout: DIR/<entware dir>/meshtown_<version>-rN_<Architecture>.ipk, Packages, Packages.gz; install.sh, README.md,
index.html and .nojekyll in DIR.
Exit codes: 0 feed built, 2 bad invocation, 1 failure while writing.
"""
import argparse
import os
import re
import sys
import tempfile
from collections import namedtuple
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.append(str(HERE.parent.parent / "openwrt" / "feed"))  # after HERE: this directory's readme.py wins

import ipk  # noqa: E402
import targets  # noqa: E402
from readme import render_readme  # noqa: E402

Arch = namedtuple("Arch", "dir architecture machine")
# Rust target -> Entware feed directory, opkg Architecture of the package, ELF machine of the binary.
ARCHES = {
    "mipsel-unknown-linux-musl": Arch("mipselsf-k3.4", "mipsel-3.4", "mipsel"),
    "mips-unknown-linux-musl": Arch("mipssf-k3.4", "mips-3.4", "mips"),
    "aarch64-unknown-linux-musl": Arch("aarch64-k3.10", "aarch64-3.10", "aarch64"),
}
INIT_D = HERE / "S99meshtown"
SCRIPTS = HERE / "scripts"


class UsageError(Exception):
    pass


def stage_tree(dst, binary) -> Path:
    """Package file tree: everything lives under /opt."""
    dst = Path(dst)
    files = [("opt/bin/meshtown", Path(binary).read_bytes(), 0o755),
             ("opt/etc/init.d/S99meshtown", INIT_D.read_bytes(), 0o755)]
    for rel, data, mode in files:
        p = dst / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data)
        p.chmod(mode)
        os.utime(p, (ipk.mtime(), ipk.mtime()))
    return dst


def render_install(base_url) -> str:
    return (HERE / "install.sh.in").read_text().replace("@BASE@", base_url.rstrip("/"))


def parse_args(argv):
    p = argparse.ArgumentParser(prog="build_feed.py", description=__doc__.splitlines()[0])
    p.add_argument("--out", required=True)
    p.add_argument("--version", required=True)
    p.add_argument("--pkg-release", default="1")
    p.add_argument("--base-url", required=True)
    p.add_argument("--bin", action="append", default=[], metavar="TRIPLE=PATH")
    return p.parse_args(argv)


def check_release(args) -> str:
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        raise UsageError(f"version must be X.Y.Z, got {args.version!r}")
    if not re.fullmatch(r"[0-9]+", args.pkg_release) or int(args.pkg_release) < 1:
        raise UsageError(f"--pkg-release must be an integer > 0, got {args.pkg_release!r}")
    return f"{args.version}-r{int(args.pkg_release)}"


def check_bin(item) -> tuple:
    triple, sep, path = item.partition("=")
    if not sep or triple not in ARCHES:
        raise UsageError(f"--bin needs TRIPLE=PATH with a known triple, got {item!r}")
    try:
        machine = targets.elf_machine(path)
    except (ValueError, OSError) as e:
        raise UsageError(str(e))
    if machine != ARCHES[triple].machine:
        raise UsageError(f"{path} is a {machine} binary, not {triple}")
    return triple, path


def check_bins(args) -> dict:
    bins = dict(check_bin(item) for item in args.bin)
    if not bins:
        raise UsageError("give --bin for at least one target")
    return bins


def check_out(args) -> Path:
    out = Path(args.out)
    if out.exists() and (not out.is_dir() or any(out.iterdir())):
        raise UsageError(f"--out {out} exists and is not empty")
    return out


def build_arch(triple, binary, out, version) -> Path:
    arch = ARCHES[triple]
    with tempfile.TemporaryDirectory() as t:
        d = out / arch.dir
        ipk.build_ipk(stage_tree(Path(t) / "s", binary), arch.architecture, version, d, SCRIPTS)
        ipk.write_index(d)
    return next(d.glob("*.ipk"))


def write_root(out, dirs, base_url) -> None:
    (out / "install.sh").write_text(render_install(base_url))
    (out / "install.sh").chmod(0o755)
    (out / ".nojekyll").write_text("")
    (out / "README.md").write_text(render_readme(base_url, dirs))
    base = base_url.rstrip("/")
    (out / "index.html").write_text(
        "<!doctype html><meta charset=utf-8><title>Mesh Town Keenetic feed</title>\n"
        f"<h1>Mesh Town Keenetic feed</h1>\n<pre>wget -O- {base}/install.sh | sh</pre>\n"
        f"<p>Entware architectures: {', '.join(dirs)}</p>\n")


def stamp(out: Path) -> None:
    t = ipk.mtime()
    for p in [out, *out.rglob("*")]:
        os.utime(p, (t, t))


def run(args) -> int:
    version = check_release(args)
    bins = check_bins(args)
    out = check_out(args)
    out.mkdir(parents=True, exist_ok=True)
    made = [build_arch(triple, path, out, version) for triple, path in bins.items()]
    write_root(out, sorted(ARCHES[t].dir for t in bins), args.base_url)
    stamp(out)
    for path in made:
        print(f"ipk {path} {path.stat().st_size}")
    return 0


def main(argv=None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        return run(args)
    except UsageError as e:
        print(f"build_feed: {e}", file=sys.stderr)
        return 2
    except OSError as e:
        print(f"build_feed: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
