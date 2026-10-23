package app.meshtown;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import java.io.InputStream;
import java.io.OutputStream;
import java.util.concurrent.TimeUnit;
import org.junit.Test;

public class ProcessTargetTest {
    /** A process whose exit state and call counts the test controls. */
    private static class FakeProcess extends Process {
        int destroyCalls;
        int forcibleCalls;
        int waitForCalls;
        int exitValueCalls;
        int runningAnswers;
        boolean exited;
        boolean waitForResult;

        @Override
        public OutputStream getOutputStream() {
            return null;
        }

        @Override
        public InputStream getInputStream() {
            return null;
        }

        @Override
        public InputStream getErrorStream() {
            return null;
        }

        @Override
        public int waitFor() {
            return 0;
        }

        @Override
        public boolean waitFor(long timeout, TimeUnit unit) {
            waitForCalls++;
            return waitForResult;
        }

        @Override
        public int exitValue() {
            exitValueCalls++;
            if (!exited || exitValueCalls <= runningAnswers) {
                throw new IllegalThreadStateException();
            }
            return 0;
        }

        @Override
        public void destroy() {
            destroyCalls++;
        }

        @Override
        public Process destroyForcibly() {
            forcibleCalls++;
            return this;
        }
    }

    /** Carries a non-positive pid field, as a process object that has not started would. */
    private static class ZeroPidProcess extends FakeProcess {
        @SuppressWarnings("unused")
        private int pid = 0;
    }

    @Test
    public void terminateDestroysProcess() {
        FakeProcess p = new FakeProcess();
        new ProcessTarget(p, 26).terminate();
        assertEquals(1, p.destroyCalls);
    }

    @Test
    public void awaitExitOnNewApiDelegatesToWaitFor() throws Exception {
        FakeProcess p = new FakeProcess();
        p.waitForResult = true;
        assertTrue(new ProcessTarget(p, 26).awaitExit(100));
        p.waitForResult = false;
        assertFalse(new ProcessTarget(p, 26).awaitExit(100));
        assertEquals(2, p.waitForCalls);
        assertEquals(0, p.exitValueCalls);
    }

    @Test
    public void awaitExitOnOldApiSeesExitedProcess() throws Exception {
        FakeProcess p = new FakeProcess();
        p.exited = true;
        assertTrue(new ProcessTarget(p, 24).awaitExit(1000));
        assertEquals(0, p.waitForCalls);
    }

    @Test
    public void awaitExitOnOldApiTimesOutOnRunningProcess() throws Exception {
        FakeProcess p = new FakeProcess();
        long start = System.currentTimeMillis();
        assertFalse(new ProcessTarget(p, 24).awaitExit(120));
        long took = System.currentTimeMillis() - start;
        assertTrue("returned too early: " + took, took >= 100);
        assertTrue("returned too late: " + took, took <= 2000);
    }

    @Test
    public void awaitExitOnOldApiNoticesLateExit() throws Exception {
        FakeProcess p = new FakeProcess();
        p.exited = true;
        p.runningAnswers = 2;
        assertTrue(new ProcessTarget(p, 24).awaitExit(2000));
        assertEquals(3, p.exitValueCalls);
    }

    @Test
    public void killForciblyDestroysWithoutPidField() {
        FakeProcess p = new FakeProcess();
        new ProcessTarget(p, 26).kill();
        assertEquals(1, p.forcibleCalls);
    }

    @Test
    public void killIgnoresNonPositivePid() {
        ZeroPidProcess p = new ZeroPidProcess();
        new ProcessTarget(p, 26).kill();
        assertEquals(1, p.forcibleCalls);
    }
}
