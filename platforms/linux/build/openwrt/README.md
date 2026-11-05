# Mesh Town on OpenWrt

Two ways to install the node on an OpenWrt router:

- **Package feed** (recommended): one command, updates through the package manager.
- **Manual install**: copy the files yourself. Use it for architectures the feed does not cover and for offline routers.

## Quick install from the feed

Supported: OpenWrt 24.10 (`opkg`, `.ipk`) and 25.12 (`apk`, `.apk`). On the router, as root:

    wget -O- https://mesh-town.github.io/meshtown-openwrt/install.sh | sh

The script reads the router architecture from `/etc/openwrt_release` (`DISTRIB_ARCH`) and downloads that architecture's
package index **before** it changes anything. If the feed has no packages for the architecture, it prints
`no packages for <architecture>` and exits with code 2, leaving the router untouched. Otherwise it:

- downloads the feed public key into the package manager key store (`/etc/opkg/keys/` or `/etc/apk/keys/`);
- adds the feed to `/etc/opkg/customfeeds.conf` (24.10) or `/etc/apk/repositories.d/customfeeds.list` (25.12);
- runs `opkg update` / `apk update` (a failure caused by someone else's feeds does not stop the install) and installs the
  package `meshtown`;
- enables and starts the service. The node starts on boot and restarts after a crash (`logread -e meshtown`).

The package puts these files on the router:

- `/usr/bin/meshtown`
- `/etc/init.d/meshtown`
- `/lib/upgrade/keep.d/meshtown` (keeps `/etc/meshtown/` across `sysupgrade`)

The node state (`config.json` with the network key, `web-token`, `docs-store/`) lives in `/etc/meshtown/`, which the
package never touches. Step 4 of the manual install (`/etc/sysupgrade.conf`) is **not** needed with the package.

### Supported architectures

OpenWrt `DISTRIB_ARCH`:

| family | architectures |
|---|---|
| aarch64 | `aarch64_cortex-a53`, `aarch64_cortex-a72`, `aarch64_cortex-a76`, `aarch64_generic` |
| x86_64 | `x86_64` |
| ARMv7 (soft-float build, no FPU instructions) | `arm_cortex-a5_vfpv4`, `arm_cortex-a7`, `arm_cortex-a7_neon-vfpv4`, `arm_cortex-a7_vfpv4`, `arm_cortex-a8_vfpv3`, `arm_cortex-a9`, `arm_cortex-a9_neon`, `arm_cortex-a9_vfpv3-d16`, `arm_cortex-a15_neon-vfpv4` |
| MIPS big-endian (soft-float build, MIPS32 release 2) | `mips_24kc` |
| MIPS little-endian (soft-float build, MIPS32 release 2) | `mipsel_24kc`, `mipsel_74kc`, `mipsel_24kc_24kf` |

Not in the feed yet: ARMv5/v6, 32-bit x86, and MIPS32 release 1 (`mips_mips32`, `mipsel_mips32`). The MIPS build uses
MIPS32 release 2 instructions, which a release 1 core does not execute. For these architectures, or if the feed is
unreachable, use the manual install below.

### Update

Run the command for your OpenWrt version only (`grep DISTRIB_RELEASE /etc/openwrt_release` shows it):

- OpenWrt 24.10 (`opkg`): `opkg update && opkg upgrade meshtown`
- OpenWrt 25.12 (`apk`): `apk update && apk upgrade meshtown`

The package restarts the service. LuCI offers the same under System, Software. The feed holds only the current version;
to go back to an older one, use the manual install.

### Remove

- 24.10: `opkg remove meshtown`
- 25.12: `apk del meshtown`

This stops and disables the service and removes the package files. Two things stay:

- `/etc/meshtown/` holds the network key and the node identity. Delete it by hand to forget the network.
- The feed entry and key. Remove the `customfeeds.conf` / `customfeeds.list` line and the key file if you want.

### After sysupgrade

A firmware upgrade does not keep third-party packages. Install the package again with the same command.
`/etc/meshtown/` is kept, so the node comes back with the same identity.

Moving from 24.10 to 25.12: run `install.sh` again. It switches the feed to the `apk` one.

### Moving from a manual install

If `/usr/bin/meshtown` exists and no package owns it, `install.sh` says that it will replace the file and installs over it:

- on `opkg` it uses `--force-overwrite`;
- on `apk` it moves the new `/etc/init.d/meshtown` into place and restarts the service.

`/etc/meshtown/` is not touched, so the network and settings are kept. The lines that step 4 of the manual install added
to `/etc/sysupgrade.conf` do no harm. To remove them: `sed -i "/meshtown/d" /etc/sysupgrade.conf`.

### Feed keys

The feed is signed. `opkg` indexes use `usign` (`meshtown-usign.pub`); `apk` indexes use an EC P-256 key
(`meshtown-apk.pem`). Both public keys are in the feed root and in `platforms/linux/build/openwrt/keys/` of the
repository. Compare the key on your router with the repository copy before trusting a mirror.

- `meshtown-usign.pub` (`usign -F`): `56bb3849e4234cf7`. On a 24.10 router the key is `/etc/opkg/keys/56bb3849e4234cf7`.
- `meshtown-apk.pem` (SHA-256 of the file): `995d527f1cee20d131b3e8e30e120b6ee9b21965cd42b3c47cf63eae4f86d3f0`.

### Mirror or local copy

Download `install.sh` from the copy and run it as `MESHTOWN_FEED=<base url of the copy> sh install.sh`. The variable
overrides the address built into the script.

# Manual install

The `init.d-meshtown` script is a procd service: the node starts when the router boots, restarts after a crash, and logs
to `logread`. It is built for OpenWrt 24.10 (aarch64).

Requirements and conventions:

- A static aarch64 binary: `make build_linux_arm64_cli` produces `dist/meshtown-linux-arm64-cli-<version>`.
- The commands below run on a computer at the repository root, with `ROUTER=root@192.168.1.1`.
- Files are copied with `cat | ssh`: dropbear on OpenWrt may lack `sftp-server`, and `scp` without `-O` fails.

## What goes where

| file | path on the router | permissions |
|---|---|---|
| binary | `/usr/bin/meshtown` | 0755, about 10 MB on the overlay |
| service script | `/etc/init.d/meshtown` | 0755 |
| config and all node state | `/etc/meshtown/` (`config.json` with the network key, `web-token`, `docs-store/`) | directory 0700, files 0600 |
| autostart links | `/etc/rc.d/S99meshtown`, `/etc/rc.d/K01meshtown` (created by `enable`) | — |

The service runs `meshtown run --config /etc/meshtown/config.json` as root. The script variables `EXIT_FLAG` and
`SOCKS_FLAG` (both empty by default) carry the `--exit`/`--no-exit` and `--socks`/`--no-socks` flags.

The script creates `/etc/meshtown/` itself. On the first start without a config, the node creates a new network and
writes the key to `config.json`. Nothing lives in `/tmp`.

## Installation steps

Step 0, checks before installation (they change nothing):

    ROUTER=root@192.168.1.1
    ssh $ROUTER 'pidof meshtown; netstat -lntup | grep -E ":(1080|7777) "; df -h /overlay; [ -z "$(tail -c1 /etc/sysupgrade.conf)" ] && echo newline_ok || echo no_newline'

Expected output:

- No process id: no node was started by hand from `/tmp`.
- Ports 1080 and 7777 free. If a port is taken, the node does not start and goes into a crash loop.
- At least 20 MB free on `/overlay`.
- `newline_ok`: `/etc/sysupgrade.conf` ends with a newline. OpenWrt has no `od`. Step 4 writes a leading newline anyway;
  without it, the first new line would stick to the last line of the file, and that line would stop being preserved on a
  firmware upgrade.

On a running node, `netstat` shows `127.0.0.1:1080` and `:7777`.

Steps 1–3, copy and start:

    cat dist/meshtown-linux-arm64-cli-<version> | ssh $ROUTER 'cat > /usr/bin/meshtown && chmod 0755 /usr/bin/meshtown'
    cat platforms/linux/build/openwrt/init.d-meshtown   | ssh $ROUTER 'cat > /etc/init.d/meshtown && chmod 0755 /etc/init.d/meshtown'
    ssh $ROUTER '/etc/init.d/meshtown enable && /etc/init.d/meshtown start'

Step 3a, networks for the proxy and the page. On a fresh install, `/etc/meshtown/config.json` does not exist yet. The
init script creates it (as the node's first start does) with `["br-lan"]` in both fields (see "Networks" below), so no
command is needed after the first start. To change the choice later:

    ssh $ROUTER 'meshtown access ifaces --config /etc/meshtown/config.json "$(uci -q get network.lan.device)" && /etc/init.d/meshtown restart'

The node is already running at this point, so the command goes through its page. The write is immediate; `socks.ifaces`
takes effect after a restart. The same command restores the "home network only" choice if it was changed.

Step 4, append to `/etc/sysupgrade.conf`. Without these lines the files are missing from the preserved list
(`sysupgrade -l`) and are lost on a firmware upgrade. Without the `rc.d` lines, the node does not start after an upgrade.

    ssh $ROUTER 'printf "\n/usr/bin/meshtown\n/etc/init.d/meshtown\n/etc/meshtown/\n/etc/rc.d/S99meshtown\n/etc/rc.d/K01meshtown\n" >> /etc/sysupgrade.conf'

The `/etc/rc.d/*` links need separate lines: `sysupgrade -l` does not show them until they are added.

### Joining an existing network

Before the first `start` (or after `stop`), run:

    /usr/bin/meshtown join --config /etc/meshtown/config.json <network key>

The key is written to `config.json`. Then `start`. The key looks like `word-word--…`; the node prints it when it creates a
network. The `mesh1-…` format is accepted too. Do not put the key into the script: it would be visible in `ps` and in
the script file.

## Check

    ssh $ROUTER '/etc/init.d/meshtown status; pidof meshtown; logread -e meshtown | tail -20'
    ssh $ROUTER 'ls -l /etc/rc.d | grep mesh; sysupgrade -l | grep -E "mesh|rc.d"'

Expected:

- `running`, a process id, and the line `[exit] working as an exit for network members …`.
- Two links, and at least nine lines in the sysupgrade list (seven files and two links). `docs-store` may add more files
  as data grows.
- In `[status]`, the node writes its memory use once a minute (`memory … KiB`; about 11–12 MB on a small router).

## Update the binary

A node installed from the package feed is updated by the package manager (see "Update" above). For a manual install,
you cannot write over a running file: write to a temporary name and `mv`:

    cat dist/meshtown-linux-arm64-cli-<version> | ssh $ROUTER 'cat > /usr/bin/meshtown.new && chmod 0755 /usr/bin/meshtown.new \
      && mv /usr/bin/meshtown.new /usr/bin/meshtown && /etc/init.d/meshtown restart'

## Rollback

    ssh $ROUTER '/etc/init.d/meshtown stop; /etc/init.d/meshtown disable; sleep 15; pidof meshtown; \
      rm -f /etc/init.d/meshtown /usr/bin/meshtown; sed -i "/meshtown/d" /etc/sysupgrade.conf'

- `stop` returns immediately; the node shuts down in about 3 s more (15 s at most, `term_timeout`).
- That is why the line has `sleep 15` and `pidof` before `rm`. `pidof` must answer empty. `rm` while the process is
  alive is harmless: the file disappears after the process exits.
- `disable` removes both links in `/etc/rc.d`.
- The rollback keeps `/etc/meshtown/`: it holds the network key and the node secret. After `rm -rf /etc/meshtown`, the
  node gets a new identity on the next install.

## Proxy and exit

### Proxy

In the script, `SOCKS_FLAG=` is empty, so `config.json` decides whether the proxy is on. By default
(`socks.access: "localhost"`) the proxy on the router is open only to the router itself.

To let phones and computers at home use it, do one of these:

- open the node page, go through the setup steps, and click "Open for the home network";
- set `"socks": {"access": "localnet"}` in `/etc/meshtown/config.json` and restart the service.

Then the node:

- listens on `0.0.0.0:1080`;
- lets in only private addresses: loopback, `10/8`, `172.16/12`, `192.168/16`, `169.254/16` (it cuts off tailscale
  addresses `100.64/10` itself);
- serves a PAC at `http://<router address on br-lan>:1080/proxy.pac`, for example `http://192.168.1.1:1080/proxy.pac`.
  The address in the PAC is the one the request came to.

The proxy has no password, so do not open the port on the wan zone. In the stock OpenWrt `firewall`, wan has
`input REJECT`: do not add rules for 1080. Check with `nft list chain inet fw4 input_wan` (no `accept` on 1080).

To turn the proxy off, set `SOCKS_FLAG=--no-socks` and `restart` (the flag overrides the file). `socks.enabled: false`
in the file, with an empty `SOCKS_FLAG`, turns it off too.

### Exit

In the script, `EXIT_FLAG=` is empty, so `config.json` decides the exit role (`exit.enabled`, on by default). The router
then works as an exit for other members. The checkbox "Allow other members to open sites through this computer" on the
node page switches it. The exit cuts off private addresses, so it does not let traffic through to the home network or
the router itself.

`EXIT_FLAG=--no-exit` or `--exit` pins the role over the file; the checkbox then has no effect.

A router installed earlier with `--no-exit` saved to the file may have written `exit.enabled: false` to `config.json`.
After you replace the script and `restart`, turn the checkbox on the page on once.

For the exit role, the script already sets `limits nofile="4096 4096"` (one descriptor per proxied connection).

### Networks

The fields `socks.ifaces` and `web.ifaces` in `config.json` decide which router networks reach the proxy (port 1080) and
the node page (port 7777). They hold interface names (`br-lan`, `br-guest`, `eth0`) and have three states:

| value | meaning |
|---|---|
| no key, or `[]` | automatic: the node closes the default-route interface (WAN) and opens the other private networks, including the guest `br-guest` |
| `["*"]` | no restrictions |
| `["br-lan"]` | only the listed interfaces |

An explicit list beats the automatic mode. This is the second line after the firewall. The port on WAN stays visible
(the connection is accepted and closed at once with no reply), but the node does not serve a request from WAN.

Before starting, the init script calls `meshtown access ifaces --if-unset <lan device>`:

- It writes `["br-lan"]` to both fields **only if the file has neither of the two keys**.
- If there is no file (a fresh install), it creates the file with these values.
- It leaves an unreadable file alone.

So after an update, the router lets in only `br-lan` (guest and WAN closed). A choice made on the page (the "ISP network"
checkbox and others) and a restored automatic mode survive a restart.

The script takes the device from `uci -q get network.lan.device`; on OpenWrt 19.07, from `network.lan.type = bridge`
(`br-lan`). If it finds none, it seeds nothing and the automatic mode works.

Check:

    ssh $ROUTER 'uci -q get network.lan.device; cat /proc/sys/net/ipv4/ip_forward; logread -e meshtown | tail'
    ssh $ROUTER "jsonfilter -i /etc/meshtown/config.json -e '@.config.socks.ifaces' -e '@.config.web.ifaces'"

- `ip_forward` equals 1: forwarding is on, and the automatic mode treats the machine as a router.
- `logread` shows the seeding line and the reasons for refusals.
- The `jsonfilter` path depends on the file section. If the output is empty, open the file and look for the `ifaces` keys.

To change the choice, run `meshtown access ifaces --config /etc/meshtown/config.json <value>` and `restart`:

| value | result |
|---|---|
| `auto` | automatic choice (the script stays silent afterwards: the key is in the file, even if `[]`) |
| `br-lan` | "home network only" |
| `'*'` | all networks |

## Good to know

- **Log.** procd puts the node output into `logread` under the name `meshtown`. The new network key and the node page
  link with the token end up there in plain text, visible to everyone with access to the router. The log ring
  `logd -S 128` (128 KiB) holds about 3.5 hours at node idle (about 620 bytes a minute). To wipe it:
  `/etc/init.d/log restart`.
- **Restart on crash.** `respawn 60 5 10` means a 5 s pause between restarts. After 10–11 quick restarts in a row, procd
  gives up on the node (`logread -e procd` writes "crash loop"). To bring it back: `/etc/init.d/meshtown restart`.
- **Clock.** The router has no RTC, so after boot the clock may be in the past until time sync runs. The node is alive
  then, but TLS connections fail ("certificate not valid yet"). Once the clock is right, they work by themselves within
  seconds, without `restart`. No time-change hook is needed. The same goes for a missing default route at start: the
  node waits and joins the network once the route appears.
- **Node page.** It listens on port 7777 according to the access mode `web.access`:
  - `localnet` (default): all IPv4 addresses; lets in only `127.0.0.0/8` and the private addresses `10/8`, `172.16/12`,
    `192.168/16`, `169.254/16`;
  - `localhost`: only `127.0.0.1`.

  The mode controls access to the node settings, not to the network and traffic.
- **Sign-in.** The page uses a password. Until one is set, the node prints the address with a one-time token for
  10 minutes (in `logread -e meshtown`). The permanent token `/etc/meshtown/web-token` never reaches the log. For a
  forgotten password, run `meshtown password reset`. Over HTTP, the password, token and cookie travel in plain text on
  the local network.
- **Reaching a `localhost` page.** From another computer, use `ssh -L 7777:127.0.0.1:7777 $ROUTER`.
- **WAN firewall.** On a router, the `localnet` mode relies on the WAN firewall. The peer filter accepts any private
  address. If the ISP gave the router a private WAN address (`10/8`, `172.16/12`), neighbors on the ISP network would
  pass the filter. OpenWrt drops inbound traffic on WAN by default: do not open port 7777 on WAN with a rule. When in
  doubt, run `meshtown access page localhost`.
- **Page networks.** On top of the firewall, `web.ifaces` restricts the page (see "Networks" above): `br-lan` is let
  in, WAN and guest are not. The page rejects (422) a `web.ifaces` write that would close the page for the current
  device.

## What changes on a live router

The manual install touches only these places:

| # | step | what changes | rollback |
|---|---|---|---|
| 1 | copy the binary | new file `/usr/bin/meshtown` (about 10 MB on the overlay) | `rm -f /usr/bin/meshtown` |
| 2 | copy the script | new file `/etc/init.d/meshtown` | `rm -f /etc/init.d/meshtown` |
| 3 | `enable` | two links `/etc/rc.d/S99meshtown`, `/etc/rc.d/K01meshtown` | `/etc/init.d/meshtown disable` |
| 4 | `start` | process `meshtown` (about 11.5 MB of memory, 7 threads, 18 descriptors, `nice 10`), UDP sockets on all addresses, SOCKS5 off by flag, node page by access mode (default: all IPv4 addresses, peer filter: loopback and private networks); directory `/etc/meshtown/` (0700) with the config, `web-token`, `docs-store/` | `/etc/init.d/meshtown stop`; the directory stays (it holds the network key) |
| 5 | lines in `/etc/sysupgrade.conf` | five lines appended to the end of the existing file (not an overwrite) | `sed -i "/meshtown/d" /etc/sysupgrade.conf` |

Beyond the table:

- **Outbound connections** of the node: DHT bootstrap and peer connections (UDP), `ipwho.is` (country), DoH `1.1.1.1`.
- **Log:** `logread` shows the new network key (first start) and the node page link with the token (every start).
- **Overlay space:** about 10 MB of binary plus a growing `/etc/meshtown/docs-store/`. Its size and the NAND wear from
  the write rate are not measured.
- **Joining an existing network** (`meshtown join`) writes the network key to `config.json`. It is a separate step.

Unchanged: `/etc/config/*`, `uci`, firewall, routes, interfaces, packages (`opkg`), other services. The install needs
neither `reboot` nor `sysupgrade`; reboot only to test autostart. It adds no firewall rules. Whether the node works
behind the router firewall without port forwarding is not tested. The exit role is on by default; its load on the router
is not measured.
