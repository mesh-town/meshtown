package app.meshtown;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.net.Uri;
import android.os.PowerManager;
import android.provider.Settings;

/** Asks once for the battery-optimization exemption that keeps the node alive in Doze. */
final class BatteryPrompt {
    private BatteryPrompt() {}

    static boolean shouldAsk(boolean ignoring, boolean alreadyAsked) {
        return !ignoring && !alreadyAsked;
    }

    static void askOnce(Activity activity) {
        String pkg = activity.getPackageName();
        PowerManager power = activity.getSystemService(PowerManager.class);
        if (!shouldAsk(power.isIgnoringBatteryOptimizations(pkg), Prefs.batteryAsked(activity))) {
            return;
        }
        Prefs.setBatteryAsked(activity, true);
        launch(activity, pkg);
    }

    private static void launch(Activity activity, String pkg) {
        try {
            activity.startActivity(new Intent(
                    Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:" + pkg)));
        } catch (ActivityNotFoundException e) {
            // No settings screen to show; the flag is already set, so it is not asked again.
        }
    }
}
