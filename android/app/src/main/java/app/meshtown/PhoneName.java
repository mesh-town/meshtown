package app.meshtown;

import android.content.Context;
import android.os.Build;
import android.provider.Settings;

/** The name a new node takes from the phone, fitted to the node-name rules of the core. */
final class PhoneName {
    /** Longest name the core accepts, in characters. */
    static final int MAX_CHARS = 64;

    private PhoneName() {}

    /** The device name from the system settings, else manufacturer and model; null when nothing fits. */
    static String forThisPhone(Context ctx) {
        String device = null;
        try {
            device = Settings.Global.getString(ctx.getContentResolver(), Settings.Global.DEVICE_NAME);
        } catch (RuntimeException ignored) {
            // some builds restrict the setting
        }
        return choose(device, Build.MANUFACTURER, Build.MODEL);
    }

    static String choose(String device, String manufacturer, String model) {
        String fromDevice = fit(device);
        return fromDevice != null ? fromDevice : fit(join(manufacturer, model));
    }

    static String join(String manufacturer, String model) {
        String m = manufacturer == null ? "" : manufacturer.trim();
        String d = model == null ? "" : model.trim();
        boolean prefixed = d.toLowerCase(java.util.Locale.ROOT).startsWith(m.toLowerCase(java.util.Locale.ROOT));
        return m.isEmpty() || prefixed ? d : m + " " + d;
    }

    /** Trims, drops control and line-separator characters, cuts to the limit; null when empty. */
    static String fit(String raw) {
        if (raw == null) {
            return null;
        }
        StringBuilder sb = new StringBuilder();
        raw.codePoints().filter(PhoneName::allowed).forEach(sb::appendCodePoint);
        String s = sb.toString().trim();
        int end = s.offsetByCodePoints(0, Math.min(MAX_CHARS, s.codePointCount(0, s.length())));
        s = s.substring(0, end).trim();
        return s.isEmpty() ? null : s;
    }

    private static boolean allowed(int c) {
        return !Character.isISOControl(c) && c != 0x2028 && c != 0x2029;
    }
}
