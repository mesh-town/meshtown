package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import org.junit.Test;

public class InviteScanTest {
    private static final String INVITE = "mesh-invite2-abcdef0123456789";

    @Test
    public void urlIsTheOnePageLinksTo() {
        assertEquals("meshtown-app://scan-invite", InviteScan.URL);
    }

    // outcome: what happened to the photo

    @Test
    public void notTakenIsCancelled() {
        assertEquals(InviteScan.Outcome.CANCELLED, InviteScan.outcome(false, null));
        assertEquals(InviteScan.Outcome.CANCELLED, InviteScan.outcome(false, INVITE));
    }

    @Test
    public void takenWithoutCodeIsNothingFound() {
        assertEquals(InviteScan.Outcome.NOTHING_FOUND, InviteScan.outcome(true, null));
    }

    @Test
    public void takenWithOtherTextIsNotAnInvite() {
        assertEquals(InviteScan.Outcome.NOT_INVITE, InviteScan.outcome(true, "https://example.com/menu"));
    }

    @Test
    public void takenWithInviteIsInvite() {
        assertEquals(InviteScan.Outcome.INVITE, InviteScan.outcome(true, INVITE));
    }

    // inviteOf: the text handed to the page

    @Test
    public void inviteIsTrimmed() {
        assertEquals(INVITE, InviteScan.inviteOf("  " + INVITE + " \n"));
    }

    @Test
    public void upperCaseInviteIsLowered() {
        assertEquals(INVITE, InviteScan.inviteOf(INVITE.toUpperCase()));
        assertEquals("mesh-invite2-abc", InviteScan.inviteOf("Mesh-Invite2-ABC"));
    }

    @Test
    public void otherTextIsNotAnInvite() {
        assertNull(InviteScan.inviteOf("https://example.com/menu"));
        assertNull(InviteScan.inviteOf("not mesh-invite2-abc"));
        assertNull(InviteScan.inviteOf(""));
        assertNull(InviteScan.inviteOf("   "));
    }

    @Test
    public void nullIsNotAnInvite() {
        assertNull(InviteScan.inviteOf(null));
    }

    // message: the toast, or null to stay silent

    @Test
    public void unavailableSaysSo() {
        assertEquals("Scanner unavailable on this device", InviteScan.message(InviteScan.Outcome.UNAVAILABLE));
    }

    @Test
    public void notInviteSaysSo() {
        assertEquals("This QR code is not an invite", InviteScan.message(InviteScan.Outcome.NOT_INVITE));
    }

    @Test
    public void nothingFoundSaysSo() {
        assertEquals("No QR code found in the photo", InviteScan.message(InviteScan.Outcome.NOTHING_FOUND));
    }

    @Test
    public void cancelledAndInviteSayNothing() {
        assertNull(InviteScan.message(InviteScan.Outcome.CANCELLED));
        assertNull(InviteScan.message(InviteScan.Outcome.INVITE));
    }
}
