package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import org.junit.Test;

public class PhoneNameTest {
    @Test
    public void deviceNameWinsOverModel() {
        assertEquals("My Phone", PhoneName.choose("My Phone", "Acme", "X1"));
    }

    @Test
    public void fallsBackToManufacturerAndModel() {
        assertEquals("Acme X1", PhoneName.choose(null, "Acme", "X1"));
        assertEquals("Acme X1", PhoneName.choose("  ", "Acme", "X1"));
    }

    @Test
    public void joinDoesNotRepeatManufacturer() {
        assertEquals("Acme X1", PhoneName.join("Acme", "Acme X1"));
        assertEquals("ACME X1", PhoneName.join("acme", "ACME X1"));
        assertEquals("Acme X1", PhoneName.join(" Acme ", " X1 "));
    }

    @Test
    public void joinHandlesMissingParts() {
        assertEquals("X1", PhoneName.join(null, "X1"));
        assertEquals("X1", PhoneName.join("", "X1"));
        assertEquals("Acme", PhoneName.choose(null, "Acme", null));
        assertEquals("", PhoneName.join(null, null));
    }

    @Test
    public void fitTrims() {
        assertEquals("abc", PhoneName.fit("  abc \n"));
    }

    @Test
    public void fitCutsToLimitByCodePoints() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 70; i++) {
            sb.appendCodePoint(0x1F600);
        }
        String fitted = PhoneName.fit(sb.toString());
        assertEquals(PhoneName.MAX_CHARS, fitted.codePointCount(0, fitted.length()));
        assertEquals("a".repeat(64), PhoneName.fit("a".repeat(100)));
    }

    @Test
    public void fitDropsControlCharacters() {
        assertEquals("ab", PhoneName.fit("a\u0000\tb"));
        assertEquals("ab", PhoneName.fit("a b "));
        assertEquals("a b", PhoneName.fit("a\n b"));
    }

    @Test
    public void fitReturnsNullWhenNothingLeft() {
        assertNull(PhoneName.fit(null));
        assertNull(PhoneName.fit(""));
        assertNull(PhoneName.fit(" \t\n"));
    }

    @Test
    public void chooseReturnsNullWhenNothingFits() {
        assertNull(PhoneName.choose(null, null, null));
        assertNull(PhoneName.choose("\n", " ", ""));
    }
}
