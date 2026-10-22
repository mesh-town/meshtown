package app.meshtown;

import android.app.PendingIntent;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.net.VpnService;
import android.os.Build;
import android.service.quicksettings.Tile;
import android.service.quicksettings.TileService;

/** Quick Settings tile: lit while the VPN is up; a tap connects or disconnects like the notification action. */
public class TunnelTile extends TileService {
    /** Asks the system to redraw the tile when the tunnel state changed. */
    static void refresh(Context ctx) {
        try {
            TileService.requestListeningState(ctx, new ComponentName(ctx, TunnelTile.class));
        } catch (RuntimeException ignored) {
            // the tile is not added; nothing to redraw
        }
    }

    @Override
    public void onStartListening() {
        draw();
    }

    @Override
    public void onClick() {
        if (TunnelService.running) {
            startForegroundService(new Intent(this, NodeService.class).setAction(NodeService.ACTION_DISCONNECT));
        } else if (canConnect()) {
            startForegroundService(new Intent(this, NodeService.class).setAction(NodeService.ACTION_CONNECT));
        } else {
            openApp();
        }
    }

    private boolean canConnect() {
        return NodeService.canConnect && VpnService.prepare(this) == null;
    }

    private void draw() {
        Tile tile = getQsTile();
        if (tile == null) {
            return;
        }
        tile.setState(TunnelService.running ? Tile.STATE_ACTIVE : Tile.STATE_INACTIVE);
        tile.updateTile();
    }

    private void openApp() {
        Intent i = new Intent(this, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        if (Build.VERSION.SDK_INT >= 34) {
            startActivityAndCollapse(PendingIntent.getActivity(this, 0, i, PendingIntent.FLAG_IMMUTABLE));
        } else {
            startActivityAndCollapse(i);
        }
    }
}
