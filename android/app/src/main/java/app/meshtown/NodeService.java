package app.meshtown;

import android.app.Service;
import android.content.Intent;
import android.net.VpnService;
import android.os.SystemClock;
import android.os.IBinder;
import android.util.Log;
import java.io.File;

/** Foreground service that owns the node process and keeps the status notification fresh. */
public class NodeService extends Service {
    static final String ACTION_BOOT = "app.meshtown.BOOT";
    static final String ACTION_CONNECT = "app.meshtown.CONNECT";
    static final String ACTION_DISCONNECT = "app.meshtown.DISCONNECT";
    static final String ACTION_QUIT = "app.meshtown.QUIT";
    private static final String TAG = "MeshTown";
    private static final long POLL_MS = 3000;

    /** Latest snapshot, read by the activity. */
    static volatile NodeState state = NodeState.UNREACHABLE;
    /** Set by Quit; the activity closes itself when it sees it. */
    static volatile boolean quitRequested;
    /** Whether Connect is offered (see {@link ConnectGate}); read by the activity and the tile. */
    static volatile boolean canConnect;

    private Process node;
    private StatusNotifier notifier;
    private Thread poller;
    private volatile boolean stopping;
    private volatile boolean restoreTunnel;
    private boolean exitWasLost;
    private final ConnectGate gate = new ConnectGate();
    private String shown = "";

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent == null ? null : intent.getAction();
        if (notifier == null) {
            notifier = new StatusNotifier(this);
        }
        startForeground(StatusNotifier.ID, notifier.status(StatusText.notice(NodeState.UNREACHABLE, false), false, false));
        if (ACTION_QUIT.equals(action)) {
            quit();
            return START_NOT_STICKY;
        }
        quitRequested = false;
        handle(action);
        if (poller == null) {
            startNode();
            poller = new Thread(this::pollLoop, "mesh-poll");
            poller.start();
        }
        return START_STICKY;
    }

    private void handle(String action) {
        if (ACTION_BOOT.equals(action)) {
            restoreTunnel = Prefs.vpnWanted(this) && VpnService.prepare(this) == null;
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
        quitRequested = true;
        stopping = true;
        stopTunnel(true);
        stopNode();
        state = NodeState.UNREACHABLE;
        gate.update(state, 0);
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
            pb.redirectErrorStream(true);
            pb.redirectOutput(new File(dir, "node.log"));
            node = pb.start();
        } catch (Exception e) {
            Log.e(TAG, "node start failed", e);
        }
    }

    private void stopNode() {
        if (node != null) {
            node.destroy();
            node = null;
        }
    }

    private void pollLoop() {
        while (!stopping) {
            try {
                tick();
                Thread.sleep(POLL_MS);
            } catch (InterruptedException e) {
                return;
            } catch (RuntimeException e) {
                Log.e(TAG, "poll failed", e);
            }
        }
    }

    private void tick() {
        if (node != null && !node.isAlive()) {
            startNode();
        }
        NodeState s = NodeApi.fetchState(this);
        state = s;
        canConnect = gate.update(s, SystemClock.elapsedRealtime());
        if (s.reachable && Prefs.namePending(this)) {
            applyPhoneName();
        }
        if (restoreTunnel && s.inNetwork && !TunnelService.running) {
            restoreTunnel = false;
            startTunnel();
        }
        boolean vpnOn = TunnelService.running;
        announceExit(s, vpnOn);
        show(StatusText.notice(s, vpnOn), vpnOn, canConnect);
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
