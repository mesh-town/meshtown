package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class NodeStopperTest {
    private static final class Fake implements NodeStopper.Target {
        final boolean exits;
        final StringBuilder calls = new StringBuilder();
        long waited;

        Fake(boolean exits) {
            this.exits = exits;
        }

        @Override
        public void terminate() {
            calls.append("T");
        }

        @Override
        public boolean awaitExit(long millis) {
            calls.append("W");
            waited = millis;
            return exits;
        }

        @Override
        public void kill() {
            calls.append("K");
        }
    }

    @Test
    public void exitingProcessIsNotKilled() {
        Fake f = new Fake(true);
        assertFalse(NodeStopper.stop(f, 2000));
        assertEquals("TW", f.calls.toString());
        assertEquals(2000, f.waited);
    }

    @Test
    public void unresponsiveProcessIsKilledAfterTheWait() {
        Fake f = new Fake(false);
        assertTrue(NodeStopper.stop(f, 2000));
        assertEquals("TWK", f.calls.toString());
    }
}
