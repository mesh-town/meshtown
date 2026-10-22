"""README.md for the root of the published Keenetic feed: Entware prerequisite, install one-liner, manual setup, removal."""

CODE_REPO = "https://github.com/mesh-town/meshtown"
ENTWARE_HELP = "https://support.keenetic.com/titan/kn-1811/en/20980.html"

TEMPLATE = """# Mesh Town packages for Keenetic (Entware)

Package feed of [Mesh Town]({code}) for Keenetic routers with [Entware](https://entware.net). CI builds it from the
releases of `github.com/mesh-town/meshtown`; the feed holds only the latest version. The packages are not signed
(Entware does not check signatures).

Not tested on a live KeeneticOS router yet: the package is checked on Entware under emulation only.

## Before you start

1. Install Entware on the router from its web interface (the "OPKG" component and an Entware package system on a USB
   drive or on the internal storage): [Keenetic help]({help}).
2. Enable SSH access to the router and log in to the Entware shell (`/opt/bin/opkg` must exist).

Architectures: {dirs}. The installer picks yours from `/opt/etc/opkg.conf`.

## Install with one command

On the router, over SSH:

    wget -O- {base}/install.sh | sh

The https address needs a `wget` with TLS. If this command fails with an https or SSL error, install it first and
run the command again:

    opkg update && opkg install ca-certificates wget-ssl && opkg remove wget-nossl

The script itself does the same when the package feed cannot be fetched over https.

## Install by hand

`ARCH` is the last part of the `src/gz entware` address in `/opt/etc/opkg.conf`, for example `mipselsf-k3.4`:

    opkg install ca-certificates wget-ssl
    opkg remove wget-nossl
    mkdir -p /opt/etc/opkg
    echo "src/gz meshtown {base}/$ARCH" > /opt/etc/opkg/meshtown.conf
    opkg update && opkg install meshtown

## Use

The service `/opt/etc/init.d/S99meshtown` starts at boot. The node page opens at `http://<router address>:7777/` from
the home network (`br0`). Restart: `/opt/etc/init.d/S99meshtown restart`.

Entware has no process supervisor: if the node crashes, it stays stopped until the next start or reboot.

The node state (`config.json` with the network key, the web token) is in `/opt/etc/meshtown/`; the package never
touches it, so it survives updates and removal.

Update:

    opkg update && opkg upgrade meshtown

Remove (stops the service; the state directory stays, delete it by hand to forget the network):

    opkg remove meshtown

## More

Source, releases and other platforms: [{code}]({code}).
"""


def render_readme(base_url, dirs) -> str:
    return TEMPLATE.format(code=CODE_REPO, help=ENTWARE_HELP, base=base_url.rstrip("/"),
                           dirs=", ".join(f"`{x}`" for x in dirs))
