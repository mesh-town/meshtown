package app.meshtown;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class RestoreGateTest {
    private static final NodeState IN = NodeState.parse("{\"network\":{\"id\":\"n1\"}}");
    private static final NodeState OUT = NodeState.parse("{}");

    private static RestoreGate armed(long t) {
        RestoreGate g = new RestoreGate();
        g.arm(true, t);
        return g;
    }

    private static boolean wait(RestoreGate g, NodeState s, boolean ready, long t) {
        return g.update(true, s, false, ready, t);
    }

    @Test
    public void idleNeverStartsNorHolds() {
        RestoreGate g = new RestoreGate();
        assertFalse(wait(g, IN, true, 60_000));
        assertFalse(g.holding());
    }

    @Test
    public void armNotWantedStaysIdle() {
        RestoreGate g = new RestoreGate();
        g.arm(false, 0);
        assertFalse(wait(g, IN, true, 1000));
        assertFalse(g.holding());
    }

    @Test
    public void waitingBeforeDeadlineDoesNotStartButHolds() {
        RestoreGate g = armed(0);
        assertFalse(wait(g, IN, false, 59_999));
        assertTrue(g.holding());
    }

    @Test
    public void pathReadyStartsOnceAndHoldsUntilTunnelSeen() {
        RestoreGate g = armed(0);
        assertTrue(wait(g, IN, true, 5000));
        assertFalse(wait(g, IN, true, 6000));
        assertTrue(g.holding());
        assertFalse(g.update(true, IN, true, true, 7000));
        assertFalse(g.holding());
    }

    @Test
    public void startedReleasesAfterGrace() {
        RestoreGate g = armed(0);
        assertTrue(wait(g, IN, true, 5000));
        assertFalse(wait(g, IN, true, 5000 + 9_999));
        assertTrue(g.holding());
        assertFalse(wait(g, IN, true, 5000 + 10_000));
        assertFalse(g.holding());
        assertFalse(wait(g, IN, true, 5000 + 11_000));
    }

    @Test
    public void deadlineStartsAtExactlySixtySeconds() {
        RestoreGate g = armed(0);
        assertFalse(wait(g, IN, false, 59_999));
        assertTrue(wait(g, IN, false, 60_000));
    }

    @Test
    public void outOfNetworkWaitsPastDeadlineThenStarts() {
        RestoreGate g = armed(0);
        assertFalse(wait(g, OUT, false, 59_999));
        assertTrue(g.holding());
        assertFalse(wait(g, OUT, false, 60_000));
        assertFalse(g.holding());
        assertTrue(wait(g, IN, false, 61_000));
    }

    @Test
    public void tunnelStartedElsewhereCancels() {
        RestoreGate g = armed(0);
        assertFalse(g.update(true, IN, true, false, 1000));
        assertFalse(wait(g, IN, true, 2000));
        assertFalse(g.holding());
    }

    @Test
    public void notWantedCancelsEvenAtDeadline() {
        RestoreGate g = armed(0);
        assertFalse(g.update(false, IN, false, true, 60_000));
        assertFalse(wait(g, IN, true, 61_000));
        assertFalse(g.holding());
    }

    @Test
    public void secondBootDoesNotExtendDeadline() {
        RestoreGate g = armed(0);
        g.arm(true, 40_000);
        assertFalse(wait(g, IN, false, 59_999));
        assertTrue(wait(g, IN, false, 60_000));
    }

    @Test
    public void armAfterCancelStartsNewWait() {
        RestoreGate g = armed(0);
        assertFalse(g.update(false, IN, false, false, 1000));
        g.arm(true, 100_000);
        assertFalse(wait(g, IN, false, 159_999));
        assertTrue(wait(g, IN, false, 160_000));
    }
}
