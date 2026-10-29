package app.meshtown;

import static org.junit.Assert.assertEquals;

import org.junit.Test;

public class PollPaceTest {
    @Test
    public void tunnelOnAndVisibleIsFast() {
        assertEquals(PollPace.FAST_MS, PollPace.nodePollMs(true, true, false));
    }

    @Test
    public void tunnelOnInBackgroundIsFast() {
        assertEquals(PollPace.FAST_MS, PollPace.nodePollMs(true, false, false));
    }

    @Test
    public void tunnelOffAndVisibleIsFast() {
        assertEquals(PollPace.FAST_MS, PollPace.nodePollMs(false, true, false));
    }

    @Test
    public void tunnelOffInBackgroundIsSlow() {
        assertEquals(PollPace.SLOW_MS, PollPace.nodePollMs(false, false, false));
    }

    @Test
    public void restoreWaitingInBackgroundIsFast() {
        assertEquals(PollPace.FAST_MS, PollPace.nodePollMs(false, false, true));
    }
}
