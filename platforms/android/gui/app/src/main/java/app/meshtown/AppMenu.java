package app.meshtown;

import android.app.Activity;
import android.app.ActivityManager;
import android.app.AlertDialog;
import android.content.Context;
import android.content.Intent;
import android.view.Menu;
import android.view.MenuItem;
import android.view.View;

/** The options menu (Quit, Start over) and the data reset behind it. */
final class AppMenu {
    private static final int MENU_QUIT = 1;
    private static final int MENU_START_OVER = 2;
    /** The page asks for the start-over dialog by navigating here. */
    static final String START_OVER_URL = "meshtown-app://start-over";
    private static final long WIPE_DELAY_MS = 1500;
    private final Activity activity;
    private final View anchor;

    AppMenu(Activity activity, View anchor) {
        this.activity = activity;
        this.anchor = anchor;
    }

    void create(Menu menu) {
        menu.add(Menu.NONE, MENU_QUIT, Menu.NONE, "Quit Mesh Town");
        menu.add(Menu.NONE, MENU_START_OVER, Menu.NONE, StatusText.resetMenuTitle(NodeService.state));
    }

    void prepare(Menu menu) {
        menu.findItem(MENU_START_OVER).setTitle(StatusText.resetMenuTitle(NodeService.state));
    }

    /** Returns true when the item belongs to this menu and was handled. */
    boolean select(MenuItem item) {
        if (item.getItemId() == MENU_START_OVER) {
            confirmStartOver();
            return true;
        }
        if (item.getItemId() != MENU_QUIT) {
            return false;
        }
        activity.startService(new Intent(activity, NodeService.class).setAction(NodeService.ACTION_QUIT));
        activity.finishAndRemoveTask();
        return true;
    }

    void confirmStartOver() {
        NodeState s = NodeService.state;
        AlertDialog dialog = new AlertDialog.Builder(activity)
                .setTitle(StatusText.resetTitle(s))
                .setMessage(StatusText.resetMessage(s))
                .setNegativeButton("Cancel", null)
                .setPositiveButton(s.inNetwork || s.excluded ? "Leave and reset" : "Reset", (d, w) -> startOver())
                .create();
        dialog.show();
        dialog.getButton(AlertDialog.BUTTON_POSITIVE).setTextColor(0xFFC62828);
    }

    /** Stops the tunnel and the node like Quit, then erases the app data; the system closes the process. */
    private void startOver() {
        activity.startService(new Intent(activity, NodeService.class).setAction(NodeService.ACTION_QUIT));
        anchor.postDelayed(() -> {
            ActivityManager am = (ActivityManager) activity.getSystemService(Context.ACTIVITY_SERVICE);
            am.clearApplicationUserData();
        }, WIPE_DELAY_MS);
    }
}
