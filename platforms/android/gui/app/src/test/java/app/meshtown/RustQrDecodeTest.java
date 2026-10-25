package app.meshtown;

import static org.junit.Assert.assertEquals;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import org.junit.Test;

/** Reads codes drawn by the node (module matrices kept as test resources). */
public class RustQrDecodeTest {
    private static final int DARK = 0xFF000000;
    private static final int LIGHT = 0xFFFFFFFF;
    private static final int SCALE = 4;
    private static final int QUIET = 4;
    private static final String ALPHABET = "abcdefghijklmnopqrstuvwxyz234567";

    @Test
    public void readsShortCode() throws IOException {
        check(159);
    }

    @Test
    public void readsMediumCode() throws IOException {
        check(754);
    }

    @Test
    public void readsLargestCode() throws IOException {
        check(1273);
    }

    private static void check(int length) throws IOException {
        List<String> rows = matrix("rust_qr_" + length + ".txt");
        int side = (rows.size() + 2 * QUIET) * SCALE;
        int[] pixels = new int[side * side];
        Arrays.fill(pixels, LIGHT);
        for (int y = 0; y < rows.size(); y++) {
            for (int x = 0; x < rows.size(); x++) {
                if (rows.get(y).charAt(x) == '1') {
                    fill(pixels, side, (x + QUIET) * SCALE, (y + QUIET) * SCALE);
                }
            }
        }
        assertEquals(invite(length), QrPhoto.decode(pixels, side, side));
    }

    private static void fill(int[] pixels, int side, int left, int top) {
        for (int dy = 0; dy < SCALE; dy++) {
            for (int dx = 0; dx < SCALE; dx++) {
                pixels[(top + dy) * side + left + dx] = DARK;
            }
        }
    }

    private static String invite(int length) {
        StringBuilder text = new StringBuilder("mesh-invite2-");
        for (int i = 0; text.length() < length; i++) {
            text.append(ALPHABET.charAt((i * 7 + 3) % 32));
        }
        return text.toString();
    }

    private static List<String> matrix(String name) throws IOException {
        List<String> rows = new ArrayList<>();
        try (InputStream in = RustQrDecodeTest.class.getResourceAsStream(name);
             BufferedReader reader = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8))) {
            for (String line = reader.readLine(); line != null; line = reader.readLine()) {
                if (!line.isEmpty()) {
                    rows.add(line);
                }
            }
        }
        return rows;
    }
}
