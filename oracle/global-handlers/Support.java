package oracle;

import core.cache.def.impl.SceneryDefinition;
import core.game.component.CloseEvent;
import core.game.component.Component;
import core.game.component.ComponentDefinition;
import core.game.component.ComponentPlugin;
import core.game.container.Container;
import core.game.container.impl.BankContainer;
import core.game.container.impl.EquipmentContainer;
import core.game.interaction.InterfaceListeners;
import core.game.interaction.Option;
import core.game.interaction.OptionHandler;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.music.MusicEntry;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.*;
import kotlin.Unit;
import kotlin.jvm.functions.Function0;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import kotlin.jvm.functions.Function3;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/** Reads one case per line, runs its operations against the original listener bodies and prints what they did. */
public class Support {
    public static class Failure extends RuntimeException {}

    static Map input;
    static List<Object> events;
    static Map<String, Integer> calls;
    static int opCalls, failAt;

    public static Map<String, Node> nodes;
    public static Player player;
    public static Map<String, Object> values, answers;
    static Map<String, Integer> answerCalls;
    public static Map<String, Container> containers;
    public static BankContainer bank;
    public static Map<Integer, MusicEntry> songs, unlocked;
    static IdentityHashMap<Object, Integer> labels, tracked;
    static List<Object> trackedList;
    static List<Object[]> interactions;

    public static final Pulse NOTHING = new Pulse() { public boolean pulse() { return true; } };

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
        if (o instanceof Node) {
            Node node = (Node) o;
            if (node.key != null) return node.key;
            return "new " + o.getClass().getSimpleName() + " " + node.getId() + " " + coord(node.getLocation());
        }
        throw new IllegalStateException("key of " + o.getClass());
    }
    public static List coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    public static Location location(Object v) {
        if (v == null) return null;
        List a = (List) v;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }
    public static Object value(Object v) {
        if (v == null || v instanceof String || v instanceof Boolean || v instanceof Integer || v instanceof Double) return v;
        if (v instanceof Long) { Map m = new TreeMap(); m.put("long", v); return m; }
        if (v instanceof Node) return key(v);
        if (v instanceof Location) { Map m = new TreeMap(); m.put("location", coord((Location) v)); return m; }
        if (v instanceof Enum) return ((Enum) v).name();
        if (v instanceof Callback) { Map m = new TreeMap(); m.put("callback", ((Callback) v).ref); return m; }
        if (v instanceof RecPlugin) { Map m = new TreeMap(); m.put("plugin", ((RecPlugin) v).ref); return m; }
        if (v instanceof Object[]) return values((Object[]) v);
        if (v instanceof List) { List l = new ArrayList(); for (Object o : (List) v) l.add(value(o)); return l; }
        if (v instanceof int[]) return values((int[]) v);
        return label(v);
    }
    public static List values(Object[] a) { if (a == null) return null; List l = new ArrayList(); for (Object o : a) l.add(value(o)); return l; }
    public static List values(int[] a) { if (a == null) return null; List l = new ArrayList(); for (int o : a) l.add(o); return l; }
    public static List keys(Item[] a) { if (a == null) return null; List l = new ArrayList(); for (Item o : a) l.add(key(o)); return l; }

    static Object attr(Object v) {
        if (v instanceof Long) return ((Long) v).intValue();
        if (v instanceof Map) {
            Map m = (Map) v;
            if (m.containsKey("callback")) return new Callback((String) m.get("callback"));
            if (m.containsKey("plugin")) return new RecPlugin((String) m.get("plugin"));
            if (m.containsKey("location")) return location(m.get("location"));
            if (m.containsKey("long")) return ((Number) m.get("long")).longValue();
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
    public static long longValue(String k, long d) { Object v = values.get(k); return v == null ? d : ((Number) v).longValue(); }
    public static String text(String k, String d) { return values.containsKey(k) ? (String) values.get(k) : d; }
    public static Node node(Object k) { return k == null ? null : nodes.get((String) k); }

    // ------------------------------------------------------------ recording objects
    /** An attribute holding a Kotlin function: the item-select callback or an experience-interface caller. */
    public static class Callback implements Function2<Object, Object, Object> {
        public final String ref;
        Callback(String ref) { this.ref = ref; }
        public Object invoke(Object a, Object b) { emit("callback", ref, value(a), value(b)); return Unit.INSTANCE; }
    }
    /** An attribute holding a plugin (the experience-interface caller). */
    public static class RecPlugin implements core.plugin.Plugin<Object> {
        public final String ref;
        RecPlugin(String ref) { this.ref = ref; }
        public core.plugin.Plugin<Object> newInstance(Object arg) { return this; }
        public Object fireEvent(String identifier, Object... args) { return null; }
        public void handleSelectionCallback(int skill, Player player) { emit("selectionCallback", ref, skill); }
    }
    /** A special attack handler: CombatTabInterface casts it to a plugin. */
    public static class SpecialHandler extends core.game.node.entity.combat.CombatSwingHandler implements core.plugin.Plugin<Object> {
        public final String ref;
        public SpecialHandler(String ref) { this.ref = ref; }
        public core.plugin.Plugin<Object> newInstance(Object arg) { return this; }
        public Object fireEvent(String identifier, Object... args) { emit("fireEvent", ref, identifier); return answer("fireEvent", null); }
        @Override public int swing(Entity entity, Entity victim, core.game.node.entity.combat.BattleState state) { emit("swing", ref, key(victim)); return 0; }
    }
    /** An item's operate handler. */
    public static class OperateHandler extends OptionHandler {
        public core.plugin.Plugin<Object> newInstance(Object arg) { return this; }
        public boolean handle(Player p, Node node, String option) { emit("operateHandle", key(node), option); return answerBool("operateHandle", false); }
    }

    // ------------------------------------------------------------ registration callbacks from the adapters
    public static void on(int[] ids, String type, String[] options, Function2 handler) {
        emit("on", values(ids), type, values(options), label(handler));
        interactions.add(new Object[] { "on", handler });
    }
    public static void onUseAnyWith(String type, int[] ids, Function3 handler) {
        emit("onUseAnyWith", type, values(ids), label(handler));
        interactions.add(new Object[] { "use", handler });
    }
    public static void defineInteraction(String type, int[] ids, String[] options, boolean persistent, int distance, Function3 handler) {
        emit("defineInteraction", type, values(ids), values(options), persistent, distance, label(handler));
        interactions.add(new Object[] { "meta", handler });
    }

    // ------------------------------------------------------------ state
    static void node(String k, Map v) {
        Node node = nodes.get(k);
        if (node == null) {
            String kind = (String) v.get("kind");
            switch (kind) {
                case "player": node = new Player(); break;
                case "bot": node = new core.game.bots.AIPlayer(); break;
                case "npc": node = new NPC(); break;
                case "banker": node = new content.global.handlers.npc.BankerNPC(); break;
                case "scenery": node = new Scenery(); break;
                case "item": node = new Item(); break;
                default: throw new IllegalArgumentException(kind);
            }
            node.key = k;
            nodes.put(k, node);
        }
        if (v.containsKey("id")) node.nodeId = n(v.get("id"));
        if (v.containsKey("name")) node.nodeName = (String) v.get("name");
        if (v.containsKey("active")) node.nodeActive = Boolean.TRUE.equals(v.get("active"));
        if (v.containsKey("location")) node.nodeLocation = location(v.get("location"));
        if (v.containsKey("amount")) ((Item) node).oracleAmount(n(v.get("amount")));
        if (node instanceof Scenery) {
            Scenery s = (Scenery) node;
            if (v.containsKey("rotation")) s.oracleRotation = n(v.get("rotation"));
            if (v.containsKey("size")) { List z = (List) v.get("size"); s.oracleSizeX = n(z.get(0)); s.oracleSizeY = n(z.get(1)); }
            if (v.containsKey("walkingFlag")) s.oracleWalkingFlag = n(v.get("walkingFlag"));
            if (v.containsKey("options")) {
                List o = (List) v.get("options");
                s.oracleOptions = o == null ? null : (String[]) o.toArray(new String[0]);
            }
        }
    }

    static void set(Map v) {
        Map ns = (Map) v.getOrDefault("nodes", Collections.emptyMap());
        for (Object k : new TreeSet(ns.keySet())) node((String) k, (Map) ns.get(k));
        Map cs = (Map) v.getOrDefault("containers", Collections.emptyMap());
        for (Object k : new TreeSet(cs.keySet())) {
            List slots = (List) cs.get(k);
            Container c = "equipment".equals(k) ? new EquipmentContainer((String) k, slots.size()) : new Container((String) k, slots.size());
            for (int i = 0; i < slots.size(); i++) c.items[i] = (Item) node(slots.get(i));
            containers.put((String) k, c);
        }
        Map as = (Map) v.getOrDefault("attributes", Collections.emptyMap());
        for (Object k : new TreeSet(as.keySet())) {
            Map attrs = (Map) as.get(k);
            Entity e = (Entity) nodes.get((String) k);
            for (Object a : attrs.keySet()) {
                Object value = attrs.get(a);
                if (value == null) e.attrs.remove(a); else e.attrs.put((String) a, attr(value));
            }
        }
        values.putAll((Map) v.getOrDefault("values", Collections.emptyMap()));
        answers.putAll((Map) v.getOrDefault("answers", Collections.emptyMap()));
        if (v.containsKey("songs")) {
            songs.clear();
            for (Object s : (List) v.get("songs")) { List l = (List) s; songs.put(n(l.get(0)), new MusicEntry(n(l.get(0)), "song", n(l.get(1)))); }
        }
        if (v.containsKey("unlocked")) {
            unlocked.clear();
            Map u = (Map) v.get("unlocked");
            for (Object s : u.keySet()) { List l = (List) u.get(s); unlocked.put(Integer.parseInt((String) s), new MusicEntry(n(l.get(0)), "song", n(l.get(1)))); }
        }
        if (v.containsKey("settings")) core.game.world.GameWorld.setSettings(Boolean.TRUE.equals(v.get("settings")) ? new core.game.world.GameSettings() : null);
        Map k = (Map) v.getOrDefault("constants", Collections.emptyMap());
        if (k.containsKey("SERVER_NAME")) core.ServerConstants.SERVER_NAME = (String) k.get("SERVER_NAME");
        if (k.containsKey("BANK_BOOTH_QUICK_OPEN")) core.ServerConstants.BANK_BOOTH_QUICK_OPEN = Boolean.TRUE.equals(k.get("BANK_BOOTH_QUICK_OPEN"));
        if (k.containsKey("BANK_BOOTH_NOTE_ENABLED")) core.ServerConstants.BANK_BOOTH_NOTE_ENABLED = Boolean.TRUE.equals(k.get("BANK_BOOTH_NOTE_ENABLED"));
        if (k.containsKey("BANK_BOOTH_NOTE_UIM")) core.ServerConstants.BANK_BOOTH_NOTE_UIM = Boolean.TRUE.equals(k.get("BANK_BOOTH_NOTE_UIM"));
    }

    static int i(Map op, String k) { return n(op.get(k)); }

    static Object instantiate(String name) throws Exception { return Class.forName(name).getDeclaredConstructor().newInstance(); }

    static OptionHandler handler(String via) {
        if (via.startsWith("option:")) {
            for (Option o : Option.oracleAll()) if (o.getName().equals(via.substring(7))) return o.getHandler();
            return null;
        }
        if (via.startsWith("scenery:")) return (OptionHandler) SceneryDefinition.OPTION_HANDLERS.get(via.substring(8));
        String[] parts = via.split(":", 3);
        return (OptionHandler) SceneryDefinition.forId(Integer.parseInt(parts[1])).getHandlers().get(parts[2]);
    }

    @SuppressWarnings("unchecked")
    static Object run(Map op) throws Throwable {
        Node node = node(op.get("node"));
        switch ((String) op.get("op")) {
            case "set": set(op); return null;
            case "define": {
                Object o = instantiate((String) op.get("class"));
                if (o instanceof core.game.interaction.InterfaceListener) ((core.game.interaction.InterfaceListener) o).defineInterfaceListeners();
                else if (o instanceof core.game.interaction.InteractionListener) ((core.game.interaction.InteractionListener) o).defineListeners();
                else ((core.plugin.Plugin<Object>) o).newInstance(null);
                return null;
            }
            case "button": return InterfaceListeners.oracleButton(player, new Component(i(op, "iface")), i(op, "opcode"), i(op, "button"), i(op, "slot"), i(op, "item"));
            case "open": return InterfaceListeners.oracleOpen(player, new Component(i(op, "iface")), false);
            case "close": return InterfaceListeners.oracleOpen(player, new Component(i(op, "iface")), true);
            case "plugin": {
                ComponentPlugin p = ComponentDefinition.forId(i(op, "iface")).getPlugin();
                if (p == null) return "none";
                return p.handle(player, new Component(i(op, "iface")), i(op, "opcode"), i(op, "button"), i(op, "slot"), i(op, "item"));
            }
            case "handler": {
                OptionHandler h = handler((String) op.get("via"));
                if (h == null) return "none";
                return h.handle(player, node, (String) op.get("option"));
            }
            case "props": {
                OptionHandler h = handler((String) op.get("via"));
                if (h == null) return "none";
                return Arrays.asList(h.isWalk(), h.isWalk(player, node), h.isDelayed(player));
            }
            case "destination": {
                OptionHandler h = handler((String) op.get("via"));
                if (h == null) return "none";
                return coord(h.getDestination(player, node));
            }
            case "interact": {
                if (i(op, "n") >= interactions.size()) return "none";
                Object[] r = interactions.get(i(op, "n"));
                Player who = op.containsKey("player") ? (Player) node(op.get("player")) : player;
                if ("on".equals(r[0])) return ((Function2) r[1]).invoke(who, node);
                if ("meta".equals(r[0])) return ((Function3) r[1]).invoke(who, node, i(op, "state"));
                return ((Function3) r[1]).invoke(who, node(op.get("used")), node(op.get("with")));
            }
            case "task": {
                if (i(op, "n") >= trackedList.size()) return "none";
                Object t = trackedList.get(i(op, "n"));
                if (t instanceof Pulse) return ((Pulse) t).pulse();
                if (t instanceof Function0) { ((Function0) t).invoke(); return null; }
                if (t instanceof CloseEvent) return ((CloseEvent) t).close(player, new Component(i(op, "iface")));
                if (t instanceof Function1) { ((Function1) t).invoke(op.get("value")); return null; }
                throw new IllegalStateException(t.getClass().getName());
            }
            case "stop": {
                if (i(op, "n") >= trackedList.size()) return "none";
                ((Pulse) trackedList.get(i(op, "n"))).stop();
                return null;
            }
            case "dialogue": {
                if (i(op, "n") >= trackedList.size()) return "none";
                return ((core.game.dialogue.DialoguePlugin) trackedList.get(i(op, "n"))).handle(0, i(op, "button"));
            }
            case "login": new content.global.handlers.player.LoginValidationPlugin().newInstance(player); return null;
            case "friends": new content.global.handlers.iface.FriendsListInterface().open(player); return null;
            case "rules": content.global.handlers.iface.RulesAndInfo.openFor(player); return null;
            case "rules_base": return content.global.handlers.iface.RulesAndInfo.setBaseRulesAndInfo(player);
            case "scroll": {
                List lines = (List) op.get("lines");
                content.global.handlers.iface.ScrollLine[] contents = new content.global.handlers.iface.ScrollLine[lines.size()];
                for (int x = 0; x < contents.length; x++) {
                    List l = (List) lines.get(x);
                    contents[x] = new content.global.handlers.iface.ScrollLine((String) l.get(0), n(l.get(1)));
                }
                content.global.handlers.iface.ScrollInterface.Companion.scrollSetup(player, i(op, "component"), contents);
                return null;
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
        out.put("values", new TreeMap<>(values));
        return out;
    }

    static void reset() {
        nodes = new TreeMap<>();
        values = new TreeMap<>(); answers = new TreeMap<>();
        answerCalls = new HashMap<>(); calls = new HashMap<>(); events = new ArrayList<>();
        containers = new TreeMap<>();
        containers.put("inventory", new Container("inventory", 28));
        containers.put("equipment", new EquipmentContainer("equipment", 14));
        containers.put("kept", new Container("kept", 4));
        containers.put("lost", new Container("lost", 4));
        containers.put("bob", new Container("bob", 4));
        bank = new BankContainer();
        songs = new LinkedHashMap<>();
        unlocked = new TreeMap<Integer, MusicEntry>() {
            @Override public MusicEntry get(Object k) { emit("unlocked", k); return super.get(k); }
        };
        labels = new IdentityHashMap<>(); tracked = new IdentityHashMap<>(); trackedList = new ArrayList<>();
        interactions = new ArrayList<>();
        InterfaceListeners.oracleReset();
        ComponentDefinition.DEFINITIONS.clear();
        SceneryDefinition.DEFINITIONS.clear();
        SceneryDefinition.OPTION_HANDLERS.clear();
        for (Option o : Option.oracleAll()) o.oracleClear();
        core.game.world.GameWorld.setSettings(new core.game.world.GameSettings());
        core.ServerConstants.SERVER_NAME = "2009scape";
        core.ServerConstants.BANK_BOOTH_QUICK_OPEN = false;
        core.ServerConstants.BANK_BOOTH_NOTE_ENABLED = true;
        core.ServerConstants.BANK_BOOTH_NOTE_UIM = true;
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
