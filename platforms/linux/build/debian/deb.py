#!/usr/bin/env python3
"""Builds the meshtown .deb (ar of debian-binary, control.tar.gz, data.tar.gz) without dpkg-deb."""
import argparse
import gzip
import hashlib
import io
import os
import re
import sys
import tarfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
NAME = "meshtown"
ARCHES = {"amd64", "arm64"}
KEYRING = HERE / "keys" / "meshtown.gpg"
MAINTAINER = "Mesh Town <340663027+mesh-town@users.noreply.github.com>"
URL = "https://github.com/mesh-town/meshtown-dist"
SHORT_DESCRIPTION = "private mesh network node"
LONG_DESCRIPTION = " Mesh Town connects your devices into one private network."
DEFAULT_MTIME = 1760000000
AR_MAGIC = b"!<arch>\n"


def mtime() -> int:
    """Fixed timestamp for every file: SOURCE_DATE_EPOCH, else a constant."""
    return int(os.environ.get("SOURCE_DATE_EPOCH", DEFAULT_MTIME))


def _tar_gz(entries) -> bytes:
    """entries: (name, bytes or None for a directory, mode); GNU format, root owner, sorted by name."""
    bio = io.BytesIO()
    with tarfile.open(fileobj=bio, mode="w", format=tarfile.GNU_FORMAT) as t:
        for name, data, mode in sorted(entries, key=lambda e: e[0]):
            ti = tarfile.TarInfo(name)
            ti.uid = ti.gid = 0
            ti.uname = ti.gname = "root"
            ti.mtime = mtime()
            ti.mode = mode
            ti.size = 0 if data is None else len(data)
            ti.type = tarfile.DIRTYPE if data is None else tarfile.REGTYPE
            t.addfile(ti, None if data is None else io.BytesIO(data))
    out = io.BytesIO()
    with gzip.GzipFile(fileobj=out, mode="wb", mtime=0) as g:
        g.write(bio.getvalue())
    return out.getvalue()


def _ar_member(name: str, data: bytes) -> bytes:
    header = f"{name:<16}{mtime():<12}{0:<6}{0:<6}{'100644':<8}{len(data):<10}`\n"
    body = header.encode("ascii") + data
    return body + (b"\n" if len(data) % 2 else b"")


def _data_files(binary: Path, arch: str, keyring: Path, license_file) -> dict:
    """Package path (no './') -> (bytes, mode)."""
    sources = (HERE / "meshtown.sources.in").read_text().replace("@ARCH@", arch)
    files = {
        "usr/bin/meshtown": (binary.read_bytes(), 0o755),
        "usr/lib/systemd/system/meshtown.service": ((HERE / "meshtown.service").read_bytes(), 0o644),
        "etc/apt/sources.list.d/meshtown.sources": (sources.encode(), 0o644),
        "usr/share/keyrings/meshtown.gpg": (keyring.read_bytes(), 0o644),
    }
    if license_file is not None:
        files["usr/share/doc/meshtown/copyright"] = (Path(license_file).read_bytes(), 0o644)
    return files


def _data_entries(files: dict):
    dirs = {"."}
    for path in files:
        parts = path.split("/")[:-1]
        dirs.update("./" + "/".join(parts[:i + 1]) for i in range(len(parts)))
    entries = [("./" if d == "." else d + "/", None, 0o755) for d in dirs]
    entries += [("./" + path, data, mode) for path, (data, mode) in files.items()]
    return entries


def _control(version: str, arch: str, installed_size: int) -> str:
    return (f"Package: {NAME}\nVersion: {version}\nArchitecture: {arch}\nMaintainer: {MAINTAINER}\n"
            f"Installed-Size: {installed_size}\nSection: net\nPriority: optional\nHomepage: {URL}\n"
            f"Description: {SHORT_DESCRIPTION}\n{LONG_DESCRIPTION}\n")


def _control_entries(files: dict, version: str, arch: str):
    size = -(-sum(len(d) for d, _ in files.values()) // 1024)
    md5 = "".join(f"{hashlib.md5(d).hexdigest()}  {p}\n" for p, (d, _) in sorted(files.items()))
    entries = [("./", None, 0o755), ("./control", _control(version, arch, size).encode(), 0o644),
               ("./md5sums", md5.encode(), 0o644)]
    for name in ("postinst", "prerm", "postrm"):
        entries.append(("./" + name, (HERE / "scripts" / name).read_bytes(), 0o755))
    return entries


def _check(binary: Path, version: str, arch: str, keyring: Path) -> None:
    if arch not in ARCHES:
        raise ValueError(f"unsupported architecture: {arch}")
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError(f"version must be X.Y.Z: {version}")
    for label, path in (("binary", binary), ("keyring", keyring)):
        if not Path(path).is_file():
            raise ValueError(f"{label} not found: {path}")


def build_deb(binary: Path, version: str, arch: str, out_dir: Path, keyring: Path = KEYRING,
              license_file: Path | None = None) -> Path:
    """Writes out_dir/meshtown_<version>_<arch>.deb and returns its path."""
    binary, keyring = Path(binary), Path(keyring)
    _check(binary, version, arch, keyring)
    if license_file is not None and not Path(license_file).is_file():
        raise ValueError(f"license not found: {license_file}")
    files = _data_files(binary, arch, keyring, license_file)
    members = [("debian-binary", b"2.0\n"),
               ("control.tar.gz", _tar_gz(_control_entries(files, version, arch))),
               ("data.tar.gz", _tar_gz(_data_entries(files)))]
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    deb = out_dir / f"{NAME}_{version}_{arch}.deb"
    deb.write_bytes(AR_MAGIC + b"".join(_ar_member(n, d) for n, d in members))
    return deb


def _ar_members(raw: bytes):
    if not raw.startswith(AR_MAGIC):
        raise ValueError("not an ar archive")
    pos = len(AR_MAGIC)
    while pos + 60 <= len(raw):
        name = raw[pos:pos + 16].decode("ascii").strip().rstrip("/")
        size = int(raw[pos + 48:pos + 58])
        yield name, raw[pos + 60:pos + 60 + size]
        pos += 60 + size + size % 2


def read_control(deb: Path) -> str:
    """Text of the control file of a .deb."""
    try:
        members = dict(_ar_members(Path(deb).read_bytes()))
    except (OSError, UnicodeDecodeError, ValueError) as e:
        raise ValueError(f"not a deb package: {deb}") from e
    if "control.tar.gz" not in members:
        raise ValueError(f"no control.tar.gz in {deb}")
    tar = tarfile.open(fileobj=io.BytesIO(gzip.decompress(members["control.tar.gz"])))
    return tar.extractfile("./control").read().decode()


def main(argv=None) -> int:
    p = argparse.ArgumentParser(description="Build the meshtown .deb")
    p.add_argument("--binary", required=True, type=Path)
    p.add_argument("--arch", required=True, choices=sorted(ARCHES))
    p.add_argument("--version", required=True)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--keyring", type=Path, default=KEYRING)
    p.add_argument("--license", type=Path, default=None)
    a = p.parse_args(argv)
    try:
        print(build_deb(a.binary, a.version, a.arch, a.out, a.keyring, a.license))
    except ValueError as e:
        print(f"deb.py: {e}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
