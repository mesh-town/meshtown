# Security policy

## Supported versions

Only the latest released 0.3.x version receives security fixes.

## Reporting a vulnerability

Do not open a public issue for a security problem. Use GitHub private vulnerability reporting for this repository
(the "Security" tab, "Report a vulnerability") with a description, the affected version and steps to reproduce. You should get an answer within a week; the fix and the disclosure date
are agreed with the reporter.

## Scope notes

- A network key lets its holder find the network's records in the DHT, see who its members are and pass the
  handshake, but not join. A node gets in only when a network admin admits it to the member list (an invite does
  that at once).
- A member can use the ports published to it and, if you are an exit, your connection. A network admin removes a node
  that should no longer be in the network; the key does not need to change.
- Keep keys secret. Pass them through `MESHTOWN_KEY` or the settings file, not the command line, where other users
  of the computer can see them.
- An invite is single-use and expires after a day. Hand the string over in person.
- The settings file (`config.json`) holds the network key and the node secret; the node creates it with mode `0600`
  in a `0700` directory. Do not share or commit it.
- The node page is served over plain HTTP. In the default `localnet` mode local network devices can reach it, and
  the password, token and cookie travel unencrypted; on an untrusted network use `meshtown access localhost`. Do not
  publish the page port through `ports.published`: such connections arrive from `127.0.0.1` and bypass the access mode.
- The exit refuses private, loopback, link-local, CGNAT and multicast addresses, but an exit still sees which hosts a
  member connects to. Mesh Town is not an anonymity network.
- macOS builds are signed ad hoc and are not notarized; verify downloads against `SHA256SUMS`.
- Problems in the DHT and transport libraries should go to their projects ([townjump](https://github.com/mesh-town/townjump)
  for the Rust port, Holesail and HyperDHT upstream for protocol-level issues); this repository covers the Mesh Town
  node, page and packaging.
