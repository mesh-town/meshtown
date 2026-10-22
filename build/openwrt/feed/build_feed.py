#!/usr/bin/env python3
"""Builds the Mesh Town OpenWrt package feed: .ipk (opkg) and .apk (apk) per architecture label, signed indexes, install.sh.

build_feed.py --out DIR --version X.Y.Z [--pkg-release N] --base-url URL --bin TRIPLE=PATH [...]
              [--labels L1,L2] [--only ipk|apk]
              (--usign-key FILE --apk-key FILE --pubkeys DIR | --ephemeral-keys) [--usign PATH] [--apk PATH]
Exit codes: 0 feed built and verified, 2 bad invocation, 1 tool failure / key mismatch / not root for apk.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import apk
import ipk
import keys
import targets
from readme import render_readme

HERE = Path(__file__).resolve().parent
INIT_D = HERE.parent / "init.d-meshtown"


class UsageError(Exception):
    pass


def stage_tree(dst, binary, fmt) -> Path:
    """Package file tree (decision 8); the apk flavour also gets its file list."""
    dst = Path(dst)
    files = [("usr/bin/meshtown", Path(binary).read_bytes(), 0o755),
             ("etc/init.d/meshtown", INIT_D.read_bytes(), 0o755),
             ("lib/upgrade/keep.d/meshtown", b"/etc/meshtown/\n", 0o644)]
    for rel, data, mode in files:
        p = dst / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data)
        p.chmod(mode)
        os.utime(p, (ipk.mtime(), ipk.mtime()))
    if fmt == "apk":
        apk.write_list(dst)
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
    p.add_argument("--labels")
    p.add_argument("--only", choices=["ipk", "apk"])
    p.add_argument("--usign-key")
    p.add_argument("--apk-key")
    p.add_argument("--pubkeys")
    p.add_argument("--ephemeral-keys", action="store_true")
    p.add_argument("--usign")
    p.add_argument("--apk")
    return p.parse_args(argv)


def check_release(args) -> str:
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        raise UsageError(f"version must be X.Y.Z, got {args.version!r}")
    if not re.fullmatch(r"[0-9]+", args.pkg_release) or int(args.pkg_release) < 1:
        raise UsageError(f"--pkg-release must be an integer > 0, got {args.pkg_release!r}")
    return f"{args.version}-r{int(args.pkg_release)}"


def check_bins(args) -> dict:
    bins = {}
    for item in args.bin:
        triple, sep, path = item.partition("=")
        if not sep or triple not in targets.TRIPLE_MACHINE:
            raise UsageError(f"--bin needs TRIPLE=PATH with a known triple, got {item!r}")
        try:
            machine = targets.elf_machine(path)
        except (ValueError, OSError) as e:
            raise UsageError(str(e))
        if machine != targets.TRIPLE_MACHINE[triple]:
            raise UsageError(f"{path} is a {machine} binary, not {triple}")
        bins[triple] = path
    return bins


def choose_labels(args, bins) -> list:
    wanted = args.labels.split(",") if args.labels else list(targets.LABELS)
    unknown = [x for x in wanted if x not in targets.LABELS]
    if unknown:
        raise UsageError(f"unknown labels: {', '.join(unknown)}")
    labels = [x for x in wanted if targets.LABELS[x] in bins]
    if not labels:
        raise UsageError("no label has a binary: give --bin for at least one target")
    return labels


def find_tool(flag, env, name):
    found = flag or os.environ.get(env) or shutil.which(name)
    if not found:
        raise UsageError(f"{name} not found: use --{name}, {env} or PATH")
    return found


def check_out(args) -> Path:
    out = Path(args.out)
    if out.exists() and (not out.is_dir() or any(out.iterdir())):
        raise UsageError(f"--out {out} exists and is not empty")
    return out


def resolve_keys(args, usign_bin, tmp, need_apk):
    if args.ephemeral_keys:
        return keys.ephemeral_keys(tmp, usign_bin)
    if not (args.usign_key and args.apk_key and args.pubkeys):
        raise UsageError("give --usign-key, --apk-key and --pubkeys, or --ephemeral-keys")
    return Path(args.usign_key), Path(args.apk_key), Path(args.pubkeys)


def check_key_pair(usign_sec, apk_sec, pubdir, usign_bin, need_apk) -> None:
    if not keys.usign_key_matches(usign_sec, pubdir / keys.USIGN_PUB, usign_bin):
        raise RuntimeError("usign secret key does not match the public key in --pubkeys")
    if need_apk and not keys.apk_key_matches(apk_sec, pubdir / keys.APK_PEM):
        raise RuntimeError("apk secret key does not match the public key in --pubkeys")


def build_label(label, ctx) -> list:
    made = []
    triple_bin = ctx["bins"][targets.LABELS[label]]
    if ctx["only"] != "apk":
        with tempfile.TemporaryDirectory() as t:
            d = ctx["out"] / "opkg" / label
            ipk.build_ipk(stage_tree(Path(t) / "s", triple_bin, "ipk"), label, ctx["ver"], d)
            index = ipk.write_index(d)
            keys.sign_usign(index, ctx["usign_sec"], ctx["usign_bin"])
            made.append(("ipk", label, next(d.glob("*.ipk"))))
    if ctx["only"] != "ipk":
        with tempfile.TemporaryDirectory() as t:
            d = ctx["out"] / "apk" / label
            apk.build_apk(stage_tree(Path(t) / "s", triple_bin, "apk"), label, ctx["ver"], d, ctx["apk_bin"])
            apk.write_index(d, ctx["apk_bin"], ctx["apk_sec"])
            made.append(("apk", label, next(d.glob("*.apk"))))
    return made


def verify_feed(ctx, labels) -> None:
    pub = ctx["pubdir"]
    for label in labels:
        if ctx["only"] != "apk":
            d = ctx["out"] / "opkg" / label
            if not keys.verify_usign(d / "Packages", pub / keys.USIGN_PUB, ctx["usign_bin"]):
                raise RuntimeError(f"opkg index of {label} does not verify")
        if ctx["only"] != "ipk":
            if not keys.verify_apk_index(ctx["out"] / "apk" / label / "packages.adb", pub, ctx["apk_bin"]):
                raise RuntimeError(f"apk index of {label} does not verify")


def write_root(ctx, labels, base_url) -> None:
    out = ctx["out"]
    (out / "install.sh").write_text(render_install(base_url))
    (out / "install.sh").chmod(0o755)
    shutil.copy(ctx["pubdir"] / keys.USIGN_PUB, out / keys.USIGN_PUB)
    shutil.copy(ctx["pubdir"] / keys.APK_PEM, out / keys.APK_PEM)
    (out / ".nojekyll").write_text("")
    (out / "README.md").write_text(render_readme(base_url, labels))
    base = base_url.rstrip("/")
    (out / "index.html").write_text(
        "<!doctype html><meta charset=utf-8><title>Mesh Town OpenWrt feed</title>\n"
        f"<h1>Mesh Town OpenWrt feed</h1>\n<pre>wget -O- {base}/install.sh | sh</pre>\n"
        f"<p>Architectures: {', '.join(labels)}</p>\n")


def stamp(out: Path) -> None:
    t = ipk.mtime()
    for p in [out, *out.rglob("*")]:
        os.utime(p, (t, t))


def run(args, tmp) -> int:
    ver = check_release(args)
    bins = check_bins(args)
    labels = choose_labels(args, bins)
    out = check_out(args)
    need_apk = args.only != "ipk"
    usign_bin = find_tool(args.usign, "MESHTOWN_USIGN", "usign")
    apk_bin = find_tool(args.apk, "MESHTOWN_APK", "apk") if need_apk else None
    usign_sec, apk_sec, pubdir = resolve_keys(args, usign_bin, tmp, need_apk)
    if need_apk:
        apk.require_root(os.geteuid())
    if not args.ephemeral_keys:
        check_key_pair(usign_sec, apk_sec, Path(pubdir), usign_bin, need_apk)
    out.mkdir(parents=True, exist_ok=True)
    ctx = {"out": out, "bins": bins, "only": args.only, "ver": ver, "usign_bin": usign_bin, "apk_bin": apk_bin,
           "usign_sec": usign_sec, "apk_sec": apk_sec, "pubdir": Path(pubdir)}
    made = [m for label in labels for m in build_label(label, ctx)]
    verify_feed(ctx, labels)
    write_root(ctx, labels, args.base_url)
    stamp(out)
    for fmt, label, path in made:
        print(f"{fmt} {label} {path} {path.stat().st_size}")
    return 0


def main(argv=None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    with tempfile.TemporaryDirectory() as tmp:
        try:
            return run(args, Path(tmp))
        except UsageError as e:
            print(f"build_feed: {e}", file=sys.stderr)
            return 2
        except (PermissionError, RuntimeError, subprocess.CalledProcessError, OSError) as e:
            print(f"build_feed: {e}", file=sys.stderr)
            return 1


if __name__ == "__main__":
    sys.exit(main())
