# Mesh Town on OpenWrt

Two ways to install the node on an OpenWrt router: from the package feed (one command, updates through the package
manager) or by hand (copy the files yourself). The feed is the recommended way; the manual way stays for architectures
the feed does not cover and for offline routers.

## Install from the package feed

Supported: OpenWrt 24.10 (`opkg`, `.ipk`) and 25.12 (`apk`, `.apk`). On the router, as root:

    wget -O- https://mesh-town.github.io/meshtown-openwrt/install.sh | sh

The script reads the router architecture from `/etc/openwrt_release` (`DISTRIB_ARCH`) and downloads that architecture's
package index **before** it changes anything. If the feed has no packages for the architecture it prints
`no packages for <architecture>` and exits with code 2, leaving the router untouched. Otherwise it:

- downloads the feed public key into the package manager key store (`/etc/opkg/keys/` or `/etc/apk/keys/`);
- adds the feed to `/etc/opkg/customfeeds.conf` (24.10) or `/etc/apk/repositories.d/customfeeds.list` (25.12);
- runs `opkg update` / `apk update` (a failure caused by someone else's feeds does not stop the install) and installs the
  package `meshtown`;
- enables and starts the service. The node starts on boot and restarts after a crash (`logread -e meshtown`).

The package puts these files on the router: `/usr/bin/meshtown`, `/etc/init.d/meshtown`
and `/lib/upgrade/keep.d/meshtown` (keeps `/etc/meshtown/` across `sysupgrade`). The node state (`config.json` with the network key, `web-token`, `docs-store/`) lives in
`/etc/meshtown/`, which the package never touches. The "Steps" below about `/etc/sysupgrade.conf` are **not** needed with
the package.

**Update.** Run the command for your OpenWrt version only (`grep DISTRIB_RELEASE /etc/openwrt_release` shows it):

- OpenWrt 24.10 (`opkg`): `opkg update && opkg upgrade meshtown`
- OpenWrt 25.12 (`apk`): `apk update && apk upgrade meshtown`

The service is restarted by the package. The same is available in LuCI: System, Software. Feed contents are only the current version;
to go back to an older one use the manual way below.

**Remove.** On 24.10 `opkg remove meshtown`, on 25.12 `apk del meshtown`: it stops and disables the service and removes
the package files.
`/etc/meshtown/` stays (it holds the network key and the node identity); delete it by hand to forget the network.
The feed entry and key stay too (remove `customfeeds.conf` line / the `customfeeds.list` line and the key file if you want).

**After `sysupgrade`.** Third-party packages are not kept by a firmware upgrade: install the package again with the same
command (`/etc/meshtown/` is kept, so the node comes back with the same identity). Moving from 24.10 to 25.12: run
`install.sh` again, it switches the feed to the `apk` one.

**Moving from a manual install.** If `/usr/bin/meshtown` is already there and no package owns it, `install.sh` says that it
will replace it and installs over it (`--force-overwrite` on `opkg`; on `apk` it moves the new `/etc/init.d/meshtown` into
place and restarts the service). `/etc/meshtown/` is not touched, so the network and
settings are kept. The lines added to `/etc/sysupgrade.conf` by step 4 of the manual way do no harm; to remove them:
`sed -i "/meshtown/d" /etc/sysupgrade.conf`.

**Supported architectures** (OpenWrt `DISTRIB_ARCH`):

| family | architectures |
|---|---|
| aarch64 | `aarch64_cortex-a53`, `aarch64_cortex-a72`, `aarch64_cortex-a76`, `aarch64_generic` |
| x86_64 | `x86_64` |
| ARMv7 (soft-float build, no FPU instructions) | `arm_cortex-a5_vfpv4`, `arm_cortex-a7`, `arm_cortex-a7_neon-vfpv4`, `arm_cortex-a7_vfpv4`, `arm_cortex-a8_vfpv3`, `arm_cortex-a9`, `arm_cortex-a9_neon`, `arm_cortex-a9_vfpv3-d16`, `arm_cortex-a15_neon-vfpv4` |
| MIPS big-endian (soft-float build, MIPS32 release 2) | `mips_24kc` |
| MIPS little-endian (soft-float build, MIPS32 release 2) | `mipsel_24kc`, `mipsel_74kc`, `mipsel_24kc_24kf` |

Not in the feed yet: ARMv5/v6, 32-bit x86, MIPS32 release 1 (`mips_mips32`, `mipsel_mips32`: the MIPS build uses MIPS32 release 2 instructions, which a release 1 core does not execute). For these (or if the feed is unreachable) use the manual way below.

**Keys.** The feed is signed: `opkg` indexes with `usign` (`meshtown-usign.pub`), `apk` indexes with an EC P-256 key
(`meshtown-apk.pem`). Both public keys are in the feed root and in `platforms/linux/build/openwrt/keys/` of the repository; compare the
key your router has with the repository copy before trusting a mirror. Key fingerprints:

- `meshtown-usign.pub` (`usign -F`): `56bb3849e4234cf7` — on a 24.10 router the key is `/etc/opkg/keys/56bb3849e4234cf7`.
- `meshtown-apk.pem` (SHA-256 of the file): `995d527f1cee20d131b3e8e30e120b6ee9b21965cd42b3c47cf63eae4f86d3f0`.

**Mirror or local copy of the feed.** Download `install.sh` from the copy and run it as
`MESHTOWN_FEED=<base url of the copy> sh install.sh`; the variable overrides the address built into the script.


# Manual install and service details

Autostart of a Mesh Town node on OpenWrt.

The `init.d-meshtown` script is a procd service: the node starts when the router boots, restarts after a crash,
and logs to `logread`. Built for OpenWrt 24.10 (aarch64). **Not yet tested on a physical router.**

A static aarch64 binary is required (`make build_linux_arm64_cli` → `dist/meshtown-linux-arm64-cli-<version>`).
The commands below run from a Mac at the repository root; `ROUTER=root@192.168.1.1`. Files are poured in with `cat | ssh`:
dropbear on OpenWrt may lack `sftp-server`, and `scp` without `-O` fails.

## What goes where

| file | path on the router | permissions |
|---|---|---|
| binary | `/usr/bin/meshtown` | 0755, about 10 MB on the overlay |
| service script | `/etc/init.d/meshtown` | 0755 |
| config and all node state | `/etc/meshtown/` (`config.json` with the network key, `web-token`, `docs-store/`) | directory 0700, files 0600 |
| autostart links | `/etc/rc.d/S99meshtown`, `/etc/rc.d/K01meshtown` (created by `enable`) | — |

The node is started with `meshtown run --config /etc/meshtown/config.json` as root (the `--exit`/`--no-exit` and `--socks`/`--no-socks` flags live in the script variables `EXIT_FLAG`/`SOCKS_FLAG`, empty by default). The directory
`/etc/meshtown/` is created by the script itself; on the first start without a config the node creates a new network and writes
the key to `config.json`. Nothing lives in `/tmp`.

## Installation (manual)

Step 0, checks before installation (they change nothing):

    ROUTER=root@192.168.1.1
    ssh $ROUTER 'pidof meshtown; netstat -lntup | grep -E ":(1080|7777) "; df -h /overlay; [ -z "$(tail -c1 /etc/sysupgrade.conf)" ] && echo newline_ok || echo no_newline'

Expected: no process id (no node started by hand from `/tmp`), ports 1080 and 7777 free (an occupied port
— the node does not start and goes into a crash loop), at least 20 MB free on `/overlay`, `newline_ok`
(`/etc/sysupgrade.conf` ends with a newline; OpenWrt has no `od`; the block below is written with a leading newline anyway:
without it our first line would stick to the last line of the file, and that line would stop being preserved on a firmware upgrade).
The step 0 check: `netstat` shows `127.0.0.1:1080` and `:7777` on a running node.

Steps 1–3: copying and starting.

    cat dist/meshtown-linux-arm64-cli-<version> | ssh $ROUTER 'cat > /usr/bin/meshtown && chmod 0755 /usr/bin/meshtown'
    cat platforms/linux/build/openwrt/init.d-meshtown   | ssh $ROUTER 'cat > /etc/init.d/meshtown && chmod 0755 /etc/init.d/meshtown'
    ssh $ROUTER '/etc/init.d/meshtown enable && /etc/init.d/meshtown start'

Step 3a, networks for the proxy and the page: seeding works on a fresh install too. The file `/etc/meshtown/config.json` does not exist yet —
the init script creates it itself (the same way as the node's first start) with `["br-lan"]` in both fields (see "Networks" below), so
no manual command is needed after the first start. It remains a way to change the choice later:

    ssh $ROUTER 'meshtown access ifaces --config /etc/meshtown/config.json "$(uci -q get network.lan.device)" && /etc/init.d/meshtown restart'

By this point the node is running, so the command goes through its page (written immediately, and `socks.ifaces` takes effect after a
restart). It is also the way to bring back the "home network only" choice if it was changed.

Step 4: append to `/etc/sysupgrade.conf` (otherwise they will not get into the preserved list, `sysupgrade -l`, and will be lost on a firmware upgrade; without the `rc.d` lines the node does not start after an upgrade — verified on a real upgrade):

    ssh $ROUTER 'printf "\n/usr/bin/meshtown\n/etc/init.d/meshtown\n/etc/meshtown/\n/etc/rc.d/S99meshtown\n/etc/rc.d/K01meshtown\n" >> /etc/sysupgrade.conf'

The `/etc/rc.d/*` links are needed as separate lines: `sysupgrade -l` does not show them until they are added.

### Joining an existing network

Before the first `start` (or after `stop`): `/usr/bin/meshtown join --config /etc/meshtown/config.json <network key>`
(the key is written to `config.json`), then `start`. The key looks like `word-word--…` (the node prints it when it creates a network; the `mesh1-…` format is
accepted too). The key is not written into the script: it would be visible in `ps` and in the script file.

## Check

    ssh $ROUTER '/etc/init.d/meshtown status; pidof meshtown; logread -e meshtown | tail -20'
    ssh $ROUTER 'ls -l /etc/rc.d | grep mesh; sysupgrade -l | grep -E "mesh|rc.d"'

Expected: `running`, a process id, the line `[exit] working as an exit for network members …`; two links
and at least nine lines in the sysupgrade list (seven files and two links; there may be more files in `docs-store` — they grow with data). In `[status]` the node itself writes its memory once a minute
(`memory … KiB`; about 11–12 MB on a small router).

## Updating the binary

A node installed from the package feed is updated by the package manager (see the first section). For a manual install: you cannot write over a running file, so write to a temporary name and `mv`:

    cat dist/meshtown-linux-arm64-cli-<version> | ssh $ROUTER 'cat > /usr/bin/meshtown.new && chmod 0755 /usr/bin/meshtown.new \
      && mv /usr/bin/meshtown.new /usr/bin/meshtown && /etc/init.d/meshtown restart'

## Rollback

    ssh $ROUTER '/etc/init.d/meshtown stop; /etc/init.d/meshtown disable; sleep 15; pidof meshtown; \
      rm -f /etc/init.d/meshtown /usr/bin/meshtown; sed -i "/meshtown/d" /etc/sysupgrade.conf'

`stop` returns immediately, and the node shuts down in about 3 s more (no longer than 15 s, `term_timeout`): that is why the line above has
`sleep 15` and `pidof` before `rm` (it must answer empty; `rm` while the process is still alive is harmless — the file disappears after it exits). `/etc/meshtown/` is not removed by the rollback: it holds the network key and the node secret (after `rm -rf /etc/meshtown`
the node gets a new identity on the next install). `disable` removes both links in `/etc/rc.d`.

## Exit for other members

In the script `SOCKS_FLAG=` is empty: whether the proxy is on comes from `config.json`. The proxy on the router starts open only to the router itself (`socks.access: "localhost"`, the default). To let phones and computers at home use it, open the node page, go through the setup steps and click "Open for the home network", or set `"socks": {"access": "localnet"}` in `/etc/meshtown/config.json` and restart the service. Then the node listens on
`0.0.0.0:1080`, lets in only private addresses (loopback, `10/8`, `172.16/12`, `192.168/16`, `169.254/16`; tailscale addresses
`100.64/10` it cuts off itself) and serves a PAC at `http://<router address on br-lan>:1080/proxy.pac` (on the router —
`http://192.168.1.1:1080/proxy.pac`; the address in the PAC is the one the request came to). The proxy has no password, so the port
must not be opened on the wan zone: in the stock OpenWrt `firewall` wan has `input REJECT`, do not add rules for 1080;
check with `nft list chain inet fw4 input_wan` (no `accept` on 1080). To turn the proxy off again — `SOCKS_FLAG=--no-socks` and `restart`
(the flag overrides the file); `socks.enabled: false` in the file with an empty `SOCKS_FLAG` turns it off too.

In the script `EXIT_FLAG=` is empty: the exit role comes from `config.json` (`exit.enabled`, default — on), the router works as an
exit for other members and is switched by the checkbox "Allow other members to open sites through this computer"
on the node page. The exit does not let traffic through to the home network or the router itself: it cuts off private addresses.
`EXIT_FLAG=--no-exit` or `--exit` pin the role over the file, and the checkbox then has no effect.
A router installed earlier with `--no-exit` saved to the file may have written `exit.enabled: false` to `config.json` — then after
replacing the script and `restart`, turn the checkbox on the page on once.
**Networks (interfaces).** Which router networks are let in to the proxy (port 1080) and the node page (port 7777) is decided by the fields
`socks.ifaces` and `web.ifaces` in `config.json`, given as interface names (`br-lan`, `br-guest`, `eth0`). The field has
three states: no key or `[]` — automatic (on a router the node itself closes the default-route interface, that is
WAN, and opens the other private networks, including the guest `br-guest`); `["*"]` — no restrictions; `["br-lan"]` —
only those. An explicit list beats the automatic mode. This is the second line after the firewall: the port on WAN stays visible (the connection is
accepted and closed at once with no reply), but the node will not serve a request from WAN. Before starting, the init script calls
`meshtown access ifaces --if-unset <lan device>`: it writes `["br-lan"]` to both fields **only if the file has neither
of the two keys**; if there is no file (a fresh install), it creates it with these values; an unreadable file is left alone. So after an update the router will let in only `br-lan` (guest and WAN closed), while a choice made on the page
(the "ISP network" checkbox and others) and a restored automatic mode survive a restart. The script takes the device from
`uci -q get network.lan.device`, on OpenWrt 19.07 from `network.lan.type = bridge` (`br-lan`); if not found — no seeding,
the automatic mode works. Check:

    ssh $ROUTER 'uci -q get network.lan.device; cat /proc/sys/net/ipv4/ip_forward; logread -e meshtown | tail'
    ssh $ROUTER "jsonfilter -i /etc/meshtown/config.json -e '@.config.socks.ifaces' -e '@.config.web.ifaces'"

(`ip_forward` equals 1 — forwarding is on, the automatic mode treats the machine as a router; `logread` has the seeding line and the reasons
for refusals; the `jsonfilter` path depends on the file section — if empty, open the file and look for the `ifaces` keys). To return
the automatic choice — `meshtown access ifaces --config /etc/meshtown/config.json auto` and `restart` (the script is silent after that:
the key is in the file, even if `[]`); to return the person's "home network only" choice — the same command with `br-lan`
instead of `auto`; all networks — `'*'`.

For the exit role the script already has `limits nofile="4096 4096"` (one descriptor per proxied connection).

## Good to know

- **Log.** procd puts the node output into `logread` under the name `meshtown`. The new network key and the node page link with
  the token end up there in plain text (visible to everyone with access to the router). The log ring `logd -S 128`
  (128 KiB) at node idle (about 620 bytes a minute) holds about 3.5 hours; to wipe everything — `/etc/init.d/log restart`.
- **Restart on crash:** `respawn 60 5 10` — a 5 s pause; after 10–11 quick restarts in a row procd gives up on the node
  (`logread -e procd` writes "crash loop"), to bring it back — `/etc/init.d/meshtown restart`.
- **Clock.** The router has no RTC, after boot the clock may stand "in the past" until time sync runs.
  The node is alive at that time but TLS connections fail ("certificate not valid yet"); once the clock is right they
  work by themselves (within seconds, without `restart`). No time-change hook is needed. The same goes for no route by
  default at start: the node waits and joins the network itself once the route appears.
- **Node page** listens on port 7777 according to the access mode `web.access` (default `localnet` — on all IPv4
  addresses, lets in only `127.0.0.0/8` and private addresses `10/8`, `172.16/12`, `192.168/16`, `169.254/16`; `localhost` —
  only `127.0.0.1`). The mode is about access to the node settings, not to the network and traffic. Sign-in is by password; until one is set,
  the node prints the address with a one-time token for 10 minutes (in the log: `logread -e meshtown`), the permanent token
  `/etc/meshtown/web-token` does not reach the log; a forgotten password — `meshtown password reset`. Over HTTP the password,
  token and cookie travel in plain text on the local network. From a Mac with `localhost` — `ssh -L 7777:127.0.0.1:7777 $ROUTER`.
  **On a router the `localnet` mode relies on the WAN firewall** (the node relies on the firewall):
  the peer filter accepts any private address, and if the ISP gave the router a private WAN address (`10/8`, `172.16/12`),
  neighbors on the ISP network would pass the filter; OpenWrt drops inbound on WAN by default — do not open port 7777
  on WAN with a rule, and when in doubt set `meshtown access page localhost`.
  On top of the firewall the page is restricted by `web.ifaces` (the same as for the proxy, the "Networks" paragraph above): `br-lan` is let in,
  WAN and guest are not; a `web.ifaces` write from the page that would close the page itself for the current device is rejected (422).

## What changes on a live router and how to roll it back

A list for allow-listing: the installation touches only these places and nothing else.

| # | step | what changes | rollback |
|---|---|---|---|
| 1 | copy the binary | new file `/usr/bin/meshtown` (about 10 MB on the overlay) | `rm -f /usr/bin/meshtown` |
| 2 | copy the script | new file `/etc/init.d/meshtown` | `rm -f /etc/init.d/meshtown` |
| 3 | `enable` | two links `/etc/rc.d/S99meshtown`, `/etc/rc.d/K01meshtown` | `/etc/init.d/meshtown disable` |
| 4 | `start` | process `meshtown` (about 11.5 MB of memory, 7 threads, 18 descriptors, `nice 10`), UDP sockets on all addresses, SOCKS5 off by flag, node page — by access mode (default — all IPv4 addresses, peer filter: loopback and private networks); directory `/etc/meshtown/` (0700) with the config, `web-token`, `docs-store/` | `/etc/init.d/meshtown stop`; the directory stays (it holds the network key) |
| 5 | lines in `/etc/sysupgrade.conf` | five lines appended to the end of the existing file (not an overwrite) | `sed -i "/meshtown/d" /etc/sysupgrade.conf` |

Beyond the table, what the installation affects:

- **outbound connections** of the node: DHT bootstrap and peer connections (UDP), `ipwho.is` (country), DoH `1.1.1.1`;
- **log:** the new network key (first start) and the node page link with the token (every start) can be read in `logread`;
- **space on the overlay:** about 10 MB of binary plus a growing `/etc/meshtown/docs-store/` (its size and the NAND wear from the write rate
  were not measured);
- **joining an existing network** (`meshtown join`) writes the network key to `config.json` — a separate step;
- **rebooting the router** to test autostart is a separate permission, not needed for the installation.

Unchanged: `/etc/config/*`, `uci`, firewall, routes, interfaces, packages (`opkg`), other services; the installation needs neither
`reboot` nor `sysupgrade`. A reboot is needed only to test autostart, which is a separate permission. The installation adds no firewall
rules; whether the node works behind the router firewall without port forwarding was not tested. The exit role is on by default; the load
on the router in this role has not been measured yet.
