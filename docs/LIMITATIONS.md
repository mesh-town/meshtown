# Limitations

What Mesh Town does not do in the 0.3.x line, in detail. The short list is in [README.md](../README.md#limitations).

- **No TUN and no capture of all system traffic.** Mesh Town gives a local SOCKS5/HTTP proxy and published TCP ports;
  a program must be able to use a proxy itself (`socks5h://`, a browser setting) or connect to `127.0.0.1:<port>`.
- **Published ports are TCP only**, and only for programs listening on the publishing computer itself.
- **No authentication in SOCKS5.** By default the proxy listens only on `127.0.0.1`.
- **UDP through the proxy only with `UDP ASSOCIATE`, and no `BIND`.** `UDP ASSOCIATE` works with an exit that
  supports it (a current build of Mesh Town); an exit without UDP support is skipped and the next one in
  `route.exits` is tried, and if none supports it the answer is code `0x07`, "command not supported". With no exits
  chosen, datagrams are sent directly from the node. Datagram fragmentation is not supported (such datagrams are
  dropped), `BIND` still gets code `0x07`, and an association without traffic for 10 minutes is closed.
- **IPv6 is partial.** A SOCKS5 request or an HTTP `CONNECT [v6]:port` with an IPv6 address works through an exit
  that has IPv6 connectivity. For a domain name an exit tries its IPv4 addresses first (up to four, one after
  another) and falls back to its IPv6 addresses (AAAA records) when none of them opens, so a name that has only
  AAAA records opens through an exit with IPv6 connectivity and is refused quickly through one without it. A node
  itself resolves names for direct connections to IPv4 addresses only, and the proxy listens on IPv4 only. An exit
  refuses IPv6 addresses in its own local network segment, as it does for private IPv4 ranges.
- **The exit is picked from a list you set.** The node uses the exits in the `route.exits` list, in order; it cannot
  choose an exit by country or operator.
- **A refusal is final.** The node moves to the next exit in the list only when the current one cannot be reached;
  if an exit answers with a refusal, the client gets it.
- **A direct connection may not open on some mobile networks.** Behind home routers and most NATs the nodes connect
  directly; some mobile operators block that, and then the node cannot reach the others from that network.
- **No traffic accounting or speed limits** per member, and no limit on simultaneous connections.
- **One hop only.** Client → your node → a member → the site; chains of two or more members are not supported.
- **No HTTP inspection inside the tunnel.** The node moves bytes and does not look into them, so there is no filter by
  path, headers or content.

Releases of the 0.3.x line are a working network node, not a hardened product. See [SECURITY.md](../SECURITY.md) for the
scope and how to report a problem.
