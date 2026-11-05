package app.meshtown;

import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;

/** Log filter for the node process, read from a `rust-log` file. */
final class RustLog {
    private RustLog() {
    }

    /** The private dir first, then the external files dir (may be null); null when neither has one. */
    static String read(File dir, File external) {
        String value = firstLine(new File(dir, "rust-log"));
        if (value != null || external == null) {
            return value;
        }
        return firstLine(new File(external, "rust-log"));
    }

    private static String firstLine(File f) {
        if (!f.isFile()) {
            return null;
        }
        try (BufferedReader r = new BufferedReader(new FileReader(f))) {
            String line = r.readLine();
            return line == null || line.trim().isEmpty() ? null : line.trim();
        } catch (Exception e) {
            return null;
        }
    }
}
