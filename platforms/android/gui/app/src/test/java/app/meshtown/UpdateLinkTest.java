package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class UpdateLinkTest {
    private static final String BASE = "https://github.com/mesh-town/meshtown-dist/raw/main/android/meshtown.apk";

    @Test
    public void exactLinkMatches() {
        assertTrue(UpdateLink.matches(BASE));
    }

    @Test
    public void queryIsIgnored() {
        assertTrue(UpdateLink.matches(BASE + "?x=1"));
    }

    @Test
    public void plainHttpIsRejected() {
        assertFalse(UpdateLink.matches(BASE.replace("https", "http")));
    }

    @Test
    public void foreignHostIsRejected() {
        assertFalse(UpdateLink.matches(BASE.replace("github.com", "github.com.evil.example")));
    }

    @Test
    public void foreignPathIsRejected() {
        assertFalse(UpdateLink.matches("https://github.com/mesh-town/other/raw/main/android/meshtown.apk"));
    }

    @Test
    public void trailingSlashAndVersionedNameAreRejected() {
        assertFalse(UpdateLink.matches(BASE + "/"));
        assertFalse(UpdateLink.matches(BASE.replace("meshtown.apk", "meshtown-0.3.33.apk")));
    }

    @Test
    public void garbageIsRejected() {
        assertFalse(UpdateLink.matches(null));
        assertFalse(UpdateLink.matches("not a url"));
    }

    @Test
    public void sourcePrefersTheTestAddress() {
        assertEquals("http://127.0.0.1:1/a.apk", UpdateLink.source(BASE, "http://127.0.0.1:1/a.apk"));
    }

    @Test
    public void sourceFallsBackToTheNavigatedAddress() {
        assertEquals(BASE, UpdateLink.source(BASE, ""));
        assertEquals(BASE, UpdateLink.source(BASE, null));
    }
}
