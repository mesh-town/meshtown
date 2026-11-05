package app.meshtown;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class ConnectIntentTest {
    private static final NodeState IN = NodeState.parse("{\"network\":{\"id\":\"n1\"}}");
    private static final NodeState OUT = NodeState.parse("{}");

    @Test
    public void press_while_searching_starts_the_tunnel_when_the_gate_opens() {
        ConnectIntent intent = new ConnectIntent();
        assertTrue(intent.press());
        assertFalse(intent.update(IN, false));
        assertTrue(intent.pending());
        assertTrue(intent.update(IN, true));
        assertFalse(intent.pending());
        assertFalse(intent.update(IN, true));
    }

    @Test
    public void second_press_cancels_the_pending_connect() {
        ConnectIntent intent = new ConnectIntent();
        intent.press();
        assertFalse(intent.press());
        assertFalse(intent.update(IN, true));
    }

    @Test
    public void leaving_the_network_drops_the_pending_connect() {
        ConnectIntent intent = new ConnectIntent();
        intent.press();
        assertFalse(intent.update(OUT, false));
        assertFalse(intent.pending());
        assertFalse(intent.update(IN, true));
    }
}
