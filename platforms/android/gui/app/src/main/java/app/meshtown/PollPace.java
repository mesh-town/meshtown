package app.meshtown;

/** How often the app asks the node for its state: fast while someone can see the result, slow otherwise. */
final class PollPace {
    static final long FAST_MS = 3000;
    static final long SLOW_MS = 30000;

    private PollPace() {}

    /** The tunnel needs a fresh notification, the screen needs a fresh page; with neither, poll rarely. */
    static long nodePollMs(boolean tunnelOn, boolean appVisible) {
        return tunnelOn || appVisible ? FAST_MS : SLOW_MS;
    }
}
