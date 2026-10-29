package app.meshtown;

/**
 * The words of the app in one place: the line above the button, the notification, the Quick Settings
 * tile and the reset dialog all read the same node snapshot and the same tunnel flag. Pure logic.
 */
final class StatusText {
    static final int GREEN = 0xFF2E7D32;
    static final int ORANGE = 0xFFEF6C00;
    private static final String DOT = "● ";
    private static final String DASH = " — ";

    private StatusText() {
    }

    /** A line of text; {@code dotColor} is the colour of the leading dot, 0 when the line has none. */
    static final class Line {
        final String text;
        final int dotColor;

        Line(String text, int dotColor) {
            this.text = text;
            this.dotColor = dotColor;
        }
    }

    /** Title and text of the status notification. */
    static final class Notice {
        final String title;
        final String text;

        Notice(String title, String text) {
            this.title = title;
            this.text = text;
        }
    }

    /** Where the tunnel stands while it is on; picked once, then every text switches on it. */
    private enum Phase { STARTING, WAITING, CHECKING_EXIT, CHECKING_DIRECT, READY_EXIT, READY_DIRECT }

    private static Phase phase(NodeState s, boolean pathReady) {
        if (!s.reachable) {
            return Phase.STARTING;
        }
        if ("unavailable".equals(s.route)) {
            return Phase.WAITING;
        }
        if ("exit".equals(s.route)) {
            return pathReady ? Phase.READY_EXIT : Phase.CHECKING_EXIT;
        }
        if (s.exitChosen) {
            return Phase.CHECKING_EXIT;
        }
        return pathReady ? Phase.READY_DIRECT : Phase.CHECKING_DIRECT;
    }

    /** The line above the Connect button. */
    static Line strip(NodeState s, boolean canConnect, boolean vpnOn, boolean pathReady) {
        if (s.excluded) {
            return new Line(NodeState.EXCLUDED_TEXT, 0);
        }
        if (!s.inNetwork) {
            return new Line("Join or create a network first", 0);
        }
        if (vpnOn) {
            return connected(s, phase(s, pathReady));
        }
        return new Line(canConnect ? "Ready. Tap Connect to open sites through Mesh Town"
                : "Looking for other members…", 0);
    }

    private static Line connected(NodeState s, Phase phase) {
        switch (phase) {
            case WAITING:
                return new Line(DOT + "Waiting for " + s.exitLabel() + DASH
                        + "choose another exit if it stays offline", ORANGE);
            case CHECKING_EXIT:
                return new Line(DOT + "Connecting via " + s.exitLabel() + "…", ORANGE);
            case READY_EXIT:
                return new Line(DOT + "Connected via " + s.exitLabel(), GREEN);
            case READY_DIRECT:
                return new Line(DOT + "Connected · sites open directly" + DASH + "choose an exit above", GREEN);
            default:
                return new Line(DOT + "Connecting…", ORANGE);
        }
    }

    /** The status notification: the title is the state, the text says where sites open. */
    static Notice notice(NodeState s, boolean vpnOn, boolean pathReady) {
        if (vpnOn && !s.reachable) {
            return new Notice("Connecting…", s.describe(false));
        }
        if (!vpnOn || !s.inNetwork) {
            return new Notice("Not connected", s.describe(false));
        }
        return connectedNotice(s, phase(s, pathReady));
    }

    private static Notice connectedNotice(NodeState s, Phase phase) {
        switch (phase) {
            case WAITING:
                return new Notice("Waiting for exit", "Waiting for " + s.exitLabel() + DASH
                        + "choose another exit if it stays offline");
            case CHECKING_EXIT:
                return new Notice("Connecting…", "Checking the path through " + s.exitLabel());
            case READY_EXIT:
                return new Notice("Connected", "Sites open via " + s.exitLabel());
            case READY_DIRECT:
                return new Notice("Connected",
                        "Sites open directly. Choose an exit in the app to open sites through it.");
            default:
                return new Notice("Connecting…", "Checking the connection");
        }
    }

    /** The alert for an exit that went offline. */
    static Notice exitOffline(NodeState s) {
        return new Notice("Exit offline", s.exitLabel() + " is offline" + DASH + "choose another exit");
    }

    /** Subtitle of the Quick Settings tile. */
    static String tileSubtitle(NodeState s, boolean vpnOn, boolean pathReady) {
        if (!vpnOn) {
            return "Off";
        }
        if (s.reachable && !s.inNetwork) {
            return "On · directly";
        }
        return tileOn(s, phase(s, pathReady));
    }

    private static String tileOn(NodeState s, Phase phase) {
        switch (phase) {
            case WAITING:
                return "Waiting for " + s.exitLabel();
            case READY_EXIT:
                return "On · " + s.exitLabel();
            case READY_DIRECT:
                return "On · directly";
            default:
                return "Connecting…";
        }
    }

    /** Menu item that wipes the app: it leaves the network when there is one. */
    static String resetMenuTitle(NodeState s) {
        return hasNetwork(s) ? "Leave network and reset…" : "Reset app…";
    }

    /** Dialog title; null when there is no network and the text alone asks the question. */
    static String resetTitle(NodeState s) {
        if (!hasNetwork(s)) {
            return null;
        }
        return s.networkName == null ? "Leave the network?" : "Leave the network \"" + s.networkName + "\"?";
    }

    static String resetMessage(NodeState s) {
        if (!hasNetwork(s)) {
            return "Erase all Mesh Town settings on this phone?";
        }
        String base = "This phone leaves the network and forgets its settings. "
                + "To come back, you'll need a new invite from a member.";
        return isLastAdmin(s) ? base + "\n\n" + onlyAdminLine(s) : base;
    }

    /** The only admin of an administered network: after the reset nobody is left to invite or remove members. */
    private static boolean isLastAdmin(NodeState s) {
        return s.inNetwork && "admin".equals(s.meRole) && s.adminCount == 1;
    }

    private static String onlyAdminLine(NodeState s) {
        String net = s.networkName == null ? "this network" : "\"" + s.networkName + "\"";
        return "You're the only admin of " + net + ". "
                + "After the reset nobody can invite or remove members there. "
                + "Make another member an admin first: Members \u2192 \u22EF \u2192 Make admin.";
    }

    private static boolean hasNetwork(NodeState s) {
        return s.inNetwork || s.excluded;
    }
}
