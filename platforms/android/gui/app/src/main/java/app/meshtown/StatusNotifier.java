package app.meshtown;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;

/** Builds the ongoing status notification and the one-shot "exit is offline" alert. */
final class StatusNotifier {
    static final int ID = 1;
    private static final int ALERT_ID = 2;
    private static final String CHANNEL = "vpn";
    private static final String ALERT_CHANNEL = "alert";

    private final Context ctx;
    private final NotificationManager nm;

    StatusNotifier(Context ctx) {
        this.ctx = ctx;
        this.nm = ctx.getSystemService(NotificationManager.class);
        nm.createNotificationChannel(new NotificationChannel(CHANNEL, "Connection status", NotificationManager.IMPORTANCE_LOW));
        nm.createNotificationChannel(
                new NotificationChannel(ALERT_CHANNEL, "Exit offline", NotificationManager.IMPORTANCE_HIGH));
    }

    Notification status(StatusText.Notice text, boolean vpnOn, boolean canConnect) {
        Notification.Builder b = new Notification.Builder(ctx, CHANNEL)
                .setContentTitle(text.title)
                .setContentText(text.text)
                .setSmallIcon(R.drawable.ic_stat_mesh)
                .setOnlyAlertOnce(true)
                .setContentIntent(openApp());
        if (vpnOn) {
            b.addAction(action("Disconnect", NodeService.ACTION_DISCONNECT));
        } else if (canConnect) {
            b.addAction(action("Connect", NodeService.ACTION_CONNECT));
        }
        b.addAction(action("Quit", NodeService.ACTION_QUIT));
        return b.build();
    }

    void update(StatusText.Notice text, boolean vpnOn, boolean canConnect) {
        nm.notify(ID, status(text, vpnOn, canConnect));
    }

    void alertExitLost(StatusText.Notice text) {
        nm.notify(ALERT_ID, new Notification.Builder(ctx, ALERT_CHANNEL)
                .setContentTitle(text.title)
                .setContentText(text.text)
                .setSmallIcon(R.drawable.ic_stat_mesh)
                .setAutoCancel(true)
                .setContentIntent(openApp())
                .build());
    }

    void clearAlert() {
        nm.cancel(ALERT_ID);
    }

    private Notification.Action action(String title, String name) {
        Intent i = new Intent(ctx, NodeService.class).setAction(name);
        PendingIntent pi = PendingIntent.getService(ctx, name.hashCode(), i, PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Action.Builder(null, title, pi).build();
    }

    private PendingIntent openApp() {
        Intent i = new Intent(ctx, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP);
        return PendingIntent.getActivity(ctx, 0, i, PendingIntent.FLAG_IMMUTABLE);
    }
}
