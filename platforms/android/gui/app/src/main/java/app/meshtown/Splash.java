package app.meshtown;

/** The local page shown while the node is not answering yet; it needs no network. */
final class Splash {
    static final String STARTING = "Starting Mesh Town…";
    static final String FAILED = "Mesh Town did not start. Try Quit and open the app again.";
    /** The node is declared failed after this long without an answer. */
    static final long TIMEOUT_MS = 30_000;

    private Splash() {}

    static String html(String message) {
        return "<!doctype html><html><head><meta charset=utf-8>"
                + "<meta name=viewport content='width=device-width,initial-scale=1'><style>"
                + "html,body{height:100%;margin:0}"
                + "body{display:flex;align-items:center;justify-content:center;text-align:center;"
                + "font:16px sans-serif;background:#f3f5f6;color:#5c6b71}"
                + "@media(prefers-color-scheme:dark){body{background:#14181a;color:#93a2a7}}"
                + "p{margin:0 24px}</style></head><body><p>" + message + "</p></body></html>";
    }

    static String message(long waitedMs) {
        return waitedMs >= TIMEOUT_MS ? FAILED : STARTING;
    }
}
