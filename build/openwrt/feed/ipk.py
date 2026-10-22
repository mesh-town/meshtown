"""Builds .ipk packages (gzip'd tar of debian-binary, data and control archives) and the opkg Packages index."""
import gzip
import hashlib
import io
import os
import tarfile
from pathlib import Path

NAME = "meshtown"
DESCRIPTION = "Mesh Town node: private mesh network over iroh (static binary)"
MAINTAINER = "Mesh Town <meshtown.app@gmail.com>"
LICENSE = "AGPL-3.0-only"
URL = "https://github.com/mesh-town/meshtown"
DEFAULT_MTIME = 1760000000
SCRIPTS = Path(__file__).resolve().parent / "scripts"


def mtime() -> int:
    """Fixed timestamp for every file in the feed: SOURCE_DATE_EPOCH, else a constant."""
    return int(os.environ.get("SOURCE_DATE_EPOCH", DEFAULT_MTIME))


def _tar_gz(entries) -> bytes:
    """entries: (name, bytes or None for a directory, mode); GNU format, root owner, sorted."""
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


def _data_entries(stage: Path):
    entries = [(".", None, 0o755)]
    for p in sorted(stage.rglob("*")):
        rel = "./" + str(p.relative_to(stage))
        if p.is_dir():
            entries.append((rel, None, 0o755))
        else:
            entries.append((rel, p.read_bytes(), p.stat().st_mode & 0o7777))
    return entries


def _control(label: str, version: str, installed_size: int) -> str:
    return (f"Package: {NAME}\nVersion: {version}\nSection: net\nArchitecture: {label}\n"
            f"Maintainer: {MAINTAINER}\nLicense: {LICENSE}\nURL: {URL}\n"
            f"Installed-Size: {installed_size}\nDescription: {DESCRIPTION}\n")


def build_ipk(stage: Path, label: str, version: str, out_dir: Path, scripts: Path = SCRIPTS) -> Path:
    """scripts: directory with the postinst and prerm control scripts."""
    entries = _data_entries(Path(stage))
    size = sum(len(e[1]) for e in entries if e[1])
    control = [("./", None, 0o755), ("./control", _control(label, version, size).encode(), 0o644)]
    for name in ("postinst", "prerm"):
        control.append(("./" + name, (Path(scripts) / name).read_bytes(), 0o755))
    members = [("./debian-binary", b"2.0\n"), ("./data.tar.gz", _tar_gz(entries)),
               ("./control.tar.gz", _tar_gz(control))]
    bio = io.BytesIO()
    with tarfile.open(fileobj=bio, mode="w", format=tarfile.GNU_FORMAT) as t:
        for name, data in members:
            ti = tarfile.TarInfo(name)
            ti.size, ti.mtime, ti.mode = len(data), mtime(), 0o644
            ti.uid = ti.gid = 0
            ti.uname = ti.gname = "root"
            t.addfile(ti, io.BytesIO(data))
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    f = out_dir / f"{NAME}_{version}_{label}.ipk"
    with f.open("wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", mtime=0) as g:
        g.write(bio.getvalue())
    return f


def _control_of(raw: bytes) -> str:
    outer = tarfile.open(fileobj=io.BytesIO(gzip.decompress(raw)))
    inner = gzip.decompress(outer.extractfile("./control.tar.gz").read())
    return tarfile.open(fileobj=io.BytesIO(inner)).extractfile("./control").read().decode()


def write_index(out_dir: Path) -> Path:
    """Packages and Packages.gz for every .ipk in out_dir (Filename/Size/SHA256sum before Description)."""
    out_dir = Path(out_dir)
    blocks = []
    for ipk in sorted(out_dir.glob("*.ipk")):
        raw = ipk.read_bytes()
        extra = f"Filename: {ipk.name}\nSize: {len(raw)}\nSHA256sum: {hashlib.sha256(raw).hexdigest()}\n"
        ctrl = _control_of(raw)
        at = ctrl.index("Description:")
        blocks.append(ctrl[:at] + extra + ctrl[at:])
    index = "\n".join(blocks).encode()
    if (64 + len(index)) % 128 in (110, 111):  # usign bug workaround, as in the OpenWrt SDK
        index += b"\n\n"
    (out_dir / "Packages").write_bytes(index)
    with (out_dir / "Packages.gz").open("wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", mtime=0, compresslevel=9) as g:
        g.write(index)
    return out_dir / "Packages"
