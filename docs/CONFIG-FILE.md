# Node settings file on disk

This document describes the second half of the node's settings: **where the file lives, how it is read at startup,
which is stronger — the file or the command line — and how the node rewrites the file** when settings are changed
from the page. The first half — the document's composition, field types and how it travels between the page and the
node — is in `WEB-API.md`, section 2 "Config schema" and sections 1.6–1.7.

**One schema for both documents.** The file on disk stores the same `config` document that the page
gets from `GET /api/config` and sends in `PUT /api/config`. The node converts nothing:
read the file — got `config`, wrote `config` — got the file. There must be no divergence between this
document and `WEB-API.md`; schema fields are edited in `WEB-API.md`, the file's rules here.

---

## 1. Where the file lives

The order:
**first next to the binary, then in the host's standard location; a new config is created in
the standard location.** The first rule that applies wins:

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
  under `/opt`. A root process whose binary is in `/opt/bin` therefore uses `/opt/etc/meshtown/` instead of `/etc/meshtown/`,
  so a command typed in a shell finds the same settings as the service.
- **A file next to the binary is the portable mode** (a flash drive, a directory on a router,
  "downloaded, ran, left nothing in the system"). It is looked up first, so a portable copy
  is always stronger than one installed in the system.
- **A new config is created only in the standard location** (rule 4), even if the binary's directory
  is writable: the binary may sit in `/usr/bin` or in `Program Files`, and the node
  must not litter there. A portable file is set up by a person — they put `config.json` next to the program (or
  copied it from another machine), and the node picks it up on the next start. The exception is an explicit
  `--config`: the file is both looked up and created at that path.

- **The state directory is the directory that holds `config.json`.** There is no separate setting for "where
  the state is": if you gave `--config`, you gave the directory too; if you put the config next to the binary,
  the other state files land there as well.
- If the directory does not exist, the node creates it together with the file (`mkdir -p`, mode `0700` on Unix).
- The path the node chose is printed at startup as the first line (`config: <path> (read)`)
  and given to the page in `config.file.path` (section 5). The subcommand `meshtown config-path` prints
  the same path and starts nothing — so that a person can find the file without bringing the node up.

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

- **`config.json` is a secret file.** Inside it are both the network key and the node secret,
  so mode `0600` and directory `0700` are not a wish but a requirement: the file gives access to
  the whole network and lets someone impersonate this node.
- **There is no separate node identity file any more.** The secret lives in the shared
  file; the earlier per-network `node-<network
  tag>.key` files are not used. The secret's place in the file is
  section 4.
- **The page token** is a separate file, not a config field: the page reads and rewrites the
  config as a whole, and the token must not be given to it, and the "New token" button changes it
  bypassing the settings. The file is created by the web server.

---

## 2. Priorities: the command line is stronger than the file

The value of each field is taken by the rule **default → file → command line**, per field, not
per document: `--socks-port 1081` overrides only `socks.port`, the other fields of the file stay in
force.

- **"Set on the command line" literally means "the flag was typed by a person"**, not "the flag has a
  value". Today half the flags have `default_value_t` (`core/cli/args.rs`),
  and what was typed cannot be told from the default. When the file is connected, all such
  flags become `Option<T>` without `default_value_t`, and the defaults move to one place — the table in section 3.
  The "was the flag typed" check is `ArgMatches::value_source(..) == Some(ValueSource::CommandLine)`.
- **The file does not change because of this.** A flag acts on the current run; the disk keeps what it kept.
  Restarted without the flag — the value from the file works.
- **The page knows about this.** Fields overridden by the command line are given by the node as the list
  `cli_overrides` (section 5): the page shows the effective value, writes under the field "set
  by a flag at startup" and **allows saving** — the saved value goes to the file and takes effect on the
  next start without the flag. Silently accepting an edit that affects nothing is not allowed;
  forbidding it is not either: a person configures the node for the future.
- **There are no environment variables for each setting.** There are exactly two, and both are about paths and the key:
  `MESHTOWN_CONFIG` and `MESHTOWN_KEY` (`core/config/config_file/overrides.rs`). We do not introduce a third layer of priorities per
  field: it cannot be shown on the page in an understandable way. `MESHTOWN_KEY` counts as
  set explicitly, on a par with a flag: it overrides the key from the file and is written to the file at
  `--save-config`.
- **`--save-config`** — write to the file what was typed on the command line (the owner: "let's add
  `--save-config` for command-line parameters — then we rewrite the config
  where they were passed on the command line"). It works at startup, before the network comes up: the node takes the file,
  applies the typed flags to it, writes by the rules of section 6 and continues in the usual
  way. Only the typed fields are written, the others in the file are not touched. After that the
  `cli_overrides` list is empty — the flag and the file say the same thing, there is nothing to warn the page about.
  This is the only way to save settings without the web page: on a node without a screen it may not
  have been enabled.
- **`--no-config`** — work on defaults and flags, neither read nor write the file. Needed for
  tests and for working out "is the config to blame or not". Incompatible with `--save-config`: together they are an error
  at argument parsing, not a silent preference of one over the other.

---

## 3. Config field ↔ command-line flag

The source of fields is the table `config_file::fields::FIELDS` (`core/config/config_file/fields/table.rs`):
a row per path, the `Mode` (live, restart with or without a prompt, edit only, read only,
file only) and the value kind. From it come the lists `read_only` and `restart_fields`, the live paths,
the known section keys, file parsing, file output and the settings' `Debug`; the row order is the order of
keys in the written file. The column "Read in the node" below is where the node reads the value from (`NodeConfig.settings`, type `NetSettings`; SOCKS and memory thresholds — `NodeConfig.socks`, `NodeConfig.memory`).

| Config path | Flag | Default | Read in the node |
|---|---|---|---|
| `network.key` | `--key`, `MESHTOWN_KEY` | — | `network_key` (`node/config.rs`) |
| `transport.mode` | `--relay-only` / `--no-relay` | `direct_and_relay` | `settings.uses_relay()`, `settings.is_relay_only()` (`core/config/config_file/net_settings.rs`). Reserved: the transport does not use relays yet; the setting has no effect. |
| `exit.enabled` | `--exit` / `--no-exit` | `true` (`false` on Android) | `settings.exit_enabled` (`core/config/config_file/net_settings.rs`) |
| `socks.enabled` | `--socks` / `--no-socks` | `true` | `cfg.socks.enabled` (`node/config.rs`) |
| `socks.port` | `--socks-port` | 1080 | `cfg.socks.port` (`node/config.rs`) |
| `proxy.net` | no flag | `main` | which network's exit the local proxy uses |
| `checks.enabled` | `--checks` / `--no-checks` | `true` | `settings.checks_enabled` (`core/config/config_file/net_settings.rs`) |
| `checks.interval_sec` | `--check-interval` | 900 | `settings.check_interval()` (`core/config/config_file/net_settings.rs`) |
| `checks.doh_server` | `--check-doh` | `1.1.1.1:443` | `settings.doh_server` (`core/config/config_file/net_settings.rs`) |
| `checks.provider_dns` | `--provider-dns` | `null` | `settings.provider_dns` (`core/config/config_file/net_settings.rs`) |
| `checks.geo_markers` | no flag (file only) | `[]` | the domain check, see the section below |
| `probe.interval_sec` | `--probe-interval` | 300 | `settings.probe_interval()` (`core/config/config_file/net_settings.rs`) |
| `probe.full_cycle` | `--probe-full-cycle` | `false` | 1 MiB in the probe round to all peers; without the flag — only to the chosen exit (`route.exits`) |
| `route.exits` | no flag | `[]` | `settings.route_exits` (`core/config/config_file/net_settings.rs`) |
| `ports.published` | no flag | `[]` | `settings.ports_published` (`core/config/config_file/net_settings.rs`) |
| `ports.opened` | no flag | `[]` | `settings.ports_opened` (`core/config/config_file/net_settings.rs`) |
| `web.enabled` | no flag | `true` | `core/web_gui` |
| `web.port` | `--web-port` | 7777 | `core/web_gui` |
| `web.access` | no flag (command `meshtown access localhost\|localnet`) | `"localnet"` | `core/web_gui/auth/access.rs` (section 8.1) |
| `log.ring_lines` | no flag | 2000 | `core/web_gui` |
| `node.name` | no flag | a name from a pair of pool words (`words.rs`) | node identity |
| `net.transports`, `net.discovery`, `net.sync` | no flag | `["hyper"]`, `["invite","cache","hyperdht"]`, `"mesh"` | section 8.3 |

`network.key` is the only file field that is not in the page document (section 4). There is never a default key:
with no flag and no line in the file, the node comes up without a network.

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
- Why: stub pages differ by site and language; the built-in list covers only common English wording, and this key
  lets each node owner teach the check the wording of the sites that matter to them.

What is **not** stored in the file even though a flag exists:

- `--domain` — the domain is added to the **node's own list**; it lives in the file `replica/domains.bin` (not in the config) and
  is not passed to other members.
- `--block-domain` — a debug flag (`blocked_domains` in `node/config.rs`), not counted as a setting.
- `--invite` — a one-time join, not state.

Paired flags (`--socks` next to `--no-socks` and so on) exist precisely because of the file: without
a positive form it is impossible to start once with SOCKS on when it is off in the file; the last flag
of a pair wins. `--relay-only` and `--no-relay` set `transport.mode` (reserved, see §3).

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
    "network":   { "key": "our-gaming-room--finch-unwary-k7m2q9x4rb8vhn3tzd6w5" },
    "transport": { "mode": "direct_and_relay" },
    "exit":      { "enabled": true },
    "socks":     { "enabled": true, "port": 1080 },
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

- **`config` is the body of `PUT /api/config` plus one field: `network.key`.** Apart from the key,
  the match is complete, field for field, with no conversion in either direction.
- **The node secret sits in the envelope, next to `config`, not inside it** (it lives
  in the shared file). The reason is not neatness: `config` as a whole goes to the page in `GET`
  `/api/config` and will become the backup file too, and the node secret must get into neither
  place — restoring a backup on a second machine would give a person two nodes with one identity, and the
  network would stop telling them apart. A separate envelope key removes this with one rule instead of a
  list of exceptions: **the page sees only `config`, the node reads the whole file**.
- **One network — one file — one identity.** A node identity is tied to its network, so a file per network
  keeps two networks on one machine from getting the same identifier; in a shared file this comes out by itself:
  the network key and the secret sit together, and a second network is a second config file (`--config`).
- **The node name is a setting, not identity**, so it is inside `config`: a person
  may change it, including to non-Latin letters. The node comes up with a name itself on first start,
  after that it lives in the file like any other setting. The page document has no `node.name` field
  today — adding it changes the page API.
- **`network.key` lives only in the file.** The network key sits in the config, but is never given to the page:
  it is not in `GET /api/config`, it is ignored in `PUT /api/config`, the page sees
  `network.label` and `network.masked`, and gets the whole key only on pressing "Copy"
  (`GET /api/network/key`). Otherwise the key would sit in every `/api/config` reply — that
  is, in the tab's history and in any debug dump. It is changed by two actions,
  `POST /api/network/join` and `/create`, and they **raise `rev`**, because they write this same file.
- **Because of the key, the whole file is secret.** Mode `0600`, directory `0700`, and a backup, when
  one appears, will need a warning on download: the file gives access to the whole network. Today
  the settings export goes through the API and contains no key — so a copy restores the settings
  but not the network.
- **`version` is one, inside `config`** (as in `WEB-API.md`, section 2). The envelope has no second
  version: two version numbers in one file always diverge.
- **`rev`** is the config revision, the same as in `GET /api/config`. It lives in the file, not in the node's
  memory: otherwise a restart would reset the counter, and a tab opened before the restart would overwrite
  others' edits without getting `stale_rev`. It grows by 1 on every successful write.
- **`saved_at`** — UTC, ISO-8601, for humans only; the node does not parse it.
- **"Read only" is about the page, not about the file.** Fields the node computes itself are not stored:
  `network.label` and `network.masked` (from the network key), `autostart.available`,
  `autostart.reason`, `full_tunnel.available`, `full_tunnel.reason` (build flags). On write
  they are not saved, on read they are silently ignored — "read
  only" is no reason to reject a document.
- **`web.enabled` is an exception: in `read_only`, but stored in the file.** The page does not change it
  (having turned itself off, it could not turn itself back on), yet it remains a setting: it is switched by
  editing the file by hand while the node is stopped — it has no flag. Dropping it
  on write is not allowed — a node without a screen on a router is configured exactly this way.
- **`autostart.enabled` and `full_tunnel.enabled` are in the file** (they are settings, not build
  flags), but while `available: false` enabling is rejected with the code `not_available`, and
  only `false` gets into the file.

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

- **A damaged file does not stop the node from coming up.** The node serves the page itself; if it fell over because of
  the config, there would be nothing to fix the settings with. So a read error is a warning in the log and a
  line on the page, not a refusal to start.
- **One bad field does not cancel the whole file.** Rejecting the whole document at once is the rule for
  `PUT /api/config`, where the page sent the document and someone can fix it. The file may have been edited
  by hand or survived a version rollback; losing all settings over one field is not acceptable.

**The node identity is read from the same file in the same pass.** The secret sits in the envelope
(`identity.secret`, section 4), so no separate read is needed. Three cases:

- **there is no secret in the file** (first start, or the file was created by an old version) — the node generates it,
  puts it into the file on the very first write and lives with it from then on;
- **the secret did not parse** (not base64, wrong length) — the field is considered damaged by the general
  rule of the table: the node takes a new secret, and a line about it goes to `field_errors`. The machine's
  identity changes in the process, and the page must say so in words, not with an icon: for the network
  this is a different node;
- **the file cannot be written** (a read-only router) — the node **works** with a temporary identity,
  new on every start, and says so in the log and on the page. Refusing to start is not allowed — on a router the node would then simply not come up.

**The first start creates a file with defaults, not with flag values**. Otherwise a flag
typed once for a trial quietly becomes a permanent setting, and there is nothing to remove it with later —
the person does not know it got written. Flag values are visible on the page as `cli_overrides`, and
the person decides whether to save them. Whoever needs the opposite starts with `--save-config`
(section 2): then the typed flags go into the file right away, **and the `cli_overrides` list is empty**, because
the file and the command line say the same thing. In that case the page does not show the
"set by a flag at startup" mark — there is nothing to show, and this is not an oversight.

### 5.1 What is added to the `GET`/`PUT /api/config` replies

Two fields on top of what is already described in `WEB-API.md`, sections 1.6–1.7:

```json
{
  "rev": 8,
  "config": { "…": "document" },
  "read_only": ["…"],
  "restart_fields": ["…"],
  "restart_pending": [],
  "cli_overrides": ["socks.port", "transport.mode"],
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

- `cli_overrides` — fields typed as flags and **not** written to the file. The page writes under them
  "set by a flag at startup, takes effect after a restart without the flag"; it allows editing. After
  a start with `--save-config` the list is empty. The network key gets into it on a par with the rest: a node brought up
  with `--key` or `MESHTOWN_KEY` shows the label and mask of the effective key, not of the one in the file.
- `file.state` — `ok` | `created` | `unreadable` | `invalid`; `message` — in English, from the node.
- `file.writable: false` — the directory is read-only (a router image, a flash drive with writing disabled).
  The page shows the settings but disables the "Save" button **before** it is pressed, not after a refusal.
- `field_errors` — fields that did not survive reading the file; the page shows them once
  in a strip above the form and clears it after the first successful save.

---

## 6. Writing

The node writes the file in four cases:

1. a successful `PUT /api/config`;
2. the first start when there is no file — with **defaults**, without flag values;
3. a start with `--save-config` — the file plus the typed flags (section 2);
4. `POST /api/network/join` and `/create` — they change `network.key`, which lives in
   this same file, and so they raise `rev` on a par with saving settings.

Other actions (`/api/actions/*`) do not touch the file.

The write order, in full:

1. **Re-read the file from disk.** If `rev` on disk differs from the revision in memory, the file
   was edited behind Mesh Town's back: 409 `stale_rev` with the document **from disk**, nothing written. The page already
   knows this state (a second tab), no separate handling is needed.
2. Build the new document: the accepted fields, none of the node-computed ones is saved
   (section 4), `rev` + 1, `saved_at` — now.
3. Write to a temporary file `config.json.tmp-<pid>` **in the same directory** (a rename across a
   file system boundary is not atomic), `fsync` the file.
4. On Unix — mode `0600` before the rename, not after. On Windows permissions are inherited from the directory and
   not set separately.
5. Rename over `config.json` (on Windows — `ReplaceFileW`), then `fsync` the directory: without
   it on ext4 the rename may not survive a power loss, and a node on a router lives with exactly
   that scenario.
6. Only after a successful rename — apply the settings in memory and reply 200.

**If the write failed — nothing is applied.** Reply 500 `config_write_failed` with the path and
the reason, `rev` does not grow. Applying without saving would be worse: the node would work differently from what
its own file says, and a restart would silently roll the settings back.

**A damaged file is set aside before being overwritten.** Suppose the node came up with `state: invalid` or
`unreadable` and the person pressed "Save" — the node first renames what is there to
`config.json.bad-<date-time>`, then writes the new one. The page warns about this before sending.

**Manual edit while the node is alive.** The file belongs to the node while the node runs: a hand edit
takes effect on the next start, and if the page manages to save earlier — it is lost. The `rev` check
catches only a manual edit that changed `rev`; a person is not obliged to edit it by hand,
and this is accepted as a tolerable loss, not a protected case. The regular way to change
the settings of a running node is the page.

---

## 7. What is not in the settings file

| What | Where it lives | Why not here |
|---|---|---|
| page password | the `web-password` file next to it (section 8.2) | the page reads and rewrites the config as a whole and serves it via `GET /api/config`, and the password hash must not be served; a config backup must not carry it away either |
| page token | the `web-token` file next to it | the page reads and rewrites the config as a whole, and the token must not be given to it; it is changed by the "New token" button, not by a setting |
| node secret | in the **envelope** of the same file, next to `config` (section 4) | identity, not a setting: it is not given to the page and does not get into a backup, otherwise a copy would multiply one identity onto two machines |
| domain list | the node's own document (`replica/`), without reconciliation | the node's personal list, not a config field |
| `blocked_domains` | only the flag `--block-domain` | debugging |

**The network key is stored in the config** (there is no separate
`network-key` file). It is taken the same way as settings: `--key` or `MESHTOWN_KEY` is stronger than the file
(`core/config/config_file/overrides.rs`). If there is neither, and the file has no key, the node comes up without a network and
shows the page: on "Home" the panel "You haven't joined a Mesh Town network yet" with the buttons "Join with an invite" and "Create a new network"; the node does not create a network by itself.

The settings export goes through the API, where there is no key, so a copy restores the settings but not
the network.

---

## 8. Versions and unknown fields

- `config.version` is exactly 1 today.
- **A field of a known version that is not in the file** → the default from the table of section 3 is taken, the file
  is completed on the next write. This is how a node update works: the new node reads the old file.
- **An unknown field inside a known version** → ignored, one line in the log, and **on the
  next write it disappears**: the node writes only the fields it knows. So rolling back to
  an old binary version after the new one has run loses the settings it added — this is accepted
  deliberately, we will not store junk for the sake of rollback.
- **`version` greater than the known one** → the file is not read and **not overwritten** until an explicit
  save from the page (section 5). That way a version rollback does not silently eat the settings.
- A version bump that changes a field's meaning is a separate migration: read the old version, write the new one,
  set the previous file aside as `config.json.v<old>-<date>`. There are no such migrations now.

### 8.1 The `web.access` field

`web.access` is the string `"localhost"` or `"localnet"`, optional. No field — `localnet`; this is an
example of the rule "a field of a known version that is not in the file" above: `config.version` does not grow,
there is no migration, the file is completed on the nearest write. Any other value (a typo, not a string) is a field
error: the value is replaced with the default, the error goes to `field_errors`. The field's meaning is who may
open the node's settings page: `localhost` — only this machine, `localnet` — also devices on the
local network (private addresses `10/8`, `172.16/12`, `192.168/16`, `169.254/16`). It is not about the Mesh Town
network and not about traffic. It is changed from the page (`PUT /api/config`) or by the command
`meshtown access localhost|localnet`; on a stopped node the command writes the field to the file itself.

### 8.2 The `web-password` file

It sits in the state directory next to `web-token`, not in `config.json`. Mode `0600` (on Windows
the permission check is not done), written via a temporary file `.web-password.tmp-<pid>` and a rename.
One line of JSON:

```json
{"version":1,"alg":"pbkdf2-sha256","iterations":600000,"salt":"<base64, 16 bytes>","hash":"<base64, 32 bytes>","set_at":1790000000}
```

- `version` — 1 today; `alg` — `pbkdf2-sha256` (PBKDF2-HMAC-SHA256: a strong function from the
  `ring` crypto library, which the project already has); `iterations` — the number of hash repetitions;
  `salt` — a random salt, `hash` — the result; `set_at` — when the password was set, Unix seconds.
- The password itself is not stored in the file, and the node does not keep it in memory either: the file is re-read on every
  sign-in and every state request, so `meshtown password reset` takes effect on a running node
  at once, and on a stopped one simply deletes the file.
- **No file — no password**: sign-in only by token, the first sign-in via a link with a temporary token
  requires setting a password.
- **An unreadable file** (not JSON of the format, `version` not 1, unknown `alg`, the number of iterations outside
  the allowed range — zero or more than 10,000,000, salt or hash of the wrong size, file larger than 4 KiB, permissions
  wider than `0600`) counts as the same "no password": sign-in only by token, and once per run a line
  is written to the log: "password file is unreadable (reason): sign-in by token only". The node does not delete the file;
  a new password overwrites it.
- A successful sign-in with `iterations` below the current constant (600,000) rehashes the password and
  rewrites the file; a rewrite error goes to the log, and the sign-in still succeeds.
- Password length is 8–256 characters; the bound is on write and is not checked on read.

### 8.3 The `net` section (choice of network stack)

An optional section: what the node uses to talk over the network. No section or field — the build default.

- `transports` — transports (ways of delivering packets between nodes); a non-empty list without repeats.
  Allowed: `["hyper"]` (default); `"mem"` — tests only, unavailable from `config.json`.
- `discovery` — peer discovery (where we learn the addresses of other nodes); a non-empty list without repeats of
  `invite`, `cache`, `hyperdht`. Default `["invite","cache","hyperdht"]`.
- `sync` — replica sync (how the node exchanges the network's shared state); the string `"mesh"` (default):
  its own topic membership (flooding over the node's connections) and its own records (signed by the author, reconciled with
  peers), stored as files in the `replica/` directory.
- An error in a field (type, empty list, repeat, unknown or not built-in name) is a warning
  and the default for the whole field; the node starts. An unknown key in `net` goes to the log, as everywhere.
- There is no command-line flag; the page neither shows nor edits the section; `PUT /api/config` stores
  it and rejects a wrong value like any field error. It is changed by restarting the node.
- **Where the section sits.** Like the other settings sections, `net` is inside the file's `"config"` field, for example
  `{"config": {…, "net": {"sync": "mem"}}}`. At the top level of the file the node skips it with the line "unknown
  fields … net" and does not change the choice.
- **Written only as differences from the build default:** a field equal to the default is not written, and if all three
  are equal there is no section either. The reason: a future change of the default must not be frozen by a written
  value. This is an exception to the rule of section 8 "the file is completed on the next write".
  An old binary loses the `net` section on write (section 8, unknown field).

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
- **Process keys are ignored.** `socks.*`, `proxy.*`, `web.*`, `log.*`, `node.memory_*`, `autostart.*` and
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
- **Left networks.** A network that was left is not deleted: its folder is renamed to
  `nets/left-<id>-<UTC time>/` (for the main network, `nets/left-main-<UTC time>/` with a copy of its
  `config.json` and its cache files). Folders named `left-*` are never started or reused as ids.
- **Rolling back to an older version.** An older build does not look into `nets/` at all: it runs the main
  network as before and leaves the slot files untouched. After the upgrade the slots start again.
