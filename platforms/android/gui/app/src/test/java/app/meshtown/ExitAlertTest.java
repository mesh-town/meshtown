package app.meshtown;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class ExitAlertTest {
    private static final String NET = "{\"network\":{\"id\":\"n1\"},\"route\":";
    private static final NodeState VIA = NodeState.parse(NET + "{\"state\":\"exit\",\"exits\":[{\"id\":\"a\"}]}}");
    private static final NodeState LOST = NodeState.parse(NET + "{\"state\":\"unavailable\",\"exits\":[{\"id\":\"a\"}]}}");
    private static final NodeState DIRECT = NodeState.parse(NET + "{\"state\":\"direct\"}}");

    @Test
    public void cold_start_without_an_exit_gives_no_alert() {
        ExitAlert alert = new ExitAlert();
        assertEquals(ExitAlert.Action.NONE, alert.update(LOST, true));
        assertEquals(ExitAlert.Action.NONE, alert.update(LOST, true));
        assertEquals(ExitAlert.Action.NONE, alert.update(DIRECT, true));
    }

    @Test
    public void online_exit_going_unavailable_alerts_once() {
        ExitAlert alert = new ExitAlert();
        assertEquals(ExitAlert.Action.NONE, alert.update(VIA, true));
        assertEquals(ExitAlert.Action.ALERT, alert.update(LOST, true));
        assertEquals(ExitAlert.Action.NONE, alert.update(LOST, true));
        assertEquals(ExitAlert.Action.CLEAR, alert.update(VIA, true));
    }
}
