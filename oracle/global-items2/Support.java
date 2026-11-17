package oracle;

import core.cache.def.impl.ItemDefinition;
import core.game.component.CloseEvent;
import core.game.component.Component;
import core.game.dialogue.DialogueFile;
import core.game.interaction.OptionHandler;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import core.tools.RandomFunction;
import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.ThreadLocalRandom;
import java.util.concurrent.atomic.AtomicLong;
import kotlin.jvm.functions.Function0;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import kotlin.jvm.functions.Function3;
import kotlin.jvm.functions.Function6;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Runs one case (Boot gives every case a class loader of its own) against the original item handlers and returns
 * what they did. Each case seeds the three generators the originals draw from (RandomFunction.RANDOM, the generator
 * private to Collections.shuffle and the calling thread's ThreadLocalRandom, which Kotlin's Random.Default uses on
 * JDK 8+); their states after every operation are part of the record.
 */
public class Support {
    public static class Failure extends RuntimeException {}

    static Map input;
    static List<Object> events;
    static Map<String, Integer> calls;
    static int opCalls, failAt, created;

    public static Map<String, Node> nodes;
    public static Player player;
    public static Map<String, Object> values, answers;
    static Map<String, Integer> answerCalls;
    static IdentityHashMap<Object, Integer> labels, tracked;
    static List<Object> trackedList;
    static List<Object[]> interactions;
    static List<Object[]> interfaces;
    public static Map<String, Object> itemHandlers, npcHandlers;
    public static Map<Integer, Object> components;
    static List<Object> loginListeners, persistPlayers;
    public static Map<Integer, ItemDefinition> definitions;
    static ByteArrayOutputStream errors;
    static List<Object> stderr;

    static int n(Object o) { return ((Number) o).intValue(); }

    /** Calls made while a class initialises (the originals' constant tables) are not recorded. */
    static boolean initialising() {
        for (StackTraceElement e : new Throwable().getStackTrace()) if ("<clinit>".equals(e.getMethodName())) return true;
        return false;
    }

    public static void emit(String name, Object... args) {
        if (initialising()) return;
        List<Object> row = new ArrayList<>();
        row.add(name);
        for (Object a : args) row.add(a);
        events.add(row);
        opCalls++;
        int count = calls.merge(name, 1, Integer::sum);
        for (Object h : (List) input.getOrDefault("hooks", Collections.emptyList())) {
            Map hook = (Map) h;
            if (name.equals(hook.get("event")) && count == n(hook.getOrDefault("at", 1L)))
                for (Object action : (List) hook.get("actions")) {
                    Map a = (Map) action;
                    if ("fail".equals(a.get("op"))) throw new Failure();
                    set(a);
                }
        }
        if (opCalls == failAt) throw new Failure();
    }

    // ------------------------------------------------------------ object labels as the events show them
    public static String label(Object o) {
        if (o == null) return null;
        Integer i = labels.get(o);
        if (i == null) labels.put(o, i = labels.size() + 1);
        return o.getClass().getName() + "#" + i;
    }
    public static int track(Object o) {
        if (o == null) return -1;
        Integer i = tracked.get(o);
        if (i == null) { tracked.put(o, i = trackedList.size()); trackedList.add(o); }
        return i;
    }
    public static Object key(Object o) {
        if (o == null) return null;
        if (o instanceof Node && ((Node) o).key != null) return ((Node) o).key;
        throw new IllegalStateException("key of " + o.getClass());
    }
    public static List coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    public static Location location(Object v) {
        if (v == null) return null;
        List a = (List) v;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }
    public static Object value(Object v) {
        if (v == null || v instanceof String || v instanceof Boolean || v instanceof Integer) return v;
        if (v instanceof Long) { Map m = new TreeMap(); m.put("long", v); return m; }
        if (v instanceof Double) { Map m = new TreeMap(); m.put("double", v); return m; }
        if (v instanceof Float) { Map m = new TreeMap(); m.put("float", (double) (Float) v); return m; }
        if (v instanceof Location) { Map m = new TreeMap(); m.put("location", coord((Location) v)); return m; }
        if (v instanceof Node && ((Node) v).key != null) return ((Node) v).key;
        if (v instanceof Enum) return ((Enum) v).name();
        if (v instanceof Object[]) { List l = new ArrayList(); for (Object o : (Object[]) v) l.add(value(o)); return l; }
        if (v instanceof int[]) return values((int[]) v);
        if (v instanceof List) { List l = new ArrayList(); for (Object o : (List) v) l.add(value(o)); return l; }
        if (v instanceof Map) { Map m = new TreeMap(); for (Object e : ((Map) v).entrySet()) m.put(((Map.Entry) e).getKey(), value(((Map.Entry) e).getValue())); return m; }
        return label(v);
    }
    public static List values(Object[] a) { if (a == null) return null; List l = new ArrayList(); for (Object o : a) l.add(value(o)); return l; }
    public static List values(int[] a) { if (a == null) return null; List l = new ArrayList(); for (int o : a) l.add(o); return l; }

    static Object typed(Object v) {
        if (v instanceof Long) return ((Long) v).intValue();
        if (v instanceof Map) {
            Map m = (Map) v;
            if (m.containsKey("location")) return location(m.get("location"));
            if (m.containsKey("long")) return ((Number) m.get("long")).longValue();
            if (m.containsKey("double")) return ((Number) m.get("double")).doubleValue();
            if (m.containsKey("node")) return nodes.get((String) m.get("node"));
        }
        return v;
    }

    // ------------------------------------------------------------ answers
    public static Object answer(String name, Object fallback) {
        if (!answers.containsKey(name)) return fallback;
        Object v = answers.get(name);
        if (v instanceof List) {
            List l = (List) v;
            int i = answerCalls.merge(name, 1, Integer::sum) - 1;
            v = l.get(Math.min(i, l.size() - 1));
        }
        return v instanceof Long ? (Object) ((Long) v).intValue() : v;
    }
    public static boolean answerBool(String name, boolean fallback) { return Boolean.TRUE.equals(answer(name, fallback)); }
    public static int answerInt(String name, int fallback) { return n(answer(name, fallback)); }
    public static int intValue(String k, int d) { Object v = values.get(k); return v == null ? d : n(v); }
    public static String text(String k, String d) { return values.containsKey(k) ? (String) values.get(k) : d; }
    public static Node node(Object k) { return k == null ? null : nodes.get((String) k); }
    /** An array answer: a list of node keys, or a list of such lists answered one per call. */
    public static Item[] items(String name) {
        List l = (List) answers.getOrDefault(name, Collections.emptyList());
        if (!l.isEmpty() && l.get(0) instanceof List) {
            int i = answerCalls.merge(name, 1, Integer::sum) - 1;
            l = (List) l.get(Math.min(i, l.size() - 1));
        }
        Item[] out = new Item[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = (Item) node(l.get(i));
        return out;
    }

    // ------------------------------------------------------------ objects the originals construct
    /** {@code new Item(id, amount, charge)}: the definition lookup is the recorded call. */
    public static void newItem(Item item, int id, int amount) {
        if (initialising()) {
            // A table's constant item: unrecorded, named by its definition.
            item.key = "const:" + id;
            item.nodeName = itemName(id);
            return;
        }
        emit("newItem", id, amount);
        item.key = "new:" + id + ":" + (++created);
        item.nodeName = itemName(id);
        nodes.put(item.key, item);
    }
    public static String itemName(int id) {
        ItemDefinition d = definitions.get(id);
        return d != null ? d.definitionName : (String) answer("itemName:" + id, null);
    }

    // ------------------------------------------------------------ registration callbacks from the adapters
    public static void on(int[] ids, String type, String[] options, Function2 handler) {
        emit("on", values(ids), type, values(options), label(handler));
        interactions.add(new Object[] { "on", handler });
    }
    public static void onUseWith(String type, int[] used, int[] with, Function3 handler) {
        emit("onUseWith", type, values(used), values(with), label(handler));
        interactions.add(new Object[] { "use", handler });
    }
    public static void onEquip(int id, Function2 handler) {
        emit("onEquip", id, label(handler));
        interactions.add(new Object[] { "equip", handler });
    }
    public static void onInterface(int id, Function6 handler) {
        emit("onInterface", id, label(handler));
        interfaces.add(new Object[] { "button", handler, id });
    }
    public static void onInterfaceClose(int id, Function2 handler) {
        emit("onInterfaceClose", id, label(handler));
        interfaces.add(new Object[] { "close", handler, id });
    }
    public static void itemHandler(int id, String k, Object handler) {
        emit("itemHandler", id, k, label(handler));
        itemHandlers.put(id + ":" + k, handler);
    }
    public static void npcHandler(int id, String k, Object handler) {
        emit("npcHandler", id, k, label(handler));
        npcHandlers.put(id + ":" + k, handler);
    }
    public static void componentPlugin(int id, Object plugin) {
        emit("componentPlugin", id, label(plugin));
        components.put(id, plugin);
    }
    public static void openDialogue(Player p, Object dialogue, Object[] args) {
        emit("openDialogue", track(dialogue), values(args));
        if (dialogue instanceof DialogueFile) {
            DialogueFile file = (DialogueFile) dialogue;
            file.setPlayer(p);
            file.setInterpreter(p.getDialogueInterpreter());
        }
    }

    // ------------------------------------------------------------ state
    static void node(String k, Map v) {
        Node node = nodes.get(k);
        if (node == null) {
            String kind = (String) v.get("kind");
            switch (kind) {
                case "player": node = new Player(); break;
                case "npc": node = new core.game.node.entity.npc.NPC(); break;
                case "scenery": node = new core.game.node.scenery.Scenery(); break;
                case "item": node = new Item(); break;
                default: throw new IllegalArgumentException(kind);
            }
            node.key = k;
            nodes.put(k, node);
        }
        if (v.containsKey("id")) node.nodeId = n(v.get("id"));
        if (v.containsKey("name")) node.nodeName = (String) v.get("name");
        if (v.containsKey("location")) node.nodeLocation = location(v.get("location"));
        if (v.containsKey("amount")) ((Item) node).oracleAmount(n(v.get("amount")));
        if (v.containsKey("slot")) ((Item) node).itemSlot = n(v.get("slot"));
        if (v.containsKey("charge")) ((Item) node).itemCharge = n(v.get("charge"));
    }

    static void set(Map v) {
        Map ns = (Map) v.getOrDefault("nodes", Collections.emptyMap());
        for (Object k : new TreeSet(ns.keySet())) node((String) k, (Map) ns.get(k));
        Map as = (Map) v.getOrDefault("attributes", Collections.emptyMap());
        for (Object k : new TreeSet(as.keySet())) {
            Map attrs = (Map) as.get(k);
            Entity e = (Entity) nodes.get((String) k);
            for (Object a : attrs.keySet()) {
                Object value = attrs.get(a);
                if (value == null) e.attrs.remove(a); else e.attrs.put((String) a, typed(value));
            }
        }
        values.putAll((Map) v.getOrDefault("values", Collections.emptyMap()));
        answers.putAll((Map) v.getOrDefault("answers", Collections.emptyMap()));
    }

    static int i(Map op, String k) { return n(op.get(k)); }

    static Object instantiate(String name) throws Exception {
        java.lang.reflect.Constructor<?> c = Class.forName(name).getDeclaredConstructor();
        c.setAccessible(true);
        return c.newInstance();
    }

    /** ClassScanner.loadPureInterfaces's order for one content class, or newInstance(null) for a plugin. */
    @SuppressWarnings("unchecked")
    static void define(Object o) throws Throwable {
        if (o instanceof core.api.ContentInterface) {
            if (o instanceof core.api.LoginListener) { emit("loginListener", label(o)); loginListeners.add(o); }
            if (o instanceof core.game.interaction.InteractionListener) ((core.game.interaction.InteractionListener) o).defineListeners();
            if (o instanceof core.game.interaction.InterfaceListener) ((core.game.interaction.InterfaceListener) o).defineInterfaceListeners();
            if (o instanceof core.api.PersistPlayer) { emit("persistPlayer", label(o)); persistPlayers.add(o); }
        } else {
            ((core.plugin.Plugin<Object>) o).newInstance(null);
        }
    }

    @SuppressWarnings("unchecked")
    static Object run(Map op) throws Throwable {
        Node node = node(op.get("node"));
        switch ((String) op.get("op")) {
            case "set": set(op); return null;
            case "define": define(instantiate((String) op.get("class"))); return null;
            case "interact": {
                if (i(op, "n") >= interactions.size()) return "none";
                Object[] r = interactions.get(i(op, "n"));
                if ("use".equals(r[0])) return ((Function3) r[1]).invoke(player, node(op.get("used")), node(op.get("with")));
                return ((Function2) r[1]).invoke(player, node);
            }
            case "button": {
                if (i(op, "n") >= interfaces.size()) return "none";
                Object[] r = interfaces.get(i(op, "n"));
                Component c = new Component((Integer) r[2]);
                if ("close".equals(r[0])) return ((Function2) r[1]).invoke(player, c);
                return ((Function6) r[1]).invoke(player, c, i(op, "opcode"), i(op, "button"), i(op, "slot"), i(op, "item"));
            }
            case "handler": {
                OptionHandler h = (OptionHandler) itemHandlers.get((String) op.get("via"));
                if (h == null) return "none";
                return h.handle(player, node, (String) op.get("option"));
            }
            case "npcHandler": {
                OptionHandler h = (OptionHandler) npcHandlers.get((String) op.get("via"));
                if (h == null) return "none";
                return h.handle(player, node, (String) op.get("option"));
            }
            case "component": {
                core.game.component.ComponentPlugin c = (core.game.component.ComponentPlugin) components.get(i(op, "id"));
                if (c == null) return "none";
                return c.handle(player, new Component(i(op, "id")), i(op, "opcode"), i(op, "button"), i(op, "slot"), i(op, "item"));
            }
            case "dialogue": {
                if (i(op, "n") >= trackedList.size()) return "none";
                ((DialogueFile) trackedList.get(i(op, "n"))).handle(i(op, "component"), i(op, "button"));
                return null;
            }
            case "stage": {
                ((DialogueFile) trackedList.get(i(op, "n"))).setStage(i(op, "value"));
                return null;
            }
            case "props": {
                OptionHandler h = (OptionHandler) itemHandlers.get((String) op.get("via"));
                if (h == null) return "none";
                return Arrays.asList(h.isWalk(), h.isWalk(player, node), h.isDelayed(player));
            }
            case "select": {
                core.plugin.Plugin p = (core.plugin.Plugin) itemHandlers.get((String) op.get("via"));
                if (p == null) return "none";
                p.handleSelectionCallback(i(op, "skill"), player);
                return null;
            }
            case "task": {
                if (i(op, "n") >= trackedList.size()) return "none";
                Object t = trackedList.get(i(op, "n"));
                if (t instanceof Pulse) return ((Pulse) t).pulse();
                if (t instanceof CloseEvent) return ((CloseEvent) t).close(player, new Component(i(op, "component")));
                if (t instanceof Function1) return ((Function1) t).invoke(i(op, "stage"));
                if (t instanceof Function0) return ((Function0) t).invoke();
                throw new IllegalStateException(t.getClass().getName());
            }
            case "login": {
                ((core.api.LoginListener) loginListeners.get(i(op, "n"))).login(player);
                return null;
            }
            case "save": {
                JSONObject save = new JSONObject();
                ((core.api.PersistPlayer) persistPlayers.get(i(op, "n"))).savePlayer(player, save);
                return saved(save);
            }
            case "parse": {
                JSONObject data = (JSONObject) new JSONParser().parse(JSONValue.toJSONString(op.get("data")));
                ((core.api.PersistPlayer) persistPlayers.get(i(op, "n"))).parsePlayer(player, data);
                return null;
            }
            case "call": return Calls.call((String) op.get("name"), op, node);
            default: throw new IllegalArgumentException((String) op.get("op"));
        }
    }

    /** A save object as written: maps with sorted keys, arrays in their order, strings as strings. */
    static Object saved(Object v) {
        if (v instanceof Map) { Map m = new TreeMap(); for (Object e : ((Map) v).entrySet()) m.put(((Map.Entry) e).getKey(), saved(((Map.Entry) e).getValue())); return m; }
        if (v instanceof List) { List l = new ArrayList(); for (Object o : (List) v) l.add(saved(o)); return l; }
        return v;
    }

    // ------------------------------------------------------------ generators
    static Field threadSeed, collectionsRandom, randomState;

    static Field field(Class<?> owner, String name) throws Exception {
        Field f = owner.getDeclaredField(name);
        f.setAccessible(true);
        return f;
    }
    static void seed(Map seeds) throws Exception {
        RandomFunction.RANDOM.setSeed(((Number) seeds.get("random")).longValue());
        collectionsRandom.set(null, new Random(((Number) seeds.get("shuffle")).longValue()));
        threadSeed.setLong(Thread.currentThread(), ((Number) seeds.get("tlr")).longValue());
    }
    /** The raw state of each generator: java.util.Random's 48-bit seed (twice) and the thread's seed. */
    static List<Object> states() throws Exception {
        return Arrays.asList(((AtomicLong) randomState.get(RandomFunction.RANDOM)).get(),
            ((AtomicLong) randomState.get(collectionsRandom.get(null))).get(), threadSeed.getLong(Thread.currentThread()));
    }

    static Map<String, Object> snapshot(Object result, String error) throws Exception {
        Map<String, Object> out = new TreeMap<>();
        out.put("error", error);
        out.put("result", value(result));
        out.put("events", new ArrayList<>(events));
        Map<String, Object> attrs = new TreeMap<>();
        for (Node node : nodes.values()) {
            if (!(node instanceof Entity)) continue;
            Map<String, Object> a = new TreeMap<>();
            for (Map.Entry<String, Object> e : ((Entity) node).attrs.entrySet()) a.put(e.getKey(), value(e.getValue()));
            if (!a.isEmpty()) attrs.put(node.key, a);
        }
        out.put("attributes", attrs);
        List<Object> stages = new ArrayList<>();
        for (Object t : trackedList) stages.add(t instanceof DialogueFile ? (Object) ((DialogueFile) t).getStage() : null);
        out.put("stages", stages);
        out.put("draws", states());
        out.put("stderr", new ArrayList<>(stderr));
        return out;
    }

    static void reset() throws Exception {
        nodes = new TreeMap<>();
        values = new TreeMap<>(); answers = new TreeMap<>();
        answerCalls = new HashMap<>(); calls = new HashMap<>(); events = new ArrayList<>();
        labels = new IdentityHashMap<>(); tracked = new IdentityHashMap<>(); trackedList = new ArrayList<>();
        interactions = new ArrayList<>(); interfaces = new ArrayList<>();
        itemHandlers = new TreeMap<>(); npcHandlers = new TreeMap<>(); components = new TreeMap<>();
        loginListeners = new ArrayList<>(); persistPlayers = new ArrayList<>();
        definitions = new LinkedHashMap<>();
        stderr = new ArrayList<>();
        created = 0;
        ThreadLocalRandom.current();
        threadSeed = field(Thread.class, "threadLocalRandomSeed");
        collectionsRandom = field(Collections.class, "r");
        randomState = field(Random.class, "seed");
    }

    static String category(Throwable e) {
        if (e instanceof Failure) return "Binding";
        if (e instanceof NullPointerException) return "Null";
        if (e instanceof IndexOutOfBoundsException) return "Index";
        if (e instanceof ClassCastException) return "Cast";
        if (e instanceof NumberFormatException) return "Number";
        if (e instanceof NoSuchElementException) return "NoSuchElement";
        if (e instanceof IllegalArgumentException) return "Argument";
        if (e instanceof IllegalStateException) return "State";
        return "Other: " + e;
    }

    /** The first line of each stack trace the originals print (e.printStackTrace()) during an operation. */
    static void collectStderr() {
        for (String line : errors.toString(StandardCharsets.UTF_8).split("\\R")) {
            if (line.isEmpty() || line.startsWith("\t") || line.startsWith("WARNING")) continue;
            stderr.add(line.split(":")[0]);
        }
        errors.reset();
    }

    @SuppressWarnings("unchecked")
    public static String runCase(String line) throws Exception {
        Locale.setDefault(Locale.US);
        input = (Map) new JSONParser().parse(line);
        Map initial = new TreeMap(input);
        reset();
        for (Object d : (List) input.getOrDefault("definitions", Collections.emptyList())) {
            Map m = (Map) d;
            ItemDefinition def = new ItemDefinition(n(m.get("id")));
            def.definitionName = (String) m.get("name");
            def.equipmentSlot = m.containsKey("slot") ? (Object) n(m.get("slot")) : null;
            def.definitionExamine = (String) m.get("examine");
            definitions.put(def.definitionId, def);
        }
        Map<String, Object> seeds = new HashMap<>();
        seeds.put("random", 530L);
        seeds.put("shuffle", 531L);
        seeds.put("tlr", 532L);
        seeds.putAll((Map) input.getOrDefault("seeds", Collections.emptyMap()));
        seed(seeds);
        Map p = new TreeMap();
        p.put("kind", "player");
        p.put("name", "tracer");
        p.put("location", Arrays.asList(3222L, 3218L, 0L));
        Map ns = new TreeMap();
        ns.put("p", p);
        Map base = new TreeMap();
        base.put("nodes", ns);
        set(base);
        player = (Player) nodes.get("p");
        set(input);
        PrintStream err = System.err;
        errors = new ByteArrayOutputStream();
        List<Object> states = new ArrayList<>();
        try {
            System.setErr(new PrintStream(errors, true, "UTF-8"));
            for (Object o : (List) input.get("ops")) {
                Map op = (Map) o;
                events.clear();
                stderr.clear();
                opCalls = 0;
                failAt = op.containsKey("failAt") ? n(op.get("failAt")) : -1;
                String error = null;
                Object value = null;
                try { value = run(op); }
                catch (Throwable e) { error = category(e); }
                collectStderr();
                states.add(snapshot(value, error));
            }
        } finally {
            System.setErr(err);
        }
        initial.put("expected", states);
        return JSONValue.toJSONString(initial);
    }
}
