package app.meshtown;

/**
 * Remembers a Connect press made while the search for members is still running: the tunnel then
 * starts once, when {@link ConnectGate} opens. A second press cancels the wait; leaving the network
 * drops it. Pure logic, safe to call from the UI and the poll thread.
 */
final class ConnectIntent {
    private boolean pending;

    /** Toggles the wait; returns whether a connect is pending now. */
    synchronized boolean press() {
        pending = !pending;
        return pending;
    }

    synchronized boolean pending() {
        return pending;
    }

    synchronized void cancel() {
        pending = false;
    }

    /** Feeds one snapshot; returns true exactly once, when a pending connect may start the tunnel. */
    synchronized boolean update(NodeState s, boolean gateOpen) {
        if (!s.inNetwork) {
            pending = false;
        }
        boolean start = pending && gateOpen;
        pending = pending && !start;
        return start;
    }
}
