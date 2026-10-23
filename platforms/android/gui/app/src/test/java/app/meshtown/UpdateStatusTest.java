package app.meshtown;

import static org.junit.Assert.assertEquals;

import android.content.pm.PackageInstaller;
import org.junit.Test;

public class UpdateStatusTest {
    private static final String OTHER_KEY =
            "This build was signed with a different key; install the official build once by hand.";

    @Test
    public void conflictMeansOtherKey() {
        assertEquals(OTHER_KEY, UpdateStatus.text(PackageInstaller.STATUS_FAILURE_CONFLICT, "x"));
    }

    @Test
    public void incompatibleMeansOtherKey() {
        assertEquals(OTHER_KEY, UpdateStatus.text(PackageInstaller.STATUS_FAILURE_INCOMPATIBLE, null));
    }

    @Test
    public void downgradeMessageMeansUpToDate() {
        assertEquals("Already up to date.",
                UpdateStatus.text(PackageInstaller.STATUS_FAILURE_INVALID, "INSTALL_FAILED_VERSION_DOWNGRADE"));
    }

    @Test
    public void storage() {
        assertEquals("Not enough storage for the update.",
                UpdateStatus.text(PackageInstaller.STATUS_FAILURE_STORAGE, ""));
    }

    @Test
    public void aborted() {
        assertEquals("Update cancelled.", UpdateStatus.text(PackageInstaller.STATUS_FAILURE_ABORTED, ""));
    }

    @Test
    public void anythingElseIsAGenericFailure() {
        assertEquals("Update failed.", UpdateStatus.text(PackageInstaller.STATUS_FAILURE, "boom"));
    }
}
