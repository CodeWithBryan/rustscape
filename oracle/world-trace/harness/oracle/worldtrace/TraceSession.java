package oracle.worldtrace;

import core.auth.AuthResponse;
import core.net.IoSession;
import core.net.packet.IoBuffer;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.AbstractExecutorService;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.TimeUnit;

/**
 * A connection without a socket. Everything above the socket is the original
 * IoSession: its event producers, write events and disconnect run unchanged.
 * Only the three members that touch the channel are replaced, and the
 * executor the original would hand events to runs them on the calling thread.
 */
public final class TraceSession extends IoSession {
    /** The reactor's pool, run in place: an event is complete before the caller continues. */
    static final ExecutorService INLINE = new AbstractExecutorService() {
        @Override public void execute(Runnable command) { command.run(); }
        @Override public void shutdown() { }
        @Override public List<Runnable> shutdownNow() { return new ArrayList<>(); }
        @Override public boolean isShutdown() { return false; }
        @Override public boolean isTerminated() { return false; }
        @Override public boolean awaitTermination(long timeout, TimeUnit unit) { return false; }
    };

    public final String id;
    /** Bytes handed to the socket since the last record was written, in order. */
    public final List<Map<String, Object>> out = new ArrayList<>();
    private Object writing;

    public TraceSession(String id) {
        super(null, INLINE, "127.0.0.1");
        this.id = id;
    }

    @Override public void write(Object context, boolean instant) {
        Object outer = writing;
        writing = context;
        try {
            super.write(context, instant);
        } finally {
            writing = outer;
        }
    }

    /** The original appends to its socket queue and flushes; here the bytes are recorded instead. */
    @Override public void queue(ByteBuffer buffer) {
        byte[] bytes = new byte[buffer.remaining()];
        buffer.duplicate().get(bytes);
        Map<String, Object> entry = new LinkedHashMap<>();
        if (writing instanceof IoBuffer && ((IoBuffer) writing).opcode() != -1) {
            IoBuffer packet = (IoBuffer) writing;
            entry.put("k", "packet");
            entry.put("op", packet.opcode());
            entry.put("size", String.valueOf(packet.getHeader()));
        } else if (writing instanceof AuthResponse) {
            entry.put("k", "login");
            entry.put("response", ((AuthResponse) writing).name());
        } else if (writing instanceof Boolean) {
            entry.put("k", "handshake");
        } else {
            entry.put("k", "raw");
        }
        entry.put("hex", Json.hex(bytes));
        // The original's write() finds the connection closed and sends nothing; the bytes are kept, marked.
        if (!isActive()) entry.put("dropped", true);
        out.add(entry);
    }

    @Override public void write() { }

    /** As the original's own test session answers. */
    @Override public String getRemoteAddress() { return "127.0.0.1"; }
}
