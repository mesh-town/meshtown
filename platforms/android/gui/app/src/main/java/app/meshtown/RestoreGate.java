package app.meshtown;

/**
 * Decides when to raise the VPN after a boot that wants it: once the first path probe has passed, or
 * after {@link #WAIT_MS} regardless, so a bad path never leaves traffic outside the VPN for good.
 *
 * <p>States: IDLE (nothing to do), WAITING (armed, tunnel not raised yet), STARTED (the start was
 * requested and the tunnel is not seen running yet). While waiting before the deadline, or while
 * started, the caller keeps probing and polling fast ({@link #holding()}). The clock is a parameter.
 * Methods are synchronized: {@code arm} runs on the main thread, the rest on the poll thread.
 */
final class RestoreGate {
    static final long WAIT_MS = 60_000;
    static final long START_GRACE_MS = 10_000;

    private enum Phase { IDLE, WAITING, STARTED }

    private Phase phase = Phase.IDLE;
    private long armedAt;
    private long startedAt;
    private boolean expired;

    /** Starts a wait from {@code nowMs}; a wait already running keeps its deadline. */
    synchronized void arm(boolean wanted, long nowMs) {
        if (!wanted) {
            phase = Phase.IDLE;
            return;
        }
        if (phase == Phase.WAITING) {
            return;
        }
        phase = Phase.WAITING;
        armedAt = nowMs;
        expired = false;
    }

    /** True exactly once, when the tunnel should be started now. */
    synchronized boolean update(boolean wanted, NodeState s, boolean tunnelOn, boolean pathReady, long nowMs) {
        if (!wanted) {
            phase = Phase.IDLE;
            return false;
        }
        if (phase == Phase.STARTED) {
            releaseStarted(tunnelOn, nowMs);
            return false;
        }
        return phase == Phase.WAITING && wait(s, tunnelOn, pathReady, nowMs);
    }

    private void releaseStarted(boolean tunnelOn, long nowMs) {
        if (tunnelOn || nowMs - startedAt >= START_GRACE_MS) {
            phase = Phase.IDLE;
        }
    }

    private boolean wait(NodeState s, boolean tunnelOn, boolean pathReady, long nowMs) {
        if (tunnelOn) {
            phase = Phase.IDLE;
            return false;
        }
        expired |= nowMs - armedAt >= WAIT_MS;
        if (!s.inNetwork || !(pathReady || expired)) {
            return false;
        }
        phase = Phase.STARTED;
        startedAt = nowMs;
        return true;
    }

    /** Whether probes and fast polling are still needed on behalf of the restore. */
    synchronized boolean holding() {
        return phase == Phase.STARTED || (phase == Phase.WAITING && !expired);
    }
}
