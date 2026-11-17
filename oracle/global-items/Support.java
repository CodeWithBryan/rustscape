package oracle;

import core.cache.def.impl.ItemDefinition;
import core.game.container.Container;
import core.game.dialogue.DialogueAction;
import core.game.dialogue.DialogueFile;
import core.game.dialogue.DialogueInterpreter;
import core.game.dialogue.DialoguePlugin;
import core.game.interaction.OptionHandler;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.*;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import kotlin.jvm.functions.Function3;
import org.json.simple.JSONObject;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/** Reads one case per line, runs its operations against the original item handlers and prints what they did. */
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
    public static Map<String, Map<String, Object>> stores;
    static IdentityHashMap<Object, Integer> labels, tracked;
    static List<Object> trackedList;
    static List<Object[]> interactions;
    public static Map<String, Object> itemHandlers;
    public static Map<Integer, DialoguePlugin> dialogues;

    static int n(Object o) { return ((Number) o).intValue(); }

    public static void emit(String name, Object... args) {
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
        if (v instanceof Long) {
            // Times near the clock the original reads itself are recorded relative to it, in seconds.
            long now = System.currentTimeMillis(), l = (Long) v;
            Map m = new TreeMap();
            if (Math.abs(l - now) < 864000000L) m.put("relative", Math.round((l - now) / 1000.0)); else m.put("long", v);
            return m;
        }
        if (v instanceof Double) { Map m = new TreeMap(); m.put("double", v); return m; }
        if (v instanceof Float) { Map m = new TreeMap(); m.put("float", (double) (Float) v); return m; }
        if (v instanceof Location) { Map m = new TreeMap(); m.put("location", coord((Location) v)); return m; }
        if (v instanceof Node) return key(v);
        if (v instanceof Enum) return ((Enum) v).name();
        if (v instanceof Object[]) { List l = new ArrayList(); for (Object o : (Object[]) v) l.add(value(o)); return l; }
        if (v instanceof List) { List l = new ArrayList(); for (Object o : (List) v) l.add(value(o)); return l; }
        return label(v);
    }
    public static List values(Object[] a) { if (a == null) return null; List l = new ArrayList(); for (Object o : a) l.add(value(o)); return l; }
    public static List values(int[] a) { if (a == null) return null; List l = new ArrayList(); for (int o : a) l.add(o); return l; }

    /** A case value as the original stores it: {"long"/"double"/"float"} maps keep their boxed type. */
    static Object typed(Object v) {
        if (v instanceof Long) return ((Long) v).intValue();
        if (v instanceof Map) {
            Map m = (Map) v;
            if (m.containsKey("location")) return location(m.get("location"));
            if (m.containsKey("long")) return ((Number) m.get("long")).longValue();
            if (m.containsKey("relative")) return System.currentTimeMillis() + ((Number) m.get("relative")).longValue() * 1000L;
            if (m.containsKey("double")) return ((Number) m.get("double")).doubleValue();
            if (m.containsKey("float")) return ((Number) m.get("float")).floatValue();
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

    // ------------------------------------------------------------ objects the originals construct
    /** {@code new Item(id, amount, charge)}: the definition lookup is the recorded call. */
    public static void newItem(Item item, int id, int amount) {
        emit("newItem", id, amount);
        item.key = "new:" + id + ":" + (++created);
        item.nodeName = (String) answer("itemName:" + id, null);
        nodes.put(item.key, item);
    }

    /** A daily store archive (ServerStore.getArchive): reads and writes are recorded. */
    public static class Archive extends JSONObject {
        final String name;
        Archive(String name) { this.name = name; }
        @Override public Object get(Object k) { emit("storeGet", name, k); return stores.get(name).get(k); }
        @SuppressWarnings("unchecked")
        @Override public Object put(Object k, Object v) { emit("storePut", name, k, value(v)); return stores.get(name).put((String) k, v); }
    }
    public static JSONObject archive(String name) {
        emit("storeArchive", name);
        stores.computeIfAbsent(name, x -> new TreeMap<>());
        return new Archive(name);
    }

    // ------------------------------------------------------------ registration callbacks from the adapters
    public static void on(int[] ids, String type, String[] options, Function2 handler) {
        emit("on", values(ids), type, values(options), label(handler));
        interactions.add(new Object[] { "on", handler });
    }
    public static void onUseWith(String type, int used, int[] with, Function3 handler) {
        emit("onUseWith", type, used, values(with), label(handler));
        interactions.add(new Object[] { "use", handler });
    }
    public static void itemHandler(int id, String k, Object handler) {
        emit("itemHandler", id, k, label(handler));
        itemHandlers.put(id + ":" + k, handler);
    }
    public static void dialogueAdd(int id, DialoguePlugin plugin) {
        emit("dialogueAdd", id, label(plugin));
        dialogues.put(id, plugin);
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
        Map ss = (Map) v.getOrDefault("stores", Collections.emptyMap());
        for (Object k : new TreeSet(ss.keySet())) {
            Map<String, Object> store = stores.computeIfAbsent((String) k, x -> new TreeMap<>());
            Map entries = (Map) ss.get(k);
            for (Object e : entries.keySet()) {
                Object value = entries.get(e);
                if (value == null) store.remove(e); else store.put((String) e, typed(value));
            }
        }
        values.putAll((Map) v.getOrDefault("values", Collections.emptyMap()));
        answers.putAll((Map) v.getOrDefault("answers", Collections.emptyMap()));
    }

    static int i(Map op, String k) { return n(op.get(k)); }

    static Object instantiate(String name) throws Exception { return Class.forName(name).getDeclaredConstructor().newInstance(); }

    static Object[] args(Map op) {
        List l = (List) op.getOrDefault("args", Collections.emptyList());
        Object[] out = new Object[l.size()];
        for (int x = 0; x < out.length; x++) {
            Object a = l.get(x);
            out[x] = a instanceof String && nodes.containsKey(a) ? nodes.get(a) : typed(a);
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    static Object run(Map op) throws Throwable {
        Node node = node(op.get("node"));
        switch ((String) op.get("op")) {
            case "set": set(op); return null;
            case "define": {
                Object o = instantiate((String) op.get("class"));
                if (o instanceof core.game.interaction.InteractionListener) ((core.game.interaction.InteractionListener) o).defineListeners();
                else ((core.plugin.Plugin<Object>) o).newInstance(null);
                return null;
            }
            case "interact": {
                if (i(op, "n") >= interactions.size()) return "none";
                Object[] r = interactions.get(i(op, "n"));
                if ("on".equals(r[0])) return ((Function2) r[1]).invoke(player, node);
                return ((Function3) r[1]).invoke(player, node(op.get("used")), node(op.get("with")));
            }
            case "handler": {
                OptionHandler h = (OptionHandler) itemHandlers.get((String) op.get("via"));
                if (h == null) return "none";
                return h.handle(player, node, (String) op.get("option"));
            }
            case "props": {
                OptionHandler h = (OptionHandler) itemHandlers.get((String) op.get("via"));
                if (h == null) return "none";
                return Arrays.asList(h.isWalk(), h.isWalk(player, node), h.isDelayed(player));
            }
            case "dialogueOpen": {
                DialoguePlugin p = dialogues.get(i(op, "key"));
                if (p == null) return "none";
                DialoguePlugin d = p.newInstance(player);
                track(d);
                return d.open(args(op));
            }
            case "dialogue": {
                if (i(op, "n") >= trackedList.size()) return "none";
                Object d = trackedList.get(i(op, "n"));
                if (d instanceof DialogueFile) { ((DialogueFile) d).handle(i(op, "component"), i(op, "button")); return null; }
                return ((DialoguePlugin) d).handle(i(op, "component"), i(op, "button"));
            }
            case "task": {
                if (i(op, "n") >= trackedList.size()) return "none";
                Object t = trackedList.get(i(op, "n"));
                if (t instanceof Pulse) return ((Pulse) t).pulse();
                if (t instanceof DialogueAction) { ((DialogueAction) t).handle(player, i(op, "button")); return null; }
                if (t instanceof Function2) { ((Function2) t).invoke(i(op, "slot"), i(op, "option")); return null; }
                if (t instanceof Function1) return ((Function1) t).invoke(i(op, "stage"));
                throw new IllegalStateException(t.getClass().getName());
            }
            default: throw new IllegalArgumentException((String) op.get("op"));
        }
    }

    static Map<String, Object> snapshot(Object result, String error) {
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
        Map<String, Object> st = new TreeMap<>();
        for (Map.Entry<String, Map<String, Object>> s : stores.entrySet()) {
            Map<String, Object> m = new TreeMap<>();
            for (Map.Entry<String, Object> e : s.getValue().entrySet()) m.put(e.getKey(), value(e.getValue()));
            st.put(s.getKey(), m);
        }
        out.put("stores", st);
        List<Object> stages = new ArrayList<>();
        for (Object t : trackedList) {
            if (t instanceof DialogueFile) stages.add(((DialogueFile) t).getStage());
            else if (t instanceof DialoguePlugin) stages.add(((DialoguePlugin) t).oracleStage());
            else stages.add(null);
        }
        out.put("stages", stages);
        return out;
    }

    static void reset() {
        nodes = new TreeMap<>();
        values = new TreeMap<>(); answers = new TreeMap<>();
        answerCalls = new HashMap<>(); calls = new HashMap<>(); events = new ArrayList<>();
        stores = new TreeMap<>();
        labels = new IdentityHashMap<>(); tracked = new IdentityHashMap<>(); trackedList = new ArrayList<>();
        interactions = new ArrayList<>();
        itemHandlers = new TreeMap<>();
        dialogues = new TreeMap<>();
        created = 0;
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.US);
        try (BufferedReader r = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            String line;
            while ((line = r.readLine()) != null) {
                input = (Map) new JSONParser().parse(line);
                Map initial = new TreeMap(input);
                reset();
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
                List<Object> states = new ArrayList<>();
                for (Object o : (List) input.get("ops")) {
                    Map op = (Map) o;
                    events.clear();
                    opCalls = 0;
                    failAt = op.containsKey("failAt") ? n(op.get("failAt")) : -1;
                    String error = null;
                    Object value = null;
                    try { value = run(op); }
                    catch (Failure e) { error = "Binding"; }
                    catch (NullPointerException e) { error = "Null"; }
                    catch (IndexOutOfBoundsException e) { error = "Index"; }
                    catch (ClassCastException e) { error = "Cast"; }
                    catch (Throwable e) { error = "Other: " + e; }
                    states.add(snapshot(value, error));
                }
                initial.put("expected", states);
                System.out.println(JSONValue.toJSONString(initial));
            }
        }
    }
}
