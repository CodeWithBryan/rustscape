package oracle;

import java.io.*;
import java.lang.reflect.*;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.*;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import core.ServerConstants;
import core.cache.def.impl.SceneryDefinition;
import core.game.activity.ActivityManager;
import core.game.activity.ActivityPlugin;
import core.game.activity.Cutscene;
import core.game.activity.CutscenePlugin;
import core.game.event.EventHook;
import core.game.event.SelfDeathEvent;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.system.task.Pulse;
import core.game.world.GameSettings;
import core.game.world.GameWorld;
import core.game.world.map.*;
import core.game.world.map.build.DynamicRegion;
import core.game.world.map.zone.*;
import core.game.system.config.XteaParser;
import oracle.content.*;

/**
 * Runs one case (oracle.boot.Boot gives every case a class loader of its own, so ActivityManager's map,
 * RegionManager's cache, DynamicRegion's reserved areas and the multiway zone start fresh) against the unchanged
 * ActivityManager, ActivityPlugin, CutscenePlugin and Cutscene over the unchanged DynamicRegion, Region,
 * RegionChunk, RegionManager, MapZone and RegionZone with the real cache regions, and returns what they did.
 */
public class Support {
    public static class Failure extends RuntimeException {}

    static Map input;
    static List<Object> events;
    static List<String> frames;
    static int opCalls, failAt;
    static Map<String, Integer> answerCalls;
    static Map cache;
    static List<Pulse> queue;
    static Map<String, Entity> entities;
    static IdentityHashMap<Object, String> labels;
    static List<Object> tracked;
    static Map<String, Integer> counters;
    public static List<Function2> actions;
    public static core.net.IoSession session;
    static Player player;
    static int npcs, copies;

    public static int n(Object v) { return ((Number) v).intValue(); }
    public static boolean b(Map m, String k) { return Boolean.TRUE.equals(m.get(k)); }
    public static Location location(Object v) { if (v == null) return null; List l = (List) v; return new Location(n(l.get(0)), n(l.get(1)), n(l.get(2))); }
    public static Object coords(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    static byte[] bytes(String s) { byte[] b = new byte[s.length() / 2]; for (int i = 0; i < b.length; i++) b[i] = (byte) Integer.parseInt(s.substring(i * 2, i * 2 + 2), 16); return b; }
    static String hex(byte[] v) { StringBuilder s = new StringBuilder(); for (byte x : v) s.append(String.format("%02x", x & 255)); return s.toString(); }
    static Map<String, Object> map(Object... v) { Map<String, Object> m = new TreeMap<>(); for (int i = 0; i < v.length; i += 2) m.put((String) v[i], v[i + 1]); return m; }

    /** A recorded call; the case's failAt-th call of an operation throws. */
    public static void emit(String name, Object... args) {
        List<Object> e = new ArrayList<>();
        e.add(name);
        Collections.addAll(e, args);
        events.add(e);
        opCalls++;
        if (opCalls == failAt) throw new Failure();
    }
    public static void frame(byte[] bytes) { frames.add(hex(bytes)); }
    public static void submit(Pulse p) { emit("submit", p.getClass().getName(), owner(p)); queue.add(p); }
    public static String copyName() { return "copy" + (++copies); }
    public static String npcName() { return "npc" + (++npcs); }
    /** A service answer from the case (a list is consumed call by call), else the default. */
    public static <T> T answer(String name, T fallback) {
        Map answers = (Map) input.get("answers");
        if (answers == null || !answers.containsKey(name)) return fallback;
        Object v = answers.get(name);
        if (v instanceof List) {
            int i = answerCalls.merge(name, 1, Integer::sum) - 1;
            List l = (List) v;
            v = i < l.size() ? l.get(i) : l.get(l.size() - 1);
        }
        return (T) v;
    }
    public static Object key(Object node) { return node instanceof Node ? ((Node) node).getName() : null; }

    public static void track(Object o, String kind) {
        int i = counters.merge(kind, 1, Integer::sum);
        labels.put(o, kind + "#" + i);
        tracked.add(o);
    }
    public static String label(Object o) { return o == null ? null : labels.get(o); }
    static String owner(Object o) {
        for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
            try { Field f = c.getDeclaredField("this$0"); f.setAccessible(true); return label(f.get(o)); }
            catch (NoSuchFieldException e) { continue; }
            catch (IllegalAccessException e) { throw new AssertionError(e); }
        }
        return null;
    }
    public static Object value(Object v) {
        if (v == null || v instanceof String || v instanceof Boolean || v instanceof Integer) return v;
        if (v instanceof Location) return map("location", coords((Location) v));
        if (labels.containsKey(v)) return map("object", labels.get(v));
        if (v instanceof kotlin.Function) return map("function", v.getClass().getName());
        if (v instanceof Long) return map("long", v);
        return map("class", v.getClass().getName());
    }
    public static ZoneRestriction[] restrictions(Map config) {
        List l = (List) config.getOrDefault("restrictions", Collections.emptyList());
        ZoneRestriction[] r = new ZoneRestriction[l.size()];
        for (int i = 0; i < r.length; i++) r[i] = ZoneRestriction.valueOf((String) l.get(i));
        return r;
    }
    static ZoneBorders borders(List l) { return new ZoneBorders(n(l.get(0)), n(l.get(1)), n(l.get(2)), n(l.get(3))); }

    /** The scripted content's configure: instances and region bases through ActivityPlugin's own members. */
    public static void configure(ActivityPlugin plugin, Map config, Object[] self) {
        for (Object raw : (List) config.getOrDefault("configure", Collections.emptyList())) {
            Map a = (Map) raw;
            switch ((String) a.get("do")) {
                case "create": {
                    DynamicRegion r = DynamicRegion.create(n(a.get("region")));
                    if (plugin instanceof TestActivity) ((TestActivity) plugin).setRegion(r); else ((TestCutscenePlugin) plugin).setRegion(r);
                    break;
                }
                case "setRegionBase":
                    if (plugin instanceof TestActivity) ((TestActivity) plugin).callSetRegionBase(); else ((TestCutscenePlugin) plugin).callSetRegionBase();
                    break;
                case "setRegionBaseArea": {
                    DynamicRegion[] all = DynamicRegion.create(borders((List) a.get("borders")));
                    if (plugin instanceof TestActivity) ((TestActivity) plugin).callSetRegionBase(all); else ((TestCutscenePlugin) plugin).callSetRegionBase(all);
                    break;
                }
                case "register": plugin.register(borders((List) a.get("borders"))); break;
                case "emit": emit("content", a.get("value")); break;
                case "fail": throw new Failure();
                default: throw new AssertionError(a);
            }
        }
    }

    // ------------------------------------------------------------------ snapshot
    static Object field(Object o, Class<?> c, String name) throws Exception { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(o); }
    static Object bordersValue(ZoneBorders b) { return b == null ? null : Arrays.asList(b.getSouthWestX(), b.getSouthWestY(), b.getNorthEastX(), b.getNorthEastY()); }
    static Object regionValue(Region r) { return r == null ? null : r.getId(); }

    static Object snapshot() throws Exception {
        List<Object> plugins = new ArrayList<>(), scenes = new ArrayList<>();
        for (Object o : tracked) {
            if (o instanceof ActivityPlugin) {
                ActivityPlugin p = (ActivityPlugin) o;
                Map<String, Object> m = map("label", label(p), "name", p.getName(), "instanced", p.isInstanced(), "multicombat", p.isMulticombat(), "safe", p.isSafe(),
                    "restriction", p.getRestriction(), "zoneType", p.getZoneType(), "fireRandoms", p.isFireRandoms(),
                    "base", coords(p.getBase()), "region", regionValue((Region) field(p, ActivityPlugin.class, "region")), "player", p.getPlayer() == null ? null : p.getPlayer().getName(),
                    "safeRespawn", coords((Location) field(p, ActivityPlugin.class, "safeRespawn")));
                if (p instanceof CutscenePlugin) { m.put("fade", ((CutscenePlugin) p).isFade()); m.put("npcs", ((CutscenePlugin) p).getNPCS().size()); }
                plugins.add(m);
            } else if (o instanceof Cutscene) {
                Cutscene c = (Cutscene) o;
                List<Object> added = new ArrayList<>();
                for (Map.Entry<Integer, ArrayList<NPC>> e : ((HashMap<Integer, ArrayList<NPC>>) field(c, Cutscene.class, "addedNPCs")).entrySet()) {
                    List<Object> names = new ArrayList<>();
                    for (NPC x : e.getValue()) names.add(x.getName());
                    added.add(Arrays.asList(e.getKey(), names));
                }
                scenes.add(map("label", label(c), "ended", c.getEnded(), "exit", coords(c.getExitLocation()), "base", coords((Location) field(c, Cutscene.class, "base")),
                    "region", regionValue((Region) field(c, Cutscene.class, "region")), "npcs", added));
            }
        }
        Map<String, Object> activities = new TreeMap<>();
        for (Map.Entry<String, ActivityPlugin> e : ((Map<String, ActivityPlugin>) field(null, ActivityManager.class, "ACTIVITIES")).entrySet()) activities.put(e.getKey(), label(e.getValue()));
        List<Object> regions = new ArrayList<>();
        for (Map.Entry<Integer, Region> e : new TreeMap<>(RegionManager.getRegionCache()).entrySet()) {
            Region r = e.getValue();
            boolean dynamic = r instanceof DynamicRegion;
            if (!dynamic && r.getRegionZones().isEmpty()) continue;
            List<Object> zones = new ArrayList<>();
            for (RegionZone z : r.getRegionZones()) zones.add(Arrays.asList(z.getZone().getName(), label(z.getZone()), bordersValue(z.getBorders())));
            regions.add(Arrays.asList(e.getKey(), dynamic, zones, dynamic ? bordersValue(((DynamicRegion) r).getBorders()) : null, dynamic && ((DynamicRegion) r).isMulticombat(),
                coords(r.getBaseLocation())));
        }
        List<Object> reserved = new ArrayList<>();
        for (ZoneBorders z : (List<ZoneBorders>) field(null, DynamicRegion.class, "RESERVED_AREAS")) reserved.add(bordersValue(z));
        List<Object> pulses = new ArrayList<>();
        Field passed = Pulse.class.getDeclaredField("ticksPassed");
        passed.setAccessible(true);
        for (Pulse p : queue) pulses.add(Arrays.asList(p.getClass().getName(), owner(p), p.getDelay(), passed.get(p), p.isRunning()));
        Entity.Properties pr = player.getProperties();
        Map<String, Object> attributes = new TreeMap<>();
        for (Map.Entry<String, Object> e : player.attributeMap.entrySet()) attributes.put(e.getKey(), value(e.getValue()));
        Map<String, Object> state = map("location", coords(player.getLocation()), "attributes", attributes, "teleport", coords(pr.teleportTarget), "spawn", coords(pr.spawnTarget),
            "safe", pr.safe, "safeRespawn", coords(pr.safeRespawn), "logout", new ArrayList<>(new TreeSet<>(player.listenerMap.keySet())), "hooks", player.hookList.size(), "actions", actions.size());
        return map("plugins", plugins, "cutscenes", scenes, "activities", activities, "regions", regions, "reserved", reserved, "queue", pulses, "player", state,
            "events", new ArrayList<>(events), "frames", new ArrayList<>(frames));
    }

    static void tick() {
        int count = queue.size();
        for (int i = 0; i < count; i++) {
            Pulse p = queue.remove(0);
            if (!p.update() && p.isRunning()) queue.add(p);
        }
    }

    static Object ref(Object name) {
        for (Object o : tracked) if (name.equals(labels.get(o))) return o;
        throw new AssertionError("unknown object " + name);
    }
    static Entity entity(Object name) { return name == null ? player : entities.get(name); }

    static Object act(Map a) throws Throwable {
        String op = (String) a.get("op");
        switch (op) {
            case "register": {
                Map config = (Map) a.get("config");
                ActivityPlugin p = "cutscene".equals(config.get("kind")) ? new TestCutscenePlugin(config) : new TestActivity(config);
                ActivityManager.register(p);
                return label(p);
            }
            case "start": {
                List args = (List) a.getOrDefault("args", Collections.emptyList());
                return ActivityManager.start(player, (String) a.get("name"), b(a, "login"), args.toArray());
            }
            case "getActivity": return label(ActivityManager.getActivity((String) a.get("name")));
            case "enter": return ((ActivityPlugin) ref(a.get("plugin"))).enter(entity(a.get("entity")));
            case "leave": return ((ActivityPlugin) ref(a.get("plugin"))).leave(entity(a.get("entity")), b(a, "logout"));
            case "fireEvent": return value(((ActivityPlugin) ref(a.get("plugin"))).fireEvent((String) a.get("id")));
            case "pluginStart": return ((ActivityPlugin) ref(a.get("plugin"))).start(player, b(a, "login"));
            case "stop": ((CutscenePlugin) ref(a.get("plugin"))).stop(b(a, "fade")); return null;
            case "end": ((CutscenePlugin) ref(a.get("plugin"))).end(); return null;
            case "unpause": ((CutscenePlugin) ref(a.get("plugin"))).unpause(); return null;
            case "submitEndPulse": GameWorld.getPulser().submit(((CutscenePlugin) ref(a.get("plugin"))).getEndPulse()); return null;
            case "submitStartPulse": GameWorld.getPulser().submit(((CutscenePlugin) ref(a.get("plugin"))).getStartPulse()); return null;
            case "cutscene": return label(new TestCutscene(player, (Map) a.get("script")));
            case "startCutscene": {
                Cutscene c = (Cutscene) ref(a.get("cutscene"));
                if (a.containsKey("hide")) c.start(b(a, "hide")); else c.start();
                return null;
            }
            case "cutsceneCall": ((TestCutscene) ref(a.get("cutscene"))).act((Map) a.get("action")); return null;
            case "continue": {
                Function2 f = actions.get(actions.size() - 1);
                f.invoke(player, 0);
                return null;
            }
            case "death": {
                for (Object h : new ArrayList<>(player.hookList)) ((EventHook) h).process(player, new SelfDeathEvent(player));
                return null;
            }
            case "logout": {
                Function1 f = player.listenerMap.get((String) a.get("key"));
                if (f == null) return false;
                f.invoke(player);
                return true;
            }
            case "tick": for (int i = 0; i < n(a.get("count")); i++) tick(); return queue.size();
            case "npc": {
                Location l = location(a.get("location"));
                NPC npc = new NPC((String) a.get("name"), n(a.get("id")), l, null);
                entities.put(npc.getName(), npc);
                RegionManager.move(npc, null, l);
                return null;
            }
            case "attr": {
                Object v = a.get("value");
                if (v instanceof Map && ((Map) v).containsKey("location")) v = location(((Map) v).get("location"));
                else if (v instanceof Map && ((Map) v).containsKey("object")) v = ref(((Map) v).get("object"));
                else if (v instanceof Long) v = ((Long) v).intValue();
                if (v == null) player.attributeMap.remove(a.get("key")); else player.attributeMap.put((String) a.get("key"), v);
                return null;
            }
            case "place": { Field f = Node.class.getDeclaredField("location"); f.setAccessible(true); f.set(player, location(a.get("location"))); return null; }
            case "active": player.setActive(b(a, "value")); return null;
            default: throw new AssertionError(a);
        }
    }

    static String error(Throwable t) {
        if (t instanceof InvocationTargetException) return error(t.getCause());
        if (t instanceof Failure) return "Failure";
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof ClassCastException) return "ClassCast";
        if (t instanceof kotlin.UninitializedPropertyAccessException) return "Uninitialized";
        if (t instanceof ArrayIndexOutOfBoundsException) return "Index";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        if (t instanceof ConcurrentModificationException) return "ConcurrentModification";
        throw new AssertionError("unexpected original exception", t);
    }

    static void setup(Map c) throws Exception {
        events = new ArrayList<>(); frames = new ArrayList<>(); queue = new ArrayList<>(); entities = new TreeMap<>();
        labels = new IdentityHashMap<>(); tracked = new ArrayList<>(); counters = new HashMap<>(); actions = new ArrayList<>();
        answerCalls = new HashMap<>(); session = new core.net.IoSession(); npcs = 0; copies = 0;
        Map world = (Map) c.getOrDefault("world", Collections.emptyMap());
        Map settings = (Map) world.get("settings");
        GameWorld.setSettings(settings == null ? null : new GameSettings((String) settings.get("name"), b(settings, "devMode")));
        ServerConstants.HOME_LOCATION = location(world.get("home"));
        ServerConstants.LOG_CUTSCENE = !Boolean.FALSE.equals(world.get("logCutscene"));
        ServerConstants.SERVER_NAME = (String) world.getOrDefault("serverName", "");
        Map defs = (Map) cache.get("definitions");
        for (Object id : new TreeSet<Object>(defs.keySet())) {
            int k = Integer.parseInt((String) id);
            SceneryDefinition.getDefinitions().put(k, SceneryDefinition.parseDefinition(k, ByteBuffer.wrap(bytes((String) defs.get(id)))));
        }
        Map keys = (Map) cache.get("keys");
        for (Object id : keys.keySet()) {
            List v = (List) keys.get(id);
            XteaParser.Companion.getREGION_XTEA().put(Integer.parseInt((String) id), new int[] { n(v.get(0)), n(v.get(1)), n(v.get(2)), n(v.get(3)) });
        }
        Map p = (Map) c.get("player");
        player = new Player((String) p.get("name"), location(p.get("location")));
        entities.put(player.getName(), player);
    }

    // ------------------------------------------------------------------ the real cache regions (dynamic-regions adapters)
    static Map cacheRegion(int id) { return (Map) ((Map) cache.get("regions")).get(Integer.toString(id)); }
    public static int archive(String s) {
        int split = s.indexOf('_');
        int x = Integer.parseInt(s.substring(1, split)), y = Integer.parseInt(s.substring(split + 1));
        Map r = cacheRegion(x << 8 | y);
        if (r == null) throw new Error("Unsupplied cache region " + s);
        return n(r.get(s.charAt(0) == 'm' ? "mapGroup" : "landGroup"));
    }
    static byte[] cacheBytes(int group, String kind) {
        for (Object v : ((Map) cache.get("regions")).values()) {
            Map r = (Map) v;
            if (n(r.get(kind + "Group")) == group) { String s = (String) r.get(kind); return s == null ? null : bytes(s); }
        }
        throw new AssertionError("Unsupplied cache group " + group);
    }
    public static byte[] mapBytes(int group) { return cacheBytes(group, "map"); }
    public static byte[] landBytes(int group) { return cacheBytes(group, "land"); }

    public static String runCase(String line) throws Exception {
        cache = (Map) System.getProperties().get("oracle.cacheMap");
        if (cache == null) {
            cache = (Map) new JSONParser().parse(new String(Files.readAllBytes(Paths.get(System.getProperty("oracle.cache"))), StandardCharsets.UTF_8));
            System.getProperties().put("oracle.cacheMap", cache);
        }
        JSONObject c = (JSONObject) new JSONParser().parse(line);
        input = c;
        setup(c);
        PrintStream err = System.err;
        System.setErr(new PrintStream(new ByteArrayOutputStream(), true) {
            @Override public void println(Object x) {
                String s = String.valueOf(x);
                if (s.contains("Unsupplied cache region")) throw new AssertionError(s);
                if (!s.startsWith("\t") && !s.startsWith("Caused by")) events.add(Arrays.asList("stackTrace"));
            }
        });
        List<Object> out = new ArrayList<>();
        try {
            for (Object raw : (List) c.get("ops")) {
                Map a = (Map) raw;
                events.clear(); frames.clear();
                opCalls = 0;
                failAt = a.containsKey("failAt") ? n(a.get("failAt")) : -1;
                Map<String, Object> result = new TreeMap<>();
                try {
                    result.put("value", act(a));
                } catch (Throwable t) {
                    result.put("error", error(t));
                }
                failAt = -1;
                Map<String, Object> s = (Map<String, Object>) snapshot();
                s.put("result", result);
                out.add(s);
            }
        } finally {
            System.setErr(err);
        }
        c.put("expected", out);
        StringWriter w = new StringWriter();
        JSONValue.writeJSONString(c, w);
        return w.toString();
    }
}
