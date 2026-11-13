package app.meshtown;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class UpdateResumeTest {
    private static final String URL = "https://example.com/app.apk";
    private static final long MIN = 60_000L;

    @Test
    public void startsWhenGrantedAndFresh() {
        assertTrue(UpdateResume.shouldStart(URL, 1000, 1000 + MIN, true));
    }

    @Test
    public void skipsWhenNotGranted() {
        assertFalse(UpdateResume.shouldStart(URL, 1000, 1000 + MIN, false));
    }

    @Test
    public void skipsWhenStale() {
        assertFalse(UpdateResume.shouldStart(URL, 1000, 1000 + 11 * MIN, true));
    }

    @Test
    public void skipsWhenNoPendingUrl() {
        assertFalse(UpdateResume.shouldStart(null, 1000, 2000, true));
        assertFalse(UpdateResume.shouldStart("", 1000, 2000, true));
    }

    @Test
    public void skipsWhenClockWentBack() {
        assertFalse(UpdateResume.shouldStart(URL, 5000, 1000, true));
    }
}
