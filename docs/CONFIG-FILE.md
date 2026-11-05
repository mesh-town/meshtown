# Node settings file on disk

This document covers where the file lives, how the node reads it at startup, whether the file or the command line
wins, and how the node rewrites the file when settings change from the page. The document's composition, field types
and transport between the page and the node are in `WEB-API.md`, section 2 "Config schema" and sections 1.6–1.7.

**One schema for both documents.** The file stores the same `config` document that the page gets from
`GET /api/config` and sends in `PUT /api/config`. The node converts nothing: it reads the file as `config` and
writes `config` as the file. Schema fields are edited in `WEB-API.md`; the file's rules are here.

---

## 1. Where the file lives

The node looks next to the binary first, then in the host's standard location. It creates a new config in the
standard location. The first rule that applies wins:

| № | Rule | Path |
|---|---|---|
| 1 | flag `--config <path>` | as given |
| 2 | variable `MESHTOWN_CONFIG` | as given |
| 3 | **next to the binary** | `<binary directory>/config.json` |
| 4 | Windows | `%APPDATA%\meshtown\config.json` |
| 4 | macOS | `~/Library/Application Support/meshtown/config.json` |
| 4 | Linux, uid 0 (a service on a router) | `/etc/meshtown/config.json` |
| 4 | Linux, uid 0, binary in `/opt/bin` (Entware) | `/opt/etc/meshtown/config.json` |
| 4 | Linux, regular user | `$XDG_CONFIG_HOME/meshtown/config.json`, otherwise `~/.config/meshtown/config.json` |

Notes:

- **Entware routers** (for example Keenetic) keep the root file system read-only and install programs
  under `/opt`. A root process whose binary is in `/opt/bin` therefore uses `/opt/etc/meshtown/`, not
  `/etc/meshtown/`, so a command typed in a shell finds the same settings as the service.
- **A file next to the binary is portable mode** (a flash drive, a directory on a router). It is looked up first,
  so a portable copy always wins over one installed in the system.
- **A new config is created only in the standard location** (rule 4), even if the binary's directory is
  writable: the binary may sit in `/usr/bin` or `Program Files`, and the node must not litter there.
  To set up a portable file, put `config.json` next to the program (or copy it from another machine); the node
  picks it up on the next start. An explicit `--config` is the exception: the file is looked up and created at
  that path.
- **The state directory is the directory that holds `config.json`.** There is no separate setting for it. With
  `--config` you choose the directory too; with a config next to the binary, the other state files land there.
- If the directory does not exist, the node creates it with the file (`mkdir -p`, mode `0700` on Unix).
- The node prints the chosen path as the first startup line (`config: <path> (read)`) and gives it to the page
  in `config.file.path` (section 5). `meshtown config path` prints the same path and starts nothing.

### 1.1 What is in the state directory

```
<state directory>/
  config.json                    settings, network key and node secret, mode 0600 — this document
  web-token                      permanent token of the web page, mode 0600 (`core/web_gui`); read by the tray and scripts
  web-password                   hash of the page password, mode 0600; no file — no password (section 8.2)
  peers-cache.json               last working addresses of network members, mode 0600; auxiliary, can be deleted
  dht-nodes.txt                  DHT nodes this node knew, read at start next to the bootstrap nodes, written every few minutes and on stop, mode 0600; auxiliary, can be deleted; one file per network slot
  peer-names.json                last known names of network members, mode 0600; auxiliary, can be deleted
  config.json.bad-<date-time>    an unreadable config, set aside before being overwritten
```

- **`config.json` is a secret file.** It holds the network key and the node secret, so mode `0600` and
  directory `0700` are required: the file gives access to the whole network and lets someone impersonate this node.
- **There is no separate node identity file.** The secret lives in `config.json` (section 4); the old per-network
  `node-<network tag>.key` files are not used.
- **The page token** is a separate file, not a config field. The page reads and rewrites the config as a whole and
  must not receive the token; the "New token" button changes it bypassing the settings. The web server creates
  the file.

---

## 2. Priorities: the command line is stronger than the file

The node takes each field by the rule **default → file → command line**, per field, not per document:
`--socks-port 1081` overrides only `socks.port`; the other file fields stay in force.

- **"Set on the command line" means a person typed the flag**, not that the flag has a value. Today half the flags
  have `default_value_t` (`core/cli/args.rs`), so a typed value cannot be told from the default. With the file
  connected, all such flags become `Option<T>` without `default_value_t`, and the defaults move to one place: the
  table in section 3. The check is `ArgMatches::value_source(..) == Some(ValueSource::CommandLine)`.
- **A flag does not change the file.** It acts on the current run only. After a restart without the flag, the file
  value applies.
- **The page shows overrides.** The node lists fields overridden by the command line in `cli_overrides`
  (section 5). The page shows the effective value, marks the field "set by a flag at startup" and **allows
  saving**: the saved value goes to the file and takes effect on the next start without the flag. The page neither
  silently accepts an edit that does nothing nor forbids it, because a person may configure the node for the future.
- **Only two environment variables exist**, both about paths and the key: `MESHTOWN_CONFIG` and `MESHTOWN_KEY`
  (`core/config/config_file/overrides.rs`). There is no per-field layer: the page cannot show it clearly.
  `MESHTOWN_KEY` counts as set explicitly, like a flag, and overrides the file key. `meshtown join <key>` stores a key.
- **`config set <path> <value>`** and **`run --set <path>=<value>`** change a stored field from the command line.
  - `config set` writes one field through the running node (`PUT /api/config`, same checks and rules as the page)
    or, when no node runs, straight into the file by the rules of section 6.
  - `run --set` changes the field for this run only; the file is untouched and the path is listed in `cli_overrides`.
  - Both take any stored field except `network.key` and the read-only ones. The file reader checks the value, so the
    bounds are the file's.
  - `config get [<path>]` shows the values.
- **`--no-config`** works on defaults and flags and neither reads nor writes the file. Use it for tests and to find
  out whether the config causes a problem.

---

## 3. Config field ↔ command-line flag

The fields come from the table `config_file::fields::FIELDS` (`core/config/config_file/fields/table.rs`). It has a
row per path, with the `Mode` (live, restart with or without a prompt, edit only, read only, file only) and the
value kind. From it come:

- the lists `read_only` and `restart_fields`, and the live paths;
- the known section keys;
- file parsing and file output, and the settings' `Debug`.

The row order is the key order in the written file. Any stored field except `network.key` and the read-only ones
can be given as `--set <path>=<value>` for one run, and saved with `config set`.

The column "Read in the node" shows where the node reads the value: `NodeConfig.settings` (type `NetSettings`),
or `NodeConfig.socks` and `NodeConfig.memory` for SOCKS and memory thresholds.

| Config path | Flag | Default | Read in the node |
|---|---|---|---|
| `network.key` | `--key`, `MESHTOWN_KEY` | — | `network_key` (`node/config.rs`) |
| `exit.enabled` | `--exit` / `--no-exit` | `true` (`false` on Android) | `settings.exit_enabled` (`core/config/config_file/net_settings.rs`) |
| `exit.max_streams` | no flag | `0` (by machine memory: 20 on ≤128 MiB, 50 on ≤512 MiB, 200 otherwise) | `process.exit_max_streams` (`core/config/config_file/process_settings.rs`); the open-stream ceiling for the whole process, 0…100000. Beyond it a new stream is refused as busy and clients try another exit. No single member may hold more than four fifths of it; when several members contend, each gets an equal part. Takes effect at once; open streams are not cut |
| `exit.stream_window_kib` | no flag | `0` (by machine memory: 128 on ≤128 MiB, off otherwise) | `process.exit_stream_window_kib` (`core/config/config_file/process_settings.rs`); KiB of site-to-client data an exit stream may have unacknowledged, 0…65536; non-zero values below 64 count as 64. Smaller values save router memory (above about 4096 the window no longer saves any) and cap one download at about window ÷ round-trip time. Takes effect for new streams; open streams keep theirs |
| `exit.interface` | no flag | `null` (any network) | `settings.exit_interface` (`core/config/config_file/net_settings.rs`); the interface name exit connections are bound to, 1–64 bytes, see "Exit through another VPN" below. Takes effect at once; open connections keep their path |
| `node.memory_warn_mib` | no flag | `400` (by machine memory: 24 on ≤128 MiB, 64 on ≤512 MiB, 400 otherwise) | `process.node_memory_warn_mib`; memory guard threshold, MiB, `0` = off. The default `400` means "by machine memory", so an explicit 400 on a machine with ≤512 MiB also counts as automatic. On such machines the exit also refuses new streams near this value; above 512 MiB with the default 400 it refuses only by the stream limit |
| `socks.enabled` | `--socks` / `--no-socks` | `true` | `cfg.socks.enabled` (`node/config.rs`) |
| `socks.port` | `--socks-port` | 1080 | `cfg.socks.port` (`node/config.rs`) |
| `socks.strict` | no flag | `false` | `process.socks_strict`; when `false`, the auto-config (PAC) file ends the route of ordinary sites with `DIRECT`, so they open directly while the node is not running; when `true` they stay closed. Takes effect at once |
| `proxy.net` | no flag | `main` | which network's exit the local proxy uses |
| `checks.enabled` | — | `true` | `settings.checks_enabled` (`core/config/config_file/net_settings.rs`) |
| `checks.interval_sec` | — | 900 | `settings.check_interval()` (`core/config/config_file/net_settings.rs`) |
| `checks.doh_server` | — | `1.1.1.1:443` | `settings.doh_server` (`core/config/config_file/net_settings.rs`) |
| `checks.provider_dns` | — | `null` | `settings.provider_dns` (`core/config/config_file/net_settings.rs`) |
| `checks.geo_markers` | no flag (file only) | `[]` | the domain check, see the section below |
| `probe.interval_sec` | — | 300 | `settings.probe_interval()` (`core/config/config_file/net_settings.rs`) |
| `probe.full_cycle` | — | `false` | 1 MiB in the probe round to all peers; when `false` — only to the chosen exit (`route.exits`) |
| `route.exits` | no flag | `[]` | `settings.route_exits` (`core/config/config_file/net_settings.rs`) |
| `ports.published` | no flag | `[]` (items `{port, proto, name, for}`, `proto` `"tcp"` by default, written only for UDP) | `settings.ports_published` (`core/config/config_file/net_settings.rs`) |
| `ports.opened` | no flag | `[]` (items `{node, port, proto, local}`, `proto` as above) | `settings.ports_opened` (`core/config/config_file/net_settings.rs`) |
| `web.enabled` | no flag | `true` | `core/web_gui` |
| `web.port` | `--web-port` | 7777 | `core/web_gui` |
| `web.access` | no flag (command `meshtown access page localhost\|localnet`) | `"localnet"` | `core/web_gui/auth/access.rs` (section 8.1) |
| `log.ring_lines` | no flag | 2000 | `core/web_gui` |
| `node.name` | no flag | a name from a pair of pool words (`words.rs`) | node identity |
| `net.transports`, `net.discovery`, `net.sync`, `net.relay` | no flag | `["hyper"]`, `["invite","cache","hyperdht"]`, `"mesh"`, `"network"` | section 8.3 |

`network.key` is the only file field that is not in the page document (section 4). There is no default key: with
no flag and no line in the file, the node comes up without a network.

### Exit through another VPN

`exit.interface` makes the node send what it opens for other members only through the named interface, for
example another VPN (`"wg0"`, `"utun4"`).

- The binding covers TCP connections to sites, UDP sessions and the exit's own DNS queries.
- It does not cover links between nodes, site checks and the direct local proxy: they use the regular routing table.
- If the interface is missing, down or cannot be bound, the exit refuses the connection (reply code 8) instead of
  falling back to the regular network, so nothing leaks.
- On Linux kernels before 5.7 the binding needs the `CAP_NET_RAW` capability.

Code: `core/iface_bind/`, `core/exit/outbound/dial.rs`.

### `checks.geo_markers`

- Key: `checks.geo_markers`; type: a list of strings; default: `[]`; file only (no command-line flag, no web editing).
- Extra phrases that mark a "not available in your region" stub page. The domain check adds them to the three
  built-in English phrases (`not available in your`, `unavailable in your country`, `geo-restricted`).
- Matching ignores case (full Unicode lowercasing), so a phrase in any script works: it is lowercased when the
  file is read, and the page text is lowercased before the search.
- Limits: up to 32 phrases, each up to 64 characters (characters, not bytes). Phrases are trimmed; empty ones and
  repeats are dropped. A longer list, a longer phrase or a non-string item rejects the whole key: the error names
  `checks.geo_markers` and the default (an empty list) stays.
- Example: `"checks": { "geo_markers": ["not offered in your area", "service unavailable here"] }`.
- Why: stub pages differ by site and language, and the built-in list covers only common English wording. This key
  lets a node owner add the wording of the sites that matter to them.

Not stored in the file even though a flag exists:

- `--site` adds the domain to the **node's own list**, which lives in `replica/domains.bin` (not in the config)
  and is not passed to other members.
- `--block-site` is a debug flag (`blocked_domains` in `node/config.rs`), not a setting.
- `--invite` is a one-time join, not state.

Paired flags (`--socks` and `--no-socks`, and so on) exist because of the file: without a positive form, one run
could not turn SOCKS on when the file turns it off. The last flag of a pair wins.

An old `transport.mode` key is still accepted and ignored. The log shows `deprecated fields ignored and dropped on the next write: transport.mode`, and the key disappears at the next write.

---

## 4. File format

```json
{
  "rev": 8,
  "saved_at": "2026-09-24T19:41:07Z",
  "identity": { "secret": "<node secret, 32 bytes in base64>" },
  "config": {
    "version": 1,
    "node":      { "name": "finch-unwary" },
    "network":   { "key": "our-gaming-room--aliens-congested-4dd5mwxvn77e6pptnnw01-glbdthzco34lodbfmhh6knwjvto4c7fqz6irhnyn367juokngaya" },
    "exit":      { "enabled": true, "max_streams": 0, "interface": "wg0" },
    "socks":     { "enabled": true, "port": 1080, "strict": false },
    "checks":    { "enabled": true, "interval_sec": 900,
                   "doh_server": "1.1.1.1:443", "provider_dns": null },
    "probe":     { "interval_sec": 300, "full_cycle": false },
    "route":     { "exits": [] },
    "ports":     { "published": [], "opened": [] },
    "web":       { "enabled": true, "port": 7777, "access": "localnet" },
    "log":       { "ring_lines": 2000 }
  }
}
```

- **`config` is the body of `PUT /api/config` plus one field: `network.key`.** Apart from the key, the match is
  complete, field for field, with no conversion in either direction.
- **The node secret sits in the envelope, next to `config`, not inside it.** `config` as a whole goes to the page
  in `GET /api/config` and may become a backup file, and the secret must reach neither. Restoring a backup on a
  second machine would create two nodes with one identity, and the network could not tell them apart. One rule
  replaces a list of exceptions: **the page sees only `config`, the node reads the whole file**.
- **One network, one file, one identity.** A node identity is tied to its network. The network key and the secret
  sit together in one file, so two networks on one machine never share an identifier; a second network is a
  second config file (`--config`).
- **The node name is a setting, not identity**, so it is inside `config`. A person may change it, including to
  non-Latin letters. The node picks a name on first start; after that it lives in the file like any other
  setting. The page document has no `node.name` field today; adding it changes the page API.
- **`network.key` lives only in the file.** The page never receives it: it is not in `GET /api/config`, it is
  ignored in `PUT /api/config`, and the page sees `network.label` and `network.masked`. The page gets the whole key
  only on "Copy" (`GET /api/network/key`); otherwise the key would sit in every `/api/config` reply, in tab
  history and in debug dumps. Two actions change it, `POST /api/network/join` and `/create`; they write this file,
  so they **raise `rev`**.
- **The whole file is secret because of the key.** Mode `0600`, directory `0700`; a future backup download will need
  a warning. The settings export goes through the API and contains no key, so a copy restores the settings but not
  the network.
- **`version` is one, inside `config`** (as in `WEB-API.md`, section 2). The envelope has no second version: two
  version numbers in one file always diverge.
- **`rev`** is the config revision, the same as in `GET /api/config`. It lives in the file, not in memory:
  otherwise a restart would reset the counter, and a tab opened before the restart could overwrite others' edits
  without getting `stale_rev`. It grows by 1 on every successful write.
- **`saved_at`** is UTC, ISO-8601, for humans only; the node does not parse it.
- **"Read only" is about the page, not the file.** The node computes these fields and does not store them:
  `network.label` and `network.masked` (from the network key), `autostart.available` and `autostart.reason`
  (build and system state), `full_tunnel.available` and `full_tunnel.reason` (build flags). On write they are not
  saved; on read they are silently ignored, so "read only" never rejects a document.
- **`web.enabled` is in `read_only` but stored in the file.** The page does not change it (a page that turned
  itself off could not turn itself back on), yet it is a setting. It has no flag: edit the file by hand while the
  node is stopped. Dropping it on write is not allowed: a node without a screen on a router is configured this way.
- **`full_tunnel.enabled` is in the file** (a setting, not a build flag). While `available: false`, enabling is
  rejected with the code `not_available`, and only `false` gets into the file.
- **`autostart.enabled` is in the file and accepts `true`.** It stores the last choice made on the page. The file
  value alone never creates or removes the login entry; only a save from the page does. The page shows the value
  read from the system.

---

## 5. Reading at startup and what the page sees

| What is on disk | What the node does | `config.file.state` |
|---|---|---|
| no file | takes defaults, **creates a file with defaults** | `created` |
| the file is readable, the document parsed | takes values from the file | `ok` |
| the file is unreadable (permissions, busy) | defaults + flags, **does not touch the file** | `unreadable` |
| JSON did not parse | same | `invalid` |
| `config.version` is greater than the known one | same | `invalid` |
| a single field is out of range or of the wrong type | **that field** gets the default, the rest come from the file | `ok`, the list in `field_errors` |

Two rules from which this follows:

- **A damaged file does not stop the node.** The node serves the page itself; if a bad config stopped it, nothing
  could fix the settings. A read error is a log warning and a line on the page, not a refusal to start.
- **One bad field does not cancel the whole file.** Rejecting the whole document is the rule for
  `PUT /api/config`, where the page sent the document and someone can fix it. The file may have been edited by hand
  or survived a version rollback, and losing all settings over one field is not acceptable.

**The node identity is read from the same file in the same pass.** The secret sits in the envelope
(`identity.secret`, section 4), so no separate read is needed. Three cases:

- **No secret in the file** (first start, or a file from an old version): the node generates one, writes it on the
  first write and keeps it from then on.
- **The secret did not parse** (not base64, wrong length): the general table rule applies. The node takes a new
  secret and adds a line to `field_errors`. The machine's identity changes, so the page must say so in words, not
  with an icon: for the network this is a different node.
- **The file cannot be written** (a read-only router): the node **works** with a temporary identity, new on every
  start, and says so in the log and on the page. It must not refuse to start, or a router node would not come up.

**The first start creates a file with defaults, not flag values.** Otherwise a flag typed once for a trial would
quietly become a permanent setting, and the person would not know it was written. The page shows flag values as
`cli_overrides`, and the person decides whether to save them. To store a flag value, run
`config set <path> <value>` (section 2). Once the file matches the flag, the page shows no "set by a flag at
startup" mark for that field.

### 5.1 What is added to the `GET`/`PUT /api/config` replies

Two fields on top of what is already described in `WEB-API.md`, sections 1.6–1.7:

```json
{
  "rev": 8,
  "config": { "…": "document" },
  "read_only": ["…"],
  "restart_fields": ["…"],
  "restart_pending": [],
  "cli_overrides": ["socks.port"],
  "file": {
    "path": "/Users/example/Library/Application Support/meshtown/config.json",
    "state": "ok",
    "writable": true,
    "message": null,
    "field_errors": [
      { "path": "socks.port", "message": "port 80 in the file is busy; default 1080 used" }
    ]
  }
}
```

- `cli_overrides` lists fields typed as flags and **not** written to the file. The page marks them "set by a flag
  at startup, takes effect after a restart without the flag" and allows editing. The network key is listed like the
  rest: a node started with `--key` or `MESHTOWN_KEY` shows the label and mask of the effective key, not the file's.
- `file.state` is `ok` | `created` | `unreadable` | `invalid`; `message` is English text from the node.
- `file.writable: false` means the directory is read-only (a router image, a write-protected flash drive). The page
  shows the settings and disables "Save" **before** it is pressed, not after a refusal.
- `field_errors` lists fields that did not survive reading the file. The page shows them once in a strip above the
  form and clears it after the first successful save.

---

## 6. Writing

The node writes the file in four cases:

1. a successful `PUT /api/config`;
2. the first start when there is no file, with **defaults** and no flag values;
3. `config set` when no node runs: the one field (section 2);
4. `POST /api/network/join` and `/create`: they change `network.key`, which lives in this file, so they raise
   `rev` like a settings save.

Other actions (`/api/actions/*`) do not touch the file.

The write order: steps 1–5 run under an exclusive lock on an empty `config.json.lock` next to the file, held by
the writing process. The lock file is safe to delete.

1. **Re-read the file from disk.** If `rev` on disk differs from the revision in memory, the file was edited
   behind the node's back: reply 409 `stale_rev` with the document **from disk**; nothing is written. The page
   already handles this state (a second tab).
2. Build the new document from the accepted fields. Node-computed fields are not saved (section 4). Set
   `rev` + 1 and `saved_at` to now.
3. Write a temporary file `config.json.tmp-<pid>-<n>` **in the same directory** (a rename across a file system
   boundary is not atomic) and `fsync` it.
4. On Unix, set mode `0600` before the rename, not after. On Windows, permissions are inherited from the directory
   and not set separately.
5. Rename over `config.json` (on Windows, `ReplaceFileW`), then `fsync` the directory. Without that, on ext4 the
   rename may not survive a power loss, which a router node must expect.
6. Only after a successful rename, apply the settings in memory and reply 200.

**If the write fails, nothing is applied.** The node replies 500 `config_write_failed` with the path and the
reason, and `rev` does not grow. Applying without saving would be worse: the node would differ from its own file,
and a restart would silently roll the settings back.

**A damaged file is set aside before being overwritten.** If the node came up with `state: invalid` or
`unreadable` and the person presses "Save", the node first renames the existing file to
`config.json.bad-<date-time>`, then writes the new one. The page warns about this before sending.

**Manual edit while the node runs.** The file belongs to the node while it runs. A hand edit takes effect on the
next start, and a page save made earlier overwrites it. The `rev` check catches only a hand edit that changed
`rev`. This loss is accepted, not protected. The regular way to change a running node's settings is the page.

---

## 7. What is not in the settings file

| What | Where it lives | Why not here |
|---|---|---|
| page password | the `web-password` file next to it (section 8.2) | the page serves the config via `GET /api/config`, and the password hash must not be served or carried away in a backup |
| page token | the `web-token` file next to it | the page rewrites the config as a whole and must not receive the token; the "New token" button changes it, not a setting |
| node secret | in the **envelope** of the same file, next to `config` (section 4) | identity, not a setting: it is not given to the page or a backup, or a copy would put one identity on two machines |
| domain list | the node's own document (`replica/`), without reconciliation | the node's personal list, not a config field |
| `blocked_domains` | only the flag `--block-site` | debugging |

**The network key is stored in the config**; there is no separate `network-key` file. `--key` or `MESHTOWN_KEY`
overrides the file (`core/config/config_file/overrides.rs`). With neither, and no key in the file, the node comes
up without a network and does not create one. On "Home" the page shows the panel "You haven't joined a Mesh Town
network yet" with the buttons "Join with an invite" and "Create a new network".

The settings export has no key, so a copy restores the settings but not the network.

---

## 8. Versions and unknown fields

- `config.version` is exactly 1 today.
- **A known-version field missing from the file** gets the default from the table in section 3, and the next write
  completes the file. This is how a node update reads an old file.
- **An unknown field inside a known version** is ignored with one log line and **disappears on the next write**:
  the node writes only the fields it knows. Rolling back to an old binary after a newer one has run therefore
  loses the settings the newer one added. This is deliberate: the file keeps no unknown fields for rollback.
- **`version` greater than the known one**: the file is not read and **not overwritten** until an explicit save
  from the page (section 5), so a version rollback does not silently eat the settings.
- A version bump that changes a field's meaning is a separate migration: read the old version, write the new one,
  set the previous file aside as `config.json.v<old>-<date>`. There are no such migrations now.

### 8.1 The `web.access` field

`web.access` is the optional string `"localhost"` or `"localnet"`. It sets who may open the node's settings page:

- `localhost`: only this machine.
- `localnet`: also devices on the local network (private addresses `10/8`, `172.16/12`, `192.168/16`,
  `169.254/16`).

It is not about the Mesh Town network or traffic.

- A missing field means `localnet`. This follows the rule above for missing known-version fields: `config.version`
  does not grow, there is no migration, and the nearest write completes the file.
- Any other value (a typo, not a string) is a field error: the default replaces it and the error goes to
  `field_errors`.
- Change it from the page (`PUT /api/config`) or with `meshtown access page localhost|localnet`. On a stopped
  node the command writes the file itself.

### 8.2 The `web-password` file

The file sits in the state directory next to `web-token`, not in `config.json`. Its mode is `0600` (Windows does
not check permissions). The node writes it through a temporary file `.web-password.tmp-<pid>` and a rename.
It holds one line of JSON:

```json
{"version":1,"alg":"pbkdf2-sha256","iterations":600000,"salt":"<base64, 16 bytes>","hash":"<base64, 32 bytes>","set_at":1790000000}
```

- `version` is 1 today. `alg` is `pbkdf2-sha256` (PBKDF2-HMAC-SHA256 from the `ring` crypto library, which the
  project already uses). `iterations` is the hash repetition count, `salt` a random salt, `hash` the result, and
  `set_at` the time the password was set, in Unix seconds.
- The file does not store the password, and the node does not keep it in memory. The node re-reads the file on every
  sign-in and state request, so `meshtown password reset` takes effect on a running node at once; on a stopped node
  it deletes the file.
- **No file means no password**: sign-in is by token only, and the first sign-in through a temporary-token link
  requires setting a password.
- **An unreadable file** counts as "no password" too: sign-in is by token only, and once per run the log gets
  "password file is unreadable (reason): sign-in by token only". The node does not delete the file; a new password
  overwrites it. A file is unreadable when it has any of these:
  - not JSON of this format, `version` other than 1, or an unknown `alg`;
  - `iterations` outside the allowed range (zero or more than 10,000,000);
  - a salt or hash of the wrong size;
  - a size over 4 KiB, or permissions wider than `0600`.
- A successful sign-in with `iterations` below the current constant (600,000) rehashes the password and rewrites the
  file. A rewrite error goes to the log, and the sign-in still succeeds.
- Password length is 8–256 characters. The bound applies on write, not on read.

### 8.3 The `net` section (choice of network stack)

An optional section that sets what the node uses to talk over the network. A missing section or field means the
build default.

- `transports`: ways of delivering packets between nodes; a non-empty list without repeats. Allowed:
  `["hyper"]` (default). `"mem"` is for tests only and unavailable from `config.json`.
- `discovery`: how the node learns other nodes' addresses; a non-empty list without repeats of `invite`, `cache`,
  `hyperdht`. Default `["invite","cache","hyperdht"]`.
- `sync`: how the node exchanges the network's shared state. The string `"mesh"` (default) means the node's own
  topic membership (flooding over its connections) and its own records (signed by the author, reconciled with
  peers), stored as files in the `replica/` directory.
- `relay`: relaying inside the network.
  - `"network"` (default) lets the node reach a member through another member of the same network and serve as
    such a relay. A relay pairs only members of its own network.
  - Every node serves unless it is behind a NAT with random port mapping. A public node is advertised with its
    address; a node behind a NAT with stable ports, by key alone.
  - `"off"` does neither.
  - A change needs a node restart; saved through the settings API, the field is listed as waiting for a restart.
- An error in a field (type, empty list, repeat, unknown or not built-in name) is a warning, and the whole field
  takes the default; the node starts. An unknown key in `net` goes to the log, as everywhere.
- There is no command-line flag, and the page neither shows nor edits the section. `PUT /api/config` stores it and
  rejects a wrong value like any field error. A restart applies changes.
- **Where the section sits.** Like the other sections, `net` is inside the file's `"config"` field, for example
  `{"config": {…, "net": {"sync": "mem"}}}`. At the top level the node skips it with the line "unknown fields … net"
  and does not change the choice.
- **Written only as differences from the build default.** A field equal to the default is not written, and if all
  four are equal there is no section. A future change of the default must not be frozen by a written value. This
  is an exception to the rule in section 8 that the next write completes the file. An old binary loses the `net`
  section on write (section 8, unknown field).

## 9. Network slot files

Besides the main network, the node can run more networks in the same process. Each extra network (a "slot")
keeps its own settings file in a folder beside the main one:

```
<state directory>/config.json              the main network and the process settings
<state directory>/nets/<id>/config.json    one extra network
```

`<id>` is six characters of `a-z` and `2-7`. A folder whose name is not such an id is skipped, and so is a
folder without `config.json`. The node reads `nets/` at startup, only when the main settings file is in use
(`--no-config` reads nothing).

- **Envelope.** The same as the main file (`rev`, `saved_at`, `identity.secret`, `config.version`) plus
  `created_unix`, the time the slot was created. Slots start in the order of `(created_unix, id)`. A missing
  `created_unix` reads as 0; a value that is not a number reads as 0 with a field error.
- **Only network settings.** `config` holds the network rows (network key, node name, transport, exit, routes,
  ports, checks, domains and the like) and nothing else. Every slot has its own identity secret and its own
  files beside it (peer cache, peer names, replica).
- **Process keys are ignored.** `exit.max_streams`, `exit.stream_window_kib`, `socks.*`, `proxy.*`, `web.*`, `log.*`, `node.memory_*`, `autostart.*` and
  `full_tunnel.*` always come from the main file. If such keys are found in a slot file (for example a copy of
  the main file) they are not an error: one log line names them, and they disappear on the next write.
- **Command-line flags** that belong to a network (`--key`, `--exit` and the like) apply to the main network only;
  the flags that belong to the process (SOCKS, the page port) are shared by all slots.
- **One proxy entrance per process.** The SOCKS/HTTP entrance is shared by all networks, so `socks.*` apply to the
  whole process; `proxy.net` picks which network's exit it uses.
- **A slot that cannot start** gets a log line `net slot <id> not started: <reason>`; its folder is never
  deleted or rewritten for that. The reasons: the file is missing, unreadable or invalid, its version is newer
  than this build knows, it has no `network.key`, it holds the same network as a running slot, or the
  limit of 8 networks per process is reached (the main network always counts).
- **Left networks.** The node does not delete a network that was left. It renames the folder to
  `nets/left-<id>-<UTC time>/` (for the main network, `nets/left-main-<UTC time>/` with a copy of its
  `config.json` and cache files). Folders named `left-*` are never started or reused as ids.
- **Rolling back to an older version.** An older build does not look into `nets/` at all: it runs the main
  network as before and leaves the slot files untouched. After the upgrade the slots start again.
