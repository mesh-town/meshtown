package app.meshtown;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

/**
 * Asks the local SOCKS5 port to open a connection to a well-known host. The reply arrives only after
 * the far side has opened the target, so a positive answer means traffic can flow through the whole
 * path. No data is sent after the reply. Pure {@code java.net}; never throws.
 */
final class PathProbe {
    static final String[] TARGETS = {"connectivitycheck.gstatic.com", "one.one.one.one"};
    static final int TARGET_PORT = 443;
    static final int TIMEOUT_MS = 30_000;
    static final int CONNECT_TIMEOUT_MS = 1_000;

    private PathProbe() {
    }

    /** The target for an attempt: even attempts use the first host, odd ones the fallback. */
    static String target(int attempt) {
        return TARGETS[attempt % TARGETS.length];
    }

    /** True only when the proxy accepts the greeting and answers the CONNECT with success. */
    static boolean connect(String proxyHost, int proxyPort, String host, int port, int timeoutMs) {
        long deadline = System.nanoTime() + timeoutMs * 1_000_000L;
        try (Socket sock = new Socket()) {
            sock.connect(new InetSocketAddress(proxyHost, proxyPort), Math.min(CONNECT_TIMEOUT_MS, timeoutMs));
            return handshake(sock, host, port, deadline);
        } catch (IOException | RuntimeException e) {
            return false;
        }
    }

    private static boolean handshake(Socket sock, String host, int port, long deadline) throws IOException {
        OutputStream out = sock.getOutputStream();
        InputStream in = sock.getInputStream();
        out.write(new byte[] {5, 1, 0});
        out.flush();
        byte[] method = read(sock, in, 2, deadline);
        if (method[0] != 5 || method[1] != 0) {
            return false;
        }
        out.write(request(host, port));
        out.flush();
        byte[] head = read(sock, in, 4, deadline);
        return head[0] == 5 && head[1] == 0;
    }

    private static byte[] request(String host, int port) {
        byte[] name = host.getBytes(StandardCharsets.US_ASCII);
        byte[] req = new byte[7 + name.length];
        req[0] = 5;
        req[1] = 1;
        req[3] = 3;
        req[4] = (byte) name.length;
        System.arraycopy(name, 0, req, 5, name.length);
        req[5 + name.length] = (byte) (port >> 8);
        req[6 + name.length] = (byte) port;
        return req;
    }

    private static byte[] read(Socket sock, InputStream in, int len, long deadline) throws IOException {
        byte[] buf = new byte[len];
        int got = 0;
        while (got < len) {
            long left = (deadline - System.nanoTime()) / 1_000_000L;
            if (left <= 0) {
                throw new IOException("timeout");
            }
            sock.setSoTimeout((int) Math.min(left, Integer.MAX_VALUE));
            int n = in.read(buf, got, len - got);
            if (n < 0) {
                throw new IOException("closed");
            }
            got += n;
        }
        return buf;
    }
}
