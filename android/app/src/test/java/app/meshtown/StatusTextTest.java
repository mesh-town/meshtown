package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import org.junit.Test;

public class StatusTextTest {
    private static final NodeState OUT = NodeState.parse("{}");
    private static final NodeState EXCLUDED =
            NodeState.parse("{\"network\":{\"name\":\"probe\"},\"members\":{\"me_excluded\":true}}");
    private static final NodeState DIRECT = NodeState.parse("{\"network\":{\"name\":\"probe\"}}");
    private static final NodeState VIA = NodeState.parse("{\"network\":{\"name\":\"probe\"},\"route\":"
            + "{\"state\":\"exit\",\"exits\":[{\"id\":\"a\",\"name\":\"peer-one\"}]}}");
    private static final NodeState LOST = NodeState.parse("{\"network\":{\"name\":\"probe\"},\"route\":"
            + "{\"state\":\"unavailable\",\"exits\":[{\"id\":\"a\",\"name\":\"peer-one\",\"online\":false}]}}");

    private static void line(String text, int dot, StatusText.Line l) {
        assertEquals(text, l.text);
        assertEquals(dot, l.dotColor);
    }

    @Test
    public void stripForExcluded() {
        line(NodeState.EXCLUDED_TEXT, 0, StatusText.strip(EXCLUDED, false, false));
    }

    @Test
    public void stripWithoutNetwork() {
        line("Join or create a network first", 0, StatusText.strip(OUT, false, false));
    }

    @Test
    public void stripWhileSearching() {
        line("Looking for other members…", 0, StatusText.strip(DIRECT, false, false));
    }

    @Test
    public void stripWhenReady() {
        line("Ready. Tap Connect to open sites through Mesh Town", 0, StatusText.strip(DIRECT, true, false));
    }

    @Test
    public void stripConnectedDirectly() {
        line("● Connected · sites open directly — choose an exit above", StatusText.GREEN,
                StatusText.strip(DIRECT, true, true));
    }

    @Test
    public void stripConnectedViaExit() {
        line("● Connected via peer-one", StatusText.GREEN, StatusText.strip(VIA, true, true));
    }

    @Test
    public void stripExitOffline() {
        line("● Connected, but peer-one is offline — choose another exit", StatusText.ORANGE,
                StatusText.strip(LOST, true, true));
    }

    @Test
    public void noticeWhenOff() {
        StatusText.Notice n = StatusText.notice(DIRECT, false);
        assertEquals("Not connected", n.title);
        assertEquals("Not connected", n.text);
        assertEquals("Join a network in the app", StatusText.notice(OUT, false).text);
    }

    @Test
    public void noticeWhenConnected() {
        StatusText.Notice n = StatusText.notice(DIRECT, true);
        assertEquals("Connected", n.title);
        assertEquals("Sites open directly. Choose an exit in the app to open blocked sites.", n.text);
        assertEquals("Sites open via peer-one", StatusText.notice(VIA, true).text);
        assertEquals("Connected", StatusText.notice(VIA, true).title);
    }

    @Test
    public void noticeWhenExitOffline() {
        StatusText.Notice n = StatusText.notice(LOST, true);
        assertEquals("Exit offline", n.title);
        assertEquals("peer-one is offline — choose another exit", n.text);
        assertEquals(n.text, StatusText.exitOffline(LOST).text);
    }

    @Test
    public void tileSubtitles() {
        assertEquals("Off", StatusText.tileSubtitle(DIRECT, false));
        assertEquals("On · directly", StatusText.tileSubtitle(DIRECT, true));
        assertEquals("On · peer-one", StatusText.tileSubtitle(VIA, true));
    }

    @Test
    public void excludedTextPointsToTheMenu() {
        assertEquals("Removed from the network — tap ⋮ → Leave network and reset",
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
}
