package oracle;

import content.global.skill.slayer.Tasks;
import content.global.skill.summoning.familiar.Familiar;
import core.game.interaction.MovementPulse;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.npc.NPCBehavior;
import core.game.node.entity.npc.RecordingBehavior;
import core.game.node.entity.npc.RecordingNpc;
import core.game.node.entity.npc.agg.AggressiveBehavior;
import core.game.node.entity.npc.agg.AggressiveHandler;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import core.game.world.map.Region;
import core.game.world.map.build.DynamicRegion;
import core.tools.RandomFunction;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.*;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/** Reads one case per line, runs its operations against the original members and prints what they did. */
public class Support {
    static Map input;
    static List<Object> events;
    static int calls, failAt;
    public static int ticks, nextPulse;
    public static long now;
    static List<Entity> entities;
    static Map<Integer, Region> regions;
    static Map<String, Integer> consumed;
    static Rng random;
    static MathRng math;

    public static class Failure extends RuntimeException {}

    static class Rng extends Random {
        Rng(long seed) { super(seed); }
        @Override public int nextInt(int bound) { int v = super.nextInt(bound); emit("next_int", bound, v); return v; }
        @Override public boolean nextBoolean() { boolean v = super.nextBoolean(); emit("next_bool", v); return v; }
    }

    static class MathRng extends Random {
        boolean armed;
        @Override public double nextDouble() { double v = super.nextDouble(); if (armed) emit("math_random", hex(v)); return v; }
    }

    static String hex(double v) { return Long.toHexString(Double.doubleToRawLongBits(v)); }

    /** Replaces java.lang.Math's lazily created generator once, before any compiled use. */
    static void installMath() throws Exception {
        Math.random();
        Field holder = Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator");
        Field access = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        access.setAccessible(true);
        sun.misc.Unsafe unsafe = (sun.misc.Unsafe) access.get(null);
        math = new MathRng();
        unsafe.putObject(unsafe.staticFieldBase(holder), unsafe.staticFieldOffset(holder), math);
        math.setSeed(7);
        double expected = new Random(7).nextDouble();
        if (Math.random() != expected) throw new IllegalStateException("Math.random is not bound to the oracle generator");
    }

    public static void emit(String name, Object... args) {
        List row = new ArrayList();
        row.add(name);
        for (Object a : args) row.add(a);
        events.add(row);
        calls++;
        if (calls == failAt) throw new Failure();
    }

    public static Object answer(String key, Object fallback) {
        Map answers = (Map) input.getOrDefault("answers", Collections.emptyMap());
        if (!answers.containsKey(key)) return fallback;
        Object v = answers.get(key);
        if (v instanceof List) {
            List l = (List) v;
            int i = consumed.merge(key, 1, Integer::sum) - 1;
            return i < l.size() ? l.get(i) : fallback;
        }
        return v;
    }

    public static boolean bool(String name, Entity e, boolean fallback) {
        emit(name, ix(e));
        return Boolean.TRUE.equals(answer(name + ":" + ix(e), fallback));
    }

    public static int integer(String name, Entity e, int fallback) {
        emit(name, ix(e));
        return ((Number) answer(name + ":" + ix(e), (long) fallback)).intValue();
    }

    public static Integer ix(Object e) {
        if (e == null) return null;
        for (int i = 0; i < entities.size(); i++) if (entities.get(i) == e) return i;
        throw new IllegalStateException("unknown entity");
    }

    public static Entity entity(int i) { return entities.get(i); }

    public static List coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }

    static Location loc(Object o) {
        if (o == null) return null;
        List a = (List) o;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }

    static int n(Object o) { return ((Number) o).intValue(); }

    public static int ticks() { emit("ticks"); return ticks; }

    public static long millis() { emit("current_time_millis"); return now; }

    public static boolean sticky() { return !Boolean.FALSE.equals(input.get("sticky")); }

    public static Region region(int id) {
        Region r = regions.get(id);
        if (r == null) {
            Map spec = (Map) ((Map) input.getOrDefault("regions", Collections.emptyMap())).get(String.valueOf(id));
            r = spec != null && Boolean.TRUE.equals(spec.get("dynamic")) ? new DynamicRegion(id) : new Region(id);
            if (spec != null && spec.get("tolerances") != null) {
                for (Object o : ((Map) spec.get("tolerances")).entrySet()) {
                    Map.Entry e = (Map.Entry) o;
                    r.getTolerances().put((String) e.getKey(), e.getValue() == null ? null : ((Number) e.getValue()).longValue());
                }
            }
            regions.put(id, r);
        }
        return r;
    }

    public static List<Player> localPlayers() {
        Object v = answer("local_players", null);
        List<Player> out = new ArrayList<>();
        if (v == null) {
            for (Entity e : entities) if (e instanceof Player) out.add((Player) e);
        } else {
            for (Object o : (List) v) out.add((Player) entities.get(n(o)));
        }
        return out;
    }

    public static Object repr(Object o) {
        if (o instanceof Entity) return ix(o);
        return o;
    }

    public static void scriptState(BattleState s) {
        Object v = answer("behaviour_state", null);
        if (v instanceof Map) {
            Map m = (Map) v;
            if (m.containsKey("estimated")) s.setEstimatedHit(n(m.get("estimated")));
            if (m.containsKey("secondary")) s.setSecondaryHit(n(m.get("secondary")));
        }
    }

    static Field field(Class<?> c, String name) {
        try { Field f = c.getDeclaredField(name); f.setAccessible(true); return f; } catch (Exception e) { throw new IllegalStateException(e); }
    }

    static AggressiveBehavior behaviour(Object name) { return "wilderness".equals(name) ? AggressiveBehavior.WILDERNESS : AggressiveBehavior.DEFAULT; }

    static CombatStyle style(Object o) { return o == null ? null : CombatStyle.valueOf((String) o); }

    static Object attributeValue(String key, Object v) {
        if (v instanceof Number) return "combat-time".equals(key) ? (Object) ((Number) v).longValue() : (Object) ((Number) v).intValue();
        if (v instanceof String && ((String) v).startsWith("e")) return entities.get(Integer.parseInt(((String) v).substring(1)));
        return v;
    }

    static void apply(Entity e, Map m) throws Exception {
        if (m.containsKey("location")) e.setLocation(loc(m.get("location")));
        if (m.containsKey("attrs")) for (Object o : ((Map) m.get("attrs")).entrySet()) {
            Map.Entry en = (Map.Entry) o;
            String key = (String) en.getKey();
            if (en.getValue() == null) e.attributes.remove(key); else e.attributes.put(key, attributeValue(key, en.getValue()));
        }
        Entity.CombatPulse c = e.getProperties().getCombatPulse();
        if (m.containsKey("attacking")) c.attacking = Boolean.TRUE.equals(m.get("attacking"));
        if (m.containsKey("victim")) c.victim = m.get("victim") == null ? null : entities.get(n(m.get("victim")));
        if (m.containsKey("nextAttack")) c.nextAttack = n(m.get("nextAttack"));
        if (m.containsKey("lastSent")) c.lastSentAttack = n(m.get("lastSent"));
        if (m.containsKey("lastReceived")) c.lastReceivedAttack = n(m.get("lastReceived"));
        if (m.containsKey("style")) c.style = style(m.get("style"));
        if (m.containsKey("invisible")) e.setInvisible(Boolean.TRUE.equals(m.get("invisible")));
        if (!(e instanceof NPC)) return;
        NPC npc = (NPC) e;
        if (m.containsKey("walks")) npc.walks = Boolean.TRUE.equals(m.get("walks"));
        if (m.containsKey("aggressive")) npc.aggressive = Boolean.TRUE.equals(m.get("aggressive"));
        if (m.containsKey("radius")) npc.setWalkRadius(n(m.get("radius")));
        if (m.containsKey("respawn")) npc.respawnTick = n(m.get("respawn"));
        if (m.containsKey("respawning")) npc.isRespawning = Boolean.TRUE.equals(m.get("respawning"));
        if (m.containsKey("spawn")) npc.getProperties().spawnLocation = loc(m.get("spawn"));
        if (m.containsKey("task")) npc.task = m.get("task") == null ? null : new Tasks(n(m.get("task")));
        if (m.containsKey("forceTalk")) npc.forceTalk = (String) m.get("forceTalk");
        if (m.containsKey("neverWalks")) npc.neverWalks = Boolean.TRUE.equals(m.get("neverWalks"));
        if (m.containsKey("returning")) npc.attributes.put("return-to-spawn", Boolean.TRUE.equals(m.get("returning")));
        if (m.containsKey("returnPulse")) {
            if (m.get("returnPulse") == null) npc.attributes.remove("return-to-spawn-pulse");
            else npc.attributes.put("return-to-spawn-pulse", new MovementPulse(npc, n(m.get("returnPulse"))));
        }
        if (m.containsKey("handler")) {
            Map h = (Map) m.get("handler");
            if (h == null) npc.setAggressiveHandler(null);
            else {
                AggressiveHandler a = new AggressiveHandler(npc, behaviour(h.get("behaviour")));
                if (h.containsKey("radius")) a.setRadius(n(h.get("radius")));
                if (h.containsKey("pause")) field(AggressiveHandler.class, "pauseTicks").setInt(a, n(h.get("pause")));
                if (h.containsKey("switching")) a.setTargetSwitching(Boolean.TRUE.equals(h.get("switching")));
                if (h.containsKey("chance")) a.setChanceRatio(n(h.get("chance")));
                if (h.containsKey("allowTolerance")) a.setAllowTolerance(Boolean.TRUE.equals(h.get("allowTolerance")));
                npc.setAggressiveHandler(a);
            }
        }
    }

    static Entity create(Map spec) {
        String kind = (String) spec.getOrDefault("kind", "npc");
        Location l = loc(spec.getOrDefault("location", Arrays.asList(3200L, 3200L, 0L)));
        int id = n(spec.getOrDefault("id", 1L));
        if ("player".equals(kind)) return new Player((String) spec.getOrDefault("name", "player"));
        if ("familiar".equals(kind)) return new Familiar(id, l);
        if ("recording".equals(spec.get("class"))) return new RecordingNpc(id, l);
        return new NPC(id, l);
    }

    static BattleState state(Map m) {
        BattleState s = new BattleState(m.get("attacker") == null ? null : entities.get(n(m.get("attacker"))), m.get("victim") == null ? null : entities.get(n(m.get("victim"))));
        if (m.containsKey("estimated")) s.setEstimatedHit(n(m.get("estimated")));
        if (m.containsKey("secondary")) s.setSecondaryHit(n(m.get("secondary")));
        if (m.containsKey("poison")) s.setPoisonDamage(n(m.get("poison")));
        s.setStyle(style(m.get("style")));
        return s;
    }

    static Map battle(BattleState s) {
        Map m = new TreeMap();
        m.put("estimated", s.getEstimatedHit());
        m.put("secondary", s.getSecondaryHit());
        m.put("poison", s.getPoisonDamage());
        return m;
    }

    @SuppressWarnings("unchecked")
    static void clearRegistry() throws Exception {
        Field f = NPCBehavior.class.getDeclaredField("idMap");
        f.setAccessible(true);
        ((Map) f.get(null)).clear();
    }

    static Object tag(NPCBehavior b) { return b instanceof RecordingBehavior ? ((RecordingBehavior) b).getTag() : "default"; }

    static Object run(Map op) throws Exception {
        Entity e = op.containsKey("entity") ? entities.get(n(op.get("entity"))) : entities.get(0);
        switch ((String) op.get("op")) {
            case "select": return ((NPC) e).getAggressiveHandler().selectTarget();
            case "tick": ((NPC) e).tick(); return null;
            case "actions": ((NPC) e).handleTickActions(); return null;
            case "advance": {
                int n = n(op.getOrDefault("ticks", 1L));
                ticks += n;
                now += op.containsKey("millis") ? ((Number) op.get("millis")).longValue() : 600L * n;
                return null;
            }
            case "set": apply(e, (Map) op.get("fields")); return null;
            case "attackable": return entities.get(n(op.get("victim"))).isAttackable(entities.get(n(op.get("attacker"))), style(op.get("style")), Boolean.TRUE.equals(op.get("message")));
            case "continue": return e.continueAttack(entities.get(n(op.get("target"))), style(op.get("style")), Boolean.TRUE.equals(op.get("message")));
            case "check_impact": { BattleState s = state((Map) op.get("state")); e.checkImpact(s); return battle(s); }
            case "on_impact": { BattleState s = state((Map) op.get("state")); e.onImpact(op.get("attacker") == null ? null : entities.get(n(op.get("attacker"))), s); return battle(s); }
            case "send_impact": e.sendImpact(state((Map) op.getOrDefault("state", Collections.emptyMap()))); return null;
            case "in_combat": return e.inCombat();
            case "invisible": return e.isInvisible();
            case "hidden": return ((NPC) e).isHidden((Player) entities.get(n(op.get("player"))));
            case "swing_handler": return ((NPC) e).getSwingHandler(Boolean.TRUE.equals(op.get("swing"))).tag;
            case "ignore_multi": return e.isIgnoreMultiBoundaries(entities.get(n(op.get("victim"))));
            case "inactivity": ((NPC) e).onRegionInactivity(); return null;
            case "restore": e.fullRestore(); return null;
            case "default_behavior": ((NPC) e).setDefaultBehavior(); return null;
            case "boss": ((NPC) e).configureBossData(); return null;
            case "can_select": return behaviour(op.get("behaviour")).canSelectTarget(e, entities.get(n(op.get("target"))));
            case "priority": return AggressiveBehavior.DEFAULT.getPriorityFlag(entities.get(n(op.get("target"))));
            case "possible": {
                List out = new ArrayList();
                for (Entity t : behaviour(op.get("behaviour")).getPossibleTargets(e, n(op.get("radius")))) out.add(ix(t));
                return out;
            }
            case "logical": {
                List<Entity> list = new ArrayList<>();
                for (Object o : (List) op.get("targets")) list.add(entities.get(n(o)));
                return ix(AggressiveBehavior.DEFAULT.getLogicalTarget(e, list));
            }
            case "allow_tolerance": return ((NPC) e).getAggressiveHandler().isAllowTolerance();
            case "pause": ((NPC) e).getAggressiveHandler().setPauseTicks(n(op.get("ticks"))); return null;
            case "remove_tolerance": ((NPC) e).getAggressiveHandler().removeTolerance(n(op.get("index"))); return null;
            case "hook": return hook((NPC) e, op);
            case "register": {
                List ids = (List) op.get("ids");
                int[] array = new int[ids.size()];
                for (int i = 0; i < array.length; i++) array[i] = n(ids.get(i));
                NPCBehavior.Companion.register(array, new RecordingBehavior((String) op.get("tag")));
                return null;
            }
            case "for_id": return tag(NPCBehavior.forId(n(op.get("id"))));
            case "refresh_behavior": ((NPC) e).behavior = NPCBehavior.forId(((NPC) e).getId()); return tag(((NPC) e).behavior);
            default: throw new AssertionError(op);
        }
    }

    static Object hook(NPC npc, Map op) {
        NPCBehavior b = npc.behavior;
        Entity other = op.get("other") == null ? null : entities.get(n(op.get("other")));
        switch ((String) op.get("name")) {
            case "tick": return b.tick(npc);
            case "canBeAttackedBy": return b.canBeAttackedBy(npc, other, style(op.get("style")), Boolean.TRUE.equals(op.get("message")));
            case "shouldIgnoreMultiRestrictions": return b.shouldIgnoreMultiRestrictions(npc, other);
            case "getXpMultiplier": return hex(b.getXpMultiplier(npc, other));
            case "getPathfinderOverride": return b.getPathfinderOverride(npc) == null ? null : "override";
            case "clip": return b.getClippingSupplier(npc).getClippingFlag(n(op.get("z")), n(op.get("x")), n(op.get("y")));
            case "onCreation": b.onCreation(npc); return null;
            case "onRemoval": b.onRemoval(npc); return null;
            case "onRespawn": b.onRespawn(npc); return null;
            case "onDeathStarted": b.onDeathStarted(npc, other); return null;
            case "onDeathFinished": b.onDeathFinished(npc, other); return null;
            case "beforeAttackFinalized": b.beforeAttackFinalized(npc, other, state((Map) op.getOrDefault("state", Collections.emptyMap()))); return null;
            case "onDropTableRolled": b.onDropTableRolled(npc, other, new ArrayList<>()); return null;
            default: throw new AssertionError(op);
        }
    }

    static Map snapshot() throws Exception {
        Map m = new TreeMap();
        m.put("ticks", ticks);
        m.put("now", now);
        m.put("nextPulse", nextPulse);
        List list = new ArrayList();
        for (Entity e : entities) {
            Map s = new TreeMap();
            Map attrs = new TreeMap();
            for (Object o : e.attributes.entrySet()) {
                Map.Entry en = (Map.Entry) o;
                if (Entity.TICK_KEYS.contains(en.getKey())) continue;
                Object v = en.getValue();
                attrs.put(en.getKey(), v instanceof Entity ? "e" + ix(v) : v);
            }
            s.put("attrs", attrs);
            Entity.CombatPulse c = e.getProperties().getCombatPulse();
            s.put("attacking", c.attacking);
            s.put("victim", ix(c.victim));
            s.put("nextAttack", c.nextAttack);
            s.put("lastSent", c.lastSentAttack);
            s.put("lastReceived", c.lastReceivedAttack);
            s.put("location", coord(e.getLocation()));
            if (e instanceof NPC) {
                NPC npc = (NPC) e;
                s.put("walkRadius", npc.getWalkRadius());
                s.put("nextWalk", npc.getNextWalk());
                s.put("respawn", npc.getRespawnTick());
                s.put("respawning", npc.isRespawning);
                s.put("aggressive", npc.isAggressive());
                s.put("invisible", npc.invisibleField());
                s.put("returning", npc.attributes.getOrDefault("return-to-spawn", false));
                Object pulse = npc.attributes.get("return-to-spawn-pulse");
                s.put("returnPulse", pulse == null ? null : ((MovementPulse) pulse).id);
                s.put("lastTalk", npc.attributes.getOrDefault("lastForceTalk", 0));
                AggressiveHandler a = npc.getAggressiveHandler();
                if (a == null) s.put("handler", null);
                else {
                    Map h = new TreeMap();
                    h.put("radius", a.getRadius());
                    h.put("pause", a.getPauseTicks());
                    h.put("switching", a.isTargetSwitching());
                    h.put("chance", a.getChanceRatio());
                    h.put("tolerance", a.getPlayerTolerance().length);
                    h.put("allowTolerance", field(AggressiveHandler.class, "allowTolerance").getBoolean(a));
                    h.put("behaviour", field(AggressiveHandler.class, "behavior").get(a) == AggressiveBehavior.WILDERNESS ? "wilderness" : "default");
                    s.put("handler", h);
                }
                s.put("behaviour", tag(npc.behavior));
            }
            list.add(s);
        }
        m.put("entities", list);
        return m;
    }

    static String classify(Throwable t) {
        if (t instanceof Failure) return "Binding";
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        return t.getClass().getSimpleName();
    }

    public static void main(String[] args) throws Exception {
        installMath();
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        String line;
        while ((line = reader.readLine()) != null) {
            input = (Map) new JSONParser().parse(line);
            events = new ArrayList<>();
            calls = 0;
            failAt = -1;
            consumed = new HashMap<>();
            regions = new HashMap<>();
            entities = new ArrayList<>();
            ticks = n(input.getOrDefault("ticks", 100L));
            now = ((Number) input.getOrDefault("now", 1000000000L)).longValue();
            nextPulse = 0;
            random = new Rng(((Number) input.getOrDefault("seed", 0L)).longValue());
            RandomFunction.RANDOM = random;
            math.armed = false;
            math.setSeed(((Number) input.getOrDefault("mathSeed", 0L)).longValue());
            math.armed = true;
            clearRegistry();
            for (Object o : (List) input.getOrDefault("behaviours", Collections.emptyList())) {
                Map b = (Map) o;
                List ids = (List) b.get("ids");
                int[] array = new int[ids.size()];
                for (int i = 0; i < array.length; i++) array[i] = n(ids.get(i));
                NPCBehavior.Companion.register(array, new RecordingBehavior((String) b.get("tag")));
            }
            for (Object o : (List) input.get("entities")) entities.add(create((Map) o));
            for (int i = 0; i < entities.size(); i++) apply(entities.get(i), (Map) ((List) input.get("entities")).get(i));
            List out = new ArrayList();
            for (Object o : (List) input.get("ops")) {
                Map op = (Map) o;
                events = new ArrayList<>();
                calls = 0;
                failAt = op.containsKey("failAt") ? n(op.get("failAt")) : -1;
                Object result = null;
                String error = null;
                try {
                    result = run(op);
                } catch (Throwable t) {
                    error = classify(t);
                }
                failAt = -1;
                Map row = new TreeMap();
                row.put("result", result);
                row.put("error", error);
                row.put("events", events);
                row.put("state", snapshot());
                out.add(row);
            }
            Map result = new TreeMap();
            result.put("input", input);
            result.put("expected", out);
            System.out.println(JSONValue.toJSONString(result));
        }
    }
}
