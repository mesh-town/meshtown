package app.meshtown;

import android.content.pm.PackageInstaller;

/** Short user-facing text for a failed install session. */
final class UpdateStatus {
    static final String OTHER_KEY =
            "This build was signed with a different key; install the official build once by hand.";
    static final String UP_TO_DATE = "Already up to date.";

    private UpdateStatus() {}

    /** Not used for success or for the pending-confirmation status. */
    static String text(int status, String systemMessage) {
        if (systemMessage != null && systemMessage.contains("DOWNGRADE")) {
            return UP_TO_DATE;
        }
        switch (status) {
            case PackageInstaller.STATUS_FAILURE_CONFLICT:
            case PackageInstaller.STATUS_FAILURE_INCOMPATIBLE:
                return OTHER_KEY;
            case PackageInstaller.STATUS_FAILURE_STORAGE:
                return "Not enough storage for the update.";
            case PackageInstaller.STATUS_FAILURE_ABORTED:
                return "Update cancelled.";
            default:
                return "Update failed.";
        }
    }
}
