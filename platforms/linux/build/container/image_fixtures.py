"""Helpers shared by the container image builder tests (Python standard library only).

release_archive   a release archive meshtown-cli-<ver>-<triple>.tar.gz holding a tiny ELF file
fetched           a directory with such archives for several targets
run_builder       runs build_image.py as a subprocess
build             a fetched directory plus one builder run: Built(done, fetched, out)
skopeo_or_skip    the skopeo executable, or SkipTest (AssertionError when MESHTOWN_IMAGE_REQUIRE_SKOPEO=1:
                  CI must never skip silently)
sample_images     one oci.PlatformImage for every platform of image_spec
tmpdir            a fresh directory removed when the test ends
"""

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from collections import namedtuple
from pathlib import Path

CONTAINER_DIR = Path(__file__).resolve().parent
BUILD_IMAGE = CONTAINER_DIR / "build_image.py"
FEED_DIR = CONTAINER_DIR.parent / "openwrt" / "feed"
if str(FEED_DIR) not in sys.path:
    sys.path.append(str(FEED_DIR))

import targets  # noqa: E402
from feed_fixtures import AARCH64, ARMV7, MIPS, MIPSEL, X86_64, fake_elf, tar_entries  # noqa: E402,F401

VERSION = "0.3.195"
ALL = [AARCH64, ARMV7, X86_64]
SUFFIX = {AARCH64: "arm64", ARMV7: "arm", X86_64: "amd64"}
PLATFORM_NAME = {AARCH64: "linux/arm64", ARMV7: "linux/arm/v7", X86_64: "linux/amd64"}
IMAGE = "ghcr.io/mesh-town/meshtown"
DIGEST = re.compile(r"^sha256:[0-9a-f]{64}$")

Built = namedtuple("Built", "done fetched out")


def tmpdir(test):
    """A fresh directory removed when the test ends."""
    path = Path(tempfile.mkdtemp(prefix="imagetest-"))
    test.addCleanup(shutil.rmtree, path, ignore_errors=True)
    return path


def sha256_hex(data):
    return hashlib.sha256(data).hexdigest()


def tar_name(version, triple):
    return f"meshtown-container-{version}-{SUFFIX[triple]}.tar"


def elf_bytes(machine, static=True):
    """The bytes of a tiny well-formed ELF file of the given machine."""
    scratch = Path(tempfile.mkdtemp(prefix="imagetest-elf-"))
    try:
        return fake_elf(scratch / "bin", machine, static=static).read_bytes()
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


def release_archive(directory, version, triple, machine=None, static=True, member="meshtown-cli", raw=None):
    """directory/meshtown-cli-<ver>-<triple>.tar.gz with <name>/<member>; `raw` replaces the ELF file content."""
    base = f"meshtown-cli-{version}-{triple}"
    content = raw if raw is not None else elf_bytes(machine or targets.TRIPLE_MACHINE[triple], static)
    archive = Path(directory) / f"{base}.tar.gz"
    scratch = Path(tempfile.mkdtemp(prefix="imagetest-src-"))
    try:
        source = scratch / "binary"
        source.write_bytes(content)
        with tarfile.open(archive, "w:gz") as tar:
            info = tarfile.TarInfo(f"{base}/{member}")
            info.size = len(content)
            info.mode = 0o755
            with source.open("rb") as handle:
                tar.addfile(info, handle)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
    return archive


def fetched(tmp, version, triples):
    """tmp/fetched with a valid release archive for each triple."""
    directory = Path(tmp) / "fetched"
    directory.mkdir(parents=True, exist_ok=True)
    for triple in triples:
        release_archive(directory, version, triple)
    return directory


def run_builder(*args):
    cmd = [sys.executable, str(BUILD_IMAGE), *map(str, args)]
    return subprocess.run(cmd, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=600)


def build(test, triples=ALL, version=VERSION, *flags):
    """Valid archives for `triples`, then one builder run into tmp/out."""
    tmp = tmpdir(test)
    directory = fetched(tmp, version, triples)
    out = tmp / "out"
    done = run_builder("--fetched", directory, "--version", version, "--out", out, *flags)
    return Built(done, directory, out)


def built_ok(test, *flags):
    """A successful build of all three platforms: Built."""
    result = build(test, ALL, VERSION, *flags)
    test.assertEqual(result.done.returncode, 0, result.done.stdout + result.done.stderr)
    return result


def platform_lines(stdout):
    """The per-platform lines of the builder output: [(platform, digest, tar name)]."""
    return [tuple(line.split()) for line in stdout.splitlines() if line.strip() and not line.startswith("::")]


def read_json(path):
    return json.loads(Path(path).read_text())


def tree(root):
    """{relative path: bytes} of every file below root."""
    root = Path(root)
    return {str(p.relative_to(root)): p.read_bytes() for p in sorted(root.rglob("*")) if p.is_file()}


def blob_path(layout, digest):
    return Path(layout) / "blobs" / "sha256" / digest.split(":", 1)[1]


def sample_images(version=VERSION):
    """One oci.PlatformImage for every platform of image_spec, each with its own tiny ELF binary."""
    import image_spec
    import oci

    return [oci.platform_image(version, p, elf_bytes(targets.TRIPLE_MACHINE[p.triple])) for p in image_spec.PLATFORMS]


# ---------- skopeo ----------

def skopeo_or_skip(test):
    """The skopeo executable; a missing one skips the test unless MESHTOWN_IMAGE_REQUIRE_SKOPEO=1."""
    found = shutil.which("skopeo")
    if found:
        return found
    if os.environ.get("MESHTOWN_IMAGE_REQUIRE_SKOPEO") == "1":
        raise AssertionError("skopeo is not available (MESHTOWN_IMAGE_REQUIRE_SKOPEO=1 forbids skipping)")
    raise unittest.SkipTest("skopeo is not available")


def run_skopeo(skopeo, *args):
    """skopeo with the signature policy switched off; the test fails with the tool's message on a non-zero exit."""
    done = subprocess.run([skopeo, "--insecure-policy", *map(str, args)], capture_output=True, text=True,
                          stdin=subprocess.DEVNULL, timeout=300)
    if done.returncode != 0:
        raise AssertionError(f"skopeo {' '.join(map(str, args))} failed ({done.returncode}): {done.stderr}")
    return done


def skopeo_json(skopeo, *args):
    return json.loads(run_skopeo(skopeo, *args).stdout)
