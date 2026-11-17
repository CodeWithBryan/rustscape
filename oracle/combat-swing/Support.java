package oracle;

import java.io.*;
import java.lang.reflect.Field;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.*;
import core.game.node.entity.combat.equipment.*;
import core.game.node.entity.combat.spell.CombatSpell;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Graphics;
import core.tools.RandomFunction;

/**
 * Drives the unmodified swing handlers for one JSON case per input line. Every
 * call a handler makes outside its slice arrives here as an ordered event; random
 * draws are events too. Doubles cross the boundary as 16 hex digits of raw bits.
 */
public class Support {
    public static Map input;
    public static final List ZERO15 = Collections.nCopies(15, 0L);
    static final List<Object> events = new ArrayList<>();
    static int calls, failAt = -1;
    static boolean quiet;
    static List<Entity> entities;
    static List<CombatSpell> spells;
    static CombatSwingHandler handler;
    static Random mathBase, intBase;
    static List mathScript, intScript;
    static int mathAt, intAt, mathDraws;
    static Double forced;

    public static class Failure extends RuntimeException {}

    public static RuntimeException unsupported(String what) {
        return new UnsupportedOperationException(what);
    }

    public static void emit(String name, Object... args) {
        if (quiet) return;
        List<Object> row = new ArrayList<>();
        row.add(name);
        row.addAll(Arrays.asList(args));
        events.add(row);
        if (++calls == failAt) throw new Failure();
    }

    public static int n(Object o) {
        // Case integers must be Java ints already; a silently wrapped input is a corpus mistake.
        long v = ((Number) o).longValue();
        if (v != (int) v || o instanceof Double) throw new IllegalStateException("corpus integer " + o);
        return (int) v;
    }
    public static int i(Map m, String key, int fallback) { return m.get(key) == null ? fallback : n(m.get(key)); }
    public static double bits(Object hex) { return Double.longBitsToDouble(Long.parseUnsignedLong((String) hex, 16)); }
    public static double d(Map m, String key, double fallback) { return m.get(key) == null ? fallback : bits(m.get(key)); }
    public static String hex(double v) { return String.format("%016x", Double.doubleToRawLongBits(v)); }
    public static boolean has(Map m, String key, String name) { List l = (List) m.get(key); return l != null && l.contains(name); }
    public static int keyed(Map m, String key, int slot, int fallback) { Map k = (Map) m.get(key); Object v = k == null ? null : k.get(String.valueOf(slot)); return v == null ? fallback : n(v); }
    public static int[] ints(Object o) { if (o == null) return null; List l = (List) o; int[] a = new int[l.size()]; for (int k = 0; k < a.length; k++) a[k] = n(l.get(k)); return a; }
    public static Location loc(Object o) { if (o == null) return null; List a = (List) o; return new Location(n(a.get(0)), n(a.get(1)), n(a.get(2))); }
    public static Location place(Map spec) { return spec.containsKey("location") ? loc(spec.get("location")) : new Location(3200, 3200, 0); }
    public static Object coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    public static Object tag(Entity e) { return e == null ? null : (Object) e.tag; }
    public static Entity entity(Object index) {
        if (index == null) return null;
        // A case naming an entity it does not define is a corpus mistake, never an original failure.
        if (n(index) < 0 || n(index) >= entities.size()) throw new IllegalStateException("corpus entity index " + index);
        return entities.get(n(index));
    }
    public static String placeholderName(int id) { return "item" + id; }

    /** Math.random's private generator: the seeded or scripted source of this case. */
    static class MathRng extends Random {
        @Override public double nextDouble() {
            double v;
            if (forced != null) v = forced;
            else if (mathScript != null) {
                if (mathAt >= mathScript.size()) throw new IllegalStateException("math draws exhausted");
                v = bits(mathScript.get(mathAt++));
            } else v = mathBase.nextDouble();
            mathDraws++;
            emit("mathRandom", hex(v));
            return v;
        }
    }

    /** RandomFunction.RANDOM: only nextInt(bound) is reached by the selected methods. */
    static class IntRng extends Random {
        @Override public int nextInt(int bound) {
            int v;
            if (intScript != null) {
                if (intAt >= intScript.size()) throw new IllegalStateException("int draws exhausted");
                Object o = intScript.get(intAt++);
                // A scripted integer is reduced into the range nextInt(bound) can return.
                v = "max".equals(o) ? bound - 1 : Math.floorMod(n(o), bound);
            } else v = intBase.nextInt(bound);
            emit("nextInt", bound, v);
            return v;
        }
        @Override public int nextInt() { throw unsupported("Random.nextInt()"); }
        @Override public boolean nextBoolean() { throw unsupported("Random.nextBoolean"); }
        @Override public double nextDouble() { throw unsupported("RandomFunction.RANDOM.nextDouble"); }
    }

    /** Replaces java.lang.Math's lazily created generator once, before any compiled use. */
    static void installMath() throws Exception {
        Math.random();
        Field holder = Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator");
        Field access = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        access.setAccessible(true);
        sun.misc.Unsafe unsafe = (sun.misc.Unsafe) access.get(null);
        unsafe.putObject(unsafe.staticFieldBase(holder), unsafe.staticFieldOffset(holder), new MathRng());
        forced = 0.25;
        quiet = true;
        if (Math.random() != 0.25) throw new IllegalStateException("Math.random is not bound to the oracle generator");
        forced = null;
        quiet = false;
    }

    static void world(Map c) {
        input = c;
        entities = new ArrayList<>();
        spells = new ArrayList<>();
        List specs = (List) c.getOrDefault("spells", Collections.emptyList());
        for (int k = 0; k < specs.size(); k++) spells.add(new CombatSpell(k, (Map) specs.get(k)));
        List es = (List) c.getOrDefault("entities", Collections.emptyList());
        for (int k = 0; k < es.size(); k++) {
            Map s = (Map) es.get(k);
            String kind = (String) s.getOrDefault("kind", "player");
            Entity e = kind.equals("player") ? new Player(k, s) : kind.equals("npc") ? new NPC(k, s)
                : kind.equals("familiar") ? new content.global.skill.summoning.familiar.Familiar(k, s) : new Entity(k, s);
            if (s.get("spell") != null) e.spell = spells.get(n(s.get("spell")));
            if (s.get("autocast") != null) e.autocast = spells.get(n(s.get("autocast")));
            entities.add(e);
        }
        Map h = (Map) c.get("handler");
        List fl = (List) h.getOrDefault("flags", Collections.emptyList());
        SwingHandlerFlag[] flags = new SwingHandlerFlag[fl.size()];
        for (int k = 0; k < flags.length; k++) flags[k] = SwingHandlerFlag.valueOf((String) fl.get(k));
        String kind = (String) h.get("kind");
        handler = kind.equals("melee") ? new MeleeSwingHandler(flags) : kind.equals("range") ? new RangeSwingHandler(flags) : new MagicSwingHandler(flags);
        RangeWeapon.getRangeWeapons().clear();
        Ammunition.getAmmunition().clear();
        Map range = (Map) c.getOrDefault("range", Collections.emptyMap());
        for (Object o : (List) range.getOrDefault("weapons", Collections.emptyList())) {
            Map w = (Map) o;
            List<Integer> ammo = new ArrayList<>();
            for (Object a : (List) w.get("ammunition")) ammo.add(n(a));
            RangeWeapon.getRangeWeapons().put(n(w.get("itemId")), new RangeWeapon(n(w.get("itemId")), null, i(w, "attackSpeed", 4), n(w.get("slot")), n(w.get("type")), !Boolean.FALSE.equals(w.get("dropAmmo")), ammo));
        }
        for (Object o : (List) range.getOrDefault("ammo", Collections.emptyList())) {
            Map a = (Map) o;
            Ammunition ammo = new Ammunition(n(a.get("itemId")), null, null, null, i(a, "poisonDamage", 0));
            // RangedConfigLoader's rule: the effect is BoltEffect.forId(itemId) when present.
            BoltEffect effect = BoltEffect.forId(n(a.get("itemId")));
            if (effect != null) ammo.setEffect(effect);
            Ammunition.getAmmunition().put(n(a.get("itemId")), ammo);
        }
        Map rng = (Map) c.getOrDefault("rng", Collections.emptyMap());
        mathScript = (List) rng.get("math");
        intScript = (List) rng.get("ints");
        mathBase = new Random(((Number) rng.getOrDefault("mathSeed", 0L)).longValue());
        intBase = new Random(((Number) rng.getOrDefault("intSeed", 0L)).longValue());
        mathAt = intAt = 0;
        RandomFunction.RANDOM = new IntRng();
    }

    static Entity ent(Map op, String key) { return entity(op.get(key)); }
    static CombatStyle style(Object o) { return o == null ? null : CombatStyle.valueOf((String) o); }

    static BattleState state(Map s) {
        if (s == null) s = Collections.emptyMap();
        BattleState b = Boolean.FALSE.equals(s.get("ctor")) ? new BattleState() : new BattleState(entity(s.get("attacker")), entity(s.get("victim")));
        if (s.containsKey("estimatedHit")) b.setEstimatedHit(n(s.get("estimatedHit")));
        if (s.containsKey("maximumHit")) b.setMaximumHit(n(s.get("maximumHit")));
        if (s.containsKey("secondaryHit")) b.setSecondaryHit(n(s.get("secondaryHit")));
        if (s.containsKey("recoilDamage")) b.setRecoilDamage(n(s.get("recoilDamage")));
        if (s.containsKey("poisonDamage")) b.setPoisonDamage(n(s.get("poisonDamage")));
        if (s.containsKey("frozen")) b.setFrozen(Boolean.TRUE.equals(s.get("frozen")));
        if (s.get("armourEffect") != null) b.setArmourEffect(ArmourSet.valueOf((String) s.get("armourEffect")));
        if (s.get("style") != null) b.setStyle(style(s.get("style")));
        if (s.containsKey("targets")) {
            List l = (List) s.get("targets");
            BattleState[] t = new BattleState[l.size()];
            for (int k = 0; k < t.length; k++) t[k] = l.get(k) == null ? null : "this".equals(l.get(k)) ? b : state((Map) l.get(k));
            b.setTargets(t);
        }
        return b;
    }

    static Object item(Item i) { return i == null ? null : Arrays.asList(i.ref, i.getId(), i.getAmount(), i.getCharge(), i.getName()); }

    static Map dump(BattleState b) {
        Map<String, Object> m = new TreeMap<>();
        m.put("attacker", tag(b.getAttacker()));
        m.put("victim", tag(b.getVictim()));
        m.put("estimatedHit", b.getEstimatedHit());
        m.put("maximumHit", b.getMaximumHit());
        m.put("secondaryHit", b.getSecondaryHit());
        m.put("recoilDamage", b.getRecoilDamage());
        m.put("poisonDamage", b.getPoisonDamage());
        m.put("frozen", b.isFrozen());
        List<Object> targets = null;
        if (b.getTargets() != null) {
            targets = new ArrayList<>();
            for (BattleState s : b.getTargets()) targets.add(s == null ? null : s == b ? "this" : dump(s));
        }
        m.put("targets", targets);
        Weapon w = b.getWeapon();
        Map<String, Object> weapon = null;
        if (w != null) {
            weapon = new TreeMap<>();
            weapon.put("id", w.getId());
            weapon.put("name", w.getName());
            weapon.put("item", item(w.getItem()));
            weapon.put("ammunitionSlot", w.getAmmunitionSlot());
            weapon.put("ammunition", item(w.getAmmunition()));
            weapon.put("type", w.getType() == null ? null : w.getType().name());
        }
        m.put("weapon", weapon);
        m.put("rangeWeapon", b.getRangeWeapon() == null ? null : (Object) b.getRangeWeapon().getItemId());
        m.put("ammunition", b.getAmmunition() == null ? null : (Object) b.getAmmunition().getItemId());
        m.put("spell", b.getSpell() == null ? null : (Object) b.getSpell().idx);
        m.put("style", b.getStyle() == null ? null : b.getStyle().name());
        m.put("armourEffect", b.getArmourEffect() == null ? null : b.getArmourEffect().name());
        m.put("totalDamage", b.getTotalDamage());
        return m;
    }

    static long key(double v) { long b = Double.doubleToRawLongBits(v); return b >= 0 ? b : -(b & Long.MAX_VALUE) - 1; }
    static double unkey(long k) { return Double.longBitsToDouble(k >= 0 ? k : (-(k + 1)) | Long.MIN_VALUE); }

    static boolean drawn(Map op, double draw) {
        forced = draw;
        mathDraws = 0;
        boolean hit = handler.isAccurateImpact(ent(op, "e"), ent(op, "v"), style(op.get("style")), bits(op.get("accMod")), bits(op.get("defMod")));
        if (mathDraws != 1) throw new IllegalStateException("nodraw");
        return hit;
    }

    /**
     * The exact acceptance threshold of the original decision: the least double d
     * (in numeric order, -0.0 before +0.0) for which a draw of d is not accurate.
     */
    static String threshold(Map op) {
        quiet = true;
        try {
            if (!drawn(op, Double.NEGATIVE_INFINITY)) return "never";
            long lo = key(Double.NEGATIVE_INFINITY), hi = key(Double.POSITIVE_INFINITY);
            while (hi - lo != 1) {
                long mid = (lo >> 1) + (hi >> 1) + (lo & hi & 1);
                if (drawn(op, unkey(mid))) lo = mid; else hi = mid;
            }
            return hex(unkey(hi));
        } catch (IllegalStateException e) {
            if ("nodraw".equals(e.getMessage())) return "nodraw";
            throw e;
        } finally {
            quiet = false;
            forced = null;
        }
    }

    static Object graphics(Object g) throws Exception { return g == null ? null : Arrays.asList(((Graphics) g).getId(), ((Graphics) g).getHeight(), ((Graphics) g).getDelay()); }
    static Object field(Class<?> c, Object target, String name) throws Exception { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(target); }

    static Object tables() throws Exception {
        Map<String, Object> out = new TreeMap<>();
        List<Object> interfaces = new ArrayList<>();
        for (WeaponInterface.WeaponInterfaces w : WeaponInterface.WeaponInterfaces.values()) {
            List<Object> styles = new ArrayList<>();
            for (WeaponInterface.AttackStyle s : w.getAttackStyles()) styles.add(Arrays.asList(s.getStyle(), s.getBonusType()));
            interfaces.add(Arrays.asList(w.name(), w.getInterfaceId(), styles));
        }
        out.put("weaponInterfaces", interfaces);
        List<Object> bolts = new ArrayList<>();
        for (BoltEffect b : BoltEffect.values()) {
            Object sound = field(BoltEffect.class, b, "sound");
            bolts.add(Arrays.asList(b.name(), b.getItemId(), graphics(field(BoltEffect.class, b, "graphics")), sound == null ? null : (Object) ((core.game.node.entity.player.link.audio.Audio) sound).id));
        }
        out.put("boltEffects", bolts);
        List<Object> lookups = new ArrayList<>();
        for (int id : new int[] { -1, 0, 9235, 9236, 9237, 9238, 9239, 9240, 9241, 9242, 9243, 9244, 9245, 9246 }) {
            BoltEffect b = BoltEffect.forId(id);
            lookups.add(Arrays.asList(id, b == null ? null : b.name()));
        }
        out.put("boltLookups", lookups);
        List<Object> sets = new ArrayList<>();
        for (ArmourSet a : ArmourSet.values()) {
            List<Object> rows = new ArrayList<>();
            for (Item[] row : (Item[][]) field(ArmourSet.class, a, "sets")) {
                List<Object> ids = new ArrayList<>();
                for (Item it : row) ids.add(it.getId());
                rows.add(ids);
            }
            sets.add(Arrays.asList(a.name(), graphics(field(ArmourSet.class, a, "endGraphic")), rows));
        }
        out.put("armourSets", sets);
        List<Object> types = new ArrayList<>();
        for (Weapon.WeaponType t : Weapon.WeaponType.values()) types.add(t.name());
        out.put("weaponTypes", types);
        List<Object> flags = new ArrayList<>();
        for (SwingHandlerFlag f : SwingHandlerFlag.values()) flags.add(f.name());
        out.put("flags", flags);
        List<Object> styles = new ArrayList<>();
        for (CombatStyle s : CombatStyle.values()) styles.add(Arrays.asList(s.name(), s.getProtectionPrayer().name(), s.getSwingHandler().getType().name()));
        out.put("combatStyles", styles);
        List<Object> interactions = new ArrayList<>();
        for (InteractionType t : InteractionType.values()) interactions.add(t.name());
        out.put("interactionTypes", interactions);
        return out;
    }

    static BattleState current;

    static Object run(Map op) throws Exception {
        String name = (String) op.get("op");
        switch (name) {
            case "accuracy": return handler.calculateAccuracy(ent(op, "e"));
            case "defence": return handler.calculateDefence(ent(op, "v"), ent(op, "a"));
            case "hit": return handler.calculateHit(ent(op, "e"), ent(op, "v"), bits(op.get("mod")));
            case "setMultiplier": return hex(handler.getSetMultiplier(ent(op, "e"), n(op.get("skill"))));
            case "armourSet": { ArmourSet a = handler.getArmourSet(ent(op, "e")); return a == null ? null : a.name(); }
            case "accurate":
                if (!op.containsKey("style")) return handler.isAccurateImpact(ent(op, "e"), ent(op, "v"));
                if (!op.containsKey("accMod")) return handler.isAccurateImpact(ent(op, "e"), ent(op, "v"), style(op.get("style")));
                return handler.isAccurateImpact(ent(op, "e"), ent(op, "v"), style(op.get("style")), bits(op.get("accMod")), bits(op.get("defMod")));
            case "threshold": return threshold(op);
            case "boundary": {
                // Three real decisions with draws just below, at and just above the threshold.
                Map<String, Object> out = new TreeMap<>();
                String t = threshold(op);
                out.put("threshold", t);
                if (t.length() != 16) return out;
                double at = bits(t);
                List<Object> draws = new ArrayList<>(), results = new ArrayList<>();
                for (double draw : new double[] { Math.nextDown(at), at, Math.nextUp(at) }) {
                    forced = draw;
                    try {
                        results.add(handler.isAccurateImpact(ent(op, "e"), ent(op, "v"), style(op.get("style")), bits(op.get("accMod")), bits(op.get("defMod"))));
                    } finally { forced = null; }
                    draws.add(hex(draw));
                }
                out.put("draws", draws);
                out.put("results", results);
                return out;
            }
            case "setType": handler.setType(style(op.get("style"))); return null;
            case "swing": current = state((Map) op.get("state")); return handler.swing(ent(op, "e"), ent(op, "v"), current);
            case "combatDistance": return handler.getCombatDistance(ent(op, "e"), ent(op, "v"), n(op.get("raw")));
            case "formatHit": return handler.formatHit(ent(op, "v"), n(op.get("raw")));
            case "dragonfire": return handler.getDragonfireMessage(n(op.get("protection")), (String) op.get("name"));
            case "total": current = state((Map) op.get("state")); return current.getTotalDamage();
            case "neutralize": current = state((Map) op.get("state")); current.neutralizeHits(); return null;
            case "tables": return tables();
            default: throw new AssertionError(name);
        }
    }

    static String classify(Throwable e) {
        if (e instanceof Failure) return "Binding";
        if (e instanceof NullPointerException) return "Null";
        if (e instanceof IndexOutOfBoundsException) return "Index";
        return e.getClass().getSimpleName() + ":" + e.getMessage();
    }

    public static void main(String[] args) throws Exception {
        installMath();
        PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), false, "UTF-8");
        try (BufferedReader r = new BufferedReader(new InputStreamReader(System.in, java.nio.charset.StandardCharsets.UTF_8))) {
            String line;
            while ((line = r.readLine()) != null) {
                Map c = (Map) new JSONParser().parse(line);
                world(c);
                List<Object> results = new ArrayList<>();
                for (Object o : (List) c.get("ops")) {
                    Map op = (Map) o;
                    events.clear();
                    calls = 0;
                    failAt = i(op, "failAt", -1);
                    current = null;
                    Map<String, Object> row = new TreeMap<>();
                    Object result = null;
                    String error = null;
                    try { result = run(op); } catch (Throwable e) { error = classify(e); }
                    failAt = -1;
                    row.put("result", result);
                    row.put("error", error);
                    row.put("events", new ArrayList<>(events));
                    if ("swing".equals(op.get("op"))) {
                        List<Object> flags = new ArrayList<>();
                        for (SwingHandlerFlag f : handler.getFlags()) flags.add(f.name());
                        row.put("flags", flags);
                    }
                    if ("setType".equals(op.get("op"))) row.put("type", handler.getType() == null ? null : handler.getType().name());
                    if (current != null) { quiet = true; row.put("state", dump(current)); quiet = false; }
                    results.add(row);
                }
                out.println(JSONValue.toJSONString(results));
            }
        }
        out.flush();
    }
}
