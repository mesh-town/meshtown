package app.meshtown;

import android.os.Build;
import java.util.concurrent.TimeUnit;

/** Adapts a {@link Process} to {@link NodeStopper.Target} on every supported API level. */
final class ProcessTarget implements NodeStopper.Target {
    private final Process process;

    ProcessTarget(Process process) {
        this.process = process;
    }

    @Override
    public void terminate() {
        process.destroy();
    }

    @Override
    public boolean awaitExit(long millis) throws InterruptedException {
        if (Build.VERSION.SDK_INT >= 26) {
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
        if (Build.VERSION.SDK_INT >= 26) {
            process.destroyForcibly();
            return;
        }
        try {
            java.lang.reflect.Field f = process.getClass().getDeclaredField("pid");
            f.setAccessible(true);
            android.os.Process.killProcess(f.getInt(process));
        } catch (ReflectiveOperationException | RuntimeException e) {
            process.destroy();
        }
    }
}
