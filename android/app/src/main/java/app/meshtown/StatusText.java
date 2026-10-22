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

    /** The line above the Connect button. */
    static Line strip(NodeState s, boolean canConnect, boolean vpnOn) {
        if (s.excluded) {
            return new Line(NodeState.EXCLUDED_TEXT, 0);
        }
        if (!s.inNetwork) {
            return new Line("Join or create a network first", 0);
        }
        if (vpnOn) {
            return connected(s);
        }
        return new Line(canConnect ? "Ready. Tap Connect to open sites through Mesh Town"
                : "Looking for other members…", 0);
    }

    private static Line connected(NodeState s) {
        if ("exit".equals(s.route)) {
            return new Line(DOT + "Connected via " + s.exitLabel(), GREEN);
        }
        if ("unavailable".equals(s.route)) {
            return new Line(DOT + "Connected, but " + s.exitLabel() + " is offline" + DASH + "choose another exit",
                    ORANGE);
        }
        return new Line(DOT + "Connected · sites open directly" + DASH + "choose an exit above", GREEN);
    }

    /** The status notification: the title is the state, the text says where sites open. */
    static Notice notice(NodeState s, boolean vpnOn) {
        if (!vpnOn || !s.inNetwork) {
            return new Notice("Not connected", s.describe(false));
        }
        if ("exit".equals(s.route)) {
            return new Notice("Connected", "Sites open via " + s.exitLabel());
        }
        if ("unavailable".equals(s.route)) {
            return exitOffline(s);
        }
        return new Notice("Connected", "Sites open directly. Choose an exit in the app to open blocked sites.");
    }

    /** The alert for an exit that went offline. */
    static Notice exitOffline(NodeState s) {
        return new Notice("Exit offline", s.exitLabel() + " is offline" + DASH + "choose another exit");
    }

    /** Subtitle of the Quick Settings tile. */
    static String tileSubtitle(NodeState s, boolean vpnOn) {
        if (!vpnOn) {
            return "Off";
        }
        boolean viaExit = s.inNetwork && ("exit".equals(s.route) || "unavailable".equals(s.route));
        return "On · " + (viaExit ? s.exitLabel() : "directly");
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
        return "This phone leaves the network and forgets its settings. "
                + "To come back, you'll need a new invite from a member.";
    }

    private static boolean hasNetwork(NodeState s) {
        return s.inNetwork || s.excluded;
    }
}
