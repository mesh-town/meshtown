package app.meshtown;

/**
 * Decides when the Connect button (and the notification and tile actions) may be offered.
 *
 * <p>"Search finished" is judged the way the node page judges it: being in a network with no peer
 * other than this node means the search is still running. Connect opens when other members are
 * known and either an exit is already chosen or none is offered, when the page had a short grace
 * period to pick the nearest offered exit, or when the search ran out of time (direct use). Once
 * open it stays open while the node is in a network; leaving the network resets everything.
 * Pure logic: the clock is a parameter.
 */
final class ConnectGate {
    static final long SEARCH_TIMEOUT_MS = 45_000;
    static final long PICK_GRACE_MS = 6_000;

    private static final long NONE = -1;

    private boolean open;
    private long searchSince = NONE;
    private long graceSince = NONE;

    /** Feeds one snapshot; returns whether Connect is available now. */
    boolean update(NodeState s, long nowMs) {
        if (!s.inNetwork) {
            reset();
            return false;
        }
        open = open || isOpen(s, nowMs);
        return open;
    }

    private void reset() {
        open = false;
        searchSince = NONE;
        graceSince = NONE;
    }

    private boolean isOpen(NodeState s, long nowMs) {
        if (!s.peersKnown) {
            return elapsed(searchSince = start(searchSince, nowMs), nowMs, SEARCH_TIMEOUT_MS);
        }
        if (s.exitChosen || !s.exitOffered) {
            return true;
        }
        return elapsed(graceSince = start(graceSince, nowMs), nowMs, PICK_GRACE_MS);
    }

    private static long start(long since, long nowMs) {
        return since == NONE ? nowMs : since;
    }

    private static boolean elapsed(long since, long nowMs, long limit) {
        return nowMs - since >= limit;
    }
}
