package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class PathGateTest {
    private static final String NET = "\"network\":{\"id\":\"n1\"}";
    private static final NodeState VIA_A = route("exit", "a");
    private static final NodeState VIA_B = route("exit", "b");
    private static final NodeState LOST = route("unavailable", "a");
    private static final NodeState CHOSEN_DIRECT = route("direct", "a");
    private static final NodeState DIRECT = NodeState.parse("{" + NET + "}");
    private static final NodeState OUT = NodeState.parse("{}");

    private static NodeState route(String state, String name) {
        return NodeState.parse("{" + NET + ",\"route\":{\"state\":\"" + state
                + "\",\"exits\":[{\"id\":\"" + name + "\",\"name\":\"" + name + "\"}]}}");
    }

    private static PathGate on(NodeState s) {
        PathGate g = new PathGate();
        g.observe(s, true);
        return g;
    }

    @Test
    public void notReadyUntilProbePasses() {
        PathGate g = on(VIA_A);
        assertFalse(g.ready());
        PathGate.Attempt a = g.begin();
        assertNotNull(a);
        assertEquals(0, a.number);
        g.finish(a.key, true);
        assertTrue(g.ready());
    }

    @Test
    public void directWithChosenExitDoesNotProbe() {
        assertNull(on(CHOSEN_DIRECT).begin());
    }

    @Test
    public void directWithoutChosenExitProbes() {
        assertNotNull(on(DIRECT).begin());
    }

    @Test
    public void unavailableResetsAndDoesNotProbe() {
        PathGate g = on(VIA_A);
        g.finish(g.begin().key, true);
        g.observe(LOST, true);
        assertFalse(g.ready());
        assertNull(g.begin());
    }

    @Test
    public void exitChangeResets() {
        PathGate g = on(VIA_A);
        g.finish(g.begin().key, true);
        g.observe(VIA_B, true);
        assertFalse(g.ready());
        assertNotNull(g.begin());
    }

    @Test
    public void lateResultForOldExitIsIgnored() {
        PathGate g = on(VIA_A);
        PathGate.Attempt old = g.begin();
        g.observe(VIA_B, true);
        g.finish(old.key, true);
        assertFalse(g.ready());
        assertNotNull(g.begin());
    }

    @Test
    public void tunnelOffResets() {
        PathGate g = on(VIA_A);
        g.finish(g.begin().key, true);
        g.observe(VIA_A, false);
        assertFalse(g.ready());
        assertNull(g.begin());
        g.observe(VIA_A, true);
        assertNotNull(g.begin());
    }

    @Test
    public void nodeUnreachableResets() {
        PathGate g = on(VIA_A);
        g.finish(g.begin().key, true);
        g.observe(NodeState.UNREACHABLE, true);
        assertFalse(g.ready());
        assertNull(g.begin());
    }

    @Test
    public void outOfNetworkDoesNotProbe() {
        assertNull(on(OUT).begin());
    }

    @Test
    public void failedProbeIsRetriedWithNextNumber() {
        PathGate g = on(VIA_A);
        PathGate.Attempt first = g.begin();
        g.finish(first.key, false);
        assertFalse(g.ready());
        PathGate.Attempt second = g.begin();
        assertEquals(1, second.number);
        g.finish(second.key, false);
        assertEquals(2, g.begin().number);
    }

    @Test
    public void onlyOneProbeAtATime() {
        PathGate g = on(VIA_A);
        assertNotNull(g.begin());
        assertNull(g.begin());
    }

    @Test
    public void readyLatchesWhileRouteHolds() {
        PathGate g = on(VIA_A);
        g.finish(g.begin().key, true);
        g.observe(VIA_A, true);
        assertTrue(g.ready());
        assertNull(g.begin());
    }
}
