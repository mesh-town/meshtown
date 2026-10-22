"""Builds .apk packages and the signed apk index with apk-tools 3.0.x (mkpkg / mkndx)."""
import subprocess
from pathlib import Path

import ipk
from keys import verify_apk_index  # noqa: F401  (re-exported: the apk checks live here too)

SCRIPTS = Path(__file__).resolve().parent / "scripts"
APK_SCRIPTS = ("post-install", "post-upgrade", "pre-upgrade", "pre-deinstall")


def require_root(euid: int) -> None:
    """mkpkg takes owner ids from the files on disk, so the build must run as uid 0."""
    if euid != 0:
        raise PermissionError("building .apk packages needs root ownership of the staged files: "
                              "run as root or under fakeroot")


def write_list(stage: Path) -> Path:
    """lib/apk/packages/meshtown.list - the file list apk-based OpenWrt keeps for each package."""
    meta = Path(stage) / "lib/apk/packages"
    target = meta / f"{ipk.NAME}.list"
    files = sorted("/" + str(p.relative_to(stage)) for p in Path(stage).rglob("*") if p.is_file() and p != target)
    meta.mkdir(parents=True, exist_ok=True)
    target.write_text("\n".join(files) + "\n")
    return target


def build_apk(stage: Path, label: str, version: str, out_dir: Path, apk_bin) -> Path:
    stage, out_dir = Path(stage).resolve(), Path(out_dir).resolve()
    write_list(stage)
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / f"{ipk.NAME}-{version}.apk"
    info = {"name": ipk.NAME, "version": version, "description": ipk.DESCRIPTION, "arch": label,
            "license": ipk.LICENSE, "origin": ipk.NAME, "url": ipk.URL, "maintainer": ipk.MAINTAINER,
            "depends": ""}
    cmd = [str(apk_bin), "mkpkg", "--xattrs=no"]
    for key, value in info.items():
        cmd += ["--info", f"{key}:{value}"]
    for name in APK_SCRIPTS:
        cmd += ["--script", f"{name}:{SCRIPTS / name}"]
    cmd += ["--files", str(stage), "--output", str(out)]
    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL)
    return out


def write_index(out_dir: Path, apk_bin, sec_key) -> Path:
    index = Path(out_dir) / "packages.adb"
    packages = [str(p.resolve()) for p in sorted(Path(out_dir).glob("*.apk"))]
    subprocess.run([str(apk_bin), "--allow-untrusted", "--sign-key", str(Path(sec_key).resolve()),
                    "mkndx", "--output", str(index.resolve())] + packages, check=True, stdout=subprocess.DEVNULL)
    return index
