package com.github.shadowsocks.bg;

/** Entry points of libtun2proxy.so; the package and class name are fixed by its JNI symbols. */
public final class Tun2proxy {
    static {
        System.loadLibrary("tun2proxy");
    }

    private Tun2proxy() {}

    public static native int run(String cliArgs, char tunMtu);

    public static native int stop();
}
