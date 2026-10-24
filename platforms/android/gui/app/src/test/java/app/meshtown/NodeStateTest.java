package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertSame;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class NodeStateTest {
    private static final String SELF = "{\"id\":\"me\",\"self\":true}";

    private static String state(String peers, String route) {
        return "{\"network\":{\"id\":\"n1\"}"
                + (peers == null ? "" : ",\"peers\":" + peers)
                + (route == null ? "" : ",\"route\":" + route) + "}";
    }

    private static String peer(String id, boolean exit, boolean online) {
        return "{\"id\":\"" + id + "\",\"exit\":" + exit + ",\"online\":" + online + "}";
    }

    @Test
    public void garbageAndEmptyAreUnreachable() {
        assertSame(NodeState.UNREACHABLE, NodeState.parse("not json"));
        assertSame(NodeState.UNREACHABLE, NodeState.parse(""));
        assertFalse(NodeState.UNREACHABLE.reachable);
    }

    @Test
    public void unreachableHasNoPeerFlags() {
        assertFalse(NodeState.UNREACHABLE.peersKnown);
        assertFalse(NodeState.UNREACHABLE.exitOffered);
        assertFalse(NodeState.UNREACHABLE.exitChosen);
    }

    @Test
    public void networkNameComesFromTheLabel() {
        assertEquals("probe", NodeState.parse("{\"network\":{\"label\":\" probe \",\"masked\":\"x\",\"id_hex\":\"ab\"}}").networkName);
        assertNull(NodeState.parse("{\"network\":{\"label\":null,\"id_hex\":\"ab\"}}").networkName);
    }

    @Test
    public void noNetworkMeansNotInNetwork() {
        NodeState s = NodeState.parse("{}");
        assertTrue(s.reachable);
        assertFalse(s.inNetwork);
        assertFalse(NodeState.parse("{\"network\":null}").inNetwork);
        assertFalse(s.peersKnown);
        assertFalse(s.exitOffered);
        assertFalse(s.exitChosen);
    }

    @Test
    public void networkWithoutPeersHasNoPeersKnown() {
        assertTrue(NodeState.parse(state(null, null)).inNetwork);
        assertFalse(NodeState.parse(state("[]", null)).peersKnown);
        assertFalse(NodeState.parse(state("[" + SELF + "]", null)).peersKnown);
    }

    @Test
    public void foreignPeerMeansPeersKnown() {
        assertTrue(NodeState.parse(state("[" + SELF + "," + peer("p1", false, true) + "]", null)).peersKnown);
        assertTrue(NodeState.parse(state("[" + peer("p1", false, false) + "]", null)).peersKnown);
    }

    @Test
    public void onlineExitPeerIsOffered() {
        assertTrue(NodeState.parse(state("[" + peer("p1", true, true) + "]", null)).exitOffered);
        assertTrue(NodeState.parse(state("[" + SELF + "," + peer("p1", true, true) + "]", null)).exitOffered);
    }

    @Test
    public void offlineOrNonExitPeerIsNotOffered() {
        assertFalse(NodeState.parse(state("[" + peer("p1", true, false) + "]", null)).exitOffered);
        assertFalse(NodeState.parse(state("[" + peer("p1", false, true) + "]", null)).exitOffered);
    }

    @Test
    public void selfExitIsNotOffered() {
        String self = "{\"id\":\"me\",\"self\":true,\"exit\":true,\"online\":true}";
        NodeState s = NodeState.parse(state("[" + self + "]", null));
        assertFalse(s.exitOffered);
        assertFalse(s.peersKnown);
    }

    @Test
    public void nonEmptyRouteExitsMeanChosen() {
        assertTrue(NodeState.parse(state(null, "{\"state\":\"exit\",\"exits\":[{\"id\":\"e1\"}]}")).exitChosen);
        assertFalse(NodeState.parse(state(null, "{\"state\":\"direct\",\"exits\":[]}")).exitChosen);
        assertFalse(NodeState.parse(state(null, "{\"state\":\"direct\"}")).exitChosen);
    }

    @Test
    public void excludedIsNotInNetwork() {
        NodeState s = NodeState.parse("{\"network\":{\"id\":\"n1\"},\"members\":{\"me_excluded\":true}}");
        assertTrue(s.excluded);
        assertFalse(s.inNetwork);
        assertEquals(NodeState.EXCLUDED_TEXT, s.describe(true));
    }

    @Test
    public void activeExitIsPickedAndLabelledByName() {
        String route = "{\"state\":\"exit\",\"exits\":[{\"id\":\"a\",\"name\":\"Alpha\"},"
                + "{\"id\":\"b\",\"name\":\" Beta \",\"active\":true}]}";
        assertEquals("Beta", NodeState.parse(state(null, route)).exitName);
    }

    @Test
    public void singleExitIsPickedWithoutActiveFlag() {
        String route = "{\"state\":\"exit\",\"exits\":[{\"id\":\"a\",\"name\":\"Alpha\"}]}";
        assertEquals("Alpha", NodeState.parse(state(null, route)).exitName);
    }

    @Test
    public void manyInactiveExitsPickNone() {
        String route = "{\"state\":\"unavailable\",\"exits\":[{\"id\":\"a\"},{\"id\":\"b\"}]}";
        NodeState s = NodeState.parse(state(null, route));
        assertNull(s.exitName);
        assertTrue(s.exitOnline);
        assertTrue(s.exitChosen);
    }

    @Test
    public void exitLabelFallsBackToIdWhenNameMissing() {
        assertEquals("a", label("{\"id\":\"a\",\"name\":null}"));
        assertEquals("a", label("{\"id\":\"a\",\"name\":\"  \"}"));
        assertEquals("a", label("{\"id\":\"a\"}"));
        assertEquals("?", label("{}"));
    }

    private static String label(String exit) {
        return NodeState.parse(state(null, "{\"state\":\"exit\",\"exits\":[" + exit + "]}")).exitName;
    }

    @Test
    public void exitOnlineFlagIsRead() {
        String route = "{\"state\":\"unavailable\",\"exits\":[{\"id\":\"a\",\"online\":false}]}";
        assertFalse(NodeState.parse(state(null, route)).exitOnline);
    }

    @Test
    public void describeCoversEveryBranch() {
        assertEquals("Mesh Town is starting\u2026", NodeState.UNREACHABLE.describe(true));
        assertEquals("Join a network in the app", NodeState.parse("{}").describe(true));
        assertEquals("Not connected", NodeState.parse(state(null, null)).describe(false));
        assertEquals("Direct (no exit)", NodeState.parse(state(null, null)).describe(true));
        assertEquals("Direct (no exit)",
                NodeState.parse(state(null, "{\"state\":\"direct\"}")).describe(true));
        String via = "{\"state\":\"exit\",\"exits\":[{\"id\":\"a\",\"name\":\"Alpha\"}]}";
        assertEquals("Connected via Alpha", NodeState.parse(state(null, via)).describe(true));
        assertEquals("Connected via exit", NodeState.parse(state(null, "{\"state\":\"exit\"}")).describe(true));
        String off = "{\"state\":\"unavailable\",\"exits\":[{\"id\":\"a\",\"name\":\"Alpha\",\"online\":false}]}";
        assertEquals("Exit Alpha is offline", NodeState.parse(state(null, off)).describe(true));
        assertEquals("Chosen exits are offline",
                NodeState.parse(state(null, "{\"state\":\"unavailable\"}")).describe(true));
    }

    @Test
    public void exitLostOnlyWhenUnavailableInNetwork() {
        assertTrue(NodeState.parse(state(null, "{\"state\":\"unavailable\"}")).exitLost());
        assertFalse(NodeState.parse(state(null, "{\"state\":\"exit\"}")).exitLost());
        assertFalse(NodeState.parse("{\"route\":{\"state\":\"unavailable\"}}").exitLost());
        assertFalse(NodeState.UNREACHABLE.exitLost());
    }

    private static String network(String extra) {
        return "{\"network\":{\"label\":\"probe\",\"id_hex\":\"ab\"" + extra + "}}";
    }

    @Test
    public void roleAndAdminsComeFromTheNetwork() {
        NodeState s = NodeState.parse(network(",\"kind\":\"Managed\",\"me_role\":\"admin\",\"admins\":[\"a\",\"b\"]"));
        assertEquals("admin", s.meRole);
        assertEquals(2, s.adminCount);
    }

    @Test
    public void memberRoleIsKept() {
        NodeState s = NodeState.parse(network(",\"me_role\":\"member\",\"admins\":[\"a\"]"));
        assertEquals("member", s.meRole);
        assertEquals(1, s.adminCount);
    }

    @Test
    public void peerNetworkHasNoRoleAndNoAdmins() {
        NodeState s = NodeState.parse(network(",\"admins\":[]"));
        assertNull(s.meRole);
        assertEquals(0, s.adminCount);
        assertNull(NodeState.parse(network("")).meRole);
        assertEquals(0, NodeState.parse(network("")).adminCount);
    }

    @Test
    public void noNetworkHasNoRole() {
        assertNull(NodeState.parse("{}").meRole);
        assertEquals(0, NodeState.parse("{}").adminCount);
        assertNull(NodeState.UNREACHABLE.meRole);
    }

    @Test
    public void wrongTypesReadAsNoRole() {
        NodeState s = NodeState.parse(network(",\"me_role\":5,\"admins\":\"x\""));
        assertNull(s.meRole);
        assertEquals(0, s.adminCount);
    }

    @Test
    public void excludedTextSaysBanned() {
        assertEquals("Banned from the network \u2014 tap \u22EE \u2192 Leave network and reset", NodeState.EXCLUDED_TEXT);
    }
}
