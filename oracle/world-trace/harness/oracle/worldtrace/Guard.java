package oracle.worldtrace;

import java.io.File;
import java.io.IOException;
import java.security.Permission;
import java.util.ArrayList;
import java.util.List;

/**
 * Refuses what a headless, isolated run must never do: open, accept or bind a
 * socket, start a process, or write or delete a file outside its own run
 * directory. A refusal is also counted, and the generator rejects a run with
 * any.
 */
public final class Guard extends SecurityManager {
    public static final List<String> refused = new ArrayList<>();
    private final String root;

    public Guard(File root) throws IOException { this.root = root.getCanonicalPath() + File.separator; }

    private static void refuse(String what) {
        synchronized (refused) { refused.add(what); }
        throw new SecurityException("world-trace oracle refused: " + what);
    }

    private void inside(String action, String file) {
        String path;
        try {
            path = new File(file).getCanonicalPath();
        } catch (IOException e) {
            path = new File(file).getAbsolutePath();
        }
        if (!(path + File.separator).startsWith(root)) refuse(action + " " + path);
    }

    @Override public void checkPermission(Permission permission) { }
    @Override public void checkPermission(Permission permission, Object context) { }
    @Override public void checkConnect(String host, int port) { refuse("connect " + host + ":" + port); }
    @Override public void checkConnect(String host, int port, Object context) { refuse("connect " + host + ":" + port); }
    @Override public void checkListen(int port) { refuse("listen " + port); }
    @Override public void checkAccept(String host, int port) { refuse("accept " + host + ":" + port); }
    @Override public void checkExec(String command) { refuse("exec " + command); }
    @Override public void checkWrite(String file) { inside("write", file); }
    @Override public void checkDelete(String file) { inside("delete", file); }
}
