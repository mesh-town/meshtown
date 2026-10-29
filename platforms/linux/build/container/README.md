# Container image builder

Builds the `ghcr.io/mesh-town/meshtown` image from the static Linux release archives, with Python 3 standard library only.

```
python3 build_image.py --fetched DIR --version X.Y.Z --out DIR [--tar-format docker|oci]
```

`--fetched` holds `meshtown-cli-<version>-<triple>.tar.gz` for `aarch64-unknown-linux-musl` (linux/arm64) and
`x86_64-unknown-linux-musl` (linux/amd64), both required, and `armv7-unknown-linux-musleabi` (linux/arm/v7),
optional: a missing optional archive prints a `::warning::` and the platform is skipped. Every binary must be a
static ELF of the architecture its triple names.

Output in `--out`, which must not exist or must be empty:

- `oci/` is an OCI image layout: one image index (ref `<version>`) and one manifest per platform
  (ref `<version>-arm64`, `<version>-arm`, `<version>-amd64`). Push it with
  `skopeo copy --all --preserve-digests oci:<out>/oci:<ref> docker://ghcr.io/mesh-town/meshtown:<ref>`.
- `meshtown-container-<version>-<arch>.tar` is one tar per platform for loaders that take a file
  (for example RouterOS `/container add file=`). `--tar-format docker` (the default, `DEFAULT_TAR_FORMAT` in
  `image_spec.py`) writes the members of `skopeo copy ... docker-archive:` in the same order; `oci` writes an OCI
  layout archive.
- `meshtown-container-<version>.sha256sums` lists the tars in `sha256sum` format.

Exit codes: 0 built, 1 refused (an `::error::` line is printed), 2 bad invocation.

The image has one layer: `/usr/bin/meshtown` and an empty `/data` volume. Layers and tars are deterministic (fixed
owner, zero timestamps, fixed member order, gzip without a name), so two builds with the same zlib are byte-identical.

Modules: `image_spec.py` (constants, image config), `oci.py` (layer, blobs, layout), `archive.py` (per-platform tars),
`build_image.py` (command line). Tests: `python3 -m unittest discover -s . -t .`; set
`MESHTOWN_IMAGE_REQUIRE_SKOPEO=1` to fail instead of skip when `skopeo` is missing.
