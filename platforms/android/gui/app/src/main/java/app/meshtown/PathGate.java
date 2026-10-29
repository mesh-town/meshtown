package app.meshtown;

/**
 * Decides when to probe the path through the exit and remembers that it answered.
 *
 * <p>The path is keyed by route and exit name. A probe is due while the tunnel is on or awaited, the
 * node is reachable and in the network, and either an exit is in use or no exit is chosen. A positive
 * answer latches until the key changes or the tunnel or node goes away; an answer for an old key is
 * ignored. After {@link #BACKOFF_AFTER} failures in a row for one key the next probe waits
 * {@link #BACKOFF_MS} from the last failure. Methods are synchronized: the poll thread and the probe
 * thread both call in.
 */
final class PathGate {
    static final int BACKOFF_AFTER = 4;
    static final long BACKOFF_MS = 30_000;

    /** One probe in flight: the key it was started for and its sequence number. */
    static final class Attempt {
        final String key;
        final int number;

        Attempt(String key, int number) {
            this.key = key;
            this.number = number;
        }
    }

    private String key;
    private boolean eligible;
    private boolean ready;
    private int number;
    private String inFlight;
    private long failedAtMs;

    /** Feeds one snapshot: recomputes the key and clears the state when the path is gone or changed.
     * {@code wanted} is true while the tunnel is on or is about to be raised. */
    synchronized void observe(NodeState s, boolean wanted) {
        if (!wanted || !s.reachable || !s.inNetwork || s.excluded || "unavailable".equals(s.route)) {
            reset();
            return;
        }
        String now = s.route + "|" + s.exitName;
        if (!now.equals(key)) {
            reset();
            key = now;
        }
        eligible = "exit".equals(s.route) || !s.exitChosen;
    }

    /** The next probe to run, or null when none is due. */
    synchronized Attempt begin(long nowMs) {
        if (!eligible || ready || inFlight != null || paused(nowMs)) {
            return null;
        }
        inFlight = key;
        return new Attempt(key, number);
    }

    private boolean paused(long nowMs) {
        return number >= BACKOFF_AFTER && nowMs - failedAtMs < BACKOFF_MS;
    }

    /** Records the outcome, at {@code nowMs}, of a probe started for {@code probeKey}. */
    synchronized void finish(String probeKey, boolean ok, long nowMs) {
        if (!probeKey.equals(inFlight)) {
            return;
        }
        inFlight = null;
        if (ok) {
            ready = true;
        } else {
            number++;
            failedAtMs = nowMs;
        }
    }

    synchronized boolean ready() {
        return ready;
    }

    private void reset() {
        key = null;
        eligible = false;
        ready = false;
        number = 0;
        failedAtMs = 0;
        inFlight = null;
    }
}
