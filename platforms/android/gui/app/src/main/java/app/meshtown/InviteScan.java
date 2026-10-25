package app.meshtown;

import java.util.Locale;

/** What a scanned photo means for the invite field; no Android types, so it runs in JVM tests. */
final class InviteScan {
    /** The link the page uses for its "Scan QR" button; the shell intercepts it. */
    static final String URL = "meshtown-app://scan-invite";

    enum Outcome { INVITE, NOT_INVITE, NOTHING_FOUND, CANCELLED, UNAVAILABLE }

    private InviteScan() {}

    /** The trimmed lower-case invite, or null when the text is not one. */
    static String inviteOf(String raw) {
        if (raw == null) {
            return null;
        }
        String text = raw.trim().toLowerCase(Locale.ROOT);
        return text.startsWith("mesh-invite") ? text : null;
    }

    static Outcome outcome(boolean taken, String decoded) {
        if (!taken) {
            return Outcome.CANCELLED;
        }
        if (decoded == null) {
            return Outcome.NOTHING_FOUND;
        }
        return inviteOf(decoded) == null ? Outcome.NOT_INVITE : Outcome.INVITE;
    }

    /** The toast text, or null when nothing needs saying. */
    static String message(Outcome o) {
        switch (o) {
            case UNAVAILABLE:
                return "Scanner unavailable on this device";
            case NOT_INVITE:
                return "This QR code is not an invite";
            case NOTHING_FOUND:
                return "No QR code found in the photo";
            default:
                return null;
        }
    }
}
