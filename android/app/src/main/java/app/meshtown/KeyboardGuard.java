package app.meshtown;

/** Decides from two heights whether the on-screen keyboard is open. */
final class KeyboardGuard {
    /** Fraction of the screen height the visible area must lose; system bars alone stay below it. */
    private static final double THRESHOLD = 0.15;

    private KeyboardGuard() {}

    static boolean isOpen(int screenHeight, int visibleHeight) {
        return screenHeight > 0 && (screenHeight - visibleHeight) > THRESHOLD * screenHeight;
    }
}
