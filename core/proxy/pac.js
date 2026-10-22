// Mesh Town proxy auto-config (PAC). Served by the node at GET /proxy.pac on the proxy port.
// Local hosts go DIRECT; everything else goes through the node's proxy.
// Decided by the host text only: no dnsResolve/isInNet, so no DNS query leaves the browser.
function FindProxyForURL(url, host) {
  if (isLocalHost(host.toLowerCase())) {
    return "DIRECT";
  }
  return "__ROUTE__";
}

function isLocalHost(h) {
  if (h.indexOf(":") >= 0) {
    return h == "::1" || h == "[::1]";
  }
  if (/^\d+\.\d+\.\d+\.\d+$/.test(h)) {
    return isPrivateIPv4(h.split("."));
  }
  return isPlainHostName(h) || dnsDomainIs(h, ".localhost") || dnsDomainIs(h, ".local");
}

function isPrivateIPv4(octets) {
  var a = parseInt(octets[0], 10);
  var b = parseInt(octets[1], 10);
  return a == 127 || a == 10 || (a == 172 && b >= 16 && b <= 31) ||
    (a == 192 && b == 168) || (a == 169 && b == 254);
}
