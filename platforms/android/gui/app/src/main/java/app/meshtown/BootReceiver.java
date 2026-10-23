package app.meshtown;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/** Starts the node after a reboot or an app update; the tunnel follows when it was on before. */
public class BootReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context ctx, Intent intent) {
        String action = intent.getAction();
        if (!Intent.ACTION_BOOT_COMPLETED.equals(action) && !Intent.ACTION_MY_PACKAGE_REPLACED.equals(action)) {
            return;
        }
        ctx.startForegroundService(new Intent(ctx, NodeService.class).setAction(NodeService.ACTION_BOOT));
    }
}
