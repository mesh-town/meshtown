"""Helpers shared by the tests of deb.py: constants, a reader of built packages, a fake root for the maintainer scripts."""

import gzip
import io
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path
from unittest import mock

import deb
from test_support import fake_binary


HERE = Path(__file__).resolve().parent
DEB_PY = HERE / "deb.py"
SCRIPTS = HERE / "scripts"
VERSION = "0.3.47"
LICENSE_TEXT = b"Test license text\n"
KEYRING_BYTES = b"\x99\x01\x0d not a real key, only bytes the package must carry verbatim\n"
# Files of the package (path without the leading slash) and their modes.
FILES = {
    "usr/bin/meshtown": 0o755,
    "usr/lib/systemd/system/meshtown.service": 0o644,
    "etc/apt/sources.list.d/meshtown.sources": 0o644,
    "usr/share/keyrings/meshtown.gpg": 0o644,
    "usr/share/doc/meshtown/copyright": 0o644,
}
SCRIPT_NAMES = ("postinst", "prerm", "postrm")
SYSTEMD_RUNNING = "run/systemd/system"
MANUAL_UNIT = "etc/systemd/system/meshtown.service"


class Member:
    def __init__(self, header, body, size):
        self.header, self.body, self.size = header, body, size
        self.name = header[:16].decode().strip()


class Opened:
    """A built .deb opened down to the ar members and both inner tar archives."""

    def __init__(self, path: Path):
        self.path = path
        self.raw = path.read_bytes()
        self.members = self.parse(self.raw)
        self.control_entries, self.control_bodies, self.control_raw_names = untar(self.member("control.tar.gz").body)
        self.data_entries, self.data_bodies, self.data_raw_names = untar(self.member("data.tar.gz").body)

    @staticmethod
    def parse(raw: bytes):
        assert raw[:8] == b"!<arch>\n", "ar magic"
        members, at = [], 8
        while at < len(raw):
            header = raw[at:at + 60]
            assert len(header) == 60 and header[58:60] == b"`\n", f"bad ar header at {at}"
            size = int(header[48:58])
            body = raw[at + 60:at + 60 + size]
            assert len(body) == size, "truncated member"
            at += 60 + size
            if size % 2:
                assert raw[at:at + 1] == b"\n", "odd-sized member is not padded with a newline"
                at += 1
            members.append(Member(header, body, size))
        return members

    def member(self, name):
        return next(m for m in self.members if m.name == name)

    def control_file(self, name):
        return self.control_bodies[f"./{name}"]

    def control_text(self):
        return self.control_file("control").decode()

    def data_file(self, path):
        return self.data_bodies[f"./{path}"]

    def data_file_modes(self):
        return {e.name[2:]: e.mode for e in self.data_entries if e.isfile()}

    def data_dir_modes(self):
        return {"." if e.name == "." else e.name[2:]: e.mode for e in self.data_entries if e.isdir()}


def untar(gz: bytes):
    """(entries, {name: bytes}, raw names exactly as written in the headers)."""
    tar = gzip.decompress(gz)
    raw_names, at = [], 0
    with tarfile.open(fileobj=io.BytesIO(tar), mode="r:") as t:
        entries = t.getmembers()
        bodies = {}
        for e in entries:
            raw_names.append(tar[e.offset:e.offset + 100].split(b"\0", 1)[0].decode())
            if e.isfile():
                bodies[e.name] = t.extractfile(e).read()
    return entries, bodies, raw_names


def tmpdir(test) -> Path:
    path = Path(tempfile.mkdtemp(prefix="debtest-"))
    test.addCleanup(shutil.rmtree, path, ignore_errors=True)
    return path


def prepared(test):
    """Inputs of a build in a fresh directory: binary, keyring, license, empty out directory."""
    class Inputs:
        pass
    inputs = Inputs()
    inputs.tmp = tmpdir(test)
    inputs.binary = fake_binary(inputs.tmp / "bin")
    inputs.keyring = inputs.tmp / "meshtown.gpg"
    inputs.keyring.write_bytes(KEYRING_BYTES)
    inputs.license = inputs.tmp / "LICENSE"
    inputs.license.write_bytes(LICENSE_TEXT)
    inputs.out = inputs.tmp / "out"
    inputs.out.mkdir()
    return inputs


def setup(test, arch="amd64", version=VERSION, with_license=True, binary_bytes=None) -> Opened:
    """A built and opened package; every call gets a directory of its own."""
    inputs = prepared(test)
    if binary_bytes is not None:
        inputs.binary.write_bytes(binary_bytes)
    path = deb.build_deb(inputs.binary, version, arch, inputs.out, keyring=inputs.keyring,
                         license_file=inputs.license if with_license else None)
    return Opened(path)


def source_date_epoch(value):
    """Context manager: SOURCE_DATE_EPOCH set to value, or removed when value is None."""
    env = dict(os.environ)
    env.pop("SOURCE_DATE_EPOCH", None)
    if value is not None:
        env["SOURCE_DATE_EPOCH"] = value
    return mock.patch.dict(os.environ, env, clear=True)


def control_fields(text):
    """Debian control paragraph to {field: value}; continuation lines are joined with newlines."""
    fields, last = {}, None
    for line in text.splitlines():
        if line.startswith(" ") and last:
            fields[last] += "\n" + line[1:]
            continue
        key, _, value = line.partition(": ")
        fields[key] = value
        last = key
    return fields


def deb822(text):
    return {k: v for k, _, v in (line.partition(": ") for line in text.splitlines() if ": " in line)}


def ar_member(name, body):
    header = f"{name:<16}{0:<12}{0:<6}{0:<6}{'100644':<8}{len(body):<10}`\n".encode()
    return header + body + (b"\n" if len(body) % 2 else b"")


def run_cli(inputs, *args):
    cmd = [sys.executable, str(DEB_PY), "--binary", str(inputs.binary), "--out", str(inputs.out),
           "--keyring", str(inputs.keyring), *args]
    return subprocess.run(cmd, capture_output=True, text=True, timeout=60)


def dpkg_deb(*args):
    return subprocess.run(["dpkg-deb", *map(str, args)], capture_output=True, text=True, timeout=60)


class Result:
    def __init__(self, done):
        self.code, self.out, self.err = done.returncode, done.stdout, done.stderr


class Sandbox:
    """A fake root for the maintainer scripts: DPKG_ROOT plus a `systemctl` that only writes a journal."""

    def __init__(self, test, systemd, manual_unit, fail_verb):
        self.base = tmpdir(test)
        self.root = self.base / "root"
        self.root.mkdir()
        self.log = self.base / "systemctl.log"
        self.log.write_text("")
        self.fail_verb = fail_verb or ""
        bin_dir = self.base / "bin"
        bin_dir.mkdir()
        stub = bin_dir / "systemctl"
        stub.write_text('#!/bin/sh\necho "$@" >> "$SYSTEMCTL_LOG"\n'
                        'for a in "$@"; do [ "$a" = "$FAIL_VERB" ] && exit 1; done\nexit 0\n')
        stub.chmod(0o755)
        self.bin_dir = bin_dir
        if systemd:
            (self.root / SYSTEMD_RUNNING).mkdir(parents=True)
        if manual_unit:
            (self.root / MANUAL_UNIT).parent.mkdir(parents=True)
            (self.root / MANUAL_UNIT).write_text("[Service]\nExecStart=/bin/true\n")

    def run(self, script, *args):
        env = {"PATH": f"{self.bin_dir}:/usr/bin:/bin", "DPKG_ROOT": str(self.root),
               "SYSTEMCTL_LOG": str(self.log), "FAIL_VERB": self.fail_verb}
        done = subprocess.run([shutil.which("sh"), str(SCRIPTS / script), *args], env=env,
                              capture_output=True, text=True, timeout=30)
        return Result(done)

    def journal(self):
        return [line for line in self.log.read_text().splitlines() if line.strip()]

    def verbs(self):
        return [next(w for w in line.split() if not w.startswith("-")) for line in self.journal()]


def sandbox(test, systemd=True, manual_unit=False, fail_verb=None) -> Sandbox:
    return Sandbox(test, systemd, manual_unit, fail_verb)
