package oracle;

import core.auth.UserAccountInfo;
import core.game.container.Container;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.info.Rights;
import core.game.node.item.Item;
import core.game.system.command.Command;
import core.game.system.command.CommandMapping;
import core.game.system.command.CommandPlugin;
import core.game.system.command.CommandSystem;
import core.game.system.command.Privilege;
import core.game.system.task.Pulse;
import core.game.world.GameWorld;
import core.game.world.map.Location;
import core.net.packet.IoBuffer;
import core.net.packet.PacketProcessor;
import core.net.packet.in.Decoders530;
import core.net.packet.in.Packet;
import core.net.packet.in.QuickChatPacketHandler;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.*;
import kotlin.Unit;
import kotlin.jvm.functions.Function0;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import org.json.simple.JSONValue;

/**
 * One case per call of run (each in a fresh class loader, see Launcher): the case's world is built, the four command
 * sets register as the class scan registers them, then every operation runs against the unchanged sources and what
 * it did is printed.
 */
public class Support {
    public static class Failure extends RuntimeException {}

    static Map input;
    static Map<String, Object> values, answers;
    static Map<String, Integer> answerCalls = new HashMap<>();
    static List<Object> events;
    static int opCalls, failAt;
    static boolean recording;
    static int clockReads;
    public static Map<String, Player> players = new LinkedHashMap<>();
    public static Map<String, UserAccountInfo> accounts = new LinkedHashMap<>();
    static Map<String, Map<String, Object>> attributes = new HashMap<>();
    public static List<Pulse> pulses = new ArrayList<>();
    public static List<Function1<Object, Unit>> inputs = new ArrayList<>();
    public static List<Function0<Unit>> tasks = new ArrayList<>();
    public static List<Runnable> jobs = new ArrayList<>();

    static int n(Object o) { return ((Number) o).intValue(); }

    /** A record the original cannot fail at (a clock read, the decoder's outcome): not counted for failure injection. */
    public static void note(String name, Object... args) {
        if (!recording) return;
        List<Object> row = new ArrayList<>();
        row.add(name);
        row.addAll(Arrays.asList(args));
        events.add(row);
    }

    public static void emit(String name, Object... args) {
        if (!recording) return;
        List<Object> row = new ArrayList<>();
        row.add(name);
        row.addAll(Arrays.asList(args));
        events.add(row);
        opCalls++;
        if (opCalls == failAt) throw new Failure();
    }

    // Host inputs at redirected call sites (tool/Rewrite.java of the social oracle).
    public static long currentTimeMillis() {
        if (!recording) return 0L;
        note("clock");
        List clock = (List) values.getOrDefault("clock", Collections.emptyList());
        if (clock.isEmpty()) return 0L;
        return ((Number) clock.get(Math.min(clockReads++, clock.size() - 1))).longValue();
    }
    public static long nanoTime() { throw new IllegalStateException("no selected source reads nanoTime"); }
    public static long nextSeed() { throw new IllegalStateException("no selected source constructs an unseeded Random"); }

    // Case values.
    public static boolean flag(String key) { return Boolean.TRUE.equals(values.get(key)); }
    public static Location home() { return location(values.get("home")); }
    public static Location castleWars() { return location(values.get("castleWars")); }
    public static String dataPath() { return (String) values.get("dataPath"); }
    public static GameWorld.Settings settings() {
        Map s = (Map) values.get("settings");
        return s == null ? null : new GameWorld.Settings(Boolean.TRUE.equals(s.get("devMode")), Boolean.TRUE.equals(s.get("beta")));
    }

    public static Object answer(String name, Object fallback) {
        if (!answers.containsKey(name)) return fallback;
        Object v = answers.get(name);
        if (v instanceof List && !name.startsWith("slots:") && !name.startsWith("ips:") && !name.startsWith("file:")) {
            List l = (List) v;
            int i = answerCalls.merge(name, 1, Integer::sum) - 1;
            v = l.get(Math.min(i, l.size() - 1));
        }
        return v instanceof Long ? (Object) ((Long) v).intValue() : v;
    }
    public static boolean answerBool(String name, boolean fallback) { return Boolean.TRUE.equals(answer(name, fallback)); }
    public static int answerInt(String name, int fallback) { return n(answer(name, fallback)); }
    public static double answerDouble(String name) {
        Object v = answer(name, 0.0);
        return v instanceof String ? Double.parseDouble((String) v) : ((Number) v).doubleValue();
    }
    public static byte[] bytes(String name) {
        Object v = answer(name, null);
        if (v == null) return null;
        String hex = (String) v;
        byte[] out = new byte[hex.length() / 2];
        for (int i = 0; i < out.length; i++) out[i] = (byte) Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
        return out;
    }
    public static List<String> names(String name) {
        List<String> out = new ArrayList<>();
        for (Object o : (List) answers.getOrDefault(name, Collections.emptyList())) out.add((String) o);
        return out;
    }

    public static Player player(Object key) { return key == null ? null : players.get((String) key); }
    public static Location location(Object v) {
        if (v == null) return null;
        List a = (List) v;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }
    public static Location location(Entity entity) {
        Map p = (Map) ((Map) input.get("players")).get(entity.key);
        return location(p.get("location"));
    }
    public static List<Object> coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }

    // Attributes: case values in, recorded representation out.
    static Object attributeValue(Object spec) {
        if (spec == null) return null;
        Map m = (Map) spec;
        if (m.containsKey("int")) return n(m.get("int"));
        if (m.containsKey("float")) return Float.parseFloat((String) m.get("float"));
        if (m.containsKey("bool")) return m.get("bool");
        if (m.containsKey("text")) return m.get("text");
        if (m.containsKey("rights")) return Rights.valueOf((String) m.get("rights"));
        throw new IllegalArgumentException(m.toString());
    }
    public static Object value(Object v) {
        if (v == null) return null;
        Map<String, Object> out = new LinkedHashMap<>();
        if (v instanceof Integer) out.put("int", v);
        else if (v instanceof Float) out.put("float", v.toString());
        else if (v instanceof Boolean) out.put("bool", v);
        else if (v instanceof String) out.put("text", v);
        else if (v instanceof Rights) out.put("rights", ((Rights) v).name());
        else throw new IllegalArgumentException(v.getClass().getName());
        return out;
    }
    public static Object getAttribute(Entity entity, String key, Object fail) {
        emit("getAttribute", entity.key, key);
        return storedAttribute(entity, key, fail);
    }
    public static Object storedAttribute(Entity entity, String key, Object fail) {
        Map<String, Object> stored = attributes.get(entity.key);
        return stored != null && stored.containsKey(key) ? stored.get(key) : fail;
    }

    // Containers and charged items.
    public static Item[] slots(Container container, Player owner, String name) {
        Object v = answers.get("slots:" + owner.key + ":" + name);
        if (v == null) return new Item[0];
        List l = (List) v;
        Item[] out = new Item[l.size()];
        for (int i = 0; i < out.length; i++) {
            List s = (List) l.get(i);
            if (s != null) out[i] = Item.inSlot(n(s.get(0)), i, n(s.get(1)), owner.key, name);
        }
        return out;
    }
    public static boolean chargedItem(int id) {
        emit("chargedItem", id);
        return answerBool("chargedItem:" + id, false);
    }
    public static int forCharge(int id, int charge) {
        emit("forCharge", id, charge);
        return answerInt("forCharge:" + id + ":" + charge, id);
    }
    public static Integer chargedCharge(int id) {
        emit("chargedCharge", id);
        Object v = answer("chargedCharge:" + id, null);
        return v == null ? null : n(v);
    }

    // Accounts.
    public static UserAccountInfo account(String name) {
        Object key = answer("info:" + name, null);
        if (key == null) {
            UserAccountInfo fresh = UserAccountInfo.createDefault();
            accounts.put("new:" + name + ":" + accounts.size(), fresh);
            return fresh;
        }
        return accounts.get((String) key);
    }
    public static String accountKey(UserAccountInfo info) {
        for (Map.Entry<String, UserAccountInfo> e : accounts.entrySet()) if (e.getValue() == info) return e.getKey();
        return null;
    }

    // Scheduling.
    public static void submit(Pulse pulse) {
        emit("submit", pulse.getClass().getName(), pulse.getDelay());
        pulses.add(pulse);
    }

    /** A legacy CommandPlugin of the case, linked to ADMINISTRATOR as the boot links the three originals. */
    public static class RecPlugin extends CommandPlugin {
        final String label;
        RecPlugin(String label) { this.label = label; }
        public core.plugin.Plugin<Object> newInstance(Object arg) { return this; }
        public boolean parse(Player player, String name, String[] args) {
            emit("legacy", label, player.key, name, Arrays.asList(args));
            return answerBool("legacy:" + label, false);
        }
    }

    static void setup(Map c) throws Exception {
        input = c;
        values = (Map<String, Object>) c.getOrDefault("values", new HashMap<>());
        answers = (Map<String, Object>) c.getOrDefault("answers", new HashMap<>());
        UserAccountInfo.Companion.getDefault();
        Map<String, Map> accountSpecs = (Map<String, Map>) c.getOrDefault("accounts", new HashMap<>());
        for (Map.Entry<String, Map> e : accountSpecs.entrySet()) {
            UserAccountInfo info = UserAccountInfo.createDefault();
            Map a = e.getValue();
            if (a.containsKey("username")) info.setUsername((String) a.get("username"));
            if (a.containsKey("password")) info.setPassword((String) a.get("password"));
            if (a.containsKey("rights")) info.setRights(n(a.get("rights")));
            if (a.containsKey("credits")) info.setCredits(n(a.get("credits")));
            if (a.containsKey("muteEndTime")) info.setMuteEndTime(((Number) a.get("muteEndTime")).longValue());
            if (a.containsKey("banEndTime")) info.setBanEndTime(((Number) a.get("banEndTime")).longValue());
            accounts.put(e.getKey(), info);
        }
        Map<String, Map> playerSpecs = (Map<String, Map>) c.getOrDefault("players", new HashMap<>());
        for (Map.Entry<String, Map> e : playerSpecs.entrySet()) {
            Map p = e.getValue();
            Player player = new Player(e.getKey(), (String) p.get("name"), (String) p.get("username"), new PlayerDetails(accounts.get((String) p.get("account"))));
            players.put(e.getKey(), player);
            Map<String, Object> stored = new HashMap<>();
            for (Map.Entry<String, Object> a : ((Map<String, Object>) p.getOrDefault("attributes", new HashMap<>())).entrySet()) stored.put(a.getKey(), attributeValue(a.getValue()));
            attributes.put(e.getKey(), stored);
        }
        // The class scan's order: ModerationCommandSet, PlayerAttributeCommandSet, SystemCommandSet, TeleportCommandSet.
        for (String name : new String[] {"ModerationCommandSet", "PlayerAttributeCommandSet", "SystemCommandSet", "TeleportCommandSet"}) {
            Class<?> set = Class.forName("core.game.system.command.sets." + name);
            try {
                ((core.plugin.Plugin<Object>) set.getDeclaredConstructor().newInstance()).newInstance(null);
            } catch (Throwable t) {
                throw new IllegalStateException(t);
            }
        }
        for (Object e : (List) c.getOrDefault("extra", Collections.emptyList())) {
            List row = (List) e;
            String label = row.get(2) + "$defineCommands$" + row.get(3);
            CommandMapping.INSTANCE.register(new Command((String) row.get(0), Privilege.valueOf((String) row.get(1)), "", "",
                (Function2<Player, String[], Unit>) (player, args) -> {
                    emit("content", row.get(2), n(row.get(3)), player.key, Arrays.asList(args));
                    if (answerBool("contentReject:" + label, false)) throw new IllegalStateException();
                    return Unit.INSTANCE;
                }));
        }
        for (String legacy : new String[] {"core.game.system.command.MapDumpCommand", "core.game.system.command.oldsys.SimpleDumpingCommands",
                "core.game.system.command.oldsys.VisualCommand"}) {
            new RecPlugin(legacy).link(core.game.system.command.CommandSet.ADMINISTRATOR);
        }
        Path data = Paths.get("data");
        if (Files.isDirectory(data)) {
            try (java.util.stream.Stream<Path> files = Files.list(data)) {
                for (Path f : (Iterable<Path>) files::iterator) Files.delete(f);
            }
        }
        Map<String, String> fileSpecs = (Map<String, String>) c.getOrDefault("files", new HashMap<>());
        if (!fileSpecs.isEmpty()) Files.createDirectories(data);
        for (Map.Entry<String, String> f : fileSpecs.entrySet()) Files.write(data.resolve(f.getKey()), f.getValue().getBytes(StandardCharsets.UTF_8));
    }

    static IoBuffer buffer(int opcode, String hex) {
        byte[] data = new byte[hex.length() / 2];
        for (int i = 0; i < data.length; i++) data[i] = (byte) Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
        return new IoBuffer(opcode, null, ByteBuffer.wrap(data));
    }

    static Object packet(Packet packet) {
        List<Object> out = new ArrayList<>();
        out.add(packet.getClass().getSimpleName());
        if (packet instanceof Packet.Command) out.add(((Packet.Command) packet).getCommandLine());
        else if (packet instanceof Packet.ReportAbuse) {
            Packet.ReportAbuse r = (Packet.ReportAbuse) packet;
            out.addAll(Arrays.asList(r.getTarget(), r.getRule(), r.getModMute()));
        } else if (packet instanceof Packet.QuickChat) {
            Packet.QuickChat q = (Packet.QuickChat) packet;
            out.addAll(Arrays.asList(q.getIndexA(), q.getIndexB(), q.getForClan(), q.getMultiplier(), q.getOffset(), q.getType().name()));
        } else if (packet instanceof Packet.DecodingError) {
            out.add(((Packet.DecodingError) packet).getMessage().split("\\R", 2)[0].trim());
        }
        return out;
    }

    static Object op(Map op) {
        String kind = (String) op.get("op");
        Player player = player(op.get("player"));
        switch (kind) {
            case "command": {
                PacketProcessor.process(new Packet.Command(player, (String) op.get("line")));
                return null;
            }
            case "parse":
                return CommandSystem.Companion.getCommandSystem().parse(player, (String) op.get("line"));
            case "packet": {
                Packet packet = Decoders530.process(player, n(op.get("opcode")), buffer(n(op.get("opcode")), (String) op.get("hex")));
                note("decoded", packet(packet));
                if (packet instanceof Packet.Command || packet instanceof Packet.ReportAbuse || packet instanceof Packet.QuickChat) {
                    PacketProcessor.process(packet);
                }
                return null;
            }
            case "qcHandler":
                new QuickChatPacketHandler().decode(player, 167, buffer(167, (String) op.get("hex")));
                return null;
            case "pulse":
                return pulses.get(n(op.get("index"))).pulse();
            case "task":
                tasks.get(n(op.get("index"))).invoke();
                return null;
            case "input": {
                Object value = op.get("value");
                inputs.get(n(op.get("index"))).invoke(value instanceof Long ? (Object) ((Long) value).intValue() : value);
                return null;
            }
            case "job":
                jobs.get(n(op.get("index"))).run();
                return null;
            case "names": {
                List<Object> out = new ArrayList<>();
                for (String name : CommandMapping.INSTANCE.getNames()) out.add(Arrays.asList(name, CommandMapping.INSTANCE.get(name).getPrivilege().name()));
                out.add(Arrays.asList(core.game.system.command.CommandSet.values()[2].getPlugins().size()));
                return out;
            }
            case "pages": {
                List<Object> out = new ArrayList<>();
                for (int index : CommandMapping.INSTANCE.getPageIndices(n(op.get("rights")))) out.add(index);
                return out;
            }
            default:
                throw new IllegalArgumentException(kind);
        }
    }

    static Object error(Throwable t) {
        if (t instanceof Failure) return Arrays.asList("Binding", null);
        if (t instanceof IllegalStateException) return Arrays.asList("IllegalState", null);
        if (t instanceof NullPointerException) return Arrays.asList("Null", null);
        if (t instanceof NumberFormatException) return Arrays.asList("NumberFormat", t.getMessage());
        if (t instanceof IndexOutOfBoundsException) return Arrays.asList("Index", t.getMessage());
        if (t instanceof ClassCastException) return Arrays.asList("Cast", null);
        if (t instanceof java.nio.BufferUnderflowException) return Arrays.asList("BufferUnderflow", null);
        return Arrays.asList(t.getClass().getName(), t.getMessage());
    }

    static Object state() {
        Map<String, Object> out = new LinkedHashMap<>();
        for (Map.Entry<String, UserAccountInfo> e : accounts.entrySet()) {
            UserAccountInfo a = e.getValue();
            out.put(e.getKey(), Arrays.asList(a.getRights(), a.getCredits(), a.getBanEndTime(), a.getMuteEndTime(), a.getPassword()));
        }
        Map<String, Object> s = new LinkedHashMap<>();
        s.put("accounts", out);
        s.put("pulses", pulses.size());
        s.put("tasks", tasks.size());
        s.put("inputs", inputs.size());
        s.put("jobs", jobs.size());
        return s;
    }

    public static String run(String line) throws Exception {
        Map c = (Map) JSONValue.parseWithException(line);
        recording = false;
        setup(c);
        recording = true;
        List<Object> results = new ArrayList<>();
        for (Object o : (List) c.get("ops")) {
            Map op = (Map) o;
            events = new ArrayList<>();
            opCalls = 0;
            failAt = op.containsKey("failAt") ? n(op.get("failAt")) : -1;
            Map<String, Object> step = new LinkedHashMap<>();
            Object result = null;
            Object error = null;
            try {
                result = op(op);
            } catch (Throwable t) {
                Throwable cause = t;
                while (cause instanceof java.lang.reflect.InvocationTargetException && cause.getCause() != null) cause = cause.getCause();
                error = error(cause);
            }
            step.put("events", events);
            step.put("error", error);
            step.put("result", result);
            step.put("state", state());
            results.add(step);
        }
        Map<String, Object> out = new LinkedHashMap<>(c);
        out.put("expected", results);
        return JSONValue.toJSONString(out);
    }
}
