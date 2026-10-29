package app.meshtown;

import android.app.PendingIntent;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.net.VpnService;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.service.quicksettings.Tile;
import android.service.quicksettings.TileService;

/** Quick Settings tile: lit while the VPN is up; a tap connects or disconnects like the notification action. */
public class TunnelTile extends TileService {
    /** The instance while the panel is open; a listening tile never gets a second onStartListening. */
    private static volatile TunnelTile listening;

    /** Redraws the tile at once when the panel is open, and asks the system to listen otherwise. */
    static void refresh(Context ctx) {
        TunnelTile open = listening;
        if (open != null) {
            new Handler(Looper.getMainLooper()).post(() -> open.draw(TunnelService.running));
        }
        try {
            TileService.requestListeningState(ctx, new ComponentName(ctx, TunnelTile.class));
        } catch (RuntimeException ignored) {
            // the tile is not added; nothing to redraw
        }
    }

    @Override
    public void onStartListening() {
        listening = this;
        draw(TunnelService.running);
    }

    @Override
    public void onStopListening() {
        listening = null;
    }

    @Override
    public void onClick() {
        boolean on = TunnelService.running;
        if (on) {
            draw(false);
            startForegroundService(new Intent(this, NodeService.class).setAction(NodeService.ACTION_DISCONNECT));
        } else if (canConnect()) {
            draw(true);
            startForegroundService(new Intent(this, NodeService.class).setAction(NodeService.ACTION_CONNECT));
        } else {
            openApp();
        }
    }

    private boolean canConnect() {
        return NodeService.canConnect && VpnService.prepare(this) == null;
    }

    /** Draws the given state; a tap draws the expected one at once, the service's refresh corrects it. */
    private void draw(boolean on) {
        Tile tile = getQsTile();
        if (tile == null) {
            return;
        }
        tile.setState(on ? Tile.STATE_ACTIVE : Tile.STATE_INACTIVE);
        if (Build.VERSION.SDK_INT >= 29) {
            tile.setSubtitle(StatusText.tileSubtitle(NodeService.state, on, NodeService.pathReady));
        }
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
