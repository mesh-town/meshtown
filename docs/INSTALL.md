# Installing Mesh Town

Builds for every supported system, how to update, and what to check before running a download. The short version is
in [README.md](../README.md#install).

## macOS

**Homebrew** (Apple silicon and Intel) is the easiest way:

```sh
brew install mesh-town/meshtown/meshtown
```

Then `meshtown` opens the tray app, and `meshtown` with arguments works as the command-line program. The formula also
installs `meshtown-cli`, the node in a terminal until Ctrl+C. To update: `brew upgrade meshtown`.

Homebrew 7 refuses formulas from a tap it does not trust yet; the full name above trusts the formula. If you prefer
`brew tap mesh-town/meshtown` first, run `brew trust mesh-town/meshtown` before `brew install meshtown`.

**Without Homebrew**, download [MeshTown-macos-arm64.zip](https://github.com/mesh-town/meshtown-dist/raw/main/macos/MeshTown-macos-arm64.zip)
(Apple silicon) or [MeshTown-macos-x86_64.zip](https://github.com/mesh-town/meshtown-dist/raw/main/macos/MeshTown-macos-x86_64.zip)
(Intel). The builds are signed ad hoc and are not notarized, so Gatekeeper blocks the first launch of a zip
downloaded in a browser. Either open **System Settings → Privacy & Security → "Open Anyway"**, or remove the
quarantine mark from a terminal:

```sh
xattr -dr com.apple.quarantine /path/to/MeshTown.app
```

Homebrew does not set this mark, which is why no window appears there.

## Windows, Linux, Android

Download from [meshtown-dist](https://github.com/mesh-town/meshtown-dist); the files without a version in the name are
always the latest build.

| System | File |
|---|---|
| Windows, with the tray icon | [meshtown.exe](https://github.com/mesh-town/meshtown-dist/raw/main/windows/meshtown.exe) |
| Windows, console only | [meshtown-cli.exe](https://github.com/mesh-town/meshtown-dist/raw/main/windows/meshtown-cli.exe) |
| Linux, x86_64 | [meshtown-linux-x86_64.tar.gz](https://github.com/mesh-town/meshtown-dist/raw/main/linux/meshtown-linux-x86_64.tar.gz) |
| Linux, aarch64 | [meshtown-linux-aarch64.tar.gz](https://github.com/mesh-town/meshtown-dist/raw/main/linux/meshtown-linux-aarch64.tar.gz) |
| Android | [meshtown.apk](https://github.com/mesh-town/meshtown-dist/raw/main/android/meshtown.apk) |

On Debian and Ubuntu, prefer the apt package: it updates with the system ([Debian, Ubuntu (apt)](#debian-ubuntu-apt)).
Console-only Linux builds are `meshtown-cli-linux-x86_64.tar.gz` and `meshtown-cli-linux-aarch64.tar.gz` in the same
folder. The folder of each system keeps the four latest versions too, with `SHA256SUMS` and notes. The Windows binary
is not signed; SmartScreen may warn on the first launch.

The same names on every system: `meshtown` is the build with the tray icon, `meshtown-cli` is the node in a terminal
until Ctrl+C. They share their settings, so do not run both at once.

## Debian, Ubuntu (apt)

The package `meshtown` (amd64 and arm64) comes from a signed apt repository,
`https://mesh-town.github.io/meshtown-dist/linux/debian`. Download the `.deb` once; it adds the repository and its key:

```sh
wget https://mesh-town.github.io/meshtown-dist/linux/debian/meshtown_amd64.deb   # or meshtown_arm64.deb
sudo apt install ./meshtown_amd64.deb
```

Later updates come with `sudo apt update && sudo apt upgrade`. The package installs the service `meshtown` but leaves it
stopped; to run the node in the background, `sudo systemctl enable --now meshtown` (settings:
`/etc/meshtown/config.json`). To join an existing network first, run
`sudo MESHTOWN_KEY=<key> meshtown run --config /etc/meshtown/config.json --save-config`, stop it with Ctrl+C, then enable
the service. On a desktop with the tray, do not enable the service: two nodes on one machine share their ports.
`sudo apt remove meshtown` (or `apt purge`) removes the package; `/etc/meshtown/` with the network key stays.
The file `/etc/apt/sources.list.d/meshtown.sources` belongs to the package: edits made by hand are overwritten when the package is updated.

## Routers

- **OpenWrt** (24.10 with `opkg` and 25.12 with `apk`). On the router, as root:

  ```sh
  wget -O- https://mesh-town.github.io/meshtown-openwrt/install.sh | sh
  ```

  The script finds the package manager of the router itself, adds the signed package feed and installs the package
  `meshtown`; the service starts on boot. Architectures: 64-bit ARM, 32-bit ARM Cortex and x86_64; MIPS, ARMv5/v6 and
  32-bit x86 routers are not covered yet.

  To update, run the command for your OpenWrt version only (`grep DISTRIB_RELEASE /etc/openwrt_release` shows it):

  - OpenWrt 24.10 (`opkg`): `opkg update && opkg upgrade meshtown`
  - OpenWrt 25.12 (`apk`): `apk update && apk upgrade meshtown`

  Manual installation and details: [platforms/linux/build/openwrt/README.md](../platforms/linux/build/openwrt/README.md).

- **Keenetic** (router with Entware installed; MIPS and aarch64 models). Over SSH on the router:

  ```sh
  wget -O- https://mesh-town.github.io/meshtown-keenetic/install.sh | sh
  ```

  The script reads the Entware architecture, adds the package feed and installs the package `meshtown`; the service
  `/opt/etc/init.d/S99meshtown` starts on boot. To update: `opkg update && opkg upgrade meshtown`. Not tested on a live
  KeeneticOS router yet.

## Release archives

[GitHub Releases](https://github.com/mesh-town/meshtown-dist/releases) have versioned archives for macOS (`MeshTown.app`
in `MeshTown-macos-arm64-<version>.zip` and `MeshTown-macos-x86_64-<version>.zip`), Linux (x86_64 and aarch64, static
musl builds, `.tar.gz`), Windows (x86_64, `.zip`), console-only builds for 32-bit ARM and MIPS Linux, and
`SHA256SUMS`. Every platform has a `meshtown-cli-…` archive without the tray; the other archives are the tray builds.
Check an archive against `SHA256SUMS` (`shasum -a 256 -c SHA256SUMS --ignore-missing`) before running it.

The node page tells you when a newer version is out.
