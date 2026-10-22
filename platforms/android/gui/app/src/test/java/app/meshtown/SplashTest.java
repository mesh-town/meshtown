package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

public class SplashTest {
    @Test
    public void messageIsStartingBeforeTimeout() {
        assertEquals(Splash.STARTING, Splash.message(0));
        assertEquals(Splash.STARTING, Splash.message(Splash.TIMEOUT_MS - 1));
    }

    @Test
    public void messageIsFailedFromTimeout() {
        assertEquals(Splash.FAILED, Splash.message(Splash.TIMEOUT_MS));
        assertEquals(Splash.FAILED, Splash.message(Splash.TIMEOUT_MS * 10));
    }

    @Test
    public void htmlContainsMessage() {
        String html = Splash.html("Hello there");
        assertTrue(html.contains("<p>Hello there</p>"));
        assertTrue(html.startsWith("<!doctype html>"));
    }
}
