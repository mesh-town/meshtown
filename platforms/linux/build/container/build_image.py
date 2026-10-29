#!/usr/bin/env python3
"""Builds the Mesh Town container image from the static Linux release archives.

build_image.py --fetched DIR --version X.Y.Z --out DIR [--tar-format docker|oci]
Reads DIR/meshtown-cli-<version>-<triple>.tar.gz for arm64, armv7 (optional) and amd64.
Writes to --out: oci/ (OCI image layout with the index and one manifest per platform),
meshtown-container-<version>-<arch>.tar for each platform and meshtown-container-<version>.sha256sums.
Prints one line per platform: <os/arch[/variant]> <manifest digest> <tar name>.
Exit codes: 0 built, 1 refused (::error:: line), 2 bad invocation.
"""
import argparse
import re
import sys
import tarfile
import tempfile
from pathlib import Path

import archive
import image_spec
import oci
import targets  # noqa: E402  (path is set up by image_spec)


class BuildError(Exception):
    pass


def version_arg(text: str) -> str:
    if not re.fullmatch(r"\d+\.\d+\.\d+", text):
        raise argparse.ArgumentTypeError(f"not a version X.Y.Z: {text}")
    return text


def read_binary(fetched: Path, version: str, triple: str):
    """Bytes of meshtown-cli from the release archive, None when the archive is absent."""
    base = f"meshtown-cli-{version}-{triple}"
    path = fetched / f"{base}.tar.gz"
    if not path.is_file():
        return None
    try:
        with tarfile.open(path) as tar:
            member = tar.extractfile(f"{base}/meshtown-cli")
            if member is None:
                raise BuildError(f"{path.name} has no {base}/meshtown-cli")
            return member.read()
    except (tarfile.TarError, OSError, KeyError, EOFError) as err:
        raise BuildError(f"{path.name}: cannot read {base}/meshtown-cli: {err}")


def check_binary(binary: bytes, triple: str) -> None:
    """Refuses a binary that is not a static ELF of the machine the triple names."""
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "meshtown-cli"
        path.write_bytes(binary)
        try:
            machine = targets.elf_machine(path)
        except ValueError as err:
            raise BuildError(f"{triple}: {err}")
    want = targets.TRIPLE_MACHINE[triple]
    if machine != want:
        raise BuildError(f"{triple}: binary is {machine}, expected {want}")


def collect(fetched: Path, version: str) -> list:
    found = []
    for p in image_spec.PLATFORMS:
        binary = read_binary(fetched, version, p.triple)
        if binary is None:
            missing(p, version)
            continue
        check_binary(binary, p.triple)
        found.append((p, binary))
    return found


def missing(p: image_spec.Platform, version: str) -> None:
    archive_name = f"meshtown-cli-{version}-{p.triple}.tar.gz"
    if p.required:
        raise BuildError(f"release has no {archive_name}")
    print(f"::warning::release has no {archive_name}; platform {p.arch} is skipped")


def platform_name(p: image_spec.Platform) -> str:
    return "/".join(["linux", p.arch] + ([p.variant] if p.variant else []))


def write_output(out: Path, version: str, tar_format: str, images: list) -> None:
    out.mkdir(parents=True, exist_ok=True)
    oci.write_layout(out / "oci", version, images)
    sums = []
    for image in images:
        name = f"meshtown-container-{version}-{image_spec.tag_suffix(image.platform)}.tar"
        data = archive.WRITERS[tar_format](image, version)
        (out / name).write_bytes(data)
        sums.append(f"{oci.sha256_hex(data)}  {name}\n")
        print(f"{platform_name(image.platform)} {image.manifest.digest} {name}")
    (out / f"meshtown-container-{version}.sha256sums").write_text("".join(sums))


def build(fetched: Path, version: str, out: Path, tar_format: str) -> None:
    if out.exists() and (not out.is_dir() or any(out.iterdir())):
        raise BuildError(f"{out} exists and is not empty")
    images = [oci.platform_image(version, p, binary) for p, binary in collect(fetched, version)]
    write_output(out, version, tar_format, images)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fetched", type=Path, required=True)
    parser.add_argument("--version", type=version_arg, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--tar-format", choices=image_spec.TAR_FORMATS, default=image_spec.DEFAULT_TAR_FORMAT)
    opts = parser.parse_args(argv)
    try:
        build(opts.fetched, opts.version, opts.out, opts.tar_format)
    except (BuildError, OSError) as err:
        print(f"::error::{err}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
