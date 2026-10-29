package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class StatusTextTest {
    private static final NodeState OUT = NodeState.parse("{}");
    private static final NodeState EXCLUDED =
            NodeState.parse("{\"network\":{\"label\":\"probe\",\"masked\":\"mt1-...\",\"id_hex\":\"ab\"},\"members\":{\"me_excluded\":true}}");
    private static final NodeState DIRECT = NodeState.parse("{\"network\":{\"label\":\"probe\",\"masked\":\"mt1-...\",\"id_hex\":\"ab\"}}");
    private static final NodeState CHOSEN_DIRECT = NodeState.parse("{\"network\":{\"label\":\"probe\",\"masked\":\"mt1-...\",\"id_hex\":\"ab\"},\"route\":"
            + "{\"state\":\"direct\",\"exits\":[{\"id\":\"a\",\"name\":\"peer-one\"}]}}");
    private static final NodeState VIA = NodeState.parse("{\"network\":{\"label\":\"probe\",\"masked\":\"mt1-...\",\"id_hex\":\"ab\"},\"route\":"
            + "{\"state\":\"exit\",\"exits\":[{\"id\":\"a\",\"name\":\"peer-one\"}]}}");
    private static final NodeState LOST = NodeState.parse("{\"network\":{\"label\":\"probe\",\"masked\":\"mt1-...\",\"id_hex\":\"ab\"},\"route\":"
            + "{\"state\":\"unavailable\",\"exits\":[{\"id\":\"a\",\"name\":\"peer-one\",\"online\":false}]}}");

    private static void line(String text, int dot, StatusText.Line l) {
        assertEquals(text, l.text);
        assertEquals(dot, l.dotColor);
    }

    @Test
    public void stripForExcluded() {
        line(NodeState.EXCLUDED_TEXT, 0, StatusText.strip(EXCLUDED, false, true, false));
    }

    @Test
    public void stripWithoutNetwork() {
        line("Join or create a network first", 0, StatusText.strip(OUT, false, false, false));
        line("Join or create a network first", 0, StatusText.strip(OUT, false, true, false));
    }

    @Test
    public void stripWhileSearching() {
        line("Looking for other members…", 0, StatusText.strip(DIRECT, false, false, false));
    }

    @Test
    public void stripWhenReady() {
        line("Ready. Tap Connect to open sites through Mesh Town", 0, StatusText.strip(DIRECT, true, false, false));
    }

    @Test
    public void stripConnectingDirectly() {
        line("● Connecting…", StatusText.ORANGE, StatusText.strip(DIRECT, true, true, false));
    }

    @Test
    public void stripConnectedDirectly() {
        line("● Connected · sites open directly — choose an exit above", StatusText.GREEN,
                StatusText.strip(DIRECT, true, true, true));
    }

    @Test
    public void stripConnectingViaExit() {
        line("● Connecting via peer-one…", StatusText.ORANGE, StatusText.strip(VIA, true, true, false));
    }

    @Test
    public void stripConnectedViaExit() {
        line("● Connected via peer-one", StatusText.GREEN, StatusText.strip(VIA, true, true, true));
    }

    @Test
    public void stripChosenExitNotFoundYet() {
        line("● Connecting via peer-one…", StatusText.ORANGE, StatusText.strip(CHOSEN_DIRECT, true, true, false));
        line("● Connecting via peer-one…", StatusText.ORANGE, StatusText.strip(CHOSEN_DIRECT, true, true, true));
    }

    @Test
    public void stripExitUnreachable() {
        line("● Waiting for peer-one — choose another exit if it stays offline", StatusText.ORANGE,
                StatusText.strip(LOST, true, true, false));
        line("● Waiting for peer-one — choose another exit if it stays offline", StatusText.ORANGE,
                StatusText.strip(LOST, true, true, true));
    }

    @Test
    public void noticeWhenOff() {
        StatusText.Notice n = StatusText.notice(DIRECT, false, false);
        assertEquals("Not connected", n.title);
        assertEquals("Not connected", n.text);
        assertEquals("Join a network in the app", StatusText.notice(OUT, false, false).text);
    }

    @Test
    public void noticeWhileNodeStarts() {
        StatusText.Notice n = StatusText.notice(NodeState.UNREACHABLE, true, false);
        assertEquals("Connecting…", n.title);
        assertEquals("Mesh Town is starting…", n.text);
        assertEquals("Not connected", StatusText.notice(NodeState.UNREACHABLE, false, false).title);
    }

    @Test
    public void noticeWhileConnecting() {
        StatusText.Notice n = StatusText.notice(VIA, true, false);
        assertEquals("Connecting…", n.title);
        assertEquals("Checking the path through peer-one", n.text);
        assertEquals("Checking the path through peer-one", StatusText.notice(CHOSEN_DIRECT, true, false).text);
        assertEquals("Checking the connection", StatusText.notice(DIRECT, true, false).text);
    }

    @Test
    public void noticeWhenConnected() {
        StatusText.Notice n = StatusText.notice(DIRECT, true, true);
        assertEquals("Connected", n.title);
        assertEquals("Sites open directly. Choose an exit in the app to open sites through it.", n.text);
        assertEquals("Sites open via peer-one", StatusText.notice(VIA, true, true).text);
        assertEquals("Connected", StatusText.notice(VIA, true, true).title);
    }

    @Test
    public void noticeWhenExitUnreachable() {
        StatusText.Notice n = StatusText.notice(LOST, true, true);
        assertEquals("Waiting for exit", n.title);
        assertEquals("Waiting for peer-one — choose another exit if it stays offline", n.text);
        assertEquals("peer-one is offline — choose another exit", StatusText.exitOffline(LOST).text);
        assertEquals("Exit offline", StatusText.exitOffline(LOST).title);
    }

    @Test
    public void tileSubtitles() {
        assertEquals("Off", StatusText.tileSubtitle(DIRECT, false, false));
        assertEquals("Connecting…", StatusText.tileSubtitle(NodeState.UNREACHABLE, true, false));
        assertEquals("Connecting…", StatusText.tileSubtitle(DIRECT, true, false));
        assertEquals("On · directly", StatusText.tileSubtitle(DIRECT, true, true));
        assertEquals("Connecting…", StatusText.tileSubtitle(VIA, true, false));
        assertEquals("On · peer-one", StatusText.tileSubtitle(VIA, true, true));
        assertEquals("Connecting…", StatusText.tileSubtitle(CHOSEN_DIRECT, true, true));
        assertEquals("Waiting for peer-one", StatusText.tileSubtitle(LOST, true, true));
    }

    @Test
    public void excludedTextPointsToTheMenu() {
        assertEquals("Banned from the network — tap ⋮ → Leave network and reset",
                NodeState.EXCLUDED_TEXT);
    }

    @Test
    public void resetInNetwork() {
        assertEquals("Leave network and reset…", StatusText.resetMenuTitle(DIRECT));
        assertEquals("Leave the network \"probe\"?", StatusText.resetTitle(DIRECT));
        assertEquals("This phone leaves the network and forgets its settings. "
                + "To come back, you'll need a new invite from a member.", StatusText.resetMessage(DIRECT));
        assertEquals("Leave network and reset…", StatusText.resetMenuTitle(EXCLUDED));
    }

    @Test
    public void resetWithoutNetwork() {
        assertEquals("Reset app…", StatusText.resetMenuTitle(OUT));
        assertNull(StatusText.resetTitle(OUT));
        assertEquals("Erase all Mesh Town settings on this phone?", StatusText.resetMessage(OUT));
    }

    private static final String RESET_BASE = "This phone leaves the network and forgets its settings. "
            + "To come back, you'll need a new invite from a member.";
    private static final String ONLY_ADMIN_LINE = "You're the only admin of \"probe\". "
            + "After the reset nobody can invite or remove members there. "
            + "Make another member an admin first: Members \u2192 \u22EF \u2192 Make admin.";

    private static NodeState managed(String role, String admins) {
        return NodeState.parse("{\"network\":{\"label\":\"probe\",\"id_hex\":\"ab\",\"kind\":\"Managed\","
                + "\"me_role\":\"" + role + "\",\"admins\":" + admins + "}}");
    }

    @Test
    public void lastAdminIsWarnedInTheResetWindow() {
        String message = StatusText.resetMessage(managed("admin", "[\"me\"]"));
        assertTrue(message.startsWith(RESET_BASE));
        assertTrue(message, message.endsWith(ONLY_ADMIN_LINE));
    }

    @Test
    public void oneOfSeveralAdminsGetsTheUsualResetMessage() {
        assertEquals(RESET_BASE, StatusText.resetMessage(managed("admin", "[\"me\",\"other\"]")));
    }

    @Test
    public void plainMemberGetsTheUsualResetMessage() {
        assertEquals(RESET_BASE, StatusText.resetMessage(managed("member", "[\"other\"]")));
    }

    @Test
    public void peerNetworkKeepsTheUsualResetMessage() {
        assertEquals(RESET_BASE, StatusText.resetMessage(DIRECT));
    }

    @Test
    public void resetButtonsAndTitleStayTheSameForTheLastAdmin() {
        NodeState s = managed("admin", "[\"me\"]");
        assertEquals("Leave network and reset\u2026", StatusText.resetMenuTitle(s));
        assertEquals("Leave the network \"probe\"?", StatusText.resetTitle(s));
    }
}
