package app.meshtown;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/** Restarts the node after an app update; the tunnel follows when it was on before. */
public class PackageReplacedReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context ctx, Intent intent) {
        ctx.startForegroundService(new Intent(ctx, NodeService.class).setAction(NodeService.ACTION_RESTORE));
    }
}
