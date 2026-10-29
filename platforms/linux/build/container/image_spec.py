"""Constants and the image configuration of the Mesh Town container image."""
import sys
from collections import namedtuple
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent.parent / "openwrt" / "feed"))

import targets  # noqa: E402

Platform = namedtuple("Platform", "triple arch variant required")
# arm64 and amd64 must be present in a release; armv7 is added when its archive exists.
PLATFORMS = (
    Platform(targets.AARCH64, "arm64", None, True),
    Platform(targets.ARMV7, "arm", "v7", False),
    Platform(targets.X86_64, "amd64", None, True),
)

IMAGE = "ghcr.io/mesh-town/meshtown"
SOURCE = "https://github.com/mesh-town/meshtown"
DESCRIPTION = "Mesh Town node: a peer-to-peer private network"
BINARY = "usr/bin/meshtown"
DATA = "data"
ENTRYPOINT = ["/usr/bin/meshtown"]
CMD = ["run", "--socks-access", "localnet"]
ENV = ["PATH=/usr/bin", "MESHTOWN_CONFIG=/data/config.json", "NO_COLOR=1"]
EXPOSED = {"7777/tcp": {}, "1080/tcp": {}}
TAR_FORMATS = ("docker", "oci")
# Layout of the per-architecture tar given to a loader that wants a file.
DEFAULT_TAR_FORMAT = "docker"


def tag_suffix(p: Platform) -> str:
    """Tag suffix naming the architecture: arm64, arm or amd64."""
    return p.arch


def labels(version: str) -> dict:
    return {
        "org.opencontainers.image.title": "meshtown",
        "org.opencontainers.image.version": version,
        "org.opencontainers.image.licenses": "AGPL-3.0-only",
        "org.opencontainers.image.source": SOURCE,
        "org.opencontainers.image.description": DESCRIPTION,
    }


def image_config(version: str, p: Platform, diff_id: str) -> dict:
    """OCI image configuration; diff_id is the digest ("sha256:<hex>") of the uncompressed layer."""
    config = {
        "architecture": p.arch,
        "os": "linux",
        "config": {
            "Entrypoint": list(ENTRYPOINT),
            "Cmd": list(CMD),
            "Env": list(ENV),
            "Volumes": {"/" + DATA: {}},
            "ExposedPorts": dict(EXPOSED),
            "StopSignal": "SIGTERM",
            "Labels": labels(version),
        },
        "rootfs": {"type": "layers", "diff_ids": [diff_id]},
    }
    if p.variant:
        config["variant"] = p.variant
    return config
