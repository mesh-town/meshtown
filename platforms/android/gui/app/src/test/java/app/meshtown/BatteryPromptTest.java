package app.meshtown;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class BatteryPromptTest {
    @Test
    public void asksWhenNotExemptAndNotAsked() {
        assertTrue(BatteryPrompt.shouldAsk(false, false));
    }

    @Test
    public void skipsWhenAlreadyExempt() {
        assertFalse(BatteryPrompt.shouldAsk(true, false));
    }

    @Test
    public void skipsWhenAlreadyAsked() {
        assertFalse(BatteryPrompt.shouldAsk(false, true));
    }

    @Test
    public void skipsWhenBothTrue() {
        assertFalse(BatteryPrompt.shouldAsk(true, true));
    }
}
