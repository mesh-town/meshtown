package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class ConnectGateTest {
    private static final String SELF = "{\"id\":\"me\",\"self\":true}";
    private static final String PLAIN = "{\"id\":\"p1\",\"exit\":false,\"online\":true}";
    private static final String EXIT = "{\"id\":\"p2\",\"exit\":true,\"online\":true}";
    private static final String CHOSEN = "{\"state\":\"exit\",\"exits\":[{\"id\":\"p2\",\"active\":true}]}";

    private static final NodeState OUT = NodeState.parse("{}");
    private static final NodeState ALONE = net("[" + SELF + "]", null);
    private static final NodeState WITH_PEER = net("[" + SELF + "," + PLAIN + "]", null);
    private static final NodeState OFFERED = net("[" + SELF + "," + EXIT + "]", null);
    private static final NodeState PICKED = net("[" + SELF + "," + EXIT + "]", CHOSEN);

    private static NodeState net(String peers, String route) {
        return NodeState.parse("{\"network\":{\"id\":\"n1\"},\"peers\":" + peers
                + (route == null ? "" : ",\"route\":" + route) + "}");
    }

    @Test
    public void outsideNetworkIsClosed() {
        assertFalse(new ConnectGate().update(OUT, 0));
        assertFalse(new ConnectGate().update(NodeState.UNREACHABLE, 1_000_000));
    }

    @Test
    public void searchWaitsForTimeoutFromFirstFrame() {
        ConnectGate gate = new ConnectGate();
        long t0 = 1_000;
        assertFalse(gate.update(ALONE, t0));
        assertFalse(gate.update(ALONE, t0 + ConnectGate.SEARCH_TIMEOUT_MS - 1));
        assertTrue(gate.update(ALONE, t0 + ConnectGate.SEARCH_TIMEOUT_MS));
    }

    @Test
    public void searchTimerStartsAtFirstFrameInNetwork() {
        ConnectGate gate = new ConnectGate();
        assertFalse(gate.update(OUT, 0));
        assertFalse(gate.update(ALONE, 50_000));
        assertFalse(gate.update(ALONE, 50_000 + ConnectGate.SEARCH_TIMEOUT_MS - 1));
        assertTrue(gate.update(ALONE, 50_000 + ConnectGate.SEARCH_TIMEOUT_MS));
    }

    @Test
    public void peersWithChosenExitOpenImmediately() {
        assertTrue(new ConnectGate().update(PICKED, 0));
    }

    @Test
    public void peersWithoutOfferedExitOpenImmediately() {
        assertTrue(new ConnectGate().update(WITH_PEER, 0));
    }

    @Test
    public void offeredButNotChosenWaitsForGrace() {
        ConnectGate gate = new ConnectGate();
        assertFalse(gate.update(OFFERED, 100));
        assertFalse(gate.update(OFFERED, 100 + ConnectGate.PICK_GRACE_MS - 1));
        assertTrue(gate.update(OFFERED, 100 + ConnectGate.PICK_GRACE_MS));
    }

    @Test
    public void chosenExitDuringGraceOpensAtOnce() {
        ConnectGate gate = new ConnectGate();
        assertFalse(gate.update(OFFERED, 0));
        assertTrue(gate.update(PICKED, 10));
    }

    @Test
    public void openStaysOpenWhenPeersVanish() {
        ConnectGate gate = new ConnectGate();
        assertTrue(gate.update(WITH_PEER, 0));
        assertTrue(gate.update(ALONE, 1));
        assertTrue(gate.update(ALONE, 2));
    }

    @Test
    public void leavingNetworkResetsLatchAndTimers() {
        ConnectGate gate = new ConnectGate();
        assertTrue(gate.update(WITH_PEER, 0));
        assertFalse(gate.update(OUT, 10));
        assertFalse(gate.update(ALONE, 20));
        assertFalse(gate.update(ALONE, 20 + ConnectGate.SEARCH_TIMEOUT_MS - 1));
        assertTrue(gate.update(ALONE, 20 + ConnectGate.SEARCH_TIMEOUT_MS));
    }

    @Test
    public void leavingNetworkResetsGraceTimer() {
        ConnectGate gate = new ConnectGate();
        assertFalse(gate.update(OFFERED, 0));
        assertFalse(gate.update(OUT, 100));
        assertFalse(gate.update(OFFERED, 200));
        assertFalse(gate.update(OFFERED, 200 + ConnectGate.PICK_GRACE_MS - 1));
        assertTrue(gate.update(OFFERED, 200 + ConnectGate.PICK_GRACE_MS));
    }
}
