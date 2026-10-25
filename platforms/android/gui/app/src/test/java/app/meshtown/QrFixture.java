package app.meshtown;

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

/** Builds photo-like frames: a QR code seen at an angle, drawn by inverse mapping with bilinear sampling. */
final class QrFixture {
    static final int DARK = 0xFF000000;
    static final int LIGHT = 0xFFFFFFFF;
    static final int BACKGROUND = 0xFF909090;
    private static final String ALPHABET = "abcdefghijklmnopqrstuvwxyz234567";

    /** Placement of a code in a frame; heights are relative to {@code height}. */
    static final class Pose {
        double centreX = 0.5;
        double centreY = 0.5;
        double width;
        double height;
        double leftHeight = 1;
        double rightHeight = 1;

        Pose size(double widthShare, double heightShare) {
            width = widthShare;
            height = heightShare;
            return this;
        }

        Pose sides(double left, double right) {
            leftHeight = left;
            rightHeight = right;
            return this;
        }

        Pose at(double x, double y) {
            centreX = x;
            centreY = y;
            return this;
        }
    }

    final String text;
    final BitMatrix code;

    QrFixture(int length) throws WriterException {
        text = invite(length);
        Map<EncodeHintType, Object> hints = new EnumMap<>(EncodeHintType.class);
        hints.put(EncodeHintType.ERROR_CORRECTION, ErrorCorrectionLevel.L);
        hints.put(EncodeHintType.MARGIN, 4);
        code = new QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, 0, 0, hints);
    }

    static String invite(int length) {
        Random random = new Random(7);
        StringBuilder text = new StringBuilder("mesh-invite2-");
        while (text.length() < length) {
            text.append(ALPHABET.charAt(random.nextInt(ALPHABET.length())));
        }
        return text.toString();
    }

    /**
     * Draws the code with a vertical trapezoid outline: columns of the code keep their order, the
     * height falls from one side to the other with perspective-correct spacing.
     */
    int[] frame(int width, int height, Pose pose) {
        int[] pixels = new int[width * height];
        Arrays.fill(pixels, BACKGROUND);
        double bw = pose.width * width;
        double left = pose.centreX * width - bw / 2;
        double p0 = 1 / pose.leftHeight;
        double p1 = 1 / pose.rightHeight;
        for (int y = 0; y < height; y++) {
            for (int x = 0; x < width; x++) {
                pixels[y * width + x] = pixel(x + 0.5, y + 0.5, left, bw, height, pose, p0, p1);
            }
        }
        return pixels;
    }

    private int pixel(double x, double y, double left, double bw, int height, Pose pose, double p0, double p1) {
        double t = (x - left) / bw;
        if (t < 0 || t > 1) {
            return BACKGROUND;
        }
        double u = t * p0 / (p1 - t * (p1 - p0));
        double span = pose.height * height / (p0 + (p1 - p0) * u);
        double v = (y - pose.centreY * height) / span + 0.5;
        return v < 0 || v >= 1 ? BACKGROUND : shade(u, v);
    }

    /** Average of a 3 x 3 grid of module lookups: edges come out soft like a camera's. */
    private int shade(double u, double v) {
        int n = code.getWidth();
        int dark = 0;
        for (int dy = -1; dy <= 1; dy++) {
            for (int dx = -1; dx <= 1; dx++) {
                dark += module(u * n + dx * 0.3, v * n + dy * 0.3) ? 1 : 0;
            }
        }
        int value = 255 - 255 * dark / 9;
        return 0xFF000000 | (value << 16) | (value << 8) | value;
    }

    private boolean module(double x, double y) {
        int n = code.getWidth();
        int mx = Math.min(n - 1, Math.max(0, (int) Math.floor(x)));
        int my = Math.min(n - 1, Math.max(0, (int) Math.floor(y)));
        return code.get(mx, my);
    }

    /** The frame turned a quarter clockwise. */
    static int[] turnRight(int[] pixels, int width, int height) {
        int[] out = new int[pixels.length];
        for (int y = 0; y < height; y++) {
            for (int x = 0; x < width; x++) {
                out[x * height + (height - 1 - y)] = pixels[y * width + x];
            }
        }
        return out;
    }
}
