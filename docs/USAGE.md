# Mesh Town usage reference

This is the reference for running a node: command-line options, the settings file, joining a network, the proxy and
exit, site checks, the node page and the log. For a short introduction see [README.md](../README.md).

## Running the node

Without a subcommand `meshtown` picks one itself: `tray` in the desktop build, `run` in a build
without it (`--no-default-features --features net-hyper`). An explicit subcommand is never changed.

The command is named the same on every system: `meshtown` is the build with the tray, `meshtown-cli` is the build
without it (the node runs in the terminal until Ctrl+C). The examples below use `meshtown`; in the build without the
tray type `meshtown-cli` instead, for example `meshtown-cli run …`. The hints the program prints name the command the
way it was started.

```sh
meshtown keygen our gaming room       # network key: label + random part
meshtown keygen                       # the same without a label
meshtown keygen --raw                 # machine key mesh1-… for scripts
meshtown ifaces NAME... [--if-unset]  # which networks the node accepts connections from (see below)
MESHTOWN_KEY="our-gaming-room--finch-unwary-k7m2q9x4rb8vhn3tz01g5" \
  meshtown run [--key KEY] [--invite TEXT] [--config PATH] [--no-config] [--save-config] \
  [--relay-only] [--no-relay] [--probe-interval SECONDS] [--probe-full-cycle] [--domain SITE]... \
  [--check-interval SECONDS] [--check-doh ADDR:PORT] [--provider-dns ADDR:PORT] [--checks] [--no-checks] \
  [--socks-port N] [--socks-access localhost|localnet] [--socks] [--no-socks] [--exit] [--no-exit] [--block-domain SITE]... \
  [--web-port N]
```

### The network key

`--key` or `MESHTOWN_KEY` accepts a key of the form `label--word-word-group` or a machine key `mesh1-…`. If neither
is given, the node reads the `network.key` field of the [settings file](#settings-file). If there is no key there
either, `--no-config` makes this an error right away; otherwise the node starts anyway and waits for you to join a
network from the page (see [First run without a key](#first-run-without-a-key)).

The key is split at the first `--`: the left part is the label, the right part is the random part, which is two words
from the EFF list plus a group of 21 characters (digits and letters, except `i`, `l`, `o`, `u`).

- **The label does not affect the key.** It is a caption for telling your networks apart, so a typo in the label does
  not lead to another network. A label without a random part is an error; a random part without a label is fine.
- In the random part, case, spaces instead of hyphens and underscores do not matter, and the letters `o`, `i` and `l`
  are read as the digits `0` and `1`. A key copied from a screen with a mixed-up letter still opens the same network.
- A word that is not in the list is an error that names the word; this is how a typo made while copying is caught.
- The random part has 130.8 bits of strength (two words, 25.8 bits, plus 21 characters, 105 bits). The key is
  derived instantly: a fast hash, no Argon2id and no large memory peak.
- Prefer `MESHTOWN_KEY` or the settings file over `--key`: an argument is visible in the process list.

### Transport mode

Reserved: the transport does not use relays yet; the setting has no effect. `--relay-only` and `--no-relay` (the settings field
`transport.mode`: `direct_and_relay`, `relay_only` or `no_relay`) only change the `mode:` word in the start-up log
(`direct + relay`, `relay only`, `no relay`). Leave them at the default.

## Tray

In a build with the tray (the macOS `.app` and the `gui` builds for Linux and Windows) `meshtown` without a subcommand
starts the tray icon (the `meshtown-cli` build has no tray: `meshtown-cli run …` keeps the node in the terminal). The tray runs the node and opens the node page from its menu. On macOS and Windows the node's
log goes to `meshtown.log` and the tray's to `meshtown-tray.log` in the settings directory; Linux does not write
these files. For the macOS package itself see [platforms/macos/build/README.md](../platforms/macos/build/README.md).

## Settings file

The node settings live in `config.json`: the network key, the node secret (the permanent node identifier is derived
from it) and the run parameters. Each field is taken in the order "default → file → flag", **per field, not per
file**: `--socks-port 1081` changes only the SOCKS5 port and everything else comes from the file. A flag applies to
one run and does not change the file.

Where the file is looked for (the first rule that applies wins):

1. `--config PATH`, as given;
2. the `MESHTOWN_CONFIG` variable, as given (the flag is stronger than the variable);
3. `config.json` next to the program, if it is there (portable mode: a flash drive, a directory on a router);
4. otherwise the standard place: Windows `%APPDATA%\meshtown\config.json`; macOS
   `~/Library/Application Support/meshtown/config.json`; Linux as root `/etc/meshtown/config.json` (`/opt/etc/meshtown/config.json` when the program is in `/opt/bin`, as with Entware); Linux as an
   ordinary user `$XDG_CONFIG_HOME/meshtown/config.json`, or `~/.config/meshtown/config.json` if that is unset.

What the flags and the subcommand do:

- First run with no file: the node creates it (in the standard place or at `--config`) with **defaults and a new node
  secret**, not with the flags you typed. Flags describe this run, not permanent settings. A restart takes the secret
  from the file, so the node identifier stays the same.
- `--save-config` writes the flags you typed to the file (only them; other fields are left alone) and keeps running.
  A `--key` of another network is not a flag to save: it is added as a separate network (see the invite section).
- `--no-config` neither reads nor writes the file and works on defaults and flags. It cannot be combined with
  `--save-config` or `--config`.
- `meshtown config-path` prints the path to the settings file and runs nothing. At a normal start the path is also the
  first line of the log: `config: <path> (…)`.

The file is secret because it holds the network key and the node secret. The node creates it with mode `0600`, and the
directory it creates with `0700`. Field formats, writing and versions are described in
[CONFIG-FILE.md](CONFIG-FILE.md). Domain lists, cached member addresses and the shared network document are also stored
on disk next to the settings file.

## First run without a key

If the key comes from neither a flag, a variable nor the file, the node still starts as long as the node page comes
up. When the page is up the node prints the line `Mesh Town page: <address> — sign-in link…` (the address carries a
one-time token while no password is set), then two more lines, and waits:

    [node] You haven't joined a Mesh Town network yet. Open the Mesh Town page (its address is printed above) and click "Join with an invite" or "Create a new network".
    [node] Got an invite? You can also join from the terminal: press Ctrl+C, then run meshtown run --invite "<invite>"

Join through the page with "Join with an invite" or "Create a new network" on the "Home" tab. The node then starts
for real, without restarting the process. The node never creates a network on its own: a key appears only through a
button on the page, `--invite` or `keygen`.

With `--no-config` and no key it is an error right away, without waiting. It is also an error right away if the page
did not come up (for example, the `--web-port` port is busy) and there is no key.

## Node name and invite

**Node name.** Every node has a name that neighbors see in the log (`[member]`) and on the node page (the entry with
`self: true` in `peers[]` of `GET /api/state`). By default the name is two words from a built-in list, for example
`finch-unwary`. At the first run, when there is no settings file yet, the node picks them at random and writes them to
the file it creates. While the file stays in place, the name does not change. If the file is unavailable or unreadable
(`--no-config`, a corrupt file), the node derives the name from the key on every run and does not save it.

Set your own name with the `node.name` field of the settings file: up to 64 characters, non-Latin scripts are fine,
spaces at the edges are trimmed. An empty name entered on the page is replaced with a random one. A name with a control
character or a line break is not accepted: the node logs why and uses the default name for this run without touching
the file. Names are not unique, so two nodes may end up with the same one.

**Invite.** On a running node type `invite` in the console (or `invite <for whom>`, a caption for yourself of up to 64
characters). The node prints a `mesh-invite2-…` string several hundred characters long. It is **single-use and valid
for a day**. The string does not contain the network key. It holds a one-time secret, the network label and the
addresses of up to five members; the new node receives the key from a member once that member has verified the secret.
Whoever presents the string first joins, and a second attempt is refused with "another node has already joined with
this invite". An unused string stops being valid after a day and is removed from the shared network document (nodes
clean up their entries once an hour and at start). Hand the string over in person anyway.

The new node joins without waiting for a DHT lookup:

    meshtown run --invite mesh-invite2-…
    MESHTOWN_INVITE=mesh-invite2-… meshtown run      # the same; the string is not visible in the process list

- If none of the members named in the string is online, the node logs "waiting for network members" and keeps
  retrying, for up to a day. While it waits, the page and the tray are not up; join from the page if you need the
  interface at once.
- The node writes the received key to the settings file itself, even without `--save-config` (only `network.key`
  changes), so after a restart it joins the network without an invite. With `--no-config` there is no file: the node
  warns that the key is not saved and **does not print it**. It works until a restart, and then you need a new invite.
- With `--key` of the same network the invite is not spent; with a key of another network it is an error.
- If the settings file already holds a key of another network, the invite is spent and the received key is saved as a
  separate network beside the main one (`nets/<id>/config.json` in the folder of the settings file). The main
  network, its key and its file stay as they were, and the node starts both. An invite of a network the node already
  has is not spent.
- The same goes for `--key` of another network without an invite: the network is added beside the main one and the
  file keeps its key. A key of a network that is already saved is simply used. One node holds up to 8 networks;
  past that the run is refused before it goes to the network, and the invite is not spent.
- A network added this way runs with the launch flags of the main network, except `--key`.
  It starts with the sites of the main network's list and the ones typed with `--domain`;
  an added network joined by an invite also remembers the peer addresses from the invite, so it finds them at start.
- The same join is available on the node page, in the "Invite from a network member" field. The answer comes at once,
  and "waiting for members" is shown under the field. That field accepts both a plain key and an invite, told apart by
  the prefix.
- One confirmation from a network member admits a new node. Nodes of an older version wait for a second
  confirmation and refuse such a node with `not admitted` until they are updated.

Console command:

```
invite [for whom] — a new single-use invite string for this network (valid for a day)
```

On the page, the "Invite a member" button on "Home" puts the same string on the clipboard.

## Managed networks

A network has one of two types, chosen when it is created and never changed afterwards.

- **Peer** (the default): every member can invite people, and removing a member takes the votes of two members.
- **Managed**: administrators decide who is in and who is out. Only an administrator can invite a person, remove a
  member, make another member an administrator or delete the network. Everyone else sees the list of members and
  uses the network as usual.

The creator of a Managed network is its first administrator and the *root*: the root keeps the role for good and
cannot be made a regular member. A key or an invite of a Managed network is not understood by a program that does not
know Managed networks, so such a program refuses it instead of joining a network of another kind.

**Create.** On the page, "Add a network" (or "Join or create a network" when there is none) asks "Who can invite and
remove members?" and offers *Peer network* (selected) and *Managed network*. The tray menu has the item
`Add a network…`, which opens the same form. A node that joins by a key or an invite sees the type of the network
before it joins.

**Invite.** Only an administrator can issue an invite to a Managed network; for anyone else the button is replaced by
a note naming the administrators, and the tray item is disabled. An invite that was not made by an administrator is
refused when it is redeemed.

**Administrators.** In the member list an administrator opens the `⋯` menu of a row and picks `Make admin` or
`Make regular member`. An administrator can give up their own role (`Give up admin rights`) unless they are the only
one; the last administrator cannot leave or give up the role, and is offered to make another member an administrator
or to delete the network. A Managed network without any administrator cannot be managed, so make a second one soon.

**Remove.** An administrator picks `Remove from network…` for a member. One administrator's decision is enough, with
no voting; the removed node is disconnected from all members and needs a new invite to come back. The removed node
sees which administrator removed it.

**Delete for everyone.** `Delete network for everyone…` (the administrator's own row) asks to type the network name.
All members leave the network, this computer too; it cannot be undone. A member that was offline leaves the network by
itself when it learns about it.

**What a regular member sees.** The network name, the line `Managed network · Admins: …`, the `Admin` badge on rows,
and a greyed `Remove from network (admins only)`. A member can still leave the network and block a node on this
computer. The HTTP shape of all this is in [WEB-API.md](WEB-API.md) (sections 1.3, 1.11 and, in [WEB-API-members.md](WEB-API-members.md), 1.12).

## Domain list

Every node has its own domain list. It is kept only on this node and is not passed to other members. The results of
site checks (with the site name) still reach all members; this is how the member columns on the "Site checks" tab
work.

Add a domain at start with `--domain` (it can be repeated):

```sh
meshtown run --domain example.com --domain example.org
```

On a running node, commands are read from stdin, one per line:

```
domains add example.com
domains remove example.com
domains list
```

Domains are accepted only in ASCII form: a non-Latin domain is entered as `xn--…`. Adding and removing affect only
this node. With a settings file the list is kept on disk next to it and survives a restart; without one
(`--no-config`) it lives in memory and is lost on stop.

## Proxy and exit

The node runs a local proxy on `127.0.0.1:1080`: SOCKS5 and HTTP on one port, told apart by the first byte. Change the
port with `--socks-port`, turn the proxy off with `--no-socks`. The HTTP side understands `CONNECT host:port` and a
request with an absolute URI `http://…`; one HTTP connection serves one request. A browser or `curl` asks for a
domain, the node picks a path and moves the bytes:

```
curl -x socks5h://127.0.0.1:1080 https://example.com/
curl -x http://127.0.0.1:1080 https://example.com/
```

`socks5h` (with the `h`) is recommended: the node then resolves the name itself. Without it `curl` resolves the name
with the system resolver and hands the node a ready address. Either way the path is chosen by the same rule.

**Path choice.** If the list of selected exits is empty, the node connects by itself. The list is the settings field
`route.exits`, an array of node identifiers in priority order. If it is not empty, the node tries the candidates in
list order (never itself; only members that are online and work as an exit). It moves on to the next only if the
stream to the chosen one did not open; any response code received from an exit is final. If the list is not empty and
no candidate fits, the program gets a refusal with code 4 ("host unreachable"). The list is edited through
`PUT /api/config` (`config.route.exits`) and applies without a restart; the node page and the tray change it the same
way. There is no automatic choice by country or operator.

**Working as an exit.** Independently of its own `route.exits`, a node serves other members as an exit. It accepts a
request only from someone who has passed the handshake, and serves any domain on any port if the address is public.
Private, loopback, link-local, CGNAT and multicast addresses and the member's own address are refused (code 7). Turn
the exit off with `--no-exit`, with `exit off` in the console, or on the node page.

`--socks-access` sets who can reach the proxy: `localhost` (this computer only, the default) or `localnet` (the
local network too). `--probe-full-cycle` is a testing flag: it tests speed with 1 MiB to all members, not only the
chosen exit.

`meshtown ifaces NAME...` sets which networks (interfaces) the node accepts connections from, for the proxy and the
page at once (`socks.ifaces` and `web.ifaces`). `NAME` is an interface name such as `br-lan`, the single word
`auto` (automatic) or `*` (no restriction). If the node is running, the command applies the change through
`PUT /api/config`; otherwise it writes the settings file (`--config PATH`; `--web-port N` names the running node's
page). With `--if-unset` it writes only when the file has neither field yet, which suits autostart scripts.

`--block-domain SITE` (repeatable) only feeds the site checks (below): it makes the node treat the domain as blocked
without checking it. It does not affect the path choice.

Console commands:

```
socks              — what the SOCKS5 server listens on and whether the node works as an exit
route <domain>     — which path the node would pick now, without opening a connection
get <domain>       — request https://<domain>/ through its own SOCKS5
exit on | exit off — turn working as an exit on or off
```

## Site checks

The node checks every domain of the list by itself and publishes the result to all members. A check goes in steps:
DNS (its own, over HTTPS), TCP, TLS with the real server name, HTTP. The result is one of these classes: `opens`,
`blocked: address spoofed`, `blocked by ISP`, `blocked for this country`, `blocked as a bot`, `site is down`.

Console commands:

```
check example.com
checks
```

The system resolver is not used. The address of the DoH server is given as a number (`--check-doh`, `1.1.1.1:443` by
default), and the domain address is taken from its answer. An address from the fake-IP pool (`198.18.0.0/15`) is
rejected by the check and no connection is opened.

`--provider-dns 192.168.1.1:53` adds a comparison with your provider's resolver: if the answers differ, the class is
`blocked: address spoofed`. Addresses from the provider's answer are used for nothing else.

`--no-checks` leaves only the domain list.

## Published ports

A member can publish a port of a program that runs on this computer and listens on `127.0.0.1`: a web page, an SSH
server, a file share. All members of the network, or only the members you select, see the port and can open it. It is
not shown to anyone else and does not open for them.

Everything is on the "Ports" tab of the node page, in two cards. In "My published ports" you enter a port (1 to
65535), an optional name (up to 64 characters) and "Shared with": "All members" or "Selected members", then press
"Publish". Each row shows a status: "responding", "not responding" (nothing listens on the port) or "not checked yet".
"Edit" changes the name and the audience, "Unpublish" removes the publication.

"Ports shared with you" lists the ports other members published to you. Click an address to open it in a browser set up
to use Mesh Town; type addresses with `http://`. For other programs press "Open on this computer": the node starts a
TCP listener on `127.0.0.1:<number>`, by default the member's port plus 2000 (80 becomes 2080); you can type another
number. "Close" stops the listener. If the number is busy, the row says to choose another number. Browsers refuse some
ports on principle; such a row is marked "not for the browser" and can only be used through "Open on this computer".

The same data lives in the settings file, per network: `ports.published` is a list of `{port, name, for}`, where `for`
is `"all"` or a list of full node ids, and `ports.opened` is a list of `{node, port, local}`. Both hold up to 64
entries; `port` is unique in `ports.published`, `local` is unique in `ports.opened` and across all networks of the
node. Opening a `local` port that another network of the node already uses is refused with 422 and the text
`local port L is already opened in network "<name>"`. See [CONFIG-FILE.md](CONFIG-FILE.md) and
[WEB-API-config.md](WEB-API-config.md). There are no command-line flags for ports: use the page or the settings file.

## Node page

The node page has a "Home" tab with the essentials for a simple setup (state, joining a network, exit choice, proxy
hint, node name, permission to be an exit for other members) and six more tabs: "Members", "Site checks", "Ports",
"Diagnostics", "Log" and "Settings". The address changes by hash (`#peers`, `#settings`, and so on); on a narrow screen
the tabs become a horizontal strip. The "Ports" tab is described in [Published ports](#published-ports).

**Removing a member that is offline.** On the "Members" tab, a member that is not connected has a "Remove from network…"
item. It differs from "Ban from network…": a ban forbids the node to enter and is held by the votes of the whole
network, while removal only cleans the list of nodes that have not been around for a long time and does not stop
them from coming back. Removal is a vote of your node and passes at the same threshold as a ban: one vote while at
most two nodes are active, otherwise two. A pending proposal shows to the others as "Confirm removal…" and is
withdrawn with "Undo removal". Removed nodes are listed in the group "Removed from the member list"; nodes offline
for more than a week are collected in "Offline for over a week" and are removed one at a time, nothing is removed
automatically. A removed node sees the card "This computer was removed from the member list" and returns with a new
invite, keeping the same computer key.

The page is a Vue application. Its sources are in `core/web_gui/ui/vue`, and the built result is one file, `core/web_gui/ui/bundle/app.js`,
which is committed to the repository (rebuilt by `make web-build`, checked by `make web-bundle-check`), so `cargo
build` does not need Node. The listener is built into the binary (`core/web_gui/mod.rs`, HTTP/1.1 on axum) and starts with the
node. The port is `--web-port N` (default 7777, the settings field `web.port`). To turn the page off entirely, set
`web.enabled: false` in the file; there is no flag for this. The node keeps a permanent access token in the `web-token`
file next to the settings file (the tray and scripts read it), but does not print it; at start it prints a link with a
temporary token instead.

### Signing in, access and password reset

- **First sign-in.** While no password is set, the node prints an address with a temporary token at every start. The
  token is single-use and lives 10 minutes, for example `http://127.0.0.1:7777/?token=…` (in `localnet` mode there are
  two addresses: this computer and the one in the local network). Open it, and the page exchanges the token for a
  session and asks you to set a password (8 characters or more) and choose the access mode. A second attempt with the
  same link fails (the API answers `bad_token`); after that you sign in with the password. The tray opens the page with
  the permanent token and needs no password. The address with the temporary token also lands in the system log of a
  service (`journalctl`, `logread`): whoever can read it in the first 10 minutes can set the password before the
  owner. On a shared machine, set the password at once.
- **Access** (`web.access`) decides who can open the page. `localhost` means only this computer; `localnet` (the
  default) also admits devices of the local network (private addresses `10/8`, `172.16/12`, `192.168/16`,
  `169.254/16`). This is only about access to the node settings, not to the Mesh Town network or its traffic. Change
  it on the page or with `meshtown access localhost|localnet [--config PATH] [--web-port N]`. Narrowing applies at
  once, widening after a restart.
- **Forgot the password.** `meshtown password reset [--config PATH] [--web-port N]` deletes the `web-password` file,
  ends all sessions and invalidates all earlier temporary tokens, then prints an address with a new temporary token
  (valid 10 minutes, once). On a stopped node it just deletes the file, and the address appears at the next start. On
  a running node the command talks to it with the permanent token from `web-token` next to the settings file.
- **Brute-force protection.** Five wrong passwords or tokens in a row from one address cause a one-minute block, and
  each further failure doubles it, up to 15 minutes. A session lives 7 days, and a node restart signs everyone out.
- **Limits.** The page is served over HTTP without TLS: on a local network the password, token and cookie travel in
  clear text, so on an untrusted network keep `localhost`. A known limitation: a page port published to the network
  through `ports.published` reaches the page from `127.0.0.1` and bypasses the access mode, so do not publish the page
  port. On a router, `localnet` relies on the WAN firewall (see [platforms/linux/build/openwrt/README.md](../platforms/linux/build/openwrt/README.md)).

### API and demo node

The HTTP API contract is [WEB-API.md](WEB-API.md): addresses, formats, error codes. The config schema is section 2 of
[WEB-API-config.md](WEB-API-config.md), and [WEB-API-embedding.md](WEB-API-embedding.md) (section 3.1) describes where
the listener lives inside the program. The page, the demo node and the built-in listener must match it exactly.

The demo node (`core/examples/web_demo/`) is a light way to try the page without starting a real node:

    make web-demo
    # http://127.0.0.1:7777/?token=dev

The full browser battery (Playwright, including an invite, joining with it and the "exit" flag of a foreign peer in
the node table):

    make web-check                            # all cases
    make web-check ONLY=10-identity           # one case (several: ONLY="1 10-identity")

## Reading the output

Every line is `+<seconds>s [tag] text`. The tags:

| Tag | What it is about |
|---|---|
| `[node]` | version, network, transport mode, what the node listens on, start and stop, the node name and identity (persistent, from the settings file, or temporary, with the reason on a separate line), joining by an invite |
| `[web]` | the node page: sign-in addresses (with a temporary token while no password is set), problems with the password file or a temporary token; the permanent token never appears here |
| `[config]` | the settings file: where it was taken from, what is in it, which fields were rejected, what flags override |
| `[invite]` | invites: joining progress ("waiting for network members", the pause between rounds), a refusal with a reason, writing the key to the settings file, addresses from an invite; the invite string itself on the `invite` command |
| `[net]` | the network choice report when it differs from the default (transport, discovery, sync) |
| `[hyper]` | the transport layer: connection closes with a reason, the network-change watcher (network lost or changed, sessions closed and resumed) |
| `[hyperdht]` | discovery on the DHT: records read from it and the number of members found |
| `[cache]` | the saved lists on disk: the address cache and member names (read and write problems), and removal of members not seen for a long time |
| `[auth]` | the handshake with a neighbor by the network secret: success or refusal |
| `[member]` | a verified member appearing or disappearing, with the short id, the neighbor's name in quotes if it sent one, and the member count |
| `[path]` | the chosen connection path to a member and the round-trip time |
| `[gossip]` | the group channel between members: inviting neighbors to it, re-invites |
| `[probe]` | the speed probe to a member: size, path, outcome |
| `[docs]` | the shared network document (checks, exit flag, country, program version): syncing with members, writing the node's own name |
| `[geo]` | detecting the node's own country (postponed when there is no network, or failed) |
| `[domains]` | local edits of the list (the `--domain` flag, console) and the output of `domains list` |
| `[check]` | the result of checking one domain, the node's own or a neighbor's; errors of the `check` command |
| `[checks]` | a summary of checks, the schedule state, problems reading the domain list or results |
| `[console]` | errors parsing stdin commands; stdin closed (commands unavailable) |
| `[socks]` | the SOCKS5 server: listen address, connection errors, and the result of each served request (path, outcome, bytes both ways, time) |
| `[http]` | the HTTP side of the proxy (and the PAC file it serves): request results and rejected requests |
| `[route]` | the chosen path for a domain: itself, a member, or a refusal with a reason |
| `[exit]` | working as an exit: whose requests were accepted and how they ended |
| `[get]` | the result of the `get` command |
| `[port]` | port forwarding between members (`ports.published` and opened local ports): opened, closed, failed |
| `[guard]` | the memory and stall watchdog: a periodic summary, a heap profile, a stall or exit report |
| `[tray]` | the tray app: node start and stop, errors |
