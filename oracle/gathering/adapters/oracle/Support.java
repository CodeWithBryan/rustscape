package oracle;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.Set;
import java.util.concurrent.ThreadLocalRandom;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import sun.misc.Unsafe;

/**
 * Recording world for the gathering oracle. Every call the unmodified sources
 * make into a system outside the slice is appended to {@link #events} as
 * [name, [arguments], result]. Results come from the simple state declared by
 * the case; they are service inputs, not ports of the original subsystems.
 */
public final class Support {
    public static final class Failure extends RuntimeException {
        public Failure() { super("injected service failure"); }
    }

    /** java.util.Random whose public draws are recorded. */
    public static final class Recording extends Random {
        private final String tag;
        public Recording(String tag) { super(0L); this.tag = tag; }
        @Override public int nextInt(int bound) { int v = super.nextInt(bound); emit("rng." + tag + ".nextInt", v, bound); return v; }
        @Override public double nextDouble() { double v = super.nextDouble(); emit("rng." + tag + ".nextDouble", bits(v)); return v; }
        @Override public boolean nextBoolean() { boolean v = super.nextBoolean(); emit("rng." + tag + ".nextBoolean", v); return v; }
    }

    public static final Unsafe U;
    public static final Recording SHARED = new Recording("shared"), MATH = new Recording("math"), SHUFFLE = new Recording("shuffle");
    private static final long TLR_SEED;
    static {
        try {
            Field f = Unsafe.class.getDeclaredField("theUnsafe");
            f.setAccessible(true);
            U = (Unsafe) f.get(null);
            // Initialise the owning classes first so their initialisers cannot overwrite the recorders.
            install(Class.forName("core.tools.RandomFunction").getDeclaredField("RANDOM"), SHARED);
            install(Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator"), MATH);
            install(Collections.class.getDeclaredField("r"), SHUFFLE);
            ThreadLocalRandom.current();
            TLR_SEED = U.objectFieldOffset(Thread.class.getDeclaredField("threadLocalRandomSeed"));
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }
    private static void install(Field field, Object value) {
        U.putObject(U.staticFieldBase(field), U.staticFieldOffset(field), value);
    }
    public static long tlr() { return U.getLong(Thread.currentThread(), TLR_SEED); }
    public static void tlr(long seed) { U.putLong(Thread.currentThread(), TLR_SEED, seed); }

    // ---- recorder -------------------------------------------------------
    public static JSONArray events = new JSONArray();
    public static int failAt = -1;
    private static long lastTlr;

    public static String bits(double v) { return "d:" + String.format("%016x", Double.doubleToRawLongBits(v)); }

    @SuppressWarnings("unchecked")
    public static <T> T emit(String name, T result, Object... args) {
        JSONArray a = new JSONArray();
        for (Object arg : args) a.add(json(arg));
        JSONArray e = new JSONArray();
        e.add(name);
        e.add(a);
        boolean fail = failAt == events.size() && !infallible.contains(name);
        e.add(fail ? "!fail" : json(result));
        long seed = tlr();
        if (seed != lastTlr) { e.add(Long.toString(seed)); lastTlr = seed; }
        events.add(e);
        if (fail) throw new Failure();
        return result;
    }
    @SuppressWarnings("unchecked")
    public static Object json(Object v) {
        if (v == null || v instanceof String || v instanceof Boolean || v instanceof JSONArray || v instanceof JSONObject) return v;
        if (v instanceof Double) return bits((Double) v);
        if (v instanceof Number) return ((Number) v).longValue();
        if (v instanceof core.game.world.map.Location) { core.game.world.map.Location l = (core.game.world.map.Location) v; return list(l.x, l.y, l.z); }
        if (v instanceof List) { JSONArray a = new JSONArray(); for (Object o : (List<?>) v) a.add(json(o)); return a; }
        if (v instanceof Object[]) { JSONArray a = new JSONArray(); for (Object o : (Object[]) v) a.add(json(o)); return a; }
        if (v instanceof Enum) return ((Enum<?>) v).name();
        throw new IllegalArgumentException(v.getClass().getName());
    }
    @SuppressWarnings("unchecked")
    public static JSONArray list(Object... values) { JSONArray a = new JSONArray(); for (Object v : values) a.add(json(v)); return a; }

    // ---- declared world state ------------------------------------------
    public static int ticks, maxPlayers, playerCount;
    public static int[] dynamic = new int[24], fixed = new int[24], clocks = new int[8], boosts = new int[24];
    public static core.game.world.map.Location location;
    public static core.game.node.item.Item[] inventory = new core.game.node.item.Item[28];
    public static Map<Integer, Integer> equipment = new HashMap<>();
    public static Set<Integer> stackable = new HashSet<>();
    public static Map<Integer, String> names = new HashMap<>(), declaredNames = new HashMap<>();
    public static Map<String, Object> attributes = new HashMap<>();
    public static Map<Integer, Double> fireXp = new HashMap<>();
    public static Set<String> perks = new HashSet<>(), timers = new HashSet<>(), diaryComplete = new HashSet<>();
    public static List<Integer> locals = new ArrayList<>();
    public static boolean animating, playerActive, scriptPersist, hasScript, addResult;
    public static int questStage, diaryLevel, armour, nestId, nestAmount, patchVarbit, patchState, nextNode, scriptNext;
    public static core.game.node.entity.player.Player player;
    public static List<core.game.node.entity.player.Player> others = new ArrayList<>();
    public static List<core.game.node.Node> nodes = new ArrayList<>();
    public static List<Runnable> queued = new ArrayList<>();
    public static List<kotlin.jvm.functions.Function1<Integer, Boolean>> scripts = new ArrayList<>();
    public static List<core.game.system.task.Pulse> pulses = new ArrayList<>();
    // Hooks of the binding corpus (null in the first corpus): real Skills/Container set-up, state and edits.
    public static java.util.function.Consumer<JSONObject> extraReset, extraSet;
    public static java.util.function.Consumer<JSONObject> extraState;
    /** Calls no failure is injected on (the binding corpus: clock reads the native skills cannot fail). */
    public static Set<String> infallible = Collections.emptySet();
    // Fishing inputs.
    public static String usedOption;
    public static boolean hasFamiliar, hasTask, settingsPresent, devMode;
    public static content.global.skill.summoning.familiar.Familiar familiar;
    public static int[] direction = {0, 1};
    public static int paths;
    private static final java.util.IdentityHashMap<Object, Integer> identities = new java.util.IdentityHashMap<>();
    /** A stable number per object, in order of first use within a case. */
    public static int identity(Object o) { Integer v = identities.get(o); if (v == null) { v = identities.size() + 1; identities.put(o, v); } return v; }
    private static void fishing(JSONObject c) {
        usedOption = c.get("usedOption") == null ? "INVALID" : (String) c.get("usedOption");
        hasTask = flag(c, "hasTask", false); settingsPresent = flag(c, "settings", false); devMode = flag(c, "devMode", false);
        JSONArray d = (JSONArray) c.get("direction"); direction = d == null ? new int[] {0, 1} : new int[] {n(d.get(0)), n(d.get(1))};
        JSONArray at = (JSONArray) c.get("familiarLocation");
        core.game.world.map.Location l = at == null ? new core.game.world.map.Location(3199, 3200, 0) : loc(at);
        Object kind = c.get("familiar");
        familiar = kind == null ? null : kind.equals("forager") ? new content.global.skill.summoning.familiar.Forager(l) : new content.global.skill.summoning.familiar.Familiar(l);
        hasFamiliar = flag(c, "hasFamiliar", familiar != null);
        paths = 0; identities.clear();
    }

    public static void reset(JSONObject c) {
        events = new JSONArray(); failAt = -1;
        ticks = i(c, "ticks", 100); maxPlayers = i(c, "maxPlayers", 2000); playerCount = i(c, "playerCount", 1);
        core.ServerConstants.MAX_PLAYERS = maxPlayers;
        for (int s = 0; s < 24; s++) { dynamic[s] = 1; fixed[s] = 1; boosts[s] = 0; }
        levels(c.get("levels"));
        java.util.Arrays.fill(clocks, 0);
        JSONArray l = (JSONArray) c.get("location");
        location = l == null ? new core.game.world.map.Location(3200, 3200, 0) : loc(l);
        inventory = new core.game.node.item.Item[28];
        if (c.get("inventory") != null) { int n = 0; for (Object o : (JSONArray) c.get("inventory")) { JSONArray p = (JSONArray) o; inventory[n++] = new core.game.node.item.Item(n(p.get(0)), n(p.get(1))); } }
        equipment = new HashMap<>(); ints((JSONObject) c.get("equipment"), equipment);
        stackable = new HashSet<>(); if (c.get("stackable") != null) for (Object o : (JSONArray) c.get("stackable")) stackable.add(n(o));
        names = new HashMap<>(declaredNames); if (c.get("names") != null) for (Object o : ((JSONObject) c.get("names")).entrySet()) { Map.Entry<?, ?> e = (Map.Entry<?, ?>) o; names.put(Integer.parseInt((String) e.getKey()), (String) e.getValue()); }
        attributes = new HashMap<>(); if (c.get("attributes") != null) for (Object o : ((JSONObject) c.get("attributes")).entrySet()) { Map.Entry<?, ?> e = (Map.Entry<?, ?>) o; Object v = e.getValue(); attributes.put((String) e.getKey(), v instanceof Number ? (Object) ((Number) v).intValue() : v); }
        fireXp = new HashMap<>(); if (c.get("fireXp") != null) for (Object o : ((JSONObject) c.get("fireXp")).entrySet()) { Map.Entry<?, ?> e = (Map.Entry<?, ?>) o; fireXp.put(Integer.parseInt((String) e.getKey()), ((Number) e.getValue()).doubleValue()); }
        perks = strings(c.get("perks")); timers = strings(c.get("timers")); diaryComplete = strings(c.get("diaryComplete"));
        locals = new ArrayList<>(); if (c.get("locals") != null) for (Object o : (JSONArray) c.get("locals")) locals.add(n(o));
        JSONObject b = (JSONObject) c.get("boosts"); if (b != null) for (Object o : b.entrySet()) { Map.Entry<?, ?> e = (Map.Entry<?, ?>) o; boosts[Integer.parseInt((String) e.getKey())] = n(e.getValue()); }
        animating = flag(c, "animating", false); playerActive = flag(c, "playerActive", true); addResult = flag(c, "addResult", true);
        hasScript = flag(c, "hasScript", true); scriptPersist = flag(c, "scriptPersist", true); scriptNext = 0;
        questStage = i(c, "questStage", 0); diaryLevel = i(c, "diaryLevel", -1); armour = i(c, "armour", -1); nestId = i(c, "nestId", 5070); nestAmount = i(c, "nestAmount", 1);
        patchVarbit = i(c, "patchVarbit", -1); patchState = i(c, "patchState", 0);
        player = new core.game.node.entity.player.Player(0); others = new ArrayList<>();
        for (int k = 1; k < 8; k++) others.add(new core.game.node.entity.player.Player(k));
        nodes = new ArrayList<>(); queued = new ArrayList<>(); scripts = new ArrayList<>(); pulses = new ArrayList<>();
        Random s = SHARED; s.setSeed(l(c, "sharedSeed", 1)); MATH.setSeed(l(c, "mathSeed", 2)); SHUFFLE.setSeed(l(c, "shuffleSeed", 3));
        tlr(Long.parseLong(String.valueOf(c.getOrDefault("tlrSeed", "4")))); lastTlr = tlr();
        fishing(c);
        if (extraReset != null) extraReset.accept(c);
    }
    public static void levels(Object o) {
        if (o == null) return;
        for (Object x : ((JSONObject) o).entrySet()) { Map.Entry<?, ?> e = (Map.Entry<?, ?>) x; JSONArray v = (JSONArray) e.getValue(); int s = Integer.parseInt((String) e.getKey()); dynamic[s] = n(v.get(0)); fixed[s] = n(v.get(1)); }
    }
    private static void ints(JSONObject o, Map<Integer, Integer> out) { if (o != null) for (Object x : o.entrySet()) { Map.Entry<?, ?> e = (Map.Entry<?, ?>) x; out.put(Integer.parseInt((String) e.getKey()), n(e.getValue())); } }
    private static Set<String> strings(Object o) { Set<String> s = new HashSet<>(); if (o != null) for (Object x : (JSONArray) o) s.add((String) x); return s; }
    public static int n(Object o) { return ((Number) o).intValue(); }
    public static int i(JSONObject c, String k, int d) { Object v = c.get(k); return v == null ? d : n(v); }
    public static long l(JSONObject c, String k, long d) { Object v = c.get(k); return v == null ? d : v instanceof String ? Long.parseLong((String) v) : ((Number) v).longValue(); }
    public static boolean flag(JSONObject c, String k, boolean d) { Object v = c.get(k); return v == null ? d : (Boolean) v; }
    public static core.game.world.map.Location loc(JSONArray l) { return new core.game.world.map.Location(n(l.get(0)), n(l.get(1)), n(l.get(2))); }

    public static int ref(core.game.node.Node n) { return n == null ? -1 : n.ref; }
    public static int register(core.game.node.Node n) { nodes.add(n); return nodes.size() - 1; }
    public static String name(int id) { String s = names.get(id); if (s == null) throw new IllegalStateException("undeclared item name " + id); return s; }
    public static int freeSlots() { int c = 0; for (core.game.node.item.Item x : inventory) if (x == null) c++; return c; }
    public static int amount(int id) { long c = 0; for (core.game.node.item.Item x : inventory) if (x != null && x.getId() == id) c += x.getAmount(); return (int) Math.min(c, Integer.MAX_VALUE); }
    /** Simple slot model: a stack merges, anything else needs one free slot per unit. Not the original Container. */
    public static boolean add(int id, int amount) {
        if (!addResult) return false;
        if (stackable.contains(id)) {
            for (core.game.node.item.Item x : inventory) if (x != null && x.getId() == id) { x.setAmount(x.getAmount() + amount); return true; }
            for (int k = 0; k < 28; k++) if (inventory[k] == null) { inventory[k] = new core.game.node.item.Item(id, amount); return true; }
            return false;
        }
        if (amount > freeSlots()) return false;
        for (int k = 0, left = amount; k < 28 && left > 0; k++) if (inventory[k] == null) { inventory[k] = new core.game.node.item.Item(id, 1); left--; }
        return true;
    }
    /** Whether the simple slot model would accept the item; a query, not the original Container. */
    public static boolean space(int id, int amount) {
        if (stackable.contains(id)) return amount(id) > 0 || freeSlots() > 0;
        return amount <= freeSlots();
    }
    /** Removes one unit of the id from the simple slot model. */
    public static void take(int id) {
        for (int k = 0; k < 28; k++) if (inventory[k] != null && inventory[k].getId() == id) {
            if (inventory[k].getAmount() > 1) inventory[k].setAmount(inventory[k].getAmount() - 1); else inventory[k] = null;
            return;
        }
    }
    @SuppressWarnings("unchecked")
    public static JSONObject state() {
        JSONObject o = new JSONObject();
        java.util.TreeMap<String, Long> inv = new java.util.TreeMap<>();
        for (core.game.node.item.Item x : inventory) if (x != null) inv.merge(Integer.toString(x.getId()), (long) x.getAmount(), Long::sum);
        JSONObject held = new JSONObject(); held.putAll(inv);
        o.put("inventory", held); o.put("free", (long) freeSlots());
        JSONObject at = new JSONObject(); for (Map.Entry<String, Object> e : new java.util.TreeMap<>(attributes).entrySet()) at.put(e.getKey(), json(e.getValue()));
        o.put("attributes", at);
        JSONArray ns = new JSONArray(); for (core.game.node.Node x : nodes) ns.add(list(x.rawId, x.rawActive));
        o.put("nodes", ns); o.put("clocks", list(clocks[0], clocks[1], clocks[7])); o.put("scriptNext", (long) scriptNext); o.put("hasScript", hasScript);
        o.put("patchState", (long) patchState); o.put("queued", (long) scripts.size()); o.put("tlr", Long.toString(tlr()));
        return o;
    }
}
