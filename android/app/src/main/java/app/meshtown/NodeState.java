package app.meshtown;

import org.json.JSONArray;
import org.json.JSONObject;

/** The part of the node's {@code /api/state} snapshot the app cares about. */
final class NodeState {
    static final String EXCLUDED_TEXT = "Removed from the network \u2014 tap \u22EE \u2192 Leave network and reset";
    static final NodeState UNREACHABLE = new NodeState(false, false, false, "direct", null, true, false, false, false);

    final boolean reachable;
    /** In a network and not removed from it. */
    final boolean inNetwork;
    /** The members removed this node (or it left): it stays in the data but is out of the network. */
    final boolean excluded;
    /** {@code route.state}: "direct", "exit" or "unavailable". */
    final String route;
    /** Label of the active (or only chosen) exit; null when none is chosen. */
    final String exitName;
    final boolean exitOnline;
    /** In a network and at least one peer other than this node is known. */
    final boolean peersKnown;
    /** Some other peer is online and offers to be an exit. */
    final boolean exitOffered;
    /** {@code route.exits} is not empty: an exit was chosen. */
    final boolean exitChosen;
    /** Name of the network the node belongs to (also when removed from it); null when unknown or none. */
    final String networkName;

    NodeState(boolean reachable, boolean inNetwork, boolean excluded, String route, String exitName, boolean exitOnline,
              boolean peersKnown, boolean exitOffered, boolean exitChosen) {
        this(reachable, inNetwork, excluded, route, exitName, exitOnline, peersKnown, exitOffered, exitChosen, null);
    }

    NodeState(boolean reachable, boolean inNetwork, boolean excluded, String route, String exitName, boolean exitOnline,
              boolean peersKnown, boolean exitOffered, boolean exitChosen, String networkName) {
        this.reachable = reachable;
        this.inNetwork = inNetwork;
        this.excluded = excluded;
        this.route = route;
        this.exitName = exitName;
        this.exitOnline = exitOnline;
        this.peersKnown = peersKnown;
        this.exitOffered = exitOffered;
        this.exitChosen = exitChosen;
        this.networkName = networkName;
    }

    boolean exitLost() {
        return reachable && inNetwork && "unavailable".equals(route);
    }

    static NodeState parse(String json) {
        try {
            JSONObject o = new JSONObject(json);
            boolean excluded = isExcluded(o);
            boolean inNetwork = !o.isNull("network") && o.has("network") && !excluded;
            JSONObject route = o.optJSONObject("route");
            boolean known = inNetwork && hasOthers(o.optJSONArray("peers"), false);
            boolean offered = inNetwork && hasOthers(o.optJSONArray("peers"), true);
            String netName = networkName(o.optJSONObject("network"));
            if (route == null) {
                return new NodeState(true, inNetwork, excluded, "direct", null, true, known, offered, false, netName);
            }
            JSONArray exits = route.optJSONArray("exits");
            JSONObject exit = pickExit(exits);
            String name = exit == null ? null : label(exit);
            boolean online = exit == null || exit.optBoolean("online", true);
            boolean chosen = inNetwork && exits != null && exits.length() > 0;
            return new NodeState(true, inNetwork, excluded, route.optString("state", "direct"), name, online,
                    known, offered, chosen, netName);
        } catch (Exception e) {
            return UNREACHABLE;
        }
    }

    private static String networkName(JSONObject network) {
        String name = network == null || network.isNull("name") ? "" : network.optString("name", "").trim();
        return name.isEmpty() ? null : name;
    }

    /** Any peer without {@code self: true}; with {@code exitOnly}, only online peers that offer an exit. */
    private static boolean hasOthers(JSONArray peers, boolean exitOnly) {
        for (int i = 0; peers != null && i < peers.length(); i++) {
            JSONObject p = peers.optJSONObject(i);
            if (isOther(p, exitOnly)) {
                return true;
            }
        }
        return false;
    }

    private static boolean isOther(JSONObject p, boolean exitOnly) {
        if (p == null || p.optBoolean("self", false)) {
            return false;
        }
        return !exitOnly || (p.optBoolean("exit", false) && p.optBoolean("online", false));
    }

    /** {@code members.me_excluded} of {@code /api/state}. */
    private static boolean isExcluded(JSONObject state) {
        JSONObject members = state.optJSONObject("members");
        return members != null && members.optBoolean("me_excluded", false);
    }

    private static JSONObject pickExit(JSONArray exits) {
        if (exits == null || exits.length() == 0) {
            return null;
        }
        for (int i = 0; i < exits.length(); i++) {
            JSONObject e = exits.optJSONObject(i);
            if (e != null && e.optBoolean("active")) {
                return e;
            }
        }
        return exits.length() == 1 ? exits.optJSONObject(0) : null;
    }

    private static String label(JSONObject exit) {
        String name = exit.isNull("name") ? "" : exit.optString("name", "").trim();
        return name.isEmpty() ? exit.optString("id", "?") : name;
    }

    /** Name of the chosen exit for display; a placeholder when it is unknown. */
    String exitLabel() {
        return exitName == null ? "exit" : exitName;
    }

    /** One-line status: the notification text when no tunnel detail applies. */
    String describe(boolean vpnOn) {
        if (!reachable) {
            return "Mesh Town is starting\u2026";
        }
        if (excluded) {
            return EXCLUDED_TEXT;
        }
        if (!inNetwork) {
            return "Join a network in the app";
        }
        if (!vpnOn) {
            return "Not connected";
        }
        if ("exit".equals(route)) {
            return "Connected via " + exitLabel();
        }
        if ("unavailable".equals(route)) {
            return exitName == null ? "Chosen exits are offline" : "Exit " + exitName + " is offline";
        }
        return "Direct (no exit)";
    }
}
