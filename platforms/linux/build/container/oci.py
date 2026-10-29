"""Deterministic layer, blobs and OCI image layout."""
import gzip
import hashlib
import io
import json
import tarfile
from collections import namedtuple
from pathlib import Path

import image_spec

Blob = namedtuple("Blob", "media_type digest size data")
Entry = namedtuple("Entry", "name mode data link", defaults=(None, None))
PlatformImage = namedtuple("PlatformImage", "platform layer_tar layer config manifest")

MANIFEST = "application/vnd.oci.image.manifest.v1+json"
INDEX = "application/vnd.oci.image.index.v1+json"
CONFIG = "application/vnd.oci.image.config.v1+json"
LAYER = "application/vnd.oci.image.layer.v1.tar+gzip"
REF_NAME = "org.opencontainers.image.ref.name"


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def blob(media_type: str, data: bytes) -> Blob:
    return Blob(media_type, "sha256:" + sha256_hex(data), len(data), data)


def json_bytes(obj) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def tar_info(entry: Entry) -> tarfile.TarInfo:
    info = tarfile.TarInfo(entry.name)
    info.mode = entry.mode
    info.uname = info.gname = ""
    info.mtime = 0
    info.type = tarfile.DIRTYPE if entry.data is None and entry.link is None else tarfile.REGTYPE
    if entry.link is not None:
        info.type, info.linkname = tarfile.SYMTYPE, entry.link
    if entry.data is not None:
        info.size = len(entry.data)
    return info


def tar_bytes(entries: list) -> bytes:
    """USTAR archive with fixed owner, zero mtimes and the given member order."""
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w", format=tarfile.USTAR_FORMAT) as tar:
        for entry in entries:
            data = io.BytesIO(entry.data) if entry.data is not None else None
            tar.addfile(tar_info(entry), data)
    return buf.getvalue()


def layer_entries(binary: bytes) -> list:
    return [
        Entry("usr/", 0o755),
        Entry("usr/bin/", 0o755),
        Entry(image_spec.BINARY, 0o755, binary),
        Entry(image_spec.DATA + "/", 0o700),
    ]


def gzip_bytes(data: bytes) -> bytes:
    return gzip.compress(data, compresslevel=9, mtime=0)


def manifest_doc(config: Blob, layer: Blob) -> dict:
    return {
        "schemaVersion": 2,
        "mediaType": MANIFEST,
        "config": descriptor(config),
        "layers": [descriptor(layer)],
    }


def descriptor(b: Blob) -> dict:
    return {"mediaType": b.media_type, "digest": b.digest, "size": b.size}


def platform_image(version: str, p: image_spec.Platform, binary: bytes) -> PlatformImage:
    layer_tar = tar_bytes(layer_entries(binary))
    diff_id = "sha256:" + sha256_hex(layer_tar)
    layer = blob(LAYER, gzip_bytes(layer_tar))
    config = blob(CONFIG, json_bytes(image_spec.image_config(version, p, diff_id)))
    manifest = blob(MANIFEST, json_bytes(manifest_doc(config, layer)))
    return PlatformImage(p, layer_tar, layer, config, manifest)


def platform_doc(p: image_spec.Platform) -> dict:
    doc = {"architecture": p.arch, "os": "linux"}
    if p.variant:
        doc["variant"] = p.variant
    return doc


def index_blob(images: list) -> Blob:
    manifests = []
    for image in images:
        manifests.append({**descriptor(image.manifest), "platform": platform_doc(image.platform)})
    annotations = {
        "org.opencontainers.image.source": image_spec.SOURCE,
        "org.opencontainers.image.description": image_spec.DESCRIPTION,
    }
    doc = {"schemaVersion": 2, "mediaType": INDEX, "manifests": manifests, "annotations": annotations}
    return blob(INDEX, json_bytes(doc))


def blob_path(b: Blob) -> str:
    return "blobs/sha256/" + b.digest.split(":", 1)[1]


def index_entries(index: Blob, images: list, version: str) -> list:
    """Descriptors for index.json: the image index, then every platform manifest, each with its ref name."""
    entries = [{**descriptor(index), "annotations": {REF_NAME: version}}]
    for image in images:
        ref = f"{version}-{image_spec.tag_suffix(image.platform)}"
        entries.append(
            {
                **descriptor(image.manifest),
                "platform": platform_doc(image.platform),
                "annotations": {REF_NAME: ref},
            }
        )
    return entries


def write_layout(root: Path, version: str, images: list) -> None:
    index = index_blob(images)
    blobs = [index]
    for image in images:
        blobs += [image.layer, image.config, image.manifest]
    (root / "blobs" / "sha256").mkdir(parents=True, exist_ok=True)
    for b in blobs:
        (root / blob_path(b)).write_bytes(b.data)
    (root / "oci-layout").write_bytes(json_bytes({"imageLayoutVersion": "1.0.0"}))
    top = {"schemaVersion": 2, "manifests": index_entries(index, images, version)}
    (root / "index.json").write_bytes(json_bytes(top))
