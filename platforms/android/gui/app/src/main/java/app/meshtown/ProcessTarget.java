package app.meshtown;

import android.os.Build;
import java.util.concurrent.TimeUnit;

/** Adapts a {@link Process} to {@link NodeStopper.Target} on every supported API level. */
final class ProcessTarget implements NodeStopper.Target {
    private final Process process;
    private final int sdk;

    ProcessTarget(Process process) {
        this(process, Build.VERSION.SDK_INT);
    }

    ProcessTarget(Process process, int sdk) {
        this.process = process;
        this.sdk = sdk;
    }

    @Override
    public void terminate() {
        process.destroy();
    }

    @Override
    public boolean awaitExit(long millis) throws InterruptedException {
        if (sdk >= 26) {
            return process.waitFor(millis, TimeUnit.MILLISECONDS);
        }
        long end = System.currentTimeMillis() + millis;
        while (System.currentTimeMillis() < end) {
            if (exited()) {
                return true;
            }
            Thread.sleep(50);
        }
        return exited();
    }

    private boolean exited() {
        try {
            process.exitValue();
            return true;
        } catch (IllegalThreadStateException e) {
            return false;
        }
    }

    @Override
    public void kill() {
        int pid = pidOf();
        if (pid > 0) {
            android.os.Process.killProcess(pid);
        }
        process.destroyForcibly();
    }

    private int pidOf() {
        try {
            java.lang.reflect.Field f = process.getClass().getDeclaredField("pid");
            f.setAccessible(true);
            return f.getInt(process);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return -1;
        }
    }
}
