package app.meshtown;

/** Pixel transforms that help the QR detector with large, rotated or oblique photos; no Android types. */
final class QrWarp {
    private static final int WHITE = 255;

    /** An opaque ARGB picture. */
    static final class Frame {
        final int[] argb;
        final int width;
        final int height;

        Frame(int[] argb, int width, int height) {
            this.argb = argb;
            this.width = width;
            this.height = height;
        }
    }

    private QrWarp() {}

    /**
     * A box-filtered copy whose longer side is {@code longSide}, or null when the photo is not larger
     * than that or the copy would be too thin to hold a code.
     */
    static Frame fit(Frame src, int longSide, int minSide) {
        int longest = Math.max(src.width, src.height);
        if (longest <= longSide) {
            return null;
        }
        double factor = (double) longest / longSide;
        int w = (int) Math.round(src.width / factor);
        int h = (int) Math.round(src.height / factor);
        return w < minSide || h < minSide ? null : box(src, w, h, factor);
    }

    private static Frame box(Frame src, int w, int h, double factor) {
        int[] out = new int[w * h];
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                out[y * w + x] = average(src, x * factor, y * factor, factor);
            }
        }
        return new Frame(out, w, h);
    }

    private static int average(Frame src, double left, double top, double size) {
        int x0 = (int) left;
        int y0 = (int) top;
        int x1 = Math.min(src.width, (int) Math.ceil(left + size));
        int y1 = Math.min(src.height, (int) Math.ceil(top + size));
        long sum = 0;
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                sum += luma(src.argb[y * src.width + x]);
            }
        }
        return grey((int) (sum / Math.max(1, (long) (x1 - x0) * (y1 - y0))));
    }

    /** Turns the picture clockwise by {@code quarterTurns} quarter turns (1..3). */
    static Frame rotate(Frame src, int quarterTurns) {
        boolean sideways = quarterTurns % 2 != 0;
        int w = sideways ? src.height : src.width;
        int h = sideways ? src.width : src.height;
        int[] out = new int[w * h];
        for (int y = 0; y < src.height; y++) {
            for (int x = 0; x < src.width; x++) {
                out[target(quarterTurns, x, y, src.width, src.height, w)] = src.argb[y * src.width + x];
            }
        }
        return new Frame(out, w, h);
    }

    private static int target(int turns, int x, int y, int srcW, int srcH, int w) {
        int tx = turns == 1 ? srcH - 1 - y : turns == 2 ? srcW - 1 - x : y;
        int ty = turns == 1 ? x : turns == 2 ? srcH - 1 - y : srcW - 1 - x;
        return ty * w + tx;
    }

    /**
     * Stretches the picture horizontally by {@code stretchX} and, through a projective map with
     * vertical edges, narrows one vertical side to {@code sideScale} of its height, which undoes part
     * of the foreshortening of a code seen at an angle. The output is grey.
     */
    static Frame skew(Frame src, double stretchX, double sideScale, boolean leftSide) {
        int w = (int) Math.round(src.width * stretchX);
        int h = src.height;
        double left = leftSide ? sideScale : 1;
        double k = left / (leftSide ? 1 : sideScale) - 1;
        float[] grey = grey(src);
        int[] out = new int[w * h];
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                double u = (x + 0.5) / (w * (1 + k) - k * (x + 0.5));
                double span = h * left / (1 + k * u);
                double sy = h / 2.0 + (y + 0.5 - h / 2.0) * h / span;
                out[y * w + x] = grey((int) Math.round(sample(grey, src.width, h, u * src.width, sy)));
            }
        }
        return new Frame(out, w, h);
    }

    private static float[] grey(Frame src) {
        float[] out = new float[src.argb.length];
        for (int i = 0; i < out.length; i++) {
            out[i] = luma(src.argb[i]);
        }
        return out;
    }

    private static int luma(int p) {
        return (((p >> 16) & 0xFF) + 2 * ((p >> 8) & 0xFF) + (p & 0xFF)) / 4;
    }

    private static int grey(int v) {
        int c = Math.max(0, Math.min(255, v));
        return 0xFF000000 | (c << 16) | (c << 8) | c;
    }

    /** Bilinear sample at pixel-edge coordinates; white outside the picture. */
    private static float sample(float[] g, int w, int h, double x, double y) {
        double px = x - 0.5;
        double py = y - 0.5;
        if (px < -1 || py < -1 || px > w || py > h) {
            return WHITE;
        }
        int x0 = (int) Math.floor(px);
        int y0 = (int) Math.floor(py);
        double fx = px - x0;
        double fy = py - y0;
        double top = at(g, w, h, x0, y0) * (1 - fx) + at(g, w, h, x0 + 1, y0) * fx;
        double bottom = at(g, w, h, x0, y0 + 1) * (1 - fx) + at(g, w, h, x0 + 1, y0 + 1) * fx;
        return (float) (top * (1 - fy) + bottom * fy);
    }

    private static float at(float[] g, int w, int h, int x, int y) {
        return x < 0 || y < 0 || x >= w || y >= h ? WHITE : g[y * w + x];
    }
}
