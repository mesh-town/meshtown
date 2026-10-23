package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import org.junit.Test;

public class UpdateCheckTest {
    @Test
    public void newerFromSameSignerIsInstalled() {
        assertNull(UpdateCheck.refusal(3070, 3099, true, true));
    }

    @Test
    public void unparsedPackageIsADownloadFailure() {
        assertEquals("Download failed.", UpdateCheck.refusal(3070, 0, false, false));
    }

    @Test
    public void olderPackageIsRefused() {
        assertEquals("Already up to date.", UpdateCheck.refusal(3070, 3050, true, true));
    }

    @Test
    public void equalVersionIsRefused() {
        assertEquals("Already up to date.", UpdateCheck.refusal(3070, 3070, true, true));
    }

    @Test
    public void otherSignerIsRefused() {
        assertEquals(UpdateStatus.OTHER_KEY, UpdateCheck.refusal(3070, 3099, false, true));
    }

    @Test
    public void olderWithOtherSignerReportsVersionFirst() {
        assertEquals("Already up to date.", UpdateCheck.refusal(3070, 3050, false, true));
    }
}
