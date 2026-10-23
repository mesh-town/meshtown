package app.meshtown;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.os.Build;
import java.util.Arrays;
import java.util.HashSet;

/** Reads version and signer of a downloaded package and compares them with the installed app. */
final class PackageProbe {
    private PackageProbe() {}

    /** Returns null when the package may be installed, otherwise the text to show the user. */
    static String refusal(Context ctx, String path) {
        PackageManager pm = ctx.getPackageManager();
        PackageInfo offered = archive(pm, path);
        if (offered == null) {
            return UpdateCheck.refusal(0, 0, false, false);
        }
        PackageInfo installed = installed(pm, ctx.getPackageName());
        if (installed == null) {
            return UpdateCheck.DOWNLOAD_FAILED;
        }
        return UpdateCheck.refusal(code(installed), code(offered), sameSigner(installed, offered), true);
    }

    private static int flags() {
        return Build.VERSION.SDK_INT >= 28 ? PackageManager.GET_SIGNING_CERTIFICATES : 0;
    }

    private static PackageInfo archive(PackageManager pm, String path) {
        try {
            return pm.getPackageArchiveInfo(path, flags());
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static PackageInfo installed(PackageManager pm, String name) {
        try {
            return pm.getPackageInfo(name, flags());
        } catch (PackageManager.NameNotFoundException | RuntimeException e) {
            return null;
        }
    }

    @SuppressWarnings("deprecation")
    private static long code(PackageInfo info) {
        return Build.VERSION.SDK_INT >= 28 ? info.getLongVersionCode() : info.versionCode;
    }

    private static boolean sameSigner(PackageInfo a, PackageInfo b) {
        if (Build.VERSION.SDK_INT < 28) {
            return true;
        }
        Signature[] x = signers(a);
        Signature[] y = signers(b);
        return x != null && y != null && new HashSet<>(Arrays.asList(x)).equals(new HashSet<>(Arrays.asList(y)));
    }

    private static Signature[] signers(PackageInfo info) {
        return info.signingInfo == null ? null : info.signingInfo.getApkContentsSigners();
    }
}
