package app.meshtown;

import java.net.URI;

/** Recognises the published Android package link and picks the address to download from. */
final class UpdateLink {
    private static final String PATH = "/mesh-town/meshtown-dist/raw/main/android/meshtown.apk";

    private UpdateLink() {}

    /** True for exactly the https package link; query and fragment are ignored. */
    static boolean matches(String url) {
        URI uri = parse(url);
        return uri != null && "https".equals(uri.getScheme())
                && "github.com".equals(uri.getHost()) && PATH.equals(uri.getPath());
    }

    /** The test address when one is set, otherwise the address that was navigated to. */
    static String source(String navUrl, String testUrl) {
        if (testUrl != null && !testUrl.isEmpty()) {
            return testUrl;
        }
        return navUrl;
    }

    private static URI parse(String url) {
        try {
            return url == null ? null : new URI(url);
        } catch (java.net.URISyntaxException e) {
            return null;
        }
    }
}
