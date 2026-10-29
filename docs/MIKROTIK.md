# Mesh Town on MikroTik RouterOS 7

A Mesh Town node can run as a container on a MikroTik router. The container holds the console node (`meshtown-cli`) and
gives your LAN the node page and a SOCKS5/HTTP proxy that reaches the other members. This page lists every command
from an empty router to a running node. The other systems are in [INSTALL.md](INSTALL.md).

All commands are RouterOS 7 console commands (WinBox terminal or SSH).

## Requirements

- **RouterOS 7.21 or newer.** The commands below use `mountlists=`, which replaced `mounts=` in 7.21. On an older 7.x
  version, write `mounts=` in the `/container/add` command.
- **A model with an ARM, ARM64 or x86 CPU** and the `container` package installed. MIPS models (mipsbe, mmips, smips)
  and PPC cannot run containers. The hEX Refresh (EN7562CT) only runs 32-bit ARMv5 images, so it does not fit.
- **About 30 MB of disk** for the image and the node data. Use an external USB drive or disk when you have one: the
  internal flash of small models is tight, and a fast disk makes the unpacking much quicker.
- **Memory.** The node needs memory for its network tables; check `/container/print` memory use after the start. To keep
  the node from taking more than you can spare, set a limit with `memory-high=` on the container.
- **A DNS server** in `/ip/dns` (or `dns=` on the container); RouterOS 7.24 and newer refuse to start a container
  that has neither.

## Enable containers

Containers are switched off in the default device mode. Allow them:

```
/system/device-mode/update container=yes
```

Then confirm on the device within 5 minutes: press the reset (or mode) button, or switch the power off and on (on an
x86 machine, a power-off). A plain reboot from the console does not confirm it. Without a confirmation the change is
cancelled. RouterOS allows 3 unconfirmed attempts in a row; after that the counter resets only by a power-off or a
button press.

Install the `container` package if it is not there yet (upload it from the "Extra packages" archive of your
RouterOS version and reboot), then check it:

```
/system/package/print
```

## Network

The container gets its own virtual Ethernet port (`veth1`) in a separate bridge with its own subnet:

```
/interface/veth/add name=veth1 address=172.17.0.2/24 gateway=172.17.0.1
/interface/bridge/add name=containers
/ip/address/add address=172.17.0.1/24 interface=containers
/interface/bridge/port add bridge=containers interface=veth1
/ip/firewall/nat/add chain=srcnat action=masquerade src-address=172.17.0.0/24
```

The masquerade rule gives the node access to the internet through the router. The node opens only outgoing
connections to other members, so it needs no incoming port from the internet.

If your firewall drops forwarded traffic by default, allow traffic between your LAN and the `containers` subnet in the
`forward` chain (containers go through `forward`, even when attached to the main bridge). Add the rules before the
final drop rule:

```
/ip/firewall/filter/add chain=forward action=accept src-address=172.17.0.0/24 comment="containers out"
/ip/firewall/filter/add chain=forward action=accept dst-address=172.17.0.0/24 comment="containers in"
```

(Use `place-before=` to put them above your drop rule.)

## Storage

The node keeps its identity, the network key and the settings in `/data`. Give it a directory on a disk, so it
survives removing and re-creating the container. The source is a directory, not a file. Replace `disk1` with the name
of your disk (`/disk/print`):

```
/container/mounts/add list=meshtown src=disk1/meshtown-data dst=/data
```

If the directory does not exist yet, RouterOS creates it. Keep it: deleting it makes a new node out of the container.
The settings file in it is secret (it holds the network key and the node secret).

## Install from the registry

```
/container/add remote-image=ghcr.io/mesh-town/meshtown:latest interface=veth1 root-dir=disk1/meshtown mountlists=meshtown dns=1.1.1.1 logging=yes start-on-boot=yes name=meshtown
```

- `root-dir` must not exist yet and must not be shared with another container; put it on the disk too.
- `logging=yes` writes the node output to the RouterOS log, where the first-start link appears.
- `start-on-boot=yes` starts the node after a router reboot.
- `dns=` is the resolver for the container; drop it if `/ip/dns` has servers.
- The image exists for each architecture: `ghcr.io/mesh-town/meshtown:<version>-arm64`, `<version>-arm` and
  `<version>-amd64`. The plain tag (`latest` or `<version>`) should pick the right one; if the container does not
  start with an "exec format" error, use the tag of your architecture (`/system/resource/print`, field
  `architecture-name`).
- A certificate error while pulling: check the router clock first. As a last resort `check-certificate=no` turns the
  check off for this container.

Wait until `/container/print` shows `status=stopped` (the image is unpacked), then start it:

```
/container/start meshtown
```

## Install from a file

When the router cannot reach the registry, load a tar for your architecture instead. The release has one file per
architecture, `meshtown-container-<version>-arm64.tar`, `-arm.tar` and `-amd64.tar`, and
`meshtown-container-<version>.sha256sums` with their checksums. Download them from the
[GitHub Releases](https://github.com/mesh-town/meshtown-dist/releases) page on a computer, check the checksum
(`shasum -a 256 -c meshtown-container-<version>.sha256sums --ignore-missing`), and put the tar on the router's disk
(WinBox "Files", `scp`, or `/tool/fetch` from a server you control). Then:

```
/container/add file=disk1/meshtown-container-<version>-arm64.tar interface=veth1 root-dir=disk1/meshtown mountlists=meshtown dns=1.1.1.1 logging=yes start-on-boot=yes name=meshtown
/container/start meshtown
```

## First start

The node prints a sign-in link to its output. Find it in the RouterOS log:

```
/log print where topics~"container"
```

The link looks like `http://172.17.0.2:7777/?token=...`. Open it as it is, in a browser on a computer in your LAN.
The token works once and expires in 10 minutes; if it was missed, restart the container
(`/container/stop meshtown`, then `/container/start meshtown`) for a new link. On the page, join a network with an
invite or create a new one.

## Page and proxy

**The page** is at `http://172.17.0.2:7777`, and only there. The computers of your LAN reach this address through the
router, which has the `containers` subnet. Do not forward the page to the router's own address with a `dst-nat`
rule: the node checks which address a browser used to open the page and refuses any other one with a "403" error.
This check protects the page from malicious websites, so it cannot be switched off.

**The proxy** is `172.17.0.2:1080`, the same way. If you prefer the router's LAN address (so that a computer that
has no route to the `containers` subnet can use it), forward the port; replace `192.168.88.1` with the router's LAN
address:

```
/ip/firewall/nat/add chain=dstnat action=dst-nat protocol=tcp dst-address=192.168.88.1 dst-port=1080 to-addresses=172.17.0.2 to-ports=1080
```

The image starts the node with `--socks-access localnet`, which lets the computers of your LAN use the proxy. If you
set your own `cmd=` on the container, repeat the flag: `cmd="run --socks-access localnet"`.

Programs that use the proxy (a browser setting, `socks5h://172.17.0.2:1080`) reach the other members by their names,
as described in [USAGE.md](USAGE.md). Do not open the proxy to the internet: forward port 1080 only for LAN
addresses.

Nothing else needs forwarding. The node's UDP port is random and only used for outgoing connections, so no UDP rule
is required.

## Network key and invite

To join a network without the page, put the key or the invite in the environment of the container:

```
/container/envs/add list=meshtown-env key=MESHTOWN_KEY value="<your network key>"
/container/set meshtown envlist=meshtown-env
```

`MESHTOWN_INVITE` works the same way for an invite. The key forms are described in [USAGE.md](USAGE.md#the-network-key).
Environment values are visible to anyone who can read the router configuration, so prefer the page for a key that
you want to keep private.

## Update

From the registry, pull the image again and restart:

```
/container/stop meshtown
/container/repull meshtown
/container/start meshtown
```

(`repull` exists since RouterOS 7.20; changing `remote-image` to a new tag does the same.) For a container made from a
file, stop and remove it, load the new tar and add the container again with the same command:

```
/container/stop meshtown
/container/remove meshtown
```

The directory from `Storage` stays, so the node keeps its identity and settings. The page tells you when a newer
version is out.

## Limitations

- The node runs inside a container as the console node: there is no tray icon, and all work is done on the page.
- The page is reachable only at the container address (see above); it is not meant to be published to the internet.
- Memory and disk use depend on the model; slow flash storage makes the first unpacking slow.
- The image supports ARM64, 32-bit ARM (ARMv7) and x86_64. A model with a MIPS CPU cannot run containers; use a
  computer or another router in the same network instead (for OpenWrt and Keenetic see [INSTALL.md](INSTALL.md#routers)).
- General limits of the program: [LIMITATIONS.md](LIMITATIONS.md).
