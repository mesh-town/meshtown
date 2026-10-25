package app.meshtown;

import com.google.zxing.BarcodeFormat;
import com.google.zxing.BinaryBitmap;
import com.google.zxing.DecodeHintType;
import com.google.zxing.ReaderException;
import com.google.zxing.RGBLuminanceSource;
import com.google.zxing.LuminanceSource;
import com.google.zxing.common.HybridBinarizer;
import com.google.zxing.qrcode.QRCodeReader;
import app.meshtown.QrWarp.Frame;
import java.util.EnumMap;
import java.util.Map;
import java.util.function.Supplier;

/** Reads a QR code out of photo pixels; no Android types. */
final class QrPhoto {
    private static final int MAX_SHRINK = 4;
    private static final int MIN_SIDE = 100;
    private static final int LARGE_SIDE = 1200;
    private static final double STRETCH_X = 1.5;
    private static final double SIDE_SCALE = 0.75;

    private QrPhoto() {}

    /**
     * The text of the code, or null when the pixels hold none. The detector sometimes misjudges the
     * module size of a large code and rejects its format, so a failed read is retried on the clean
     * pixel grid and on shrunken copies; then on turned copies, since the detector never turns the
     * picture itself; then on copies stretched and narrowed to undo the foreshortening of a code
     * photographed at an angle.
     */
    static String decode(int[] argb, int width, int height) {
        Frame photo = new Frame(argb, width, height);
        String text = plain(photo);
        if (text != null) {
            return text;
        }
        Frame large = QrWarp.fit(photo, LARGE_SIDE, MIN_SIDE);
        return firstRead(() -> turned(photo, large), () -> oblique(large == null ? photo : large));
    }

    @SafeVarargs
    private static String firstRead(Supplier<String>... stages) {
        for (Supplier<String> stage : stages) {
            String text = stage.get();
            if (text != null) {
                return text;
            }
        }
        return null;
    }

    static String plain(Frame photo) {
        String text = read(photo, false);
        if (text == null) {
            text = read(photo, true);
        }
        for (int factor = 2; text == null && factor <= MAX_SHRINK; factor++) {
            text = readShrunk(photo, factor);
        }
        return text;
    }

    /** The photo and its reduced copy (null when the photo is not larger), each turned a quarter. */
    static String turned(Frame photo, Frame large) {
        String text = readTurns(photo, 1);
        return text != null ? text : readTurns(large, 1);
    }

    /** A copy stretched along x with one side narrowed, read upright and on its side. */
    static String oblique(Frame base) {
        String text = readSkewed(base, true);
        return text != null ? text : readSkewed(base, false);
    }

    private static String readSkewed(Frame base, boolean leftSide) {
        Frame skewed = QrWarp.skew(base, STRETCH_X, SIDE_SCALE, leftSide);
        return readTurns(skewed, 0, 1);
    }

    /** Reads the frame turned by each of the quarter turns in order; a null frame holds nothing. */
    private static String readTurns(Frame frame, int... quarterTurns) {
        if (frame == null) {
            return null;
        }
        for (int turns : quarterTurns) {
            String text = read(turns == 0 ? frame : QrWarp.rotate(frame, turns), false);
            if (text != null) {
                return text;
            }
        }
        return null;
    }

    private static String readShrunk(Frame photo, int factor) {
        int w = photo.width / factor;
        int h = photo.height / factor;
        if (w < MIN_SIDE || h < MIN_SIDE) {
            return null;
        }
        return read(new Frame(shrink(photo.argb, photo.width, w, h, factor), w, h), false);
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

    private static String read(Frame frame, boolean pure) {
        Map<DecodeHintType, Object> hints = new EnumMap<>(DecodeHintType.class);
        hints.put(DecodeHintType.TRY_HARDER, Boolean.TRUE);
        hints.put(DecodeHintType.POSSIBLE_FORMATS, java.util.Collections.singletonList(BarcodeFormat.QR_CODE));
        if (pure) {
            hints.put(DecodeHintType.PURE_BARCODE, Boolean.TRUE);
        }
        LuminanceSource source = new RGBLuminanceSource(frame.width, frame.height, frame.argb);
        try {
            return new QRCodeReader().decode(new BinaryBitmap(new HybridBinarizer(source)), hints).getText();
        } catch (ReaderException e) {
            return null;
        }
    }
}
