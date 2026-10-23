package app.meshtown;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.fail;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import org.junit.Test;

public class LimitedCopyTest {
    private static byte[] data(int size) {
        byte[] b = new byte[size];
        for (int i = 0; i < size; i++) {
            b[i] = (byte) i;
        }
        return b;
    }

    @Test
    public void dataBelowTheLimitIsCopiedWhole() throws IOException {
        byte[] in = data(100_000);
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        assertEquals(100_000, LimitedCopy.copy(new ByteArrayInputStream(in), out, 200_000));
        assertArrayEquals(in, out.toByteArray());
    }

    @Test
    public void dataExactlyAtTheLimitIsCopiedWhole() throws IOException {
        byte[] in = data(70_000);
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        assertEquals(70_000, LimitedCopy.copy(new ByteArrayInputStream(in), out, 70_000));
        assertArrayEquals(in, out.toByteArray());
    }

    @Test
    public void dataOneByteOverTheLimitFails() {
        try {
            LimitedCopy.copy(new ByteArrayInputStream(data(70_001)), new ByteArrayOutputStream(), 70_000);
            fail("expected IOException");
        } catch (IOException e) {
            assertEquals("too large", e.getMessage());
        }
    }
}
