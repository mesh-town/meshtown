package app.meshtown;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class KeyboardGuardTest {
    @Test
    public void fullScreenIsClosed() {
        assertFalse(KeyboardGuard.isOpen(2400, 2400));
    }

    @Test
    public void systemBarsAloneAreClosed() {
        assertFalse(KeyboardGuard.isOpen(2400, 2200));
    }

    @Test
    public void tallLossIsOpen() {
        assertTrue(KeyboardGuard.isOpen(2400, 1400));
    }

    @Test
    public void boundaryIsClosed() {
        assertFalse(KeyboardGuard.isOpen(2000, 1700));
        assertTrue(KeyboardGuard.isOpen(2000, 1699));
    }

    @Test
    public void unknownScreenIsClosed() {
        assertFalse(KeyboardGuard.isOpen(0, 0));
    }
}
