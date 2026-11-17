package oracle.worldtrace;

import core.game.world.GameWorld;
import core.net.packet.PacketWriteQueue;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedList;
import java.util.List;
import java.util.Map;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/**
 * Runs a world-trace scenario exactly as WorldTrace.main does (same statements,
 * same records) and additionally records, for every entry a session was sent,
 * the call stack that queued it.
 *
 * Only one thing differs from WorldTrace: after the boot record, the two lists
 * of the original's PacketWriteQueue (packetsToWrite, packetsToFlush) are
 * replaced by lists of the same class that, on add, remember the stack of the
 * caller, and on pollFirst attribute the session entries written since the
 * previous poll to the packet polled before. The original's own code, the
 * order of its calls and its data are unchanged; the generator accepts a run
 * only if every record it writes is byte-identical to the world-trace
 * fixture's.
 *
 * Entries written directly (not through the queue: the login reply, the
 * handshake, UpdateSceneGraph and the other packets PacketWriteQueue.handle
 * sends at once) are attributed "direct"; entries written while the harness
 * pumps hand-offs inside WorldTrace.record are attributed "pump".
 *
 * Usage (working directory = the scenario's world directory):
 * LoginOrigins SCENARIO_JSON TRACE_OUT ORIGINS_OUT
 */
public final class LoginOrigins {
    static final List<String> stacks = new ArrayList<>();
    static final Map<String, Integer> stackIds = new HashMap<>();
    static final Map<String, List<Object>> attributed = new LinkedHashMap<>();
    static final Map<String, Integer> marks = new HashMap<>();
    static final String DIRECT = "direct";
    static Object previous = DIRECT;
    static int captured;

    /** The caller's frames above the queue, up to the harness: "class.method:line", newest first. */
    static Integer capture() {
        StackTraceElement[] frames = new Throwable().getStackTrace();
        StringBuilder key = new StringBuilder();
        int kept = 0;
        for (StackTraceElement frame : frames) {
            String owner = frame.getClassName();
            if (owner.startsWith("oracle.worldtrace.LoginOrigins")) continue;
            if (kept == 0 && (owner.startsWith("core.net.packet.PacketWriteQueue") || owner.equals("core.net.packet.PacketRepository"))) continue;
            if (owner.startsWith("oracle.worldtrace.")) break;
            if (kept > 0) key.append('\n');
            key.append(owner).append('.').append(frame.getMethodName()).append(':').append(frame.getLineNumber());
            kept++;
        }
        String text = key.toString();
        Integer id = stackIds.get(text);
        if (id == null) {
            id = stacks.size();
            stacks.add(text);
            stackIds.put(text, id);
        }
        captured++;
        return id;
    }

    /** Attributes every session entry written since the last call to the packet that was being written. */
    static void close() {
        for (TraceSession session : WorldTrace.sessions.values()) {
            int mark = marks.getOrDefault(session.id, 0);
            List<Object> list = attributed.computeIfAbsent(session.id, k -> new ArrayList<>());
            while (mark < session.out.size()) {
                list.add(previous == null ? DIRECT : previous);
                mark++;
            }
            marks.put(session.id, mark);
        }
    }

    /** A LinkedList of the original's queued packets that remembers who queued each one. */
    static final class OriginList extends LinkedList<Object> {
        private final LinkedList<Integer> origins = new LinkedList<>();

        @Override public boolean add(Object packet) {
            origins.addLast(capture());
            return super.add(packet);
        }

        @Override public Object pollFirst() {
            close();
            Object packet = super.pollFirst();
            previous = packet == null ? DIRECT : origins.pollFirst();
            return packet;
        }

        @Override public boolean addAll(java.util.Collection<?> c) { throw new UnsupportedOperationException("addAll"); }
        @Override public void addFirst(Object e) { throw new UnsupportedOperationException("addFirst"); }
        @Override public void addLast(Object e) { throw new UnsupportedOperationException("addLast"); }
        @Override public Object poll() { throw new UnsupportedOperationException("poll"); }
        @Override public Object removeFirst() { throw new UnsupportedOperationException("removeFirst"); }
        @Override public Object remove() { throw new UnsupportedOperationException("remove"); }
    }

    static void installQueues() throws ReflectiveOperationException {
        for (String name : new String[] { "packetsToWrite", "packetsToFlush" }) {
            Field field = PacketWriteQueue.class.getDeclaredField(name);
            field.setAccessible(true);
            if (!((LinkedList<?>) field.get(null)).isEmpty()) throw new IllegalStateException(name + " is not empty after start-up");
            Pin.install(field, new OriginList());
        }
    }

    /** Calls WorldTrace.record and returns, per session, the origin of each entry it recorded. */
    static Map<String, Object> record(String type, Map<String, Object> extra) throws Exception {
        close();
        previous = DIRECT;
        WorldTrace.record(type, extra);
        JSONObject written = (JSONObject) new JSONParser().parse(WorldTrace.lines.get(WorldTrace.lines.size() - 1));
        JSONObject out = (JSONObject) written.get("out");
        Map<String, Object> origins = new LinkedHashMap<>();
        for (Object key : out.keySet()) {
            String id = (String) key;
            int count = ((JSONArray) out.get(key)).size();
            List<Object> list = new ArrayList<>(attributed.getOrDefault(id, new ArrayList<>()));
            if (list.size() > count) throw new IllegalStateException("more attributed entries than recorded for " + id);
            // Written while WorldTrace.record pumped hand-offs, after the last close.
            while (list.size() < count) list.add("pump");
            origins.put(id, list);
        }
        for (Map.Entry<String, List<Object>> entry : attributed.entrySet()) {
            if (!entry.getValue().isEmpty() && !out.containsKey(entry.getKey())) throw new IllegalStateException("attributed entries not recorded for " + entry.getKey());
        }
        attributed.clear();
        marks.clear();
        Map<String, Object> line = new LinkedHashMap<>();
        line.put("i", written.get("i"));
        line.put("origins", origins);
        return line;
    }

    public static void main(String[] args) throws Throwable {
        // As WorldTrace.main, statement for statement, with the queue lists replaced after the boot record.
        WorldTrace.world = Paths.get("").toAbsolutePath();
        JSONObject scenario = (JSONObject) new JSONParser().parse(new String(Files.readAllBytes(Paths.get(args[0])), StandardCharsets.UTF_8));
        System.setErr(new PrintStream(new WorldTrace.CountingErr(System.err), true));
        System.setSecurityManager(new Guard(WorldTrace.world.getParent().toFile()));
        Pin.installGenerators((Long) scenario.get("clockStart"), (Long) scenario.get("threadLocalSeed"));
        Handoffs.install();
        WorldTrace.files = WorldTrace.snapshot();

        Map<String, Object> header = new LinkedHashMap<>();
        header.put("t", "scenario");
        header.put("name", scenario.get("name"));
        header.put("clockStart", scenario.get("clockStart"));
        header.put("threadLocalSeed", scenario.get("threadLocalSeed"));
        header.put("tickMillis", Pin.TICK_MILLIS);
        WorldTrace.lines.add(Json.write(header));

        List<String> originLines = new ArrayList<>();
        WorldTrace.record("boot", WorldTrace.boot());
        installQueues();
        for (Object item : (JSONArray) scenario.get("steps")) {
            JSONObject step = (JSONObject) item;
            String op = (String) step.get("op");
            Map<String, Object> extra = new LinkedHashMap<>();
            extra.put("op", op);
            if (op.equals("tick")) {
                Pin.now += Pin.TICK_MILLIS;
                GameWorld.getMajorUpdateWorker().handleTickActions(false);
                originLines.add(Json.write(record("tick", extra)));
                continue;
            }
            String id = (String) step.get("session");
            extra.put("session", id);
            if (op.equals("connect")) {
                if (WorldTrace.sessions.put(id, new TraceSession(id)) != null) throw new IllegalArgumentException("session reused: " + id);
            } else if (op.equals("recv")) {
                TraceSession session = WorldTrace.sessions.get(id);
                extra.put("hex", step.get("hex"));
                session.getProducer().produceReader(session, ByteBuffer.wrap(Json.unhex((String) step.get("hex")))).run();
            } else if (op.equals("close")) {
                WorldTrace.sessions.get(id).disconnect();
            } else {
                throw new IllegalArgumentException(op);
            }
            originLines.add(Json.write(record("io", extra)));
        }
        Map<String, Object> end = new LinkedHashMap<>();
        originLines.add(Json.write(record("end", end)));
        Files.write(Paths.get(args[1]), (String.join("\n", WorldTrace.lines) + "\n").getBytes(StandardCharsets.UTF_8));
        Map<String, Object> table = new LinkedHashMap<>();
        table.put("captured", captured);
        List<Object> list = new ArrayList<>();
        for (String stack : stacks) {
            List<Object> frames = new ArrayList<>();
            for (String frame : stack.split("\n")) frames.add(frame);
            list.add(frames);
        }
        table.put("stacks", list);
        originLines.add(Json.write(table));
        Files.write(Paths.get(args[2]), (String.join("\n", originLines) + "\n").getBytes(StandardCharsets.UTF_8));
        System.out.flush();
        System.err.flush();
        Runtime.getRuntime().halt(0);
    }
}
