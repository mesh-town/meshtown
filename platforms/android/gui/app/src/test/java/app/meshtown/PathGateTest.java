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
        PathGate.Attempt a = g.begin(0);
        assertNotNull(a);
        assertEquals(0, a.number);
        g.finish(a.key, true, 0);
        assertTrue(g.ready());
    }

    @Test
    public void directWithChosenExitDoesNotProbe() {
        assertNull(on(CHOSEN_DIRECT).begin(0));
    }

    @Test
    public void directWithoutChosenExitProbes() {
        assertNotNull(on(DIRECT).begin(0));
    }

    @Test
    public void unavailableResetsAndDoesNotProbe() {
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, true, 0);
        g.observe(LOST, true);
        assertFalse(g.ready());
        assertNull(g.begin(0));
    }

    @Test
    public void exitChangeResets() {
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, true, 0);
        g.observe(VIA_B, true);
        assertFalse(g.ready());
        assertNotNull(g.begin(0));
    }

    @Test
    public void lateResultForOldExitIsIgnored() {
        PathGate g = on(VIA_A);
        PathGate.Attempt old = g.begin(0);
        g.observe(VIA_B, true);
        g.finish(old.key, true, 0);
        assertFalse(g.ready());
        assertNotNull(g.begin(0));
    }

    @Test
    public void tunnelOffResets() {
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, true, 0);
        g.observe(VIA_A, false);
        assertFalse(g.ready());
        assertNull(g.begin(0));
        g.observe(VIA_A, true);
        assertNotNull(g.begin(0));
    }

    @Test
    public void nodeUnreachableResets() {
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, true, 0);
        g.observe(NodeState.UNREACHABLE, true);
        assertFalse(g.ready());
        assertNull(g.begin(0));
    }

    @Test
    public void outOfNetworkDoesNotProbe() {
        assertNull(on(OUT).begin(0));
    }

    @Test
    public void failedProbeIsRetriedWithNextNumber() {
        PathGate g = on(VIA_A);
        PathGate.Attempt first = g.begin(0);
        g.finish(first.key, false, 0);
        assertFalse(g.ready());
        PathGate.Attempt second = g.begin(0);
        assertEquals(1, second.number);
        g.finish(second.key, false, 0);
        assertEquals(2, g.begin(0).number);
    }

    @Test
    public void onlyOneProbeAtATime() {
        PathGate g = on(VIA_A);
        assertNotNull(g.begin(0));
        assertNull(g.begin(0));
    }

    @Test
    public void readyLatchesWhileRouteHolds() {
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, true, 0);
        g.observe(VIA_A, true);
        assertTrue(g.ready());
        assertNull(g.begin(0));
    }

    private static NodeState unreachableButInNetwork() {
        return new NodeState(false, true, false, "exit", "a", true, false, false, true);
    }

    @Test
    public void excludedNodeDoesNotProbeAndResets() {
        NodeState removed = NodeState.parse("{" + NET + ",\"members\":{\"me_excluded\":true}}");
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, true, 0);
        g.observe(removed, true);
        assertFalse(g.ready());
        assertNull(g.begin(0));
    }

    @Test
    public void excludedFlagAloneBlocksProbe() {
        NodeState removed = new NodeState(true, true, true, "exit", "a", true, false, false, true);
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, true, 0);
        g.observe(removed, true);
        assertFalse(g.ready());
        assertNull(g.begin(0));
    }

    @Test
    public void unavailableRouteAloneResetsAndBlocksProbe() {
        NodeState lost = new NodeState(true, true, false, "unavailable", "a", true, false, false, false);
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, true, 0);
        g.observe(lost, true);
        assertFalse(g.ready());
        assertNull(g.begin(0));
    }

    @Test
    public void unreachableNodeInNetworkResetsAndBlocksProbe() {
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, true, 0);
        g.observe(unreachableButInNetwork(), true);
        assertFalse(g.ready());
        assertNull(g.begin(0));
    }

    @Test
    public void unreachableNodeInNetworkDoesNotStartProbe() {
        PathGate g = new PathGate();
        g.observe(unreachableButInNetwork(), true);
        assertNull(g.begin(0));
    }

    @Test
    public void resultForForeignKeyKeepsProbeInFlight() {
        PathGate g = on(VIA_A);
        assertNotNull(g.begin(0));
        g.finish("exit|other", true, 0);
        assertFalse(g.ready());
        assertNull(g.begin(0));
    }

    @Test
    public void resultForOldKeyNeverMakesNewKeyReady() {
        PathGate g = on(VIA_A);
        String oldKey = g.begin(0).key;
        g.observe(VIA_B, true);
        assertNotNull(g.begin(0));
        g.finish(oldKey, true, 0);
        assertFalse(g.ready());
        assertNull(g.begin(0));
    }

    @Test
    public void exitReturningAfterUnavailableProbesAgainFromZero() {
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, false, 0);
        g.observe(LOST, true);
        assertNull(g.begin(0));
        g.observe(VIA_A, true);
        PathGate.Attempt again = g.begin(0);
        assertNotNull(again);
        assertEquals(0, again.number);
        g.finish(again.key, true, 0);
        assertTrue(g.ready());
    }

    @Test
    public void retryAfterFailureThenSuccessBecomesReady() {
        PathGate g = on(VIA_A);
        g.finish(g.begin(0).key, false, 0);
        assertFalse(g.ready());
        PathGate.Attempt second = g.begin(0);
        assertEquals(1, second.number);
        g.finish(second.key, true, 0);
        assertTrue(g.ready());
        assertNull(g.begin(0));
    }

    private static void fail(PathGate g, long t) {
        PathGate.Attempt a = g.begin(t);
        assertNotNull(a);
        g.finish(a.key, false, t);
    }

    private static void failThrice(PathGate g) {
        fail(g, 0);
        fail(g, 0);
        fail(g, 0);
    }

    @Test
    public void thirdFailureRetriesAtOnce() {
        PathGate g = on(VIA_A);
        failThrice(g);
        PathGate.Attempt a = g.begin(0);
        assertNotNull(a);
        assertEquals(3, a.number);
    }

    @Test
    public void fourthFailurePausesUntilThirtySeconds() {
        PathGate g = on(VIA_A);
        failThrice(g);
        fail(g, 1000);
        assertNull(g.begin(1000 + 29_999));
        PathGate.Attempt a = g.begin(1000 + 30_000);
        assertNotNull(a);
        assertEquals(4, a.number);
    }

    @Test
    public void everyFailureAfterFourthPausesAgain() {
        PathGate g = on(VIA_A);
        failThrice(g);
        fail(g, 1000);
        fail(g, 31_000);
        assertNull(g.begin(31_000 + 29_999));
        assertNotNull(g.begin(31_000 + 30_000));
    }

    @Test
    public void pauseCountsFromFailureNotFromStart() {
        PathGate g = on(VIA_A);
        failThrice(g);
        PathGate.Attempt a = g.begin(0);
        g.finish(a.key, false, 8000);
        assertNull(g.begin(8000 + 29_999));
        assertNotNull(g.begin(8000 + 30_000));
    }

    @Test
    public void successAfterPauseLatchesReady() {
        PathGate g = on(VIA_A);
        failThrice(g);
        fail(g, 0);
        PathGate.Attempt a = g.begin(30_000);
        g.finish(a.key, true, 31_000);
        assertTrue(g.ready());
        assertNull(g.begin(100_000));
    }

    @Test
    public void keyChangeDuringPauseProbesAtOnce() {
        PathGate g = on(VIA_A);
        failThrice(g);
        fail(g, 0);
        g.observe(VIA_B, true);
        PathGate.Attempt a = g.begin(1);
        assertNotNull(a);
        assertEquals(0, a.number);
    }

    @Test
    public void resetDuringPauseProbesAtOnce() {
        PathGate g = on(VIA_A);
        failThrice(g);
        fail(g, 0);
        g.observe(VIA_A, false);
        g.observe(VIA_A, true);
        PathGate.Attempt a = g.begin(1);
        assertNotNull(a);
        assertEquals(0, a.number);
    }

    @Test
    public void lateFailureForOldKeyDoesNotPauseNewKey() {
        PathGate g = on(VIA_A);
        PathGate.Attempt old = g.begin(0);
        g.observe(VIA_B, true);
        failThrice(g);
        g.finish(old.key, false, 5000);
        PathGate.Attempt a = g.begin(5001);
        assertNotNull(a);
        assertEquals(3, a.number);
    }
}
