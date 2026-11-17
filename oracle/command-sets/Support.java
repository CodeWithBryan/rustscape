package oracle;

import core.auth.UserAccountInfo;
import core.game.container.Container;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.info.Rights;
import core.game.node.item.Item;
import core.game.system.SystemManager;
import core.game.system.SystemShutdownHook;
import core.game.system.SystemUpdate;
import core.game.system.command.Command;
import core.game.system.command.CommandMapping;
import core.game.system.command.CommandSystem;
import core.game.system.command.Privilege;
import core.game.system.task.Pulse;
import core.game.world.GameWorld;
import core.game.world.map.Location;
import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.util.*;
import kotlin.Unit;
import kotlin.jvm.functions.Function0;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import kotlin.jvm.functions.Function3;
import org.json.simple.JSONValue;

/**
 * One case per call of run (each in a fresh class loader, see oracle/social's Launcher): the case's world is built,
 * the command sets register as the class scan registers them, then every operation runs against the unchanged sources
 * and what it did is printed. Every adapter records through emit (a call that can be made to fail) or note (a host
 * input that cannot) and answers through ask, whose lookup order is: the event with all its arguments, then with
 * fewer trailing arguments, then the event name alone, then the adapter's fallback.
 */
public class Support {
    public static class Failure extends RuntimeException {}

    /** What System.exit throws under the case's security manager. */
    public static class Exit extends SecurityException {
        public final int status;
        Exit(int status) { this.status = status; }
    }

    static Map input;
    static Map<String, Object> values, answers;
    static Map<String, Integer> answerCalls = new HashMap<>();
    static List<Object> events;
    static int opCalls, failAt;
    static boolean recording;
    static int clockReads;
    public static Map<String, Player> players = new LinkedHashMap<>();
    public static Map<String, UserAccountInfo> accounts = new LinkedHashMap<>();
    public static List<Pulse> pulses = new ArrayList<>();
    public static List<Function1<Object, Unit>> inputs = new ArrayList<>();
    public static List<Function0<Unit>> tasks = new ArrayList<>();
    public static List<Runnable> jobs = new ArrayList<>();
    public static List<Object[]> books = new ArrayList<>();
    static int npcs, sceneries;

    static int n(Object o) { return ((Number) o).intValue(); }

    public static void note(String name, Object... args) {
        if (!recording) return;
        List<Object> row = new ArrayList<>();
        row.add(name);
        row.addAll(Arrays.asList(args));
        events.add(row);
    }

    static final String WORK = System.getProperty("user.dir");

    /** The working directory the oracle runs in, as the native host names it. */
    static Object portable(Object o) {
        if (o instanceof String && ((String) o).contains(WORK)) return ((String) o).replace(WORK, "<work>").replace('\\', '/');
        return o;
    }

    public static void emit(String name, Object... args) {
        if (!recording) return;
        for (int i = 0; i < args.length; i++) args[i] = portable(args[i]);
        List<Object> row = new ArrayList<>();
        row.add(name);
        row.addAll(Arrays.asList(args));
        events.add(row);
        opCalls++;
        if (opCalls == failAt) throw new Failure();
    }

    static String text(Object o) { return String.valueOf(o); }

    /** The answer for an event: the longest argument prefix the case names, then the event alone. */
    public static Object answer(String name, Object fallback, Object... args) {
        for (int k = args.length; k >= 0; k--) {
            StringBuilder key = new StringBuilder(name);
            for (int i = 0; i < k; i++) key.append(':').append(text(args[i]));
            String s = key.toString();
            if (answers.containsKey(s)) {
                Object v = answers.get(s);
                if (v instanceof List) {
                    List l = (List) v;
                    int i = answerCalls.merge(s, 1, Integer::sum) - 1;
                    v = l.get(Math.min(i, l.size() - 1));
                }
                return v;
            }
        }
        return fallback;
    }

    public static Object ask(String name, Object fallback, Object... args) {
        emit(name, args);
        return answer(name, fallback, args);
    }
    public static int askInt(String name, int fallback, Object... args) { return n(ask(name, fallback, args)); }
    public static long askLong(String name, long fallback, Object... args) { return ((Number) ask(name, fallback, args)).longValue(); }
    public static boolean askBool(String name, boolean fallback, Object... args) { return Boolean.TRUE.equals(ask(name, fallback, args)); }
    public static String askStr(String name, String fallback, Object... args) { return (String) ask(name, fallback, args); }
    /** A list the case states (not an answer sequence). */
    public static List<Object> list(String name) {
        Object v = values.get(name);
        return v == null ? new ArrayList<>() : new ArrayList<Object>((List) v);
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
    public static Object caseValue(String key) { return values.get(key); }
    public static boolean flag(String key) { return Boolean.TRUE.equals(values.get(key)); }
    public static int number(String key, int fallback) { Object v = values.get(key); return v == null ? fallback : n(v); }
    public static Location home() { return location(values.get("home")); }
    public static String dataPath() { return (String) values.get("dataPath"); }
    public static GameWorld.Settings settings() {
        Map s = (Map) values.get("settings");
        return s == null ? null : new GameWorld.Settings(Boolean.TRUE.equals(s.get("devMode")), Boolean.TRUE.equals(s.get("beta")));
    }

    public static Player player(Object key) { return key == null ? null : players.get((String) key); }
    public static Location location(Object v) {
        if (v == null) return null;
        List a = (List) v;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }
    public static List<Object> coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }

    // Attributes: case values in, recorded representation out.
    static Object attributeValue(Object spec) {
        if (spec == null) return null;
        Map m = (Map) spec;
        if (m.containsKey("int")) return n(m.get("int"));
        if (m.containsKey("long")) return ((Number) m.get("long")).longValue();
        if (m.containsKey("bool")) return m.get("bool");
        if (m.containsKey("text")) return m.get("text");
        if (m.containsKey("rights")) return Rights.valueOf((String) m.get("rights"));
        if (m.containsKey("list")) return new ArrayList<Object>((List) m.get("list"));
        if (m.containsKey("player")) return players.get((String) m.get("player"));
        throw new IllegalArgumentException(m.toString());
    }
    public static Object value(Object v) {
        if (v == null) return null;
        Map<String, Object> out = new LinkedHashMap<>();
        if (v instanceof Integer) out.put("int", v);
        else if (v instanceof Long) out.put("long", v);
        else if (v instanceof Boolean) out.put("bool", v);
        else if (v instanceof String) out.put("text", v);
        else if (v instanceof Rights) out.put("rights", ((Rights) v).name());
        else if (v instanceof List) out.put("list", new ArrayList<Object>((List) v));
        else if (v instanceof Player) out.put("player", ((Player) v).key);
        else throw new IllegalArgumentException(v.getClass().getName());
        return out;
    }

    // Containers (SystemCommandSet).
    public static Item[] slots(Container container, Player owner, String name) {
        Object v = values.get("slots:" + owner.key + ":" + name);
        if (v == null) return new Item[0];
        List l = (List) v;
        Item[] out = new Item[l.size()];
        for (int i = 0; i < out.length; i++) {
            List s = (List) l.get(i);
            if (s != null) out[i] = Item.inSlot(n(s.get(0)), i, n(s.get(1)), owner.key, name);
        }
        return out;
    }

    // Accounts.
    public static UserAccountInfo account(String name) {
        Object key = answer("info", null, name);
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

    // Scheduling and kept callbacks.
    public static void submit(Pulse pulse) {
        emit("submit", pulse.getClass().getName(), pulse.getDelay());
        pulses.add(pulse);
    }
    public static String npcKey() { return "npc" + (npcs++); }
    public static String sceneryKey() { return "obj" + (sceneries++); }

    static PrintStream realOut = System.out, realErr = System.err;

    /** println on System.out and printStackTrace on System.err: host outputs the original cannot fail at. */
    static final class Capture extends PrintStream {
        final boolean err;
        Capture(boolean err) { super(new ByteArrayOutputStream()); this.err = err; }
        @Override public void println(String x) { if (!err) note("println", x); }
        @Override public void println(Object x) {
            if (x instanceof Throwable) note("printStackTrace", error((Throwable) x));
            else if (!err) note("println", String.valueOf(x));
        }
    }

    /** System.exit throws Exit; everything else is allowed. */
    static final class Guard extends SecurityManager {
        @Override public void checkExit(int status) { throw new Exit(status); }
        @Override public void checkPermission(java.security.Permission perm) {}
        @Override public void checkPermission(java.security.Permission perm, Object context) {}
    }

    static void deleteTree(java.nio.file.Path path) throws Exception {
        if (!java.nio.file.Files.isDirectory(path)) return;
        List<java.nio.file.Path> all = new ArrayList<>();
        try (java.util.stream.Stream<java.nio.file.Path> walk = java.nio.file.Files.walk(path)) {
            walk.forEach(all::add);
        }
        Collections.reverse(all);
        for (java.nio.file.Path p : all) java.nio.file.Files.delete(p);
    }

    static void setup(Map c) throws Exception {
        input = c;
        values = (Map<String, Object>) c.getOrDefault("values", new HashMap<>());
        answers = (Map<String, Object>) c.getOrDefault("answers", new HashMap<>());
        if (!(System.getSecurityManager() instanceof Guard)) System.setSecurityManager(new Guard());
        System.setOut(new Capture(false));
        System.setErr(new Capture(true));
        UserAccountInfo.Companion.getDefault();
        Map<String, Map> accountSpecs = (Map<String, Map>) c.getOrDefault("accounts", new HashMap<>());
        for (Map.Entry<String, Map> e : accountSpecs.entrySet()) {
            UserAccountInfo info = UserAccountInfo.createDefault();
            Map a = e.getValue();
            if (a.containsKey("username")) info.setUsername((String) a.get("username"));
            if (a.containsKey("password")) info.setPassword((String) a.get("password"));
            if (a.containsKey("rights")) info.setRights(n(a.get("rights")));
            if (a.containsKey("credits")) info.setCredits(n(a.get("credits")));
            accounts.put(e.getKey(), info);
        }
        Map<String, Map> playerSpecs = (Map<String, Map>) c.getOrDefault("players", new HashMap<>());
        for (Map.Entry<String, Map> e : playerSpecs.entrySet()) {
            Map p = e.getValue();
            Player player = new Player(e.getKey(), (String) p.get("name"), (String) p.get("username"), new PlayerDetails(accounts.get((String) p.get("account"))), p);
            players.put(e.getKey(), player);
        }
        for (Map.Entry<String, Map> e : playerSpecs.entrySet()) {
            for (Map.Entry<String, Object> a : ((Map<String, Object>) e.getValue().getOrDefault("attributes", new HashMap<>())).entrySet()) {
                players.get(e.getKey()).oracleAttributes().setAttribute(a.getKey(), attributeValue(a.getValue()));
            }
        }
        for (Object d : (List) values.getOrDefault("cleanDirs", Collections.emptyList())) deleteTree(java.nio.file.Paths.get((String) d));
        for (Object d : (List) values.getOrDefault("existingDirs", Collections.emptyList())) java.nio.file.Files.createDirectories(java.nio.file.Paths.get((String) d));
        // The class scan's order of the compiled sets.
        for (String name : new String[] {"MiscCommandSet", "SpawnCommandSet", "StatsCommandSet", "SystemCommandSet"}) {
            Class<?> set = Class.forName("core.game.system.command.sets." + name);
            try {
                ((core.plugin.Plugin<Object>) set.getDeclaredConstructor().newInstance()).newInstance(null);
            } catch (Throwable t) {
                throw new IllegalStateException(t);
            }
        }
        for (Object e : (List) c.getOrDefault("extra", Collections.emptyList())) {
            List row = (List) e;
            CommandMapping.INSTANCE.register(new Command((String) row.get(0), Privilege.valueOf((String) row.get(1)), (String) row.get(4), (String) row.get(5),
                (Function2<Player, String[], Unit>) (player, args) -> {
                    emit("content", row.get(2), n(row.get(3)), player.key, Arrays.asList(args));
                    return Unit.INSTANCE;
                }));
        }
    }

    static Object op(Map op) throws Exception {
        String kind = (String) op.get("op");
        Player player = player(op.get("player"));
        SystemUpdate updater = SystemManager.getUpdater();
        switch (kind) {
            case "parse":
                return CommandSystem.Companion.getCommandSystem().parse(player, (String) op.get("line"));
            case "input": {
                Object value = op.get("value");
                inputs.get(n(op.get("index"))).invoke(value instanceof Long ? (Object) ((Long) value).intValue() : value);
                return null;
            }
            case "job":
                jobs.get(n(op.get("index"))).run();
                return null;
            case "book": {
                Object[] book = books.get(n(op.get("index")));
                return ((Function3<Object, Object, Object, Object>) book[1]).invoke(player, n(op.get("page")), n(op.get("button")));
            }
            case "flag":
                SystemManager.flag(core.game.system.SystemState.valueOf((String) op.get("state")));
                return null;
            case "countdown":
                updater.setCountdown(n(op.get("ticks")));
                return null;
            case "backup":
                updater.setCreateBackup(Boolean.TRUE.equals(op.get("value")));
                return null;
            case "cancel":
                updater.cancel();
                return null;
            case "notify":
                updater.notifyPlayers();
                return null;
            case "update":
                return updater.update();
            case "hook":
                new SystemShutdownHook().run();
                return null;
            case "terminate":
                SystemManager.getTerminator().terminate();
                return null;
            case "save":
                SystemManager.getTerminator().save((String) op.get("directory"));
                return null;
            case "query":
                return Arrays.asList(SystemManager.isActive(), SystemManager.isUpdating(), SystemManager.isPrivate(), SystemManager.isTerminated(),
                    updater.isCreateBackup());
            case "config": {
                core.game.system.SystemConfig config = SystemManager.getSystemConfig();
                String what = (String) op.get("what");
                switch (what) {
                    case "validLogin": return config.validLogin(player);
                    case "doubleExp": return config.isDoubleExp();
                    case "split": return config.split((String) op.get("data"), (String) op.get("regex"));
                    case "beta": return config.isBetaUser((String) op.get("name"));
                    case "get": return config.getConfig((String) op.get("key"));
                    case "getOr": return config.getConfig((String) op.get("key"), op.get("fail"));
                    case "sizes": return Arrays.asList(config.getConfigs().size(), config.getBetaUsers().size());
                    default: throw new IllegalArgumentException(what);
                }
            }
            case "packet":
                return Packets.write(op, player);
            default:
                throw new IllegalArgumentException(kind);
        }
    }

    public static Object error(Throwable t) {
        if (t instanceof Failure) return Arrays.asList("Binding", null);
        if (t instanceof Exit) return Arrays.asList("Exit", ((Exit) t).status);
        if (t instanceof IllegalStateException) return Arrays.asList("IllegalState", null);
        if (t instanceof NullPointerException) return Arrays.asList("Null", null);
        if (t instanceof NumberFormatException) return Arrays.asList("NumberFormat", t.getMessage());
        if (t instanceof IndexOutOfBoundsException) return Arrays.asList("Index", t.getMessage());
        if (t instanceof ClassCastException) return Arrays.asList("Cast", null);
        if (t instanceof java.awt.HeadlessException) return Arrays.asList("Headless", null);
        if (t instanceof NoSuchElementException) return Arrays.asList("NoSuchElement", t.getMessage());
        return Arrays.asList(t.getClass().getName(), t.getMessage());
    }

    static Object state() {
        Map<String, Object> out = new LinkedHashMap<>();
        for (Map.Entry<String, UserAccountInfo> e : accounts.entrySet()) {
            UserAccountInfo a = e.getValue();
            out.put(e.getKey(), Arrays.asList(a.getRights(), a.getCredits()));
        }
        Map<String, Object> people = new LinkedHashMap<>();
        for (Map.Entry<String, Player> e : players.entrySet()) {
            Player p = e.getValue();
            Map<String, Object> attrs = new TreeMap<>();
            for (Map.Entry<String, Object> a : p.oracleAttributes().getAttributes().entrySet()) attrs.put(a.getKey(), value(a.getValue()));
            List<String> saved = new ArrayList<>(p.oracleAttributes().getSavedAttributes());
            people.put(e.getKey(), Arrays.asList(attrs, saved, p.oracleState()));
        }
        List<Object> dirs = new ArrayList<>();
        for (Object d : (List) values.getOrDefault("cleanDirs", Collections.emptyList())) dirs.add(java.nio.file.Files.isDirectory(java.nio.file.Paths.get((String) d)));
        SystemUpdate u = SystemManager.getUpdater();
        Map<String, Object> s = new LinkedHashMap<>();
        s.put("accounts", out);
        s.put("players", people);
        s.put("system", Arrays.asList(SystemManager.state().name(), u.getDelay(), u.getTicksPassed(), u.isRunning(), u.isCreateBackup(),
            GameWorld.getMajorUpdateWorker().getStarted()));
        s.put("pending", Arrays.asList(pulses.size(), inputs.size(), jobs.size(), books.size()));
        s.put("dirs", dirs);
        return s;
    }

    public static String run(String line) throws Exception {
        Map c = (Map) JSONValue.parseWithException(line);
        recording = false;
        List<Object> results = new ArrayList<>();
        try {
            setup(c);
            recording = true;
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
        } finally {
            System.setOut(realOut);
            System.setErr(realErr);
        }
        Map<String, Object> out = new LinkedHashMap<>(c);
        out.put("expected", results);
        return JSONValue.toJSONString(out);
    }
}
