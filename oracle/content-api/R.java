package oracle;

import core.game.node.Node;
import core.game.world.map.Location;
import java.util.*;

/**
 * The recorder every adapter calls. A call is recorded as {@code [name, args...]}; its answer comes from the case's
 * {@code answers} (a list per name, consumed in order; the last element repeats; an absent name answers null).
 * {@code failAt} makes the n-th recorded call of the operation throw {@link Failure} after it is recorded.
 */
public final class R {
    public static final class Failure extends RuntimeException {
        public Failure() { super("injected"); }
    }

    public static List<Object> events = new ArrayList<>();
    public static Map<String, Object> answers = new HashMap<>();
    static Map<String, Integer> answerCalls = new HashMap<>();
    public static int calls, failAt;
    public static Map<String, Object> nodes = new LinkedHashMap<>();
    static Map<String, Integer> sequence = new HashMap<>();
    public static IdentityHashMap<Object, String> labels = new IdentityHashMap<>();
    public static Map<String, core.game.container.Container> containers = new HashMap<>();

    public static void reset() {
        events = new ArrayList<>();
        answerCalls = new HashMap<>();
        calls = 0;
    }

    /** A new label for an object the originals construct ({@code kind#n}). */
    public static String fresh(Object o, String kind) {
        int n = sequence.merge(kind, 1, Integer::sum);
        String key = kind + "#" + n;
        labels.put(o, key);
        nodes.put(key, o);
        return key;
    }

    public static String label(Object o) {
        if (o == null) return null;
        if (o instanceof Node && ((Node) o).key != null) return ((Node) o).key;
        if (o instanceof core.game.container.Container && ((core.game.container.Container) o).key != null) return ((core.game.container.Container) o).key;
        String l = labels.get(o);
        if (l != null) return l;
        return fresh(o, o instanceof kotlin.Function ? "fn" : o.getClass().getSimpleName());
    }

    public static Object value(Object v) {
        if (v == null || v instanceof String || v instanceof Boolean || v instanceof Integer) return v;
        if (v instanceof Location) {
            Location l = (Location) v;
            Map<String, Object> m = new TreeMap<>();
            m.put("loc", Arrays.asList(l.getX(), l.getY(), l.getZ()));
            return m;
        }
        if (v instanceof Short) { Map<String, Object> m = new TreeMap<>(); m.put("short", (int) (Short) v); return m; }
        if (v instanceof Double) { Map<String, Object> m = new TreeMap<>(); m.put("double", v); return m; }
        if (v instanceof Long) { Map<String, Object> m = new TreeMap<>(); m.put("long", v); return m; }
        if (v instanceof Enum) return ((Enum<?>) v).name();
        if (v instanceof int[]) { List<Object> l = new ArrayList<>(); for (int x : (int[]) v) l.add(x); return l; }
        if (v instanceof Object[]) { List<Object> l = new ArrayList<>(); for (Object x : (Object[]) v) l.add(value(x)); return l; }
        if (v instanceof List) { List<Object> l = new ArrayList<>(); for (Object x : (List<?>) v) l.add(value(x)); return l; }
        if (v instanceof Map) { Map<Object, Object> m = new TreeMap<>(); for (Map.Entry<?, ?> e : ((Map<?, ?>) v).entrySet()) m.put(e.getKey(), value(e.getValue())); return m; }
        if (v instanceof Describable) return value(((Describable) v).describe());
        if (v.getClass().getSimpleName().equals("ForceMoveCtx")) return v.toString();
        return label(v);
    }

    /** Objects that record as a value of their own (animations, graphics, components, contexts). */
    public interface Describable { Object describe(); }

    public static Object ev(String name, Object... args) {
        if (initialising()) return null;
        List<Object> row = new ArrayList<>();
        row.add(name);
        for (Object a : args) row.add(value(a));
        events.add(row);
        calls++;
        if (calls == failAt) throw new Failure();
        return answer(name);
    }

    static boolean initialising() {
        for (StackTraceElement e : new Throwable().getStackTrace()) if ("<clinit>".equals(e.getMethodName())) return true;
        return false;
    }

    public static Object answer(String name) {
        Object v = answers.get(name);
        if (v instanceof List) {
            List<?> l = (List<?>) v;
            if (l.isEmpty()) return null;
            int i = answerCalls.merge(name, 1, Integer::sum) - 1;
            v = l.get(Math.min(i, l.size() - 1));
        }
        return v;
    }

    public static void v(String name, Object... args) { ev(name, args); }
    public static int i(String name, Object... args) { Object a = ev(name, args); return a == null ? 0 : ((Number) a).intValue(); }
    public static boolean b(String name, Object... args) { return Boolean.TRUE.equals(ev(name, args)); }
    public static String s(String name, Object... args) { return (String) ev(name, args); }
    public static double d(String name, Object... args) { Object a = ev(name, args); return a == null ? 0 : ((Number) a).doubleValue(); }
    /** A node answer (its key), or null. */
    @SuppressWarnings("unchecked")
    public static <T> T n(String name, Object... args) { return (T) node(ev(name, args)); }
    /** A list of nodes answer. */
    @SuppressWarnings("unchecked")
    public static <T> List<T> list(String name, Object... args) {
        Object a = ev(name, args);
        List<T> out = new ArrayList<>();
        if (a == null) return out;
        for (Object k : (List<?>) a) out.add((T) node(k));
        return out;
    }
    public static Object node(Object key) {
        if (key == null) return null;
        if (key instanceof Map && ((Map<?, ?>) key).containsKey("loc")) {
            List<?> l = (List<?>) ((Map<?, ?>) key).get("loc");
            return Location.create(((Number) l.get(0)).intValue(), ((Number) l.get(1)).intValue(), ((Number) l.get(2)).intValue());
        }
        Object o = nodes.get((String) key);
        if (o == null) throw new IllegalStateException("no node " + key);
        return o;
    }
    public static int n(Object o) { return ((Number) o).intValue(); }

    // ------------------------------------------------------------ typed answers and case state
    /** A stored value as the case gives it: ints, {"long":n}, {"double":d}, {"node":key}, {"loc":[x,y,z]}, text, booleans. */
    public static Object typed(Object a) {
        if (a instanceof Long) return ((Long) a).intValue();
        if (a instanceof Map) {
            Map<?, ?> m = (Map<?, ?>) a;
            if (m.containsKey("long")) return ((Number) m.get("long")).longValue();
            if (m.containsKey("double")) return ((Number) m.get("double")).doubleValue();
            if (m.containsKey("node")) return node(m.get("node"));
            if (m.containsKey("loc")) return node(m);
        }
        return a;
    }

    public static Map<Integer, core.cache.def.impl.ItemDefinition> items = new HashMap<>();
    public static Map<Integer, core.cache.def.impl.VarbitDefinition> varbits = new HashMap<>();
    public static List<Object> pulses = new ArrayList<>(), scripts = new ArrayList<>(), handlers = new ArrayList<>();
    public static Map<String, core.game.node.entity.player.link.quest.Quest> quests = new HashMap<>();
    public static Map<Object, core.game.ge.GrandExchangeRecords> records = new IdentityHashMap<>();
    public static Map<Object, content.global.skill.slayer.SlayerManager> slayers = new IdentityHashMap<>();

    public static core.cache.def.impl.ItemDefinition itemDefinition(int id) { return items.get(id); }
    public static core.cache.def.impl.VarbitDefinition varbit(int id) { return varbits.get(id); }

    /** {@code new Item(id, amount, charge)}: the definition lookup is the recorded call; a missing one fails as the original's. */
    public static String newItem(int id, int amount, int charge) {
        ev("Item", id, amount, charge);
        core.cache.def.impl.ItemDefinition d = itemDefinition(id);
        if (d == null) throw new NullPointerException("definition");
        return d.getName();
    }

    public static int[] ints(String name, Object... args) {
        Object a = ev(name, args);
        if (a == null) return null;
        List<?> l = (List<?>) a;
        int[] out = new int[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = ((Number) l.get(i)).intValue();
        return out;
    }

    public static void pulse(String name, Object owner, core.game.system.task.Pulse pulse) {
        String l = fresh(pulse, "pulse");
        pulses.add(pulse);
        ev(name, owner, l, pulse.getDelay());
    }

    public static void script(Object entity, core.game.interaction.Script<?> script, Object strength) {
        String l = fresh(script, "script");
        scripts.add(script);
        ev("ScriptProcessor.addToQueue", entity, l, strength, script.getNextExecution(), script.getPersist());
    }

    public static void handler(String name, Object handler, Object player, Object type, Object items) {
        String l = fresh(handler, "handler");
        handlers.add(handler);
        ev(name, l, player, type, items);
    }

    public static core.game.node.entity.player.link.quest.Quest quest(String key) {
        if (key == null) return null;
        return quests.computeIfAbsent(key, k -> { core.game.node.entity.player.link.quest.Quest q = new core.game.node.entity.player.link.quest.Quest(); q.key = k; return q; });
    }

    public static Map<content.data.Quests, core.game.node.entity.player.link.quest.Quest> quests(Object answer) {
        Map<content.data.Quests, core.game.node.entity.player.link.quest.Quest> m = new LinkedHashMap<>();
        if (answer != null) for (Map.Entry<?, ?> e : ((Map<?, ?>) answer).entrySet()) m.put(content.data.Quests.valueOf((String) e.getKey()), quest((String) e.getValue()));
        return m;
    }

    public static core.game.world.map.zone.ZoneBorders borders(Object answer) {
        if (answer == null) return null;
        List<?> l = (List<?>) answer;
        return new core.game.world.map.zone.ZoneBorders(n(l.get(0)), n(l.get(1)), n(l.get(2)), n(l.get(3)), n(l.get(4)));
    }

    public static content.data.consumables.Consumables consumables(int itemId) {
        Object a = ev("Consumables.getConsumableById", itemId);
        if (a == null) return null;
        content.data.consumables.Consumables c = new content.data.consumables.Consumables();
        c.consumable = (core.game.consumable.Consumable) nodes.get((String) a);
        return c;
    }

    public static core.game.ge.GrandExchangeRecords records(Object player) {
        ev("GrandExchangeRecords.getInstance", player);
        core.game.ge.GrandExchangeRecords r = records.get(player);
        if (r == null) throw new IllegalStateException("no records");
        return r;
    }

    public static content.global.skill.slayer.SlayerManager slayer(core.game.node.entity.player.Player player) {
        ev("SlayerManager.getInstance", player);
        return slayers.computeIfAbsent(player, p -> new content.global.skill.slayer.SlayerManager(player));
    }

    public static content.global.skill.construction.decoration.pohstorage.StorageInterface.BookcaseBitfields.BookcaseBits bits(Object answer) {
        if (answer == null) throw new NullPointerException("bits");
        List<?> l = (List<?>) answer;
        return new content.global.skill.construction.decoration.pohstorage.StorageInterface.BookcaseBitfields.BookcaseBits(((Number) l.get(0)).longValue(), ((Number) l.get(1)).longValue());
    }

    public static kotlin.Pair<Boolean, List<core.game.requirement.Requirement>> evaluate(Object requirement, Object player) {
        Map<?, ?> a = (Map<?, ?>) ev("Requirement.evaluate", requirement, player);
        List<core.game.requirement.Requirement> unmet = new ArrayList<>();
        if (a == null) throw new NullPointerException("evaluate");
        for (Object o : (List<?>) a.get("unmet")) {
            List<?> r = (List<?>) o;
            switch ((String) r.get(0)) {
                case "QPCumulative": unmet.add(new core.game.requirement.QPCumulative(n(r.get(1)))); break;
                case "QPReq": unmet.add(new core.game.requirement.QPReq(n(r.get(1)))); break;
                case "QuestReq": unmet.add(new core.game.requirement.QuestReq(core.game.requirement.QuestRequirements.valueOf((String) r.get(1)), n(r.get(2)))); break;
                case "SkillReq": unmet.add(new core.game.requirement.SkillReq(n(r.get(1)), n(r.get(2)), (Boolean) r.get(3))); break;
                default: throw new IllegalArgumentException((String) r.get(0));
            }
        }
        return new kotlin.Pair<>((Boolean) a.get("met"), unmet);
    }

    /** A logged stack trace with its frames (host and JVM specific) replaced by one marker line. */
    public static String frames(String message) {
        return message.replaceAll("(\\r?\\n\\tat [^\\n]*)+\\r?\\n?", "\n<frames>");
    }
}
