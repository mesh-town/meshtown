package app.meshtown;

import android.app.Activity;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.IntentSender;
import android.content.pm.PackageInstaller;
import android.net.Uri;
import android.os.Build;
import android.provider.Settings;
import android.widget.Toast;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.concurrent.atomic.AtomicBoolean;

/** Downloads the package straight into an installer session and hands it to the system for confirmation. */
final class UpdateInstaller {
    static final String EXTRA_UPDATE_NOW = "app.meshtown.extra.UPDATE_NOW";
    static final AtomicBoolean busy = new AtomicBoolean(false);
    private static final long LIMIT = 100L * 1024 * 1024;

    private UpdateInstaller() {}

    /** Test hook: starts only in a build that has a test address, when the intent asks for it. */
    static void startIfRequested(Activity a, Intent intent) {
        if (!BuildConfig.UPDATE_TEST_URL.isEmpty() && intent.getBooleanExtra(EXTRA_UPDATE_NOW, false)) {
            start(a, BuildConfig.UPDATE_TEST_URL);
        }
    }

    static void start(Activity a, String url) {
        if (!a.getPackageManager().canRequestPackageInstalls()) {
            askPermission(a);
            return;
        }
        if (!busy.compareAndSet(false, true)) {
            toast(a, "Update already in progress.");
            return;
        }
        toast(a, "Downloading the update…");
        Context app = a.getApplicationContext();
        new Thread(() -> run(app, url), "update-download").start();
    }

    private static void askPermission(Activity a) {
        toast(a, "Allow installs from Mesh Town, then tap Download again.");
        a.startActivity(new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                Uri.parse("package:" + a.getPackageName())));
    }

    private static void run(Context ctx, String url) {
        PackageInstaller installer = ctx.getPackageManager().getPackageInstaller();
        File file = new File(ctx.getCacheDir(), "update.apk");
        int id = -1;
        try {
            abandonOwn(installer);
            download(url, file);
            String refusal = PackageProbe.refusal(ctx, file.getPath());
            if (refusal != null) {
                finish(ctx, refusal);
                return;
            }
            id = installer.createSession(params(ctx, file.length()));
            transfer(ctx, installer, id, file);
        } catch (IOException | RuntimeException e) {
            abandon(installer, id);
            finish(ctx, UpdateCheck.DOWNLOAD_FAILED);
        } finally {
            file.delete();
        }
    }

    private static void finish(Context ctx, String message) {
        busy.set(false);
        toast(ctx, message);
    }

    private static void download(String url, File file) throws IOException {
        HttpURLConnection c = open(url);
        try (InputStream in = c.getInputStream(); OutputStream out = new FileOutputStream(file)) {
            LimitedCopy.copy(in, out, LIMIT);
        } finally {
            c.disconnect();
        }
    }

    private static void transfer(Context ctx, PackageInstaller installer, int id, File file) throws IOException {
        try (PackageInstaller.Session s = installer.openSession(id)) {
            write(s, file);
            s.commit(resultSender(ctx));
        }
    }

    private static PackageInstaller.SessionParams params(Context ctx, long len) {
        PackageInstaller.SessionParams p =
                new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        p.setAppPackageName(ctx.getPackageName());
        if (len > 0) {
            p.setSize(len);
        }
        return p;
    }

    private static HttpURLConnection open(String url) throws IOException {
        HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
        c.setConnectTimeout(15000);
        c.setReadTimeout(30000);
        if (c.getResponseCode() != 200) {
            c.disconnect();
            throw new IOException("http " + c.getResponseCode());
        }
        return c;
    }

    private static void write(PackageInstaller.Session s, File file) throws IOException {
        try (InputStream in = new FileInputStream(file); OutputStream out = s.openWrite("meshtown.apk", 0, file.length())) {
            LimitedCopy.copy(in, out, LIMIT);
            s.fsync(out);
        }
    }

    private static IntentSender resultSender(Context ctx) {
        Intent i = new Intent(ctx, UpdateReceiver.class);
        int flags = PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0);
        return PendingIntent.getBroadcast(ctx, 0, i, flags).getIntentSender();
    }

    private static void abandon(PackageInstaller installer, int id) {
        if (id < 0) {
            return;
        }
        try {
            installer.abandonSession(id);
        } catch (RuntimeException ignored) {
            // The session is already gone.
        }
    }

    private static void abandonOwn(PackageInstaller installer) {
        for (PackageInstaller.SessionInfo info : installer.getMySessions()) {
            abandon(installer, info.getSessionId());
        }
    }

    static void toast(Context ctx, String text) {
        new android.os.Handler(android.os.Looper.getMainLooper())
                .post(() -> Toast.makeText(ctx, text, Toast.LENGTH_LONG).show());
    }
}
