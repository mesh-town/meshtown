package app.meshtown;

/** Decides whether a downloaded package is worth handing to the installer. */
final class UpdateCheck {
    static final String DOWNLOAD_FAILED = "Download failed.";

    private UpdateCheck() {}

    /** Returns null when the package may be installed, otherwise the text to show the user. */
    static String refusal(long installed, long offered, boolean sameSigner, boolean parsed) {
        if (!parsed) {
            return DOWNLOAD_FAILED;
        }
        if (offered <= installed) {
            return UpdateStatus.UP_TO_DATE;
        }
        if (!sameSigner) {
            return UpdateStatus.OTHER_KEY;
        }
        return null;
    }
}
