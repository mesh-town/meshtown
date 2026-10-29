package app.meshtown;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotEquals;
import static org.junit.Assert.assertTrue;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import org.junit.After;
import org.junit.Test;

public class PathProbeTest {
    /** What the fake proxy does with an accepted connection. */
    private interface Script {
        void run(Socket c, InputStream in, OutputStream out) throws Exception;
    }

    private ServerSocket server;
    private volatile byte[] request;

    @After
    public void close() throws IOException {
        if (server != null) {
            server.close();
        }
    }

    private int serve(Script script) throws IOException {
        server = new ServerSocket(0);
        Thread t = new Thread(() -> {
            try (Socket c = server.accept()) {
                script.run(c, c.getInputStream(), c.getOutputStream());
            } catch (Exception ignored) {
                // the test closes the server at the end
            }
        });
        t.setDaemon(true);
        t.start();
        return server.getLocalPort();
    }

    private static void skip(InputStream in, int n) throws IOException {
        for (int i = 0; i < n; i++) {
            in.read();
        }
    }

    private Script replying(byte[] method, byte[] reply) {
        return (c, in, out) -> {
            skip(in, 3);
            out.write(method);
            out.flush();
            byte[] head = new byte[5];
            for (int i = 0; i < 5; i++) {
                head[i] = (byte) in.read();
            }
            byte[] rest = new byte[(head[4] & 0xff) + 2];
            for (int i = 0; i < rest.length; i++) {
                rest[i] = (byte) in.read();
            }
            byte[] all = new byte[5 + rest.length];
            System.arraycopy(head, 0, all, 0, 5);
            System.arraycopy(rest, 0, all, 5, rest.length);
            request = all;
            out.write(reply);
            out.flush();
            Thread.sleep(200);
        };
    }

    private static final byte[] OK_METHOD = {5, 0};
    private static final byte[] OK_REPLY = {5, 0, 0, 1, 0, 0, 0, 0, 0, 0};

    private boolean probe(int port, int timeoutMs) {
        return PathProbe.connect("127.0.0.1", port, "example.org", 443, timeoutMs);
    }

    @Test
    public void okReplyMeansReady() throws Exception {
        assertTrue(probe(serve(replying(OK_METHOD, OK_REPLY)), 5_000));
    }

    @Test
    public void hostUnreachableIsNotReady() throws Exception {
        byte[] reply = {5, 4, 0, 1, 0, 0, 0, 0, 0, 0};
        assertFalse(probe(serve(replying(OK_METHOD, reply)), 5_000));
    }

    @Test
    public void silenceTimesOut() throws Exception {
        int port = serve((c, in, out) -> {
            skip(in, 3);
            out.write(OK_METHOD);
            out.flush();
            Thread.sleep(5_000);
        });
        long t0 = System.nanoTime();
        assertFalse(probe(port, 300));
        assertTrue((System.nanoTime() - t0) / 1_000_000L < 3_000);
    }

    @Test
    public void droppedConnectionIsNotReady() throws Exception {
        int port = serve((c, in, out) -> {
            skip(in, 3);
            out.write(OK_METHOD);
            out.flush();
        });
        assertFalse(probe(port, 5_000));
    }

    @Test
    public void wrongVersionIsNotReady() throws Exception {
        assertFalse(probe(serve(replying(new byte[] {4, 0}, OK_REPLY)), 5_000));
    }

    @Test
    public void refusedMethodIsNotReady() throws Exception {
        assertFalse(probe(serve(replying(new byte[] {5, (byte) 0xFF}, OK_REPLY)), 5_000));
    }

    @Test
    public void closedPortIsNotReady() throws Exception {
        ServerSocket s = new ServerSocket(0);
        int port = s.getLocalPort();
        s.close();
        assertFalse(probe(port, 5_000));
    }

    @Test
    public void sendsDomainAndPort() throws Exception {
        assertTrue(probe(serve(replying(OK_METHOD, OK_REPLY)), 5_000));
        byte[] name = "example.org".getBytes(StandardCharsets.US_ASCII);
        byte[] want = new byte[7 + name.length];
        want[0] = 5;
        want[1] = 1;
        want[3] = 3;
        want[4] = (byte) name.length;
        System.arraycopy(name, 0, want, 5, name.length);
        want[5 + name.length] = 0x01;
        want[6 + name.length] = (byte) 0xBB;
        assertArrayEquals(want, request);
    }

    @Test
    public void targetsAlternate() {
        assertEquals("connectivitycheck.gstatic.com", PathProbe.target(0));
        assertEquals("one.one.one.one", PathProbe.target(1));
        assertEquals(PathProbe.target(0), PathProbe.target(2));
        assertNotEquals(PathProbe.target(0), PathProbe.target(3));
    }
}
