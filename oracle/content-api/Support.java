package oracle;

import core.game.container.Container;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.world.map.Location;
import java.io.OutputStream;
import java.io.PrintStream;
import java.util.*;
import org.json.simple.JSONValue;

/**
 * Runs one case (Boot gives every case a class loader of its own): builds the case's nodes and world state, calls the
 * named function through {@link Calls}, runs the case's follow-up steps (pulses, queued scripts, skill dialogue
 * callbacks) and returns the recorded calls, the result or error and a snapshot of the in-memory state the originals
 * change without a call (varp maps, clocks, script fields, charges).
 */
public class Support {
    static int n(Object o) { return ((Number) o).intValue(); }

    static java.lang.reflect.Field threadSeed;
    /** Kotlin's Random.Default is the thread's ThreadLocalRandom: seed it per case (its raw state is part of the record). */
    static void seed(long seed) throws Exception {
        java.util.concurrent.ThreadLocalRandom.current();
        if (threadSeed == null) {
            threadSeed = Thread.class.getDeclaredField("threadLocalRandomSeed");
            threadSeed.setAccessible(true);
        }
        threadSeed.setLong(Thread.currentThread(), seed);
    }

    static String error(Throwable t) {
        if (t instanceof R.Failure) return "Binding";
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof ClassCastException) return "Cast";
        if (t instanceof IllegalStateException) return "State";
        if (t instanceof IllegalArgumentException) return "Argument";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        if (t instanceof NoSuchElementException) return "NoSuchElement";
        if (t instanceof kotlin.UninitializedPropertyAccessException) return "Lateinit";
        if (t instanceof ArithmeticException) return "Arithmetic";
        return t.getClass().getName();
    }

    /** printStackTrace's first line becomes a recorded {@code @printStackTrace} call. */
    static final class Err extends PrintStream {
        Err() { super(OutputStream.nullOutputStream()); }
        @Override public void println(Object x) {
            if (x instanceof Throwable) {
                Throwable t = (Throwable) x;
                String code = error(t);
                R.events.add(Arrays.asList("@printStackTrace", code, code.equals("Binding") || code.equals("Null") ? null : t.getMessage()));
            }
        }
        @Override public void println(String x) {}
    }

    @SuppressWarnings("unchecked")
    static void build(Map<String, Object> c) {
        Map<String, Object> defs = (Map<String, Object>) c.getOrDefault("definitions", Collections.emptyMap());
        Map<String, Object> items = (Map<String, Object>) defs.getOrDefault("items", Collections.emptyMap());
        for (Map.Entry<String, Object> e : items.entrySet()) {
            Map<String, Object> v = (Map<String, Object>) e.getValue();
            core.cache.def.impl.ItemDefinition d = new core.cache.def.impl.ItemDefinition();
            d.id = Integer.parseInt(e.getKey());
            d.name = (String) v.get("name");
            d.stackable = Boolean.TRUE.equals(v.get("stackable"));
            d.unnoted = !Boolean.FALSE.equals(v.get("unnoted"));
            d.noteId = v.containsKey("noteId") ? n(v.get("noteId")) : -1;
            Map<String, Object> config = (Map<String, Object>) v.getOrDefault("config", Collections.emptyMap());
            for (Map.Entry<String, Object> k : config.entrySet()) d.configurations.put(k.getKey(), R.typed(k.getValue()));
            R.items.put(d.id, d);
        }
        Map<String, Object> varbits = (Map<String, Object>) defs.getOrDefault("varbits", Collections.emptyMap());
        for (Map.Entry<String, Object> e : varbits.entrySet()) {
            List<Object> v = (List<Object>) e.getValue();
            core.cache.def.impl.VarbitDefinition d = new core.cache.def.impl.VarbitDefinition();
            d.id = Integer.parseInt(e.getKey());
            d.varpId = n(v.get(0)); d.startBit = n(v.get(1)); d.endBit = n(v.get(2));
            R.varbits.put(d.id, d);
        }
        Map<String, Object> nodes = (Map<String, Object>) c.getOrDefault("nodes", Collections.emptyMap());
        // Two passes: objects first, then their fields (which may name other nodes).
        for (String k : new TreeSet<>(nodes.keySet())) {
            Map<String, Object> v = (Map<String, Object>) nodes.get(k);
            Object o;
            switch ((String) v.get("kind")) {
                case "player": o = new Player(); break;
                case "npc": o = new NPC(); break;
                case "familiar": o = new content.global.skill.summoning.familiar.Familiar(); break;
                case "bob": o = new content.global.skill.summoning.familiar.BurdenBeast(); break;
                case "item": o = new Item(); break;
                case "grounditem": o = new core.game.node.item.GroundItem(); break;
                case "scenery": o = new core.game.node.scenery.Scenery(); break;
                case "poison": o = new core.game.system.timer.impl.Poison(); break;
                case "poisonImmunity": o = new core.game.system.timer.impl.PoisonImmunity(); break;
                case "dragonfireImmunity": o = new core.game.system.timer.impl.DragonFireImmunity(); break;
                case "script": o = new core.game.interaction.QueuedScript(s -> false, core.game.interaction.QueueStrength.WEAK, Boolean.TRUE.equals(v.get("persist"))); break;
                case "potion": o = new core.game.consumable.Potion(ints(v.get("ids"))); break;
                case "consumable": o = new core.game.consumable.Consumable(ints(v.get("ids"))); break;
                case "projectile": o = new core.game.node.entity.impl.Projectile(); break;
                case "chunk": o = new core.game.world.map.RegionChunk(); break;
                case "region": o = new core.game.world.map.build.DynamicRegion(null); R.events.clear(); R.calls = 0; break;
                case "shop": o = new core.game.shops.Shop(); break;
                case "offer": o = new core.game.ge.GrandExchangeOffer(); break;
                case "toml": o = new com.moandjiezana.toml.Toml(); R.events.clear(); R.calls = 0; break;
                case "cutscene": o = new core.game.activity.Cutscene(); break;
                default: throw new IllegalArgumentException((String) v.get("kind"));
            }
            R.nodes.put(k, o);
            R.labels.put(o, k);
            if (o instanceof Node) ((Node) o).key = k;
            if (o instanceof core.game.system.timer.RSTimer) ((core.game.system.timer.RSTimer) o).setKey(k);
            if (o instanceof core.game.consumable.Consumable) { ((core.game.consumable.Consumable) o).key = k; ((core.game.consumable.Consumable) o).identityHash = n(v.getOrDefault("hash", 0L)); }
            if (o instanceof Player) {
                Player p = (Player) o;
                for (Container cont : new Container[] { p.o_inventory, p.o_equipment, p.o_bank, p.o_bankSecondary, p.POHStorageProxyContainer }) label(cont, k + "." + cont.name);
            }
            if (o instanceof content.global.skill.summoning.familiar.BurdenBeast) label(((content.global.skill.summoning.familiar.BurdenBeast) o).container, k + ".bob");
        }
        for (String k : new TreeSet<>(nodes.keySet())) {
            Map<String, Object> v = (Map<String, Object>) nodes.get(k);
            Object o = R.nodes.get(k);
            if (o instanceof Item && !(o instanceof core.game.node.item.GroundItem && !v.containsKey("id"))) {
                ((Item) o).oracleItem(n(v.getOrDefault("id", 0L)), n(v.getOrDefault("amount", 1L)), n(v.getOrDefault("charge", 1000L)), n(v.getOrDefault("slot", -1L)));
            }
            if (o instanceof Node && !(o instanceof Item)) {
                Node node = (Node) o;
                node.nodeId = n(v.getOrDefault("id", 0L));
                node.oracleState((String) v.get("name"), (Location) R.node(v.get("location")), 0, n(v.getOrDefault("size", 1L)));
            }
            if (o instanceof Item && v.containsKey("location")) ((Item) o).oracleState(((Item) o).getName(), (Location) R.node(v.get("location")), ((Item) o).getSlot(), 1);
            if (o instanceof core.game.node.scenery.Scenery) {
                core.game.node.scenery.Scenery s = (core.game.node.scenery.Scenery) o;
                s.o_rotation = n(v.getOrDefault("rotation", 0L));
                s.o_type = n(v.getOrDefault("type", 10L));
                s.o_charge = n(v.getOrDefault("charge", 1000L));
            }
            if (o instanceof Entity && v.containsKey("clocks")) ((Entity) o).o_clocks = ints(v.get("clocks"));
            if (o instanceof Player) {
                Player p = (Player) o;
                p.o_male = !Boolean.FALSE.equals(v.get("male"));
                p.o_username = (String) v.getOrDefault("username", k);
                p.useSecondaryBank = Boolean.TRUE.equals(v.get("secondary"));
                for (Map.Entry<String, Object> e : ((Map<String, Object>) v.getOrDefault("varps", Collections.emptyMap())).entrySet()) p.varpMap.put(Integer.parseInt(e.getKey()), n(e.getValue()));
                for (Map.Entry<String, Object> e : ((Map<String, Object>) v.getOrDefault("saved", Collections.emptyMap())).entrySet()) p.saveVarp.put(Integer.parseInt(e.getKey()), (Boolean) e.getValue());
                for (Object name : (List<Object>) v.getOrDefault("listeners", Collections.emptyList())) p.logoutListeners.put((String) name, Calls.INSTANCE.playerHandler("listener:" + name));
                if (v.containsKey("records")) {
                    core.game.ge.GrandExchangeRecords r = new core.game.ge.GrandExchangeRecords(p);
                    List<Object> recs = (List<Object>) v.get("records");
                    core.game.ge.OfferRecord[] array = new core.game.ge.OfferRecord[recs.size()];
                    for (int i = 0; i < array.length; i++) if (recs.get(i) != null) { array[i] = new core.game.ge.OfferRecord(i, i); R.labels.put(array[i], "record:" + i); }
                    r.setOfferRecords(array);
                    R.records.put(p, r);
                }
            }
            if (o instanceof NPC && v.containsKey("invisible")) ((NPC) o).o_invisible = (Boolean) v.get("invisible");
            if (o instanceof core.game.system.timer.impl.Poison) {
                core.game.system.timer.impl.Poison t = (core.game.system.timer.impl.Poison) o;
                R.events.clear(); R.calls = 0;
                t.oracleSeverity(n(v.getOrDefault("severity", 0L)));
                t.oracleSource((Entity) (v.get("source") == null ? null : R.node(v.get("source"))));
            }
            if (o instanceof core.game.interaction.Script) {
                core.game.interaction.Script<?> s = (core.game.interaction.Script<?>) o;
                s.setNextExecution(n(v.getOrDefault("nextExecution", 0L)));
                s.setState(n(v.getOrDefault("state", 0L)));
            }
            if (o instanceof core.game.ge.GrandExchangeOffer) {
                List<Object> w = (List<Object>) v.getOrDefault("withdraw", Arrays.asList(null, null));
                Item[] array = new Item[w.size()];
                for (int i = 0; i < array.length; i++) array[i] = (Item) R.node(w.get(i));
                ((core.game.ge.GrandExchangeOffer) o).setWithdraw(array);
            }
        }
        Map<String, Object> world = (Map<String, Object>) c.getOrDefault("world", Collections.emptyMap());
        core.game.world.GameWorld.setTicks(n(world.getOrDefault("ticks", 0L)));
        if (world.containsKey("devMode")) core.game.world.GameWorld.setSettings(world.get("devMode") == null ? null : new core.game.world.GameSettings((Boolean) world.get("devMode")));
        if (world.containsKey("maxPathfind")) core.ServerConstants.Companion.setMAX_PATHFIND_DISTANCE(n(world.get("maxPathfind")));
        if (world.get("toml") != null) core.game.system.config.ServerConfigParser.INSTANCE.setTomlData((com.moandjiezana.toml.Toml) R.node(world.get("toml")));
        for (Map.Entry<String, Object> e : ((Map<String, Object>) world.getOrDefault("shops", Collections.emptyMap())).entrySet())
            core.game.shops.Shops.getShopsByNpc().put(Integer.parseInt(e.getKey()), (core.game.shops.Shop) R.node(e.getValue()));
        if (world.containsKey("consumables")) {} // answered through Consumables.getConsumableById
    }

    static void label(Container c, String key) { R.labels.put(c, key); c.key = key; R.containers.put(key, c); }

    static int[] ints(Object o) {
        List<?> l = (List<?>) o;
        int[] out = new int[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = n(l.get(i));
        return out;
    }

    /** Follow-up steps: run a recorded pulse, a queued script or a skill dialogue handler's callbacks. */
    @SuppressWarnings("unchecked")
    static Object step(Map<String, Object> s) {
        String kind = (String) s.get("step");
        switch (kind) {
            case "pulse": {
                core.game.system.task.Pulse p = (core.game.system.task.Pulse) R.pulses.get(n(s.get("index")));
                boolean done = p.pulse();
                R.events.add(Arrays.asList("@pulse", R.label(p), done));
                return done;
            }
            case "script": {
                core.game.interaction.Script<?> sc = (core.game.interaction.Script<?>) R.scripts.get(n(s.get("index")));
                kotlin.jvm.functions.Function1<Integer, Boolean> f = (kotlin.jvm.functions.Function1<Integer, Boolean>) sc.getExecution();
                boolean done = f.invoke(n(s.get("stage")));
                R.events.add(Arrays.asList("@script", R.label(sc), n(s.get("stage")), done));
                return done;
            }
            case "create": {
                core.game.dialogue.SkillDialogueHandler h = (core.game.dialogue.SkillDialogueHandler) R.handlers.get(0);
                h.create(n(s.get("amount")), n(s.get("index")));
                return null;
            }
            case "getAll": {
                core.game.dialogue.SkillDialogueHandler h = (core.game.dialogue.SkillDialogueHandler) R.handlers.get(0);
                return h.getAll(n(s.get("index")));
            }
            case "ticks": {
                core.game.world.GameWorld.setTicks(n(s.get("ticks")));
                return null;
            }
            default: throw new IllegalArgumentException(kind);
        }
    }

    @SuppressWarnings("unchecked")
    static Map<String, Object> snapshot() {
        Map<String, Object> out = new TreeMap<>();
        for (Map.Entry<String, Object> e : R.nodes.entrySet()) {
            Object o = e.getValue();
            Map<String, Object> m = new TreeMap<>();
            if (o instanceof Player) {
                Player p = (Player) o;
                m.put("varps", new TreeMap<>(p.varpMap).toString());
                m.put("saved", new TreeMap<>(p.saveVarp).toString());
                m.put("listeners", new TreeSet<>(p.logoutListeners.keySet()).toString());
                m.put("secondary", p.useSecondaryBank);
            }
            if (o instanceof Entity) m.put("clocks", R.value(((Entity) o).o_clocks));
            if (o instanceof NPC) m.put("invisible", ((NPC) o).o_invisible);
            if (o instanceof Item) m.put("item", Arrays.asList(((Item) o).getId(), ((Item) o).getAmount(), ((Item) o).getCharge()));
            if (o instanceof core.game.node.scenery.Scenery) m.put("charge", ((core.game.node.scenery.Scenery) o).o_charge);
            if (o instanceof core.game.interaction.Script) {
                core.game.interaction.Script<?> s = (core.game.interaction.Script<?>) o;
                m.put("script", Arrays.asList(s.getNextExecution(), s.getState()));
            }
            if (o instanceof core.game.system.timer.impl.Poison) m.put("poison", ((core.game.system.timer.impl.Poison) o).oracleState());
            if (!m.isEmpty()) out.put(e.getKey(), m);
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    public static String runCase(String line) throws Exception {
        Map<String, Object> c = (Map<String, Object>) JSONValue.parseWithException(line);
        PrintStream err = System.err;
        System.setErr(new Err());
        Map<String, Object> expected = new TreeMap<>();
        try {
            R.answers = (Map<String, Object>) c.getOrDefault("answers", new HashMap<>());
            R.failAt = 0;
            seed(n(c.getOrDefault("seed", 532L)));
            build(c);
            R.reset();
            R.failAt = n(c.getOrDefault("failAt", 0L));
            Object result = null;
            String error = null;
            try {
                Object raw = Calls.INSTANCE.call((String) c.get("fn"), (List<Object>) c.getOrDefault("args", Collections.emptyList()));
                result = raw instanceof kotlin.Unit ? null : R.value(raw);
                if (result instanceof kotlin.Unit) result = null;
            } catch (Throwable t) {
                error = error(t);
            }
            List<Object> steps = new ArrayList<>();
            if (error == null) {
                for (Object s : (List<Object>) c.getOrDefault("steps", Collections.emptyList())) {
                    try {
                        Object r = step((Map<String, Object>) s);
                        steps.add(Arrays.asList(r instanceof kotlin.Unit ? null : R.value(r), null));
                    } catch (Throwable t) {
                        steps.add(Arrays.asList(null, error(t)));
                        break;
                    }
                }
            }
            expected.put("events", R.events);
            expected.put("result", result instanceof kotlin.Unit ? null : result);
            expected.put("error", error);
            expected.put("steps", steps);
            Map<String, Object> state = snapshot();
            state.put("@tlr", Collections.singletonMap("long", threadSeed.getLong(Thread.currentThread())));
            expected.put("state", state);
        } finally {
            System.setErr(err);
        }
        c.put("expected", expected);
        return JSONValue.toJSONString(c);
    }
}
