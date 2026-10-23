package app.meshtown;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

/** Stream copy that refuses to move more than a fixed number of bytes. */
final class LimitedCopy {
    private static final int BUFFER = 64 * 1024;

    private LimitedCopy() {}

    /** Copies everything and returns the count; throws once more than {@code limit} bytes were read. */
    static long copy(InputStream in, OutputStream out, long limit) throws IOException {
        byte[] buf = new byte[BUFFER];
        long total = 0;
        int n;
        while ((n = in.read(buf)) >= 0) {
            total += n;
            check(total, limit);
            out.write(buf, 0, n);
        }
        return total;
    }

    private static void check(long total, long limit) throws IOException {
        if (total > limit) {
            throw new IOException("too large");
        }
    }
}
