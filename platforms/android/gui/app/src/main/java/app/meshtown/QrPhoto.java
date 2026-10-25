package app.meshtown;

import com.google.zxing.BarcodeFormat;
import com.google.zxing.BinaryBitmap;
import com.google.zxing.DecodeHintType;
import com.google.zxing.ReaderException;
import com.google.zxing.RGBLuminanceSource;
import com.google.zxing.common.HybridBinarizer;
import com.google.zxing.qrcode.QRCodeReader;
import java.util.EnumMap;
import java.util.Map;

/** Reads a QR code out of photo pixels; no Android types. */
final class QrPhoto {
    private static final int MAX_SHRINK = 4;
    private static final int MIN_SIDE = 100;

    private QrPhoto() {}

    /**
     * The text of the code, or null when the pixels hold none. The detector sometimes misjudges the
     * module size of a large code and rejects its format, so a failed read is retried on the clean
     * pixel grid and on shrunken copies of the photo.
     */
    static String decode(int[] argb, int width, int height) {
        String text = read(argb, width, height, false);
        if (text == null) {
            text = read(argb, width, height, true);
        }
        for (int factor = 2; text == null && factor <= MAX_SHRINK; factor++) {
            text = readShrunk(argb, width, height, factor);
        }
        return text;
    }

    private static String readShrunk(int[] argb, int width, int height, int factor) {
        int w = width / factor;
        int h = height / factor;
        if (w < MIN_SIDE || h < MIN_SIDE) {
            return null;
        }
        return read(shrink(argb, width, w, h, factor), w, h, false);
    }

    /** Box average of {@code factor} x {@code factor} blocks, per channel. */
    private static int[] shrink(int[] argb, int width, int w, int h, int factor) {
        int[] out = new int[w * h];
        int area = factor * factor;
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                out[y * w + x] = average(argb, width, x * factor, y * factor, factor, area);
            }
        }
        return out;
    }

    private static int average(int[] argb, int width, int left, int top, int factor, int area) {
        int r = 0;
        int g = 0;
        int b = 0;
        for (int dy = 0; dy < factor; dy++) {
            for (int dx = 0; dx < factor; dx++) {
                int p = argb[(top + dy) * width + left + dx];
                r += (p >> 16) & 0xFF;
                g += (p >> 8) & 0xFF;
                b += p & 0xFF;
            }
        }
        return 0xFF000000 | (r / area << 16) | (g / area << 8) | (b / area);
    }

    private static String read(int[] argb, int width, int height, boolean pure) {
        Map<DecodeHintType, Object> hints = new EnumMap<>(DecodeHintType.class);
        hints.put(DecodeHintType.TRY_HARDER, Boolean.TRUE);
        hints.put(DecodeHintType.POSSIBLE_FORMATS, java.util.Collections.singletonList(BarcodeFormat.QR_CODE));
        if (pure) {
            hints.put(DecodeHintType.PURE_BARCODE, Boolean.TRUE);
        }
        BinaryBitmap bitmap = new BinaryBitmap(new HybridBinarizer(new RGBLuminanceSource(width, height, argb)));
        try {
            return new QRCodeReader().decode(bitmap, hints).getText();
        } catch (ReaderException e) {
            return null;
        }
    }
}
