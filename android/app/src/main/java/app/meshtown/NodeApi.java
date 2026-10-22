package app.meshtown;

import android.content.Context;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** Talks to the local node page: address, token and the state snapshot. */
final class NodeApi {
    static final int WEB_PORT = 7777;

    private NodeApi() {}

    static String token(Context ctx) {
        try {
            File f = new File(ctx.getFilesDir(), "web-token");
            return new String(Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8).trim();
        } catch (Exception e) {
            return null;
        }
    }

    static String pageUrl(Context ctx) {
        String base = "http://127.0.0.1:" + WEB_PORT + "/";
        String token = token(ctx);
        return token == null ? base : base + "?token=" + token;
    }

    /** Fetches {@code /api/state}; any failure is "unreachable". */
    static NodeState fetchState(Context ctx) {
        String token = token(ctx);
        if (token == null) {
            return NodeState.UNREACHABLE;
        }
        HttpURLConnection c = null;
        try {
            c = (HttpURLConnection) new URL("http://127.0.0.1:" + WEB_PORT + "/api/state").openConnection();
            c.setConnectTimeout(1500);
            c.setReadTimeout(2500);
            c.setRequestProperty("X-Meshtown-Token", token);
            if (c.getResponseCode() != 200) {
                return NodeState.UNREACHABLE;
            }
            return NodeState.parse(readAll(c.getInputStream()));
        } catch (Exception e) {
            return NodeState.UNREACHABLE;
        } finally {
            if (c != null) {
                c.disconnect();
            }
        }
    }

    /** Sets {@code node.name} through the settings API; true when the node accepted it. */
    static boolean setNodeName(Context ctx, String name) {
        try {
            String token = token(ctx);
            String current = request(token, "GET", "/api/config", null);
            JSONObject config = new JSONObject().put("node", new JSONObject().put("name", name));
            String body = new JSONObject().put("rev", new JSONObject(current).getLong("rev")).put("config", config)
                    .toString();
            request(token, "PUT", "/api/config", body);
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    /** One call to the local node; throws on any status but 200. */
    private static String request(String token, String method, String path, String body) throws Exception {
        HttpURLConnection c = (HttpURLConnection) new URL("http://127.0.0.1:" + WEB_PORT + path).openConnection();
        try {
            c.setConnectTimeout(1500);
            c.setReadTimeout(2500);
            c.setRequestMethod(method);
            c.setRequestProperty("X-Meshtown-Token", token);
            if (body != null) {
                // The node requires the page origin on every write.
                c.setRequestProperty("Origin", "http://127.0.0.1:" + WEB_PORT);
                c.setDoOutput(true);
                c.setRequestProperty("Content-Type", "application/json");
                c.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));
            }
            if (c.getResponseCode() != 200) {
                throw new java.io.IOException("status " + c.getResponseCode());
            }
            return readAll(c.getInputStream());
        } finally {
            c.disconnect();
        }
    }

    private static String readAll(InputStream in) throws Exception {
        try (InputStream s = in) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[8192];
            for (int n = s.read(buf); n > 0; n = s.read(buf)) {
                out.write(buf, 0, n);
            }
            return out.toString("UTF-8");
        }
    }
}
