package app.meshtown;

import android.content.Context;

/** Persistent flag: was the VPN on when the device went down. */
final class Prefs {
    private static final String FILE = "app";
    private static final String VPN_WANTED = "vpn_wanted";
    private static final String NAME_DECIDED = "name_decided";
    private static final String NAME_PENDING = "name_pending";
    private static final String BATTERY_ASKED = "battery_asked";

    private Prefs() {}

    static boolean vpnWanted(Context ctx) {
        return ctx.getSharedPreferences(FILE, Context.MODE_PRIVATE).getBoolean(VPN_WANTED, false);
    }

    static void setVpnWanted(Context ctx, boolean wanted) {
        ctx.getSharedPreferences(FILE, Context.MODE_PRIVATE).edit().putBoolean(VPN_WANTED, wanted).apply();
    }

    /** True once the first start has decided whether this install names its node after the phone. */
    static boolean nameDecided(Context ctx) {
        return ctx.getSharedPreferences(FILE, Context.MODE_PRIVATE).getBoolean(NAME_DECIDED, false);
    }

    static boolean namePending(Context ctx) {
        return ctx.getSharedPreferences(FILE, Context.MODE_PRIVATE).getBoolean(NAME_PENDING, false);
    }

    static void setNamePending(Context ctx, boolean pending) {
        ctx.getSharedPreferences(FILE, Context.MODE_PRIVATE).edit()
                .putBoolean(NAME_DECIDED, true).putBoolean(NAME_PENDING, pending).apply();
    }

    /** True once the battery-optimization exemption has been requested; it is asked only once. */
    static boolean batteryAsked(Context ctx) {
        return ctx.getSharedPreferences(FILE, Context.MODE_PRIVATE).getBoolean(BATTERY_ASKED, false);
    }

    static void setBatteryAsked(Context ctx, boolean asked) {
        ctx.getSharedPreferences(FILE, Context.MODE_PRIVATE).edit().putBoolean(BATTERY_ASKED, asked).apply();
    }
}
