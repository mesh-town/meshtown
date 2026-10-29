"""Helpers shared by the feed builder tests (Python standard library only).

fake_elf         a tiny ELF file of a given machine, with or without PT_INTERP
tools_or_skip    the usign and apk executables, or SkipTest (AssertionError when
                 MESHTOWN_FEED_REQUIRE_TOOLS=1: CI must never skip silently)
root_or_skip     the same rule for "running as root or under fakeroot"
make_keys        a fresh usign pair and a P-256 apk pair, public halves in one directory
run_cli          runs build_feed.py as a subprocess: (exit code, stdout, stderr)
Unpacked         an .ipk opened down to its data and control archives
"""

import gzip
import hashlib
import io
import os
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from collections import namedtuple
from pathlib import Path

FEED_DIR = Path(__file__).resolve().parent
OPENWRT_DIR = FEED_DIR.parent  # platforms/linux/build/openwrt: init.d-meshtown
BUILD_FEED = FEED_DIR / "build_feed.py"

AARCH64 = "aarch64-unknown-linux-musl"
X86_64 = "x86_64-unknown-linux-musl"
ARMV7 = "armv7-unknown-linux-musleabi"
MIPS = "mips-unknown-linux-musl"
MIPSEL = "mipsel-unknown-linux-musl"
TRIPLE_MACHINES = {AARCH64: "aarch64", X86_64: "x86_64", ARMV7: "arm", MIPS: "mips", MIPSEL: "mipsel"}
# The MIPS32 release 2 labels and the Rust target each one is built from.
MIPS_LABELS = {
    "mips_24kc": MIPS,
    "mipsel_24kc": MIPSEL,
    "mipsel_74kc": MIPSEL,
    "mipsel_24kc_24kf": MIPSEL,
}
# All 18 labels and the Rust target each one is built from.
EXPECTED_LABELS = {
    "aarch64_cortex-a53": AARCH64,
    "aarch64_cortex-a72": AARCH64,
    "aarch64_cortex-a76": AARCH64,
    "aarch64_generic": AARCH64,
    "x86_64": X86_64,
    "arm_cortex-a5_vfpv4": ARMV7,
    "arm_cortex-a7": ARMV7,
    "arm_cortex-a7_neon-vfpv4": ARMV7,
    "arm_cortex-a7_vfpv4": ARMV7,
    "arm_cortex-a8_vfpv3": ARMV7,
    "arm_cortex-a9": ARMV7,
    "arm_cortex-a9_neon": ARMV7,
    "arm_cortex-a9_vfpv3-d16": ARMV7,
    "arm_cortex-a15_neon-vfpv4": ARMV7,
    **MIPS_LABELS,
}
# Files of the package (path without the leading slash) and their modes.
PACKAGE_FILES = {
    "usr/bin/meshtown": 0o755,
    "etc/init.d/meshtown": 0o755,
    "lib/upgrade/keep.d/meshtown": 0o644,
}
# The environment the CLI tests control themselves.
CLEARED_ENV = ("SOURCE_DATE_EPOCH", "MESHTOWN_USIGN", "MESHTOWN_APK")

_ELF_MACHINE = {"x86_64": 62, "aarch64": 183, "arm": 40, "mipsel": 8, "mips": 8}
_PT_LOAD, _PT_INTERP = 1, 3


def fake_elf(path, machine, static=True, pad=0):
    """Write a small but well-formed ELF file; `pad` extra zero bytes make it bigger."""
    is64 = machine in ("aarch64", "x86_64")
    endian = ">" if machine == "mips" else "<"
    ehsize, phentsize = (64, 56) if is64 else (52, 32)
    interp = b"" if static else b"/lib/ld-musl-fake.so.1\0"
    phnum = 1 if static else 2
    body_off = ehsize + phentsize * phnum
    body = interp + b"fake-meshtown-binary" + bytes(pad)
    phdrs = b""
    if not static:
        phdrs += _phdr(is64, endian, _PT_INTERP, body_off, len(interp))
    phdrs += _phdr(is64, endian, _PT_LOAD, 0, body_off + len(body))
    ident = b"\x7fELF" + bytes([2 if is64 else 1, 2 if endian == ">" else 1, 1, 0]) + bytes(8)
    flags = 0 if is64 else 0x05000200
    em = _ELF_MACHINE[machine]
    if is64:
        head = struct.pack(endian + "HHIQQQIHHHHHH", 2, em, 1, 0x400000, ehsize, 0, flags, ehsize, phentsize, phnum, 64, 0, 0)
    else:
        head = struct.pack(endian + "HHIIIIIHHHHHH", 2, em, 1, 0x10000, ehsize, 0, flags, ehsize, phentsize, phnum, 40, 0, 0)
    path = Path(path)
    path.write_bytes(ident + head + phdrs + body)
    return path


def _phdr(is64, endian, ptype, offset, size):
    if is64:
        return struct.pack(endian + "IIQQQQQQ", ptype, 5, offset, 0x400000 + offset, 0x400000 + offset, size, size, 0x1000)
    return struct.pack(endian + "IIIIIIII", ptype, offset, 0x10000 + offset, 0x10000 + offset, size, size, 5, 0x1000)


def tmpdir(test):
    """A fresh directory removed when the test ends."""
    path = Path(tempfile.mkdtemp(prefix="feedtest-"))
    test.addCleanup(shutil.rmtree, path, ignore_errors=True)
    return path


def stub_tool(path):
    """An executable that exists and does nothing: stands in for a tool the test never needs."""
    path = Path(path)
    path.write_text("#!/bin/sh\nexit 0\n")
    path.chmod(0o755)
    return path


def listing(directory):
    return sorted(p.name for p in Path(directory).iterdir())


def sha256_of(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


# ---------- tools ----------

def _find_tool(name, var):
    for cand in (os.environ.get(var), shutil.which(name)):
        if cand and os.path.isfile(cand) and os.access(cand, os.X_OK):
            return str(cand)
    return None


def _unavailable(what):
    if os.environ.get("MESHTOWN_FEED_REQUIRE_TOOLS") == "1":
        raise AssertionError(f"{what} (MESHTOWN_FEED_REQUIRE_TOOLS=1 forbids skipping)")
    raise unittest.SkipTest(what)


def usign_or_skip():
    return _find_tool("usign", "MESHTOWN_USIGN") or _unavailable("usign is not available")


def apk_or_skip():
    return _find_tool("apk", "MESHTOWN_APK") or _unavailable("apk is not available")


def openssl_or_skip():
    return shutil.which("openssl") or _unavailable("openssl is not available")


def tools_or_skip():
    """(usign, apk) executables."""
    return usign_or_skip(), apk_or_skip()


def root_or_skip():
    if os.geteuid() != 0:
        _unavailable("needs root or fakeroot")


# ---------- keys ----------

def make_keys(tmp):
    """(usign_sec, apk_sec, pubdir); pubdir holds meshtown-usign.pub and meshtown-apk.pem."""
    usign, openssl = usign_or_skip(), openssl_or_skip()
    tmp = Path(tmp)
    pubdir = tmp / "pub"
    pubdir.mkdir(parents=True)
    usign_sec, apk_sec = tmp / "usign.sec", tmp / "apk.sec.pem"
    _run([usign, "-G", "-c", "Test feed key", "-s", usign_sec, "-p", pubdir / "meshtown-usign.pub"])
    _run([openssl, "ecparam", "-name", "prime256v1", "-genkey", "-noout", "-out", apk_sec])
    _run([openssl, "ec", "-in", apk_sec, "-pubout", "-out", pubdir / "meshtown-apk.pem"])
    return usign_sec, apk_sec, pubdir


def dummy_keys(tmp):
    """Files with the names of real keys but no key in them: for calls that must be refused early."""
    tmp = Path(tmp)
    pubdir = tmp / "pub"
    pubdir.mkdir(parents=True)
    paths = (tmp / "usign.sec", tmp / "apk.sec.pem", pubdir / "meshtown-usign.pub", pubdir / "meshtown-apk.pem")
    for p in paths:
        p.write_text("not a key\n")
    return paths[0], paths[1], pubdir


def bare_apk(apk_bin, out):
    """The smallest package `apk mkpkg` accepts (metadata only): out/meshtown-0.2.7-r1.apk."""
    package = Path(out) / "meshtown-0.2.7-r1.apk"
    _run([apk_bin, "mkpkg", "--info", "name:meshtown", "--info", "version:0.2.7-r1",
          "--info", "arch:x86_64", "--output", package])
    return package


def _run(cmd):
    return subprocess.run([str(c) for c in cmd], check=True, capture_output=True, text=True)


# ---------- CLI ----------

def run_cli(*args, env=None):
    """Run build_feed.py; env maps names to values (None removes a name)."""
    environ = {k: v for k, v in os.environ.items() if k not in CLEARED_ENV}
    for name, value in (env or {}).items():
        if value is None:
            environ.pop(name, None)
        else:
            environ[name] = str(value)
    cmd = [sys.executable, str(BUILD_FEED), *map(str, args)]
    done = subprocess.run(cmd, capture_output=True, text=True, env=environ, stdin=subprocess.DEVNULL, timeout=600)
    return done.returncode, done.stdout, done.stderr


# ---------- reading packages ----------

def tar_entries(data):
    """Members of a tar (plain or gzip) in archive order: [(name, TarInfo, bytes or None)]."""
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:*") as t:
        return [(m.name, m, t.extractfile(m).read() if m.isfile() else None) for m in t.getmembers()]


def plain_name(name):
    """Member name without the leading './' and the trailing '/'."""
    name = name[2:] if name.startswith("./") else name
    return name.rstrip("/")


def files_by_name(entries):
    return {plain_name(n): (info, data) for n, info, data in entries if info.isfile()}


def control_fields(text):
    """Fields of a control file, in order: {name: value}."""
    fields = {}
    for line in text.splitlines():
        if ": " in line and not line.startswith((" ", "\t")):
            name, value = line.split(": ", 1)
            fields[name] = value
    return fields


Built = namedtuple("Built", "path binary")


class Unpacked:
    """An .ipk opened down to its data and control archives."""

    def __init__(self, ipk_path):
        self.outer = tar_entries(Path(ipk_path).read_bytes())
        parts = {name: data for name, _, data in self.outer}
        self.data = tar_entries(parts["./data.tar.gz"])
        self.control = tar_entries(parts["./control.tar.gz"])

    def data_files(self):
        return files_by_name(self.data)

    def control_files(self):
        return files_by_name(self.control)

    def control_text(self):
        return self.control_files()["control"][1].decode()


def build_test_ipk(tmp, label="aarch64_generic", version="0.2.7-r1", machine="aarch64", pad=0, out_dir=None,
                   touch_stage=False):
    """A real .ipk from build_feed.stage_tree and ipk.build_ipk: Built(path, binary).

    tmp must be new for every call (it holds the binary and the stage tree); touch_stage moves
    the mtime of every stage file a year ahead (an archive must not depend on it)."""
    import build_feed
    import ipk

    tmp = Path(tmp)
    tmp.mkdir(parents=True, exist_ok=True)
    binary = fake_elf(tmp / "meshtown-bin", machine, pad=pad)
    stage = tmp / "stage"
    build_feed.stage_tree(stage, binary, "ipk")
    if touch_stage:
        later = os.stat(binary).st_mtime + 365 * 86400
        for path in stage.rglob("*"):
            os.utime(path, (later, later))
    out = Path(out_dir) if out_dir else tmp / "out"
    out.mkdir(parents=True, exist_ok=True)
    return Built(Path(ipk.build_ipk(stage, label, version, out)), binary)


def gunzip_file(path):
    return gzip.decompress(Path(path).read_bytes())
