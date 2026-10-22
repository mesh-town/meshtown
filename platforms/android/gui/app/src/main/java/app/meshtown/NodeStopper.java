package app.meshtown;

/** Stops a child process politely first, then forcibly if it has not exited within the grace period. */
final class NodeStopper {
    /** The few process operations the stop sequence needs. */
    interface Target {
        void terminate();

        /** Waits up to {@code millis}; returns true when the process has exited. */
        boolean awaitExit(long millis) throws InterruptedException;

        void kill();
    }

    static final long GRACE_MS = 2000;

    private NodeStopper() {
    }

    /** Returns true when a forced kill was needed. */
    static boolean stop(Target target, long graceMs) {
        target.terminate();
        try {
            if (target.awaitExit(graceMs)) {
                return false;
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
        target.kill();
        return true;
    }
}
