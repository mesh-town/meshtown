package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;

import com.google.zxing.BarcodeFormat;
import com.google.zxing.EncodeHintType;
import com.google.zxing.WriterException;
import com.google.zxing.common.BitMatrix;
import com.google.zxing.qrcode.QRCodeWriter;
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel;

import java.util.Arrays;
import java.util.EnumMap;
import java.util.Map;
import java.util.Random;

import org.junit.Test;

public class QrPhotoTest {
    private static final int DARK = 0xFF000000;
    private static final int LIGHT = 0xFFFFFFFF;
    private static final int GREY = 0xFF808080;
    private static final String ALPHABET = "abcdefghijklmnopqrstuvwxyz234567";

    @Test
    public void decodesInviteFromRenderedCode() throws WriterException {
        String invite = invite(754);
        BitMatrix code = encode(invite);
        int side = code.getWidth() * 4;

        String decoded = QrPhoto.decode(render(code, 4, side, side, 0, 0, LIGHT), side, side);

        assertEquals(invite, decoded);
    }

    @Test
    public void decodesCodeOffCentreOnGrey() throws WriterException {
        String invite = invite(754);
        BitMatrix code = encode(invite);
        int side = code.getWidth() * 4;
        int width = side * 2;
        int height = side * 2 - side / 3;

        int[] pixels = render(code, 4, width, height, side / 2 + 37, side / 5, GREY);

        assertEquals(invite, QrPhoto.decode(pixels, width, height));
    }

    @Test
    public void blankPhotoHasNoCode() {
        int[] white = new int[400 * 300];
        Arrays.fill(white, LIGHT);
        int[] grey = new int[400 * 300];
        Arrays.fill(grey, GREY);

        assertNull(QrPhoto.decode(white, 400, 300));
        assertNull(QrPhoto.decode(grey, 400, 300));
    }

    @Test
    public void shortCodeIsDecodedToo() throws WriterException {
        BitMatrix code = encode("mesh-invite2-short");
        int side = code.getWidth() * 4;

        String decoded = QrPhoto.decode(render(code, 4, side, side, 0, 0, LIGHT), side, side);

        assertNotNull(decoded);
        assertEquals("mesh-invite2-short", decoded);
    }

    // helpers

    private static String invite(int length) {
        Random random = new Random(7);
        StringBuilder text = new StringBuilder("mesh-invite2-");
        while (text.length() < length) {
            text.append(ALPHABET.charAt(random.nextInt(ALPHABET.length())));
        }
        return text.toString();
    }

    /** The matrix includes the quiet zone, one pixel per module. */
    private static BitMatrix encode(String text) throws WriterException {
        Map<EncodeHintType, Object> hints = new EnumMap<>(EncodeHintType.class);
        hints.put(EncodeHintType.ERROR_CORRECTION, ErrorCorrectionLevel.L);
        hints.put(EncodeHintType.MARGIN, 4);
        return new QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, 0, 0, hints);
    }

    /** Draws the matrix scaled by {@code scale} at (left, top) over a {@code background} frame. */
    private static int[] render(BitMatrix code, int scale, int width, int height, int left, int top, int background) {
        int[] pixels = new int[width * height];
        Arrays.fill(pixels, background);
        for (int y = 0; y < code.getHeight() * scale; y++) {
            for (int x = 0; x < code.getWidth() * scale; x++) {
                pixels[(top + y) * width + left + x] = code.get(x / scale, y / scale) ? DARK : LIGHT;
            }
        }
        return pixels;
    }
}
