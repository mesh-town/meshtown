package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import com.google.zxing.WriterException;

import java.util.Arrays;
import java.util.Random;

import org.junit.Test;

public class QrPhotoSkewTest {
    private static final int W = 960;
    private static final int H = 1280;
    private static final int INVITE_LENGTH = 754;
    private static final int SHORT_LENGTH = 178;
    /** Share of the frame width taken by the large code. */
    private static final double LARGE = 0.45;
    /** Width of an oblique code relative to its height: the angle squeezes it. */
    private static final double SQUEEZE = 0.6;

    /** A code {@code heightShare} of the frame tall, squeezed and narrowed on one side. */
    private static QrFixture.Pose oblique(double heightShare, double left, double right) {
        return new QrFixture.Pose().size(heightShare * SQUEEZE * H / W, heightShare).sides(left, right);
    }

    @Test
    public void largeCodeWithPerspectiveIsRead() throws WriterException {
        QrFixture fixture = new QrFixture(INVITE_LENGTH);
        QrFixture.Pose pose = new QrFixture.Pose().size(LARGE, LARGE * W / H).sides(1, 0.8);

        assertEquals(fixture.text, QrPhoto.decode(fixture.frame(W, H, pose), W, H));
    }

    @Test
    public void rotatedPhotoIsRead() throws WriterException {
        QrFixture fixture = new QrFixture(INVITE_LENGTH);
        QrFixture.Pose pose = new QrFixture.Pose().size(LARGE, LARGE * W / H).sides(1, 0.8);
        int[] upright = fixture.frame(W, H, pose);

        assertEquals(fixture.text, QrPhoto.decode(QrFixture.turnRight(upright, W, H), H, W));
    }

    @Test
    public void skewedLeftIsRead() throws WriterException {
        QrFixture fixture = new QrFixture(SHORT_LENGTH);
        QrFixture.Pose pose = oblique(0.30, 0.75, 1);

        assertEquals(fixture.text, QrPhoto.decode(fixture.frame(W, H, pose), W, H));
    }

    @Test
    public void skewedRightIsRead() throws WriterException {
        QrFixture fixture = new QrFixture(SHORT_LENGTH);
        QrFixture.Pose pose = oblique(0.30, 1, 0.7);

        assertEquals(fixture.text, QrPhoto.decode(fixture.frame(W, H, pose), W, H));
    }

    @Test
    public void smallSkewedIsRead() throws WriterException {
        QrFixture fixture = new QrFixture(SHORT_LENGTH);
        QrFixture.Pose pose = oblique(0.25, 0.7, 1);

        assertEquals(fixture.text, QrPhoto.decode(fixture.frame(W, H, pose), W, H));
    }

    @Test
    public void noCodeStaysNull() {
        Random random = new Random(3);
        int[] noise = new int[W * H];
        for (int i = 0; i < noise.length; i++) {
            int v = 100 + random.nextInt(60) + (i % W) / 16;
            noise[i] = 0xFF000000 | (v << 16) | (v << 8) | v;
        }

        assertNull(QrPhoto.decode(noise, W, H));
    }

    @Test
    public void tinyPhotoDoesNotThrow() {
        int[] tiny = new int[60 * 40];
        Arrays.fill(tiny, QrFixture.BACKGROUND);

        assertNull(QrPhoto.decode(tiny, 60, 40));
        assertNull(QrPhoto.decode(new int[1], 1, 1));
    }
}
