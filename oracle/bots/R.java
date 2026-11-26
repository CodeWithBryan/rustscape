package oracle;

import core.game.node.Node;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import java.util.*;

/**
 * The recorder every stand-in calls. A call is recorded as {@code [name, args...]}. Its answer comes from the case's
 * {@code answers}: the most specific of {@code name@arg0#arg1}, {@code name@arg0}, {@code name}; a list answers one
 * element per call (the last repeats), {@code {"throw": "Null"}} makes the stand-in throw. {@code failAt} makes the
 * n-th recorded call of the operation throw {@link Failure} after it is recorded and after its hooks ran. Names that
 * start with {@code @} are observations, not calls: they are never counted and never fail.
 */
public final class R {
    public static final class Failure extends RuntimeException {
        public Failure() { super("injected"); }
    }

    public static Map input = new HashMap();
    public static List<Object> events = new ArrayList<>();
    public static Map answers = new HashMap();
    static Map<String, Integer> answerCalls = new HashMap<>();
    static Map<String, Integer> counts = new HashMap<>();
    public static int calls, failAt;
    public static boolean quiet;
    public static Map<String, Object> nodes = new LinkedHashMap<>();
    static Map<String, Integer> sequence = new HashMap<>();
    public static IdentityHashMap<Object, String> labels = new IdentityHashMap<>();
    public static List<Object> bots = new ArrayList<>();
    public static List<Runnable> launched = new ArrayList<>();
    public static String lastPulse;

    public static void resetOp() {
        events = new ArrayList<>();
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
        String l = labels.get(o);
        if (l != null) return l;
        if (o instanceof Pulse) return fresh(o, "pulse");
        throw new IllegalStateException("no label for " + o.getClass());
    }

    public static Object bits(double d) { Map<String, Object> m = new TreeMap<>(); m.put("double", Double.doubleToRawLongBits(d)); return m; }
    public static Object loc(Location l) {
        if (l == null) return null;
        Map<String, Object> m = new TreeMap<>(); m.put("loc", Arrays.asList(l.getX(), l.getY(), l.getZ())); return m;
    }
    public static Object item(Item i) {
        if (i == null) return null;
        if (i.key != null) return i.key;
        Map<String, Object> m = new TreeMap<>(); m.put("item", Arrays.asList(i.getId(), i.getAmount())); return m;
    }

    public static Object value(Object v) {
        if (v == null || v instanceof String || v instanceof Boolean || v instanceof Integer) return v;
        if (v instanceof Location) return loc((Location) v);
        if (v instanceof Item) return item((Item) v);
        if (v instanceof Double) return bits((Double) v);
        if (v instanceof Enum) return ((Enum<?>) v).name();
        if (v instanceof int[]) { List<Object> l = new ArrayList<>(); for (int x : (int[]) v) l.add(x); return l; }
        if (v instanceof Object[]) { List<Object> l = new ArrayList<>(); for (Object x : (Object[]) v) l.add(value(x)); return l; }
        if (v instanceof List) { List<Object> l = new ArrayList<>(); for (Object x : (List<?>) v) l.add(value(x)); return l; }
        if (v instanceof Map) { Map<Object, Object> m = new TreeMap<>(); for (Map.Entry<?, ?> e : ((Map<?, ?>) v).entrySet()) m.put(e.getKey(), value(e.getValue())); return m; }
        if (v instanceof Describable) return value(((Describable) v).describe());
        return label(v);
    }

    /** Objects that record as a value of their own. */
    public interface Describable { Object describe(); }

    static String scalar(Object v) { return v instanceof String || v instanceof Integer || v instanceof Boolean ? String.valueOf(v) : null; }

    public static Object ev(String name, Object... args) {
        if (quiet) return null;
        List<Object> row = new ArrayList<>();
        row.add(name);
        for (Object a : args) row.add(value(a));
        events.add(row);
        if (name.startsWith("@")) return null;
        int index = ++calls;
        int count = counts.merge(name, 1, Integer::sum);
        for (Object h : (List) input.getOrDefault("hooks", Collections.emptyList())) {
            Map hook = (Map) h;
            if (name.equals(hook.get("event")) && count == ((Number) hook.getOrDefault("at", 1L)).intValue()) {
                for (Object action : (List) hook.get("actions")) Support.hook((Map) action);
            }
        }
        if (index == failAt) throw new Failure();
        String a0 = row.size() > 1 ? scalar(row.get(1)) : null, a1 = row.size() > 2 ? scalar(row.get(2)) : null;
        Object answer = null;
        String[] keys = { a0 != null && a1 != null ? name + "@" + a0 + "#" + a1 : null, a0 != null ? name + "@" + a0 : null, name };
        for (String key : keys) {
            if (key == null || !answers.containsKey(key)) continue;
            answer = answers.get(key);
            if (answer instanceof List) {
                List<?> l = (List<?>) answer;
                if (l.isEmpty()) { answer = null; break; }
                int i = answerCalls.merge(key, 1, Integer::sum) - 1;
                answer = l.get(Math.min(i, l.size() - 1));
            }
            break;
        }
        if (answer instanceof Map && ((Map) answer).containsKey("throw")) {
            String kind = (String) ((Map) answer).get("throw");
            switch (kind) {
                case "Null": throw new NullPointerException("answer");
                case "Cast": throw new ClassCastException("answer");
                case "Index": throw new IndexOutOfBoundsException("answer");
                default: throw new IllegalStateException(kind);
            }
        }
        return answer;
    }

    public static void v(String name, Object... args) { ev(name, args); }
    public static int i(String name, Object... args) { Object a = ev(name, args); return a == null ? 0 : ((Number) a).intValue(); }
    public static int i(int fallback, String name, Object... args) { Object a = ev(name, args); return a == null ? fallback : ((Number) a).intValue(); }
    public static boolean b(String name, Object... args) { return Boolean.TRUE.equals(ev(name, args)); }
    public static boolean b(boolean fallback, String name, Object... args) { Object a = ev(name, args); return a == null ? fallback : (Boolean) a; }
    public static double d(String name, Object... args) { Object a = ev(name, args); return a == null ? 0 : ((Number) a).doubleValue(); }
    @SuppressWarnings("unchecked")
    public static <T> T n(String name, Object... args) { return (T) node(ev(name, args)); }
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
        Object o = nodes.get((String) key);
        if (o == null) throw new IllegalStateException("no node " + key);
        return o;
    }
    public static int n(Object o) { return ((Number) o).intValue(); }
    public static Location location(Object v) {
        if (v == null) return null;
        List<?> a = (List<?>) v;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }
    public static Item itemOf(Object v) {
        if (v == null) return null;
        if (v instanceof String) return (Item) node(v);
        List<?> a = (List<?>) v;
        return new Item(n(a.get(0)), a.size() > 1 ? n(a.get(1)) : 1);
    }
    /** An item answer: {@code [id, amount]}, a node key, or null. */
    public static Item itemAnswer(String name, Object... args) { return itemOf(ev(name, args)); }

    public static Map itemSpec(int id) {
        Map m = (Map) ((Map) input.getOrDefault("items", Collections.emptyMap())).get(Integer.toString(id));
        return m == null ? Collections.emptyMap() : m;
    }

    /** A pulse handed to a pulse manager or the pulser: labelled, kept, recorded with its delay, checks and class. */
    public static void pulse(String name, Object owner, Pulse pulse, Object... rest) {
        String l = label(pulse);
        lastPulse = l;
        List<Object> args = new ArrayList<>();
        if (owner != null) args.add(owner);
        args.add(l);
        args.add(pulse.getDelay());
        args.add(pulse.checkKeys());
        args.add(pulse.getClass().getName());
        args.addAll(Arrays.asList(rest));
        ev(name, args.toArray());
    }

    public static void launch(Runnable task) {
        launched.add(task);
        ev("GlobalScope.launch", "task#" + launched.size());
    }

    /** A pulse that is not one of the engine's: {@code foreign:movement} or {@code foreign:plain}. */
    public static Pulse foreign(String key) {
        Object o = nodes.get(key);
        if (o != null) return (Pulse) o;
        Pulse p = key.equals("foreign:movement") ? new ForeignMovement() : new ForeignPulse();
        labels.put(p, key);
        nodes.put(key, p);
        return p;
    }
    public static final class ForeignPulse extends Pulse {
        @Override public boolean pulse() { return false; }
        @Override public String toString() { return label(this); }
    }
    public static final class ForeignMovement extends core.game.interaction.MovementPulse {
        ForeignMovement() { super(); }
        @Override public boolean pulse() { return false; }
        @Override public String toString() { return label(this); }
    }
}
