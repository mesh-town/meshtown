package app.meshtown;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import android.widget.Toast;

/** Receives the result of an update install session. */
public final class UpdateReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context ctx, Intent intent) {
        int status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
        if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            confirm(ctx, intent);
            return;
        }
        UpdateInstaller.busy.set(false);
        if (status != PackageInstaller.STATUS_SUCCESS) {
            String message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
            Toast.makeText(ctx, UpdateStatus.text(status, message), Toast.LENGTH_LONG).show();
        }
    }

    private static void confirm(Context ctx, Intent intent) {
        Intent next = intent.getParcelableExtra(Intent.EXTRA_INTENT);
        if (next != null) {
            ctx.startActivity(next.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        }
    }
}
