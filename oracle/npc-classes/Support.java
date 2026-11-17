package oracle;

import content.global.ame.RandomEventNPC;
import content.global.handlers.npc.BankerNPC;
import core.game.interaction.OptionHandler;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.node.entity.npc.AbstractNPC;
import core.game.node.entity.npc.IdleAbstractNPC;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import core.game.worldevents.holiday.HolidayRandomEventNPC;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.*;
import kotlin.jvm.functions.Function2;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/** Reads one case per line, runs its operations against the original NPC classes and listeners, prints what they did. */
public class Support {
    public static class Failure extends RuntimeException {}

    static Map input;
    static List<Object> events;
    static Map<String, Integer> calls;
    static int opCalls, failAt;
    static boolean quiet;

    public static Map<String, Node> nodes;
    public static Player player;
    public static Map<String, Object> values, answers;
    static Map<String, Integer> answerCalls;
    static IdentityHashMap<Object, Integer> labels;
    static List<Function2> interactions;
    static List<Function2> destinations;
    public static Map<String, OptionHandler> handlers;
    static long now;

    static int n(Object o) { return ((Number) o).intValue(); }

    public static void emit(String name, Object... args) {
        if (quiet) return;
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

    // ------------------------------------------------------------ labels and values as the events show them
    public static String label(Object o) {
        if (o == null) return null;
        Integer i = labels.get(o);
        if (i == null) labels.put(o, i = labels.size() + 1);
        return o.getClass().getName() + "#" + i;
    }
    public static String handler(Object o) { return label(o); }
    public static Object key(Object o) {
        if (o == null) return null;
        if (o instanceof Node) {
            Node node = (Node) o;
            if (node.key != null) return node.key;
            return label(o);
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
        if (v instanceof Long) {
            long l = (Long) v;
            Map m = new TreeMap();
            // The clock the original reads itself: recorded relative to it, in whole seconds.
            if (Math.abs(l - System.currentTimeMillis()) < 86_400_000L) m.put("clockPlusSeconds", Math.round((l - System.currentTimeMillis()) / 1000.0));
            else m.put("long", v);
            return m;
        }
        if (v instanceof Item) { Map m = new TreeMap(); m.put("item", Arrays.asList(((Item) v).getId(), ((Item) v).oracleAmount)); return m; }
        if (v instanceof Node) return key(v);
        if (v instanceof Location) { Map m = new TreeMap(); m.put("location", coord((Location) v)); return m; }
        if (v instanceof Enum) return ((Enum) v).name();
        if (v instanceof Object[]) return values((Object[]) v);
        if (v instanceof List) { List l = new ArrayList(); for (Object o : (List) v) l.add(value(o)); return l; }
        if (v instanceof int[]) return values((int[]) v);
        return label(v);
    }
    public static List values(Object[] a) { if (a == null) return null; List l = new ArrayList(); for (Object o : a) l.add(value(o)); return l; }
    public static List values(int[] a) { if (a == null) return null; List l = new ArrayList(); for (int o : a) l.add(o); return l; }

    static Object attr(Object v) {
        if (v instanceof Long) return ((Long) v).intValue();
        if (v instanceof Map) {
            Map m = (Map) v;
            if (m.containsKey("location")) return location(m.get("location"));
            if (m.containsKey("long")) return ((Number) m.get("long")).longValue();
            if (m.containsKey("clockPlusSeconds")) return System.currentTimeMillis() + ((Number) m.get("clockPlusSeconds")).longValue() * 1000L;
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
    public static Node node(Object k) { return k == null ? null : nodes.get((String) k); }
    public static List<Integer> ints(String name) {
        List<Integer> out = new ArrayList<>();
        for (Object o : (List) values.getOrDefault(name, Collections.emptyList())) out.add(n(o));
        return out;
    }

    // ------------------------------------------------------------ registration callbacks from the adapters
    public static void on(int[] ids, String type, String[] options, Function2 handler) {
        emit("on", values(ids), type, values(options), label(handler));
        interactions.add(handler);
    }
    public static void setDest(String type, int[] ids, String[] options, Function2 handler) {
        emit("setDest", type, values(ids), values(options), label(handler));
        destinations.add(handler);
    }

    // ------------------------------------------------------------ state
    static void node(String k, Map v) {
        Node node = nodes.get(k);
        if (node == null) {
            String kind = (String) v.get("kind");
            quiet = true;
            try {
                switch (kind) {
                    case "player": node = new Player(); break;
                    case "npc": node = new NPC(); break;
                    case "randomNpc": node = new RandomEventNPC(n(v.getOrDefault("id", 0L))); break;
                    case "holidayNpc": node = new HolidayRandomEventNPC(n(v.getOrDefault("id", 0L))); break;
                    case "idle": node = new IdleProbe(n(v.get("id")), n(v.get("activeId")), location(v.get("location"))); break;
                    case "scenery": node = new Scenery(); break;
                    case "item": node = new Item(n(v.get("id"))); break;
                    default: throw new IllegalArgumentException(kind);
                }
            } finally { quiet = false; }
            node.key = k;
            nodes.put(k, node);
        }
        if (v.containsKey("id")) node.nodeId = n(v.get("id"));
        if (v.containsKey("location")) node.nodeLocation = location(v.get("location"));
        if (node instanceof Entity && v.containsKey("direction")) ((Entity) node).oracleDirection = v.get("direction") == null ? null : Direction.valueOf((String) v.get("direction"));
        if (node instanceof IdleProbe) {
            if (v.containsKey("idle")) ((IdleProbe) node).setIdle(Boolean.TRUE.equals(v.get("idle")));
            if (v.containsKey("timeout")) ((IdleProbe) node).oracleSetTimeout(n(v.get("timeout")));
        }
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
                if (value == null) e.attrs.remove(a); else e.attrs.put((String) a, attr(value));
            }
        }
        values.putAll((Map) v.getOrDefault("values", Collections.emptyMap()));
        answers.putAll((Map) v.getOrDefault("answers", Collections.emptyMap()));
        Map k = (Map) v.getOrDefault("constants", Collections.emptyMap());
        if (k.containsKey("SECOND_BANK")) core.ServerConstants.SECOND_BANK = Boolean.TRUE.equals(k.get("SECOND_BANK"));
    }

    static int i(Map op, String k) { return n(op.get(k)); }

    static Object instantiate(String name) throws Throwable {
        try { return Class.forName(name).getDeclaredConstructor().newInstance(); }
        catch (java.lang.reflect.InvocationTargetException e) { throw e.getCause(); }
    }

    static NPC npc(Map op, String k) { return (NPC) node(op.get(k)); }
    static Entity entity(Map op, String k) { return (Entity) node(op.get(k)); }

    @SuppressWarnings("unchecked")
    static Object run(Map op) throws Throwable {
        switch ((String) op.get("op")) {
            case "set": set(op); return null;
            // ClassScanner.loadContentInterfaces: defineListeners, then defineDestinationOverrides.
            case "listeners": {
                core.game.interaction.InteractionListener l = (core.game.interaction.InteractionListener) instantiate((String) op.get("class"));
                l.defineListeners();
                l.defineDestinationOverrides();
                return null;
            }
            // ClassScanner.definePlugin with no manifest: newInstance(null).
            case "plugin": {
                Object o = instantiate((String) op.get("class"));
                ((core.plugin.Plugin<Object>) o).newInstance(null);
                return null;
            }
            case "forId": return label(AbstractNPC.forId(i(op, "id")));
            case "create": {
                NPC created = op.containsKey("direction")
                        ? NPC.create(i(op, "id"), location(op.get("location")), Direction.valueOf((String) op.get("direction")))
                        : NPC.create(i(op, "id"), location(op.get("location")));
                String result = label(created);
                created.key = (String) op.get("key");
                nodes.put(created.key, created);
                return result;
            }
            case "init": npc(op, "node").init(); return null;
            case "tick": npc(op, "node").tick(); return null;
            case "tickActions": npc(op, "node").handleTickActions(); return null;
            case "death": npc(op, "node").finalizeDeath(entity(op, "killer")); return null;
            case "attacked": npc(op, "node").onAttack(entity(op, "target")); return null;
            case "impact": npc(op, "node").onImpact(entity(op, "entity"), new BattleState()); return null;
            case "swing": {
                CombatSwingHandler h = npc(op, "node").getSwingHandler(true);
                return Arrays.asList(label(h), h.swing(npc(op, "node"), entity(op, "victim"), new BattleState()));
            }
            case "radius": return npc(op, "node").getWalkRadius();
            case "interact": {
                if (i(op, "n") >= interactions.size()) return "none";
                Player who = op.containsKey("player") ? (Player) node(op.get("player")) : player;
                return interactions.get(i(op, "n")).invoke(who, node(op.get("node")));
            }
            case "dest": {
                if (i(op, "n") >= destinations.size()) return "none";
                return coord((Location) destinations.get(i(op, "n")).invoke(node(op.get("entity")), node(op.get("node"))));
            }
            case "handler": {
                OptionHandler h = handlers.get((String) op.get("via"));
                if (h == null) return "none";
                return h.handle(player, node(op.get("node")), (String) op.get("option"));
            }
            case "isWalk": {
                OptionHandler h = handlers.get((String) op.get("via"));
                if (h == null) return "none";
                return Arrays.asList(h.isWalk(), h.isWalk(player, node(op.get("node"))));
            }
            case "idle": {
                IdleAbstractNPC idle = (IdleAbstractNPC) node(op.get("node"));
                Entity other = entity(op, "entity");
                switch ((String) op.get("call")) {
                    case "isAttackable": return idle.isAttackable(other, op.get("style") == null ? null : CombatStyle.valueOf((String) op.get("style")), Boolean.TRUE.equals(op.get("message")));
                    case "inDisturbingRange": return idle.inDisturbingRange(other);
                    case "canDisturb": return idle.canDisturb(other);
                    case "disturb": idle.disturb(other); return null;
                    case "goIdle": idle.goIdle(); return null;
                    default: throw new IllegalArgumentException((String) op.get("call"));
                }
            }
            case "bank": return BankerNPC.Companion.attemptBank(player, node(op.get("node")));
            case "collect": return BankerNPC.Companion.attemptCollect(player, node(op.get("node")));
            case "lunar": return BankerNPC.Companion.checkLunarIsleRestriction(player, node(op.get("node")));
            case "access": return BankerNPC.Companion.canAccessBank(player, (NPC) node(op.get("node")));
            default: throw new IllegalArgumentException((String) op.get("op"));
        }
    }

    static Map<String, Object> snapshot(Object result, String error) {
        Map<String, Object> out = new TreeMap<>();
        out.put("error", error);
        out.put("result", value(result));
        out.put("events", new ArrayList<>(events));
        Map<String, Object> attrs = new TreeMap<>();
        Map<String, Object> npcs = new TreeMap<>();
        for (Node node : nodes.values()) {
            if (!(node instanceof Entity)) continue;
            Map<String, Object> a = new TreeMap<>();
            for (Map.Entry<String, Object> e : ((Entity) node).attrs.entrySet()) a.put(e.getKey(), value(e.getValue()));
            if (!a.isEmpty()) attrs.put(node.key, a);
            if (node instanceof NPC) {
                NPC n = (NPC) node;
                Map<String, Object> s = new TreeMap<>();
                s.put("class", n.getClass().getName());
                s.put("id", n.nodeId);
                s.put("walks", n.oracleWalks);
                s.put("aggressive", n.oracleAggressive);
                s.put("direction", n.oracleDirection == null ? null : n.oracleDirection.name());
                if (n instanceof IdleProbe) {
                    s.put("idle", ((IdleProbe) n).isIdle());
                    s.put("timeout", ((IdleProbe) n).oracleTimeout());
                }
                npcs.put(node.key, s);
            }
        }
        out.put("attributes", attrs);
        out.put("npcs", npcs);
        return out;
    }

    static void reset() {
        nodes = new TreeMap<>();
        values = new TreeMap<>(); answers = new TreeMap<>();
        answerCalls = new HashMap<>(); calls = new HashMap<>(); events = new ArrayList<>();
        labels = new IdentityHashMap<>();
        interactions = new ArrayList<>();
        destinations = new ArrayList<>();
        handlers = new TreeMap<>();
        core.game.node.entity.npc.OracleRegistry.reset();
        core.ServerConstants.SECOND_BANK = true;
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
