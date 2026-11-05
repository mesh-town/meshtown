package app.meshtown;

/**
 * Decides when the "exit offline" alert is shown: only on the step from an exit that carried traffic
 * to an unavailable one while the tunnel is up. A start where the exit was never found gives none.
 * Pure logic.
 */
final class ExitAlert {
    enum Action { NONE, ALERT, CLEAR }

    private boolean wasOnline;
    private boolean alerted;

    Action update(NodeState s, boolean vpnOn) {
        boolean lost = vpnOn && s.exitLost();
        boolean online = vpnOn && s.reachable && s.inNetwork && "exit".equals(s.route);
        Action action = lost && wasOnline && !alerted ? Action.ALERT
                : !lost && alerted ? Action.CLEAR : Action.NONE;
        alerted = action == Action.ALERT || (alerted && lost);
        wasOnline = online || (wasOnline && lost);
        return action;
    }
}
