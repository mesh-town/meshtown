package app.meshtown;

import android.app.Service;
import android.content.Intent;
import android.net.VpnService;
import android.os.SystemClock;
import android.os.IBinder;
import android.util.Log;
import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;

/** Foreground service that owns the node process and keeps the status notification fresh. */
public class NodeService extends Service {
    static final String ACTION_BOOT = "app.meshtown.BOOT";
    static final String ACTION_CONNECT = "app.meshtown.CONNECT";
    static final String ACTION_DISCONNECT = "app.meshtown.DISCONNECT";
    static final String ACTION_QUIT = "app.meshtown.QUIT";
    private static final String TAG = "MeshTown";
    /** Guarded wait of the poll loop; {@link #wake()} cuts the wait short. */
    private static final Object POLL_LOCK = new Object();

    /** Latest snapshot, read by the activity. */
    static volatile NodeState state = NodeState.UNREACHABLE;
    /** Set by Quit; the activity closes itself when it sees it. */
    static volatile boolean quitRequested;
    /** Whether Connect is offered (see {@link ConnectGate}); read by the activity and the tile. */
    static volatile boolean canConnect;
    /** Whether a connection through the chosen path was opened (see {@link PathGate}); read by the activity and the tile. */
    static volatile boolean pathReady;
    /** Whether the activity is on screen; a hidden app with the tunnel off and no restore pending polls the node rarely. */
    static volatile boolean appVisible;

    /** Ends the current wait of the poll loop so a change of pace or a new command shows at once. */
    static void wake() {
        synchronized (POLL_LOCK) {
            POLL_LOCK.notifyAll();
        }
    }

    private Process node;
    private StatusNotifier notifier;
    private Thread poller;
    private volatile boolean stopping;
    private final RestoreGate restore = new RestoreGate();
    private boolean exitWasLost;
    private final ConnectGate gate = new ConnectGate();
    private final PathGate path = new PathGate();
    private String shown = "";

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    @Override
    public void onCreate() {
        super.onCreate();
        pathReady = false;
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent == null ? null : intent.getAction();
        if (notifier == null) {
            notifier = new StatusNotifier(this);
        }
        startForeground(StatusNotifier.ID, notifier.status(StatusText.notice(NodeState.UNREACHABLE, false, false), false, false));
        if (ACTION_QUIT.equals(action)) {
            quit();
            return START_NOT_STICKY;
        }
        quitRequested = false;
        handle(action);
        wake();
        if (poller == null) {
            startNode();
            poller = new Thread(this::pollLoop, "mesh-poll");
            poller.start();
        }
        return START_STICKY;
    }

    private void handle(String action) {
        if (ACTION_BOOT.equals(action)) {
            restore.arm(Prefs.vpnWanted(this) && VpnService.prepare(this) == null,
                    SystemClock.elapsedRealtime());
        } else if (ACTION_CONNECT.equals(action)) {
            connectFromNotification();
        } else if (ACTION_DISCONNECT.equals(action)) {
            stopTunnel(true);
        }
    }

    private void connectFromNotification() {
        if (VpnService.prepare(this) != null) {
            startActivity(new Intent(this, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            return;
        }
        startTunnel();
    }

    private void startTunnel() {
        Prefs.setVpnWanted(this, true);
        startService(new Intent(this, TunnelService.class));
    }

    private void stopTunnel(boolean forget) {
        if (forget) {
            Prefs.setVpnWanted(this, false);
        }
        startService(new Intent(this, TunnelService.class).setAction(TunnelService.ACTION_STOP));
    }

    private void quit() {
        Log.i(TAG, "quit requested, node " + (node != null ? "present" : "absent"));
        quitRequested = true;
        stopping = true;
        stopTunnel(true);
        stopNode();
        state = NodeState.UNREACHABLE;
        gate.update(state, 0);
        path.observe(state, false);
        pathReady = false;
        canConnect = false;
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
    }

    /** The first start of an install with no settings file yet is the one that names the node after the phone. */
    private void decideName(File dir) {
        if (!Prefs.nameDecided(this)) {
            Prefs.setNamePending(this, !new File(dir, "config.json").exists());
        }
    }

    private void applyPhoneName() {
        String name = PhoneName.forThisPhone(this);
        if (name == null || NodeApi.setNodeName(this, name)) {
            Prefs.setNamePending(this, false);
        }
    }

    private void startNode() {
        try {
            File dir = getFilesDir();
            decideName(dir);
            File bin = new File(getApplicationInfo().nativeLibraryDir, "libmeshtown.so");
            ProcessBuilder pb = new ProcessBuilder(bin.getPath(), "run", "--config", new File(dir, "config.json").getPath());
            pb.directory(dir);
            pb.environment().put("HOME", dir.getPath());
            String rustLog = readRustLog(dir);
            if (rustLog != null) {
                pb.environment().put("RUST_LOG", rustLog);
            }
            pb.redirectErrorStream(true);
            pb.redirectOutput(new File(dir, "node.log"));
            node = pb.start();
        } catch (Exception e) {
            Log.e(TAG, "node start failed", e);
        }
    }

    /** Log filter from a `rust-log` file: the app's private dir first, then the external files dir. */
    private String readRustLog(File dir) {
        String value = readFirstLine(new File(dir, "rust-log"));
        if (value != null) {
            return value;
        }
        File external = getExternalFilesDir(null);
        return external == null ? null : readFirstLine(new File(external, "rust-log"));
    }

    private static String readFirstLine(File f) {
        if (!f.isFile()) {
            return null;
        }
        try (BufferedReader r = new BufferedReader(new FileReader(f))) {
            String line = r.readLine();
            return line == null || line.trim().isEmpty() ? null : line.trim();
        } catch (Exception e) {
            return null;
        }
    }

    private void stopNode() {
        if (node != null) {
            Process dying = node;
            node = null;
            new Thread(() -> stopQuietly(dying), "mesh-node-stop").start();
        }
    }

    private static void stopQuietly(Process dying) {
        try {
            boolean forced = NodeStopper.stop(new ProcessTarget(dying), NodeStopper.GRACE_MS);
            Log.i(TAG, "node stopped, forced=" + forced);
        } catch (Throwable e) {
            Log.e(TAG, "node stop failed", e);
        }
    }

    private void pollLoop() {
        while (!stopping) {
            try {
                tick();
                pause();
            } catch (InterruptedException e) {
                return;
            } catch (RuntimeException e) {
                Log.e(TAG, "poll failed", e);
            }
        }
    }

    private void pause() throws InterruptedException {
        synchronized (POLL_LOCK) {
            POLL_LOCK.wait(PollPace.nodePollMs(TunnelService.running, appVisible, restore.holding()));
        }
    }

    private void tick() {
        if (node != null && !node.isAlive()) {
            startNode();
        }
        NodeState s = NodeApi.fetchState(this);
        state = s;
        long now = SystemClock.elapsedRealtime();
        canConnect = gate.update(s, now);
        if (s.reachable && Prefs.namePending(this)) {
            applyPhoneName();
        }
        boolean vpnOn = TunnelService.running;
        path.observe(s, vpnOn || restore.holding());
        publishPath();
        if (restore.update(Prefs.vpnWanted(this), s, vpnOn, path.ready(), now)) {
            Log.i(TAG, "restore: starting tunnel, path ready=" + path.ready());
            startTunnel();
        }
        startProbe();
        announceExit(s, vpnOn);
        show(StatusText.notice(s, vpnOn, pathReady), vpnOn, canConnect);
    }

    private void publishPath() {
        boolean ready = path.ready();
        if (ready != pathReady) {
            pathReady = ready;
            TunnelTile.refresh(this);
        }
    }

    /** Opens a probe connection through the local proxy when one is due; the result wakes the poll loop. */
    private void startProbe() {
        PathGate.Attempt a = path.begin(SystemClock.elapsedRealtime());
        if (a == null) {
            return;
        }
        Thread t = new Thread(() -> {
            boolean ok = PathProbe.connect("127.0.0.1", TunnelService.SOCKS_PORT, PathProbe.target(a.number),
                    PathProbe.TARGET_PORT, PathProbe.TIMEOUT_MS);
            path.finish(a.key, ok, SystemClock.elapsedRealtime());
            if (ok) {
                Log.i(TAG, "path ready: " + a.key);
                wake();
            }
        }, "mesh-probe");
        t.setDaemon(true);
        t.start();
    }

    /** One alert on the online -> offline transition of the chosen exit, not on every poll. */
    private void announceExit(NodeState s, boolean vpnOn) {
        boolean lost = vpnOn && s.exitLost();
        if (lost && !exitWasLost) {
            notifier.alertExitLost(StatusText.exitOffline(s));
        } else if (!lost && exitWasLost) {
            notifier.clearAlert();
        }
        exitWasLost = lost;
    }

    private void show(StatusText.Notice text, boolean vpnOn, boolean canConnect) {
        String key = text.title + "|" + text.text + vpnOn + canConnect;
        if (key.equals(shown)) {
            return;
        }
        shown = key;
        notifier.update(text, vpnOn, canConnect);
    }

    @Override
    public void onDestroy() {
        stopping = true;
        stopNode();
        super.onDestroy();
    }
}
