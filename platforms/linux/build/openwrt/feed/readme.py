"""README.md for the root of the published feed: install one-liner, then setup, update and removal per package manager."""

CODE_REPO = "https://github.com/mesh-town/meshtown"

TEMPLATE = """# Mesh Town packages for OpenWrt

Signed package feed of [Mesh Town]({code}) for OpenWrt. CI builds it from the releases of
`github.com/mesh-town/meshtown`; the feed holds only the latest version.

## Install

On the router, as root:

    wget -O- {base}/install.sh | sh

The script reads the router architecture, adds the feed and its key, and installs the package `meshtown`.

Architectures: {labels}.

The commands below differ by OpenWrt version: 24.10 uses `opkg`, 25.12 uses `apk`. Check yours with
`grep DISTRIB_RELEASE /etc/openwrt_release` and follow only the matching section.

## OpenWrt 24.10 (opkg)

Manual setup instead of the script (`ARCH` is the value of `DISTRIB_ARCH` in `/etc/openwrt_release`):

    wget -O /tmp/meshtown-usign.pub {base}/meshtown-usign.pub
    opkg-key add /tmp/meshtown-usign.pub
    echo "src/gz meshtown {base}/opkg/$ARCH" >> /etc/opkg/customfeeds.conf
    opkg update && opkg install meshtown

Update:

    opkg update && opkg upgrade meshtown

Remove:

    opkg remove meshtown

## OpenWrt 25.12 (apk)

Manual setup instead of the script (`ARCH` is the value of `DISTRIB_ARCH` in `/etc/openwrt_release`):

    wget -O /etc/apk/keys/meshtown.pem {base}/meshtown-apk.pem
    echo "{base}/apk/$ARCH/packages.adb" >> /etc/apk/repositories.d/customfeeds.list
    apk update && apk add meshtown

Update:

    apk update && apk upgrade meshtown

Remove:

    apk del meshtown

After removal the node state in `/etc/meshtown/` stays on the router.

## Keys

The public keys are `meshtown-usign.pub` and `meshtown-apk.pem` in this feed; the same files are kept in
[platforms/linux/build/openwrt/keys]({code}/tree/main/platforms/linux/build/openwrt/keys) of the code repository, compare them before trusting the feed.

## More

Details and the fully manual install: [platforms/linux/build/openwrt/README.md]({code}/blob/main/platforms/linux/build/openwrt/README.md).
"""


def render_readme(base_url, labels) -> str:
    return TEMPLATE.format(code=CODE_REPO, base=base_url.rstrip("/"), labels=", ".join(f"`{x}`" for x in labels))
