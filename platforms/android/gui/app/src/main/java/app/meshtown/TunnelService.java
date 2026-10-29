package app.meshtown;

import android.content.Intent;
import android.net.VpnService;
import android.os.ParcelFileDescriptor;
import android.util.Log;
import com.github.shadowsocks.bg.Tun2proxy;

/** Routes all device traffic into the node's SOCKS5 port; the node itself lives in {@link NodeService}. */
public class TunnelService extends VpnService {
    static final String ACTION_STOP = "app.meshtown.STOP";
    private static final String TAG = "MeshTown";
    static final int SOCKS_PORT = 1080;
    /** The DNS server given to the system; the pool of made-up answer addresses must not contain it. */
    private static final String DNS_SERVER = "198.18.0.1";
    private static final String FAKE_IP_POOL = "198.19.0.0/16";

    static volatile boolean running;

    private ParcelFileDescriptor tun;
    private Thread bridge;

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && ACTION_STOP.equals(intent.getAction())) {
            shutdown();
            stopSelf();
            return START_NOT_STICKY;
        }
        if (!running) {
            running = true;
            TunnelTile.refresh(this);
            NodeService.wake();
            // Without an intent the system started us (always-on VPN): make sure the node is up too.
            if (intent == null) {
                startForegroundService(new Intent(this, NodeService.class));
            }
            new Thread(this::bringUp, "mesh-up").start();
        }
        return START_NOT_STICKY;
    }

    private void bringUp() {
        try {
            Builder b = new Builder()
                    .setSession("MeshTown")
                    .addAddress("10.0.0.2", 24)
                    .addRoute("0.0.0.0", 0)
                    .addAddress("fd00:6d74::2", 64)
                    .addRoute("::", 0)
                    .addDnsServer(DNS_SERVER)
                    .setMtu(1500);
            b.addDisallowedApplication(getPackageName());
            tun = b.establish();
            if (tun == null) {
                throw new IllegalStateException("VPN permission was revoked");
            }
            final int fd = tun.getFd();
            bridge = new Thread(() -> {
                String args = "tun2proxy --proxy socks5://127.0.0.1:" + SOCKS_PORT
                        + " --tun-fd " + fd + " --close-fd-on-drop false --dns virtual --virtual-dns-pool " + FAKE_IP_POOL
                        + " --udpgw-server 240.0.0.1:7300"
                        + " --verbosity info";
                Log.i(TAG, "tun2proxy exited with " + Tun2proxy.run(args, (char) 1500));
            }, "tun2proxy");
            bridge.start();
        } catch (Exception e) {
            Log.e(TAG, "start failed", e);
            shutdown();
            stopSelf();
        }
    }

    private synchronized void shutdown() {
        running = false;
        TunnelTile.refresh(this);
        NodeService.wake();
        Tun2proxy.stop();
        if (bridge != null) {
            try {
                bridge.join(3000);
            } catch (InterruptedException ignored) {
                Thread.currentThread().interrupt();
            }
            bridge = null;
        }
        closeTun();
    }

    private void closeTun() {
        if (tun == null) {
            return;
        }
        try {
            tun.close();
        } catch (Exception ignored) {
            // already closed
        }
        tun = null;
    }

    @Override
    public void onRevoke() {
        Prefs.setVpnWanted(this, false);
        shutdown();
        stopSelf();
    }

    @Override
    public void onDestroy() {
        shutdown();
        super.onDestroy();
    }
}
