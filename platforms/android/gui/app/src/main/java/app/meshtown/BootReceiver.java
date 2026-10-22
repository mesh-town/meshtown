package app.meshtown;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/** Starts the node after a reboot; the tunnel follows when it was on before. */
public class BootReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context ctx, Intent intent) {
        if (!Intent.ACTION_BOOT_COMPLETED.equals(intent.getAction())) {
            return;
        }
        ctx.startForegroundService(new Intent(ctx, NodeService.class).setAction(NodeService.ACTION_BOOT));
    }
}
