//! Answer page texts. `{key}` marks a substitution filled (and escaped) by the renderer.

pub(super) const PAGE_BRAND: &str = "Mesh Town";
pub(super) const PAGE_WHO: &str = "This page is from Mesh Town on {me}, not from {host}.";
pub(super) const PAGE_TRY_AGAIN: &str = "Try again";
pub(super) const PAGE_COPY: &str = "Copy";
pub(super) const PAGE_COPY_MESSAGE: &str = "Copy message";
pub(super) const PAGE_COPIED: &str = "Copied";
pub(super) const PAGE_COPY_FAIL_TEXT: &str =
    "Clipboard unavailable — select the text and copy it manually.";
pub(super) const PAGE_COPY_FAIL_ADDRESS: &str =
    "Clipboard unavailable — select the address and copy it manually.";
pub(super) const PAGE_ONLINE: &str = "online";
pub(super) const PAGE_OFFLINE: &str = "offline";

pub(super) const PAGE_TITLE_NOT_FOUND: &str = "No member named {slug}";
pub(super) const PAGE_TITLE_NO_NAME: &str = "This address has no member name";
pub(super) const PAGE_H1_NOT_FOUND: &str = "There is no member called “{slug}”";
pub(super) const PAGE_BODY_NOT_FOUND: &str =
    "Mesh Town didn't find a member with this name in your networks.";
pub(super) const PAGE_DID_YOU_MEAN: &str = "Did you mean";
pub(super) const PAGE_OR: &str = " or ";
pub(super) const PAGE_HINT_NOT_FOUND: &str = "Addresses are made from member names — “Home OpenWrt” becomes home-openwrt. Some names change more than that, so it's easiest to copy a ready-made address from the Ports tab of Mesh Town.";

pub(super) const PAGE_TITLE_AMBIGUOUS: &str = "Choose a member · {host}";
pub(super) const PAGE_H1_AMBIGUOUS: &str = "{count} members match {host}";
pub(super) const PAGE_BODY_AMBIGUOUS: &str = "Pick one below. Each of these addresses always leads to the same member — bookmark it instead of {host}.";
pub(super) const PAGE_COPY_ADDRESS_LABEL: &str = "Copy address {url}";
pub(super) const PAGE_HINT_AMBIGUOUS: &str =
    "The Ports tab of Mesh Town shows the network of each member that shared ports with you.";

pub(super) const PAGE_TITLE_NOT_OPENED: &str = "Port {port} isn't shared with you · {host}";
pub(super) const PAGE_H1_NOT_OPENED: &str = "{name} hasn't shared port {port} with you";
pub(super) const PAGE_BODY_NOT_OPENED: &str = "{name} is a member of your network, but its owner hasn't opened this port to you, so this address doesn't open.";
pub(super) const PAGE_OPEN_PORT: &str = "Open port {p}";
pub(super) const PAGE_OPEN_PORT_NAMED: &str = "Open port {p} ({port_name})";
pub(super) const PAGE_H2_SHARED_PORTS: &str = "Ports {name} shared with you";
pub(super) const PAGE_NO_SHARED_PORTS: &str = "{name} hasn't shared any ports with you yet.";
pub(super) const PAGE_ALSO_OFFLINE: &str = "{name} is also offline right now.";
pub(super) const PAGE_H2_GET_ACCESS: &str = "How to get access";
pub(super) const PAGE_BODY_GET_ACCESS: &str = "Ask the owner of {name} to share port {port} with you. On {name}'s Mesh Town page, they need to:";
pub(super) const PAGE_STEP_OPEN_TAB: &str = "Open the Ports tab.";
pub(super) const PAGE_STEP_PUBLISH: &str = "Under “My published ports”, publish port {port} — or click “Edit” if it's already there.";
pub(super) const PAGE_STEP_SHARE: &str =
    "In “Shared with”, choose “All members”, or “Selected members” and tick {me}.";
pub(super) const PAGE_MESSAGE_LABEL: &str = "Message for the owner:";
pub(super) const PAGE_MESSAGE: &str = "Hi! Could you share port {port} of {name} with me in Mesh Town? Ports tab → My published ports → port {port}, Shared with: All members, or Selected members → {me}.";
pub(super) const PAGE_COPY_MESSAGE_LABEL: &str = "Copy message for the owner of {name}";
pub(super) const PAGE_THEN_TRY_AGAIN: &str = "Then click “Try again”.";

pub(super) const PAGE_TITLE_OFFLINE: &str = "{name} is offline";
pub(super) const PAGE_BODY_OFFLINE: &str = "Its owner has shared port {port} with you, so this address will open as soon as {name} is back online.";
pub(super) const PAGE_H2_OTHER_PORTS: &str = "Other ports {name} shared with you";
pub(super) const PAGE_H2_WHAT_TO_DO: &str = "What to do";
pub(super) const PAGE_BODY_OFFLINE_TODO: &str = "Make sure {name} is turned on and connected to the internet, or ask its owner. Then click “Try again”.";

pub(super) const PAGE_TITLE_NO_SERVICE: &str = "Nothing answers on port {port} · {host}";
pub(super) const PAGE_H1_NO_SERVICE: &str = "Nothing answers on port {port} of {name}";
pub(super) const PAGE_BODY_REFUSED: &str =
    "{name} is online, but no program on port {port} accepted the connection.";
pub(super) const PAGE_BODY_TIMED_OUT: &str =
    "{name} is online, but port {port} didn't answer in time.";
pub(super) const PAGE_BODY_NO_SERVICE_TODO: &str = "Ask the owner of {name} to check that port {port} is shared with you and that the program on it is running. On their Mesh Town page, the Ports tab shows who it's shared with and whether it's “responding”.";

pub(super) const PAGE_SCRIPT: &str = "document.addEventListener('click',function(e){\
var b=e.target.closest('[data-copy]');if(!b)return;\
var t=b.getAttribute('data-copy');\
if(t.charAt(0)==='#'){t=document.getElementById(t.slice(1)).textContent.trim();}\
var n=document.getElementById('copy-note');\
var fail=function(){n.textContent=b.getAttribute('data-fail');n.hidden=false;};\
if(!navigator.clipboard){fail();return;}\
navigator.clipboard.writeText(t).then(function(){\
n.hidden=true;var o=b.textContent;b.textContent=b.getAttribute('data-done');\
setTimeout(function(){b.textContent=o;},2000);},fail);});";

pub(super) const PAGE_CSS: &str = "\
:root{color-scheme:light dark;--bg:#f4f5f7;--card:#fff;--fg:#1d2127;--muted:#5f6772;--line:#d9dde3;--accent:#2563eb;--on-accent:#fff}\
@media(prefers-color-scheme:dark){:root{--bg:#14171b;--card:#1d2127;--fg:#e8eaed;--muted:#9aa3ad;--line:#343b44;--accent:#6aa0ff;--on-accent:#0b1220}}\
body{margin:0;padding:24px 16px;background:var(--bg);color:var(--fg);font:14px/1.5 system-ui,-apple-system,Segoe UI,sans-serif}\
.brand{max-width:560px;margin:0 auto 12px;font-weight:600;color:var(--muted)}\
.card{max-width:560px;margin:0 auto;padding:20px 24px;background:var(--card);border:1px solid var(--line);border-radius:12px;box-shadow:0 1px 3px rgba(0,0,0,.08)}\
.card .card{margin:12px 0;padding:12px 16px;box-shadow:none}\
h1{font-size:18px;margin:0 0 12px}h2{font-size:14px;margin:20px 0 8px}p{margin:0 0 12px}\
.hint,.muted{color:var(--muted)}a{color:var(--accent)}\
.mono{font-family:ui-monospace,Menlo,Consolas,monospace;word-break:break-all}\
.badge{margin-left:8px;padding:1px 8px;border:1px solid var(--line);border-radius:10px;font-size:12px;color:var(--muted)}\
.btn{display:inline-block;margin:4px 8px 4px 0;padding:6px 14px;border:1px solid var(--line);border-radius:8px;background:transparent;color:var(--fg);font:inherit;text-decoration:none;cursor:pointer}\
.btn-primary{background:var(--accent);border-color:var(--accent);color:var(--on-accent)}\
.msg{margin:8px 0;padding:10px 12px;border:1px solid var(--line);border-radius:8px;user-select:all}\
ol{margin:0 0 12px;padding-left:22px}ul{margin:0 0 12px;padding-left:18px}";
