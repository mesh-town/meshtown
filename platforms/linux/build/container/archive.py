"""Per-architecture tar files for loaders that take a file: docker-archive and OCI layout."""
import json

import image_spec
import oci
from oci import Entry


def docker_tar(image: oci.PlatformImage, repo_tag: str) -> bytes:
    """docker-archive with the members and their order that `skopeo copy ... docker-archive:` writes."""
    diff_hex = oci.sha256_hex(image.layer_tar)
    config_hex = image.config.digest.split(":", 1)[1]
    legacy_id = oci.sha256_hex(f"sha256:{diff_hex} {image.config.digest}".encode())
    config = json.loads(image.config.data)
    legacy = {
        "id": legacy_id,
        "architecture": config["architecture"],
        "os": config["os"],
        "config": config["config"],
    }
    manifest = [{"Config": f"{config_hex}.json", "RepoTags": [repo_tag], "Layers": [f"{diff_hex}.tar"]}]
    repo, tag = repo_tag.rsplit(":", 1)
    ro = 0o444
    return oci.tar_bytes(
        [
            Entry(f"{diff_hex}.tar", ro, image.layer_tar),
            Entry(f"{config_hex}.json", ro, image.config.data),
            Entry(f"{legacy_id}/layer.tar", 0, None, f"../{diff_hex}.tar"),
            Entry(f"{legacy_id}/VERSION", ro, b"1.0"),
            Entry(f"{legacy_id}/json", ro, oci.json_bytes(legacy)),
            Entry("manifest.json", ro, oci.json_bytes(manifest)),
            Entry("repositories", ro, oci.json_bytes({repo: {tag: legacy_id}})),
        ]
    )


def oci_tar(image: oci.PlatformImage, ref: str) -> bytes:
    """OCI image layout holding one manifest, reachable by `ref`."""
    entry = {
        **oci.descriptor(image.manifest),
        "platform": oci.platform_doc(image.platform),
        "annotations": {oci.REF_NAME: ref},
    }
    index = {"schemaVersion": 2, "manifests": [entry]}
    rw = 0o644
    entries = [
        Entry("oci-layout", rw, oci.json_bytes({"imageLayoutVersion": "1.0.0"})),
        Entry("index.json", rw, oci.json_bytes(index)),
        Entry("blobs/", 0o755),
        Entry("blobs/sha256/", 0o755),
    ]
    for b in (image.layer, image.config, image.manifest):
        entries.append(Entry(oci.blob_path(b), rw, b.data))
    return oci.tar_bytes(entries)


def docker_tar_for(image: oci.PlatformImage, version: str) -> bytes:
    suffix = image_spec.tag_suffix(image.platform)
    return docker_tar(image, f"{image_spec.IMAGE}:{version}-{suffix}")


def oci_tar_for(image: oci.PlatformImage, version: str) -> bytes:
    return oci_tar(image, f"{version}-{image_spec.tag_suffix(image.platform)}")


WRITERS = {"docker": docker_tar_for, "oci": oci_tar_for}
