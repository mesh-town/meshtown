package app.meshtown;

/** Decides whether an update download interrupted by the install-permission screen should resume. */
final class UpdateResume {
    static final long MAX_AGE_MS = 10L * 60 * 1000;

    private UpdateResume() {}

    static boolean shouldStart(String url, long savedAtMs, long nowMs, boolean granted) {
        if (url == null || url.isEmpty() || !granted) {
            return false;
        }
        long age = nowMs - savedAtMs;
        return age >= 0 && age <= MAX_AGE_MS;
    }
}
