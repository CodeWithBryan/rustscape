package oracle;

import java.io.*;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.*;
import core.game.node.entity.combat.equipment.*;
import core.game.node.entity.combat.spell.CombatSpell;
import core.game.node.entity.impl.Animator.Priority;
import core.game.node.entity.impl.Projectile;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.Settings;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import core.game.interaction.NodeUsageEvent;
import core.game.system.task.Pulse;
import core.game.system.timer.RSTimer;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import core.tools.RandomFunction;
import content.global.handlers.item.equipment.*;
import content.global.handlers.item.equipment.special.ChinchompaSwingHandler;
import content.global.handlers.item.equipment.special.SalamanderSwingHandler;
import content.global.handlers.npc.*;

/**
 * Drives the unmodified equipment registers (BarrowsEquipment,
 * EquipmentDegrader and its three registers, HastaEquipListener), the
 * chinchompa and salamander swing handlers (on the unmodified combat
 * handlers, CombatPulse, ImpactHandler and Pulse) and the creature classes
 * (SheepBehavior, ChinchompaDeathListener, ShooAwayStrayDogListener,
 * CowMilkingPlugin, DecantListener) for one JSON case per input line. Every
 * call they make outside their slice arrives here as an ordered event; random
 * draws are events too. The world model (ticks, the pulse queue and what the
 * stand-ins keep per entity) is an oracle input. Doubles cross the boundary
 * as 16 hex digits of raw bits.
 */
public class Support {
    public static Map input;
    public static final List ZERO15 = Collections.nCopies(15, 0L);
    /** CalquatDecant's keg ids: another slice, a fixed list here. */
    public static final int[] KEGS = {5779, 5777, 5775, 5773, 5859, 5857, 5855, 5853};
    static final List<Object> events = new ArrayList<>();
    static int calls, failAt = -1;
    static List<Entity> entities;
    static List<CombatSpell> spells;
    public static List<CombatSwingHandler> handlers;
    /** The handlers the listeners registered: by option, then the use-with and equip handlers in order. */
    static Map<String, Object> listeners;
    static List<Object> useWiths, equips;
    static Map<String, Integer> answered;
    static CowMilkingPlugin cow;
    public static int dialogueTag;
    public static final LinkedList<Pulse> queue = new LinkedList<>();
    public static int tick;
    public static Map clip, pclip, uids;
    /** The case entity an AbstractNPC constructor is making. */
    public static int pendingTag;
    public static Map pendingSpec;
    public static List inactive;
    public static ArrayList<Object> duelAmmo;
    static Random mathBase, intBase;
    static List mathScript, intScript;
    static int mathAt, intAt;
    static PrintStream realErr;

    public static class Failure extends RuntimeException {}

    public static RuntimeException unsupported(String what) {
        return new UnsupportedOperationException(what);
    }

    public static void emit(String name, Object... args) {
        List<Object> row = new ArrayList<>();
        row.add(name);
        row.addAll(Arrays.asList(args));
        events.add(row);
        if (++calls == failAt) throw new Failure();
    }

    static void mark(Object... args) {
        events.add(new ArrayList<>(Arrays.asList(args)));
    }

    public static int n(Object o) {
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
    public static Object graphic(Graphics g) { return g == null ? null : Arrays.asList(g.getId(), g.getHeight(), g.getDelay()); }
    static Graphics graphics(Object o) { if (o == null) return null; List g = (List) o; return new Graphics(n(g.get(0)), n(g.get(1)), n(g.get(2))); }
    public static Entity entity(Object index) {
        if (index == null) return null;
        if (n(index) < 0 || n(index) >= entities.size()) throw new IllegalStateException("corpus entity index " + index);
        return entities.get(n(index));
    }
    public static String placeholderName(int id) { return "item" + id; }
    public static int flag(Map m, int z, int x, int y) { Object v = m == null ? null : m.get(x + "," + y + "," + z); return v == null ? 0 : n(v); }
    public static Player byUid(int uid) { emit("playerByUid", uid); Object v = uids == null ? null : uids.get(String.valueOf(uid)); return (Player) entity(v); }
    /** hasTimerActive<T>(entity): the case lists the active timers by class simple name. */
    public static boolean timer(Entity e, String name) {
        if (name.equals("Miasmic")) { emit("miasmic", e.tag); return Boolean.TRUE.equals(e.spec.get("miasmic")); }
        emit("timerActive", e.tag, name);
        return has(e.spec, "timers", name);
    }
    public static void script(BattleState s, Object spec) {
        if (spec == null) return;
        Map m = (Map) spec;
        if (m.containsKey("estimatedHit")) s.setEstimatedHit(n(m.get("estimatedHit")));
        if (m.containsKey("secondaryHit")) s.setSecondaryHit(n(m.get("secondaryHit")));
    }
    /** Skills.updateLevel's answer: the entity's next scripted value, 0 when none is left. */
    public static int levelAnswer(Entity e) {
        List l = (List) e.spec.get("updateLevelReturns");
        if (l == null || e.levelAnswer >= l.size()) return 0;
        return n(l.get(e.levelAnswer++));
    }
    /** A scripted boolean answer: the entity's next value of the list under key, the fallback when none is left. */
    public static boolean answer(Entity e, String key, boolean fallback) {
        List l = (List) e.spec.get(key);
        String k = e.tag + "|" + key;
        int at = answered.getOrDefault(k, 0);
        answered.put(k, at + 1);
        if (l == null || at >= l.size()) return fallback;
        return Boolean.TRUE.equals(l.get(at));
    }
    /** A scripted pair of item lists (remove, add): [[id, amount, charge]...]. */
    public static kotlin.Pair<List<Item>, List<Item>> lists(Entity e, String key) {
        List l = (List) e.spec.get(key);
        List<Item> remove = new ArrayList<>(), add = new ArrayList<>();
        if (l != null) {
            for (Object o : (List) l.get(0)) { List r = (List) o; remove.add(new Item(n(r.get(0)), n(r.get(1)), n(r.get(2)))); }
            for (Object o : (List) l.get(1)) { List r = (List) o; add.add(new Item(n(r.get(0)), n(r.get(1)), n(r.get(2)))); }
        }
        return new kotlin.Pair<>(remove, add);
    }

    /** A timer the registry answers: its identifier. */
    public static class Spawned extends RSTimer {
        public Spawned(String identifier) { super(0, identifier, false, false, new core.game.system.timer.TimerFlag[0]); }
        @Override public boolean run(Entity e) { throw unsupported("spawned timer run"); }
    }
    static List<Object> args(Object[] args) {
        List<Object> out = new ArrayList<>();
        for (Object a : args) {
            if (a instanceof Integer || a instanceof Boolean) out.add(a);
            else throw unsupported("timer argument " + a);
        }
        return out;
    }
    public static RSTimer spawnTimer(String identifier, Object[] args) {
        emit("spawnTimer", identifier, args(args));
        return has(input, "nullTimers", identifier) ? null : new Spawned(identifier);
    }
    public static void registerTimer(Entity e, RSTimer t) {
        if (!(t instanceof Spawned)) throw new IllegalStateException("unknown timer " + t.getClass().getName());
        List<Object> flags = new ArrayList<>();
        for (core.game.system.timer.TimerFlag f : t.getFlags()) flags.add(f.name());
        emit("registerTimer", e.tag, "spawned", t.getIdentifier(), t.getRunInterval(), t.isSoft(), t.isAuto(), flags);
    }

    static List<Object> list(int[] ids) {
        if (ids == null) return null;
        List<Object> shown = new ArrayList<>();
        for (int id : ids) shown.add(id);
        return shown;
    }
    /** InteractionListener.on([ids,] type, options, handler): recorded, the handler kept per option. */
    public static void listen(int[] ids, int type, String[] options, Object handler) {
        emit("on", list(ids), type, new ArrayList<>(Arrays.asList(options)));
        for (String o : options) listeners.put(o, handler);
    }
    /** InteractionListener.onUseWith(type, used, with..., handler): recorded, the handler kept in order. */
    public static void useWith(int[] used, int type, int[] with, Object handler) {
        emit("onUseWith", list(used), type, list(with));
        useWiths.add(handler);
    }
    /** InteractionListener.onEquip(ids, handler): recorded, the handler kept in order. */
    public static void equip(int[] ids, Object handler) {
        emit("onEquip", list(ids));
        equips.add(handler);
    }

    /** ContentAPI replaceSlot's effect on the model: the slot holds the new item. */
    public static void replaceSlot(Player p, int slot, Item item, String container) {
        if (container.equals("EQUIPMENT")) p.getEquipment().store(slot, item);
        else if (container.equals("INVENTORY")) p.getInventory().store(slot, item);
        else throw unsupported("replaceSlot " + container);
    }

    public static String label(Pulse p) {
        if (p instanceof CombatPulse) return "combat:" + ((CombatPulse) p).getEntity().tag;
        String name = p.getClass().getName();
        if (name.startsWith("core.game.node.entity.combat.CombatPulse$Companion$swing$")) return "impact";
        if (name.startsWith("core.game.node.entity.combat.RangeSwingHandler$impact$")) return "secondHit";
        if (name.startsWith("core.game.node.entity.combat.ImpactHandler$")) return "manualHit";
        if (name.startsWith("content.global.handlers.npc.SheepBehavior$")) return "retransform";
        if (name.startsWith("content.global.handlers.npc.CowMilkingPlugin$")) return "milk";
        throw new IllegalStateException("unknown pulse " + name);
    }

    static class MathRng extends Random {
        @Override public double nextDouble() {
            double v;
            if (mathScript != null) {
                if (mathAt >= mathScript.size()) throw new IllegalStateException("math draws exhausted");
                v = bits(mathScript.get(mathAt++));
            } else v = mathBase.nextDouble();
            emit("mathRandom", hex(v));
            return v;
        }
    }

    static class IntRng extends Random {
        @Override public int nextInt(int bound) {
            int v;
            if (intScript != null) {
                if (intAt >= intScript.size()) throw new IllegalStateException("int draws exhausted");
                Object o = intScript.get(intAt++);
                v = "max".equals(o) ? bound - 1 : Math.floorMod(n(o), bound);
            } else v = intBase.nextInt(bound);
            emit("nextInt", bound, v);
            return v;
        }
        @Override public int nextInt() { throw unsupported("Random.nextInt()"); }
        @Override public boolean nextBoolean() { throw unsupported("Random.nextBoolean"); }
        @Override public double nextDouble() { throw unsupported("RandomFunction.RANDOM.nextDouble"); }
    }

    static void installMath() throws Exception {
        Math.random();
        Field holder = Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator");
        Field access = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        access.setAccessible(true);
        sun.misc.Unsafe unsafe = (sun.misc.Unsafe) access.get(null);
        unsafe.putObject(unsafe.staticFieldBase(holder), unsafe.staticFieldOffset(holder), new MathRng());
        mathScript = Collections.singletonList(hex(0.25));
        mathAt = 0;
        if (Math.random() != 0.25) throw new IllegalStateException("Math.random is not bound to the oracle generator");
        events.clear();
    }

    static void installErr() {
        realErr = System.err;
        System.setErr(new PrintStream(new OutputStream() { @Override public void write(int b) {} }) {
            @Override public void println(Object x) { if (x instanceof Throwable) emit("printStackTrace", classify((Throwable) x)); }
        });
    }

    static Object field(Class<?> c, Object target, String name) throws Exception { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(target); }
    static void setField(Class<?> c, Object target, String name, Object value) throws Exception { Field f = c.getDeclaredField(name); f.setAccessible(true); f.set(target, value); }

    static CombatSwingHandler make(Map h) {
        List fl = (List) h.getOrDefault("flags", Collections.emptyList());
        SwingHandlerFlag[] flags = new SwingHandlerFlag[fl.size()];
        for (int k = 0; k < flags.length; k++) flags[k] = SwingHandlerFlag.valueOf((String) fl.get(k));
        String kind = (String) h.get("kind");
        return kind.equals("melee") ? new MeleeSwingHandler(flags) : kind.equals("range") ? new RangeSwingHandler(flags) : new MagicSwingHandler(flags);
    }

    static void reset(CombatSwingHandler h, CombatStyle type) throws Exception {
        h.setType(type);
        h.getFlags().clear();
        setField(CombatSwingHandler.class, h, "specialHandlers", null);
        handlers.add(h);
    }

    static void register(String name) throws Throwable {
        switch (name) {
            case "barrows": new BarrowsEquipmentRegister().newInstance(null); break;
            case "pvp": new PVPEquipmentRegister().newInstance(null); break;
            case "crystal": new CrystalEquipmentRegister().startup(); break;
            default: throw new AssertionError(name);
        }
    }

    static void world(Map c) throws Throwable {
        input = c;
        entities = new ArrayList<>();
        spells = new ArrayList<>();
        queue.clear();
        duelAmmo = new ArrayList<>();
        listeners = new HashMap<>();
        useWiths = new ArrayList<>();
        equips = new ArrayList<>();
        answered = new HashMap<>();
        cow = null;
        dialogueTag = -1;
        tick = i(c, "tick", 0);
        clip = (Map) c.get("clip");
        pclip = (Map) c.get("pclip");
        uids = (Map) c.get("uids");
        inactive = (List) c.getOrDefault("inactiveRegions", Collections.emptyList());
        core.ServerConstants.DRAGON_AXE_USE_OSRS_SPEC = false;
        core.ServerConstants.BETTER_DFS = false;
        CombatSwingHandler.EXPERIENCE_MOD = 4.0;
        // The CombatStyle singletons and the two handler singletons are process state: every case starts them fresh.
        handlers = new ArrayList<>();
        for (CombatStyle s : CombatStyle.values()) reset(s.getSwingHandler(), s);
        reset(MagicalRangedSwingHandler.INSTANCE, CombatStyle.RANGE);
        reset(MagicalMeleeSwingHandler.INSTANCE, CombatStyle.MELEE);
        reset(RangedMeleeSwingHandler.INSTANCE, CombatStyle.MELEE);
        reset(ChinchompaSwingHandler.getInstance(), CombatStyle.RANGE);
        SalamanderSwingHandler salamander = SalamanderSwingHandler.Companion.getINSTANCE();
        setField(SalamanderSwingHandler.class, salamander, "style", CombatStyle.RANGE);
        reset(salamander, CombatStyle.RANGE);
        // EquipmentDegrader's companion lists are process state too: every case registers afresh.
        EquipmentDegrader.StaticDegrader.getItemList().clear();
        EquipmentDegrader.StaticDegrader.getSetList().clear();
        EquipmentDegrader.StaticDegrader.getItemCharges().clear();
        for (Object o : (List) c.getOrDefault("registers", Arrays.asList("barrows", "pvp", "crystal"))) register((String) o);
        List specs = (List) c.getOrDefault("spells", Collections.emptyList());
        for (int k = 0; k < specs.size(); k++) spells.add(new CombatSpell(k, (Map) specs.get(k)));
        List es = (List) c.getOrDefault("entities", Collections.emptyList());
        for (int k = 0; k < es.size(); k++) {
            Map s = (Map) es.get(k);
            String kind = (String) s.getOrDefault("kind", "player");
            pendingTag = k;
            pendingSpec = s;
            Entity e = kind.equals("player") ? new Player(k, s) : kind.equals("npc") ? new NPC(k, s)
                : kind.equals("familiar") ? new content.global.skill.summoning.familiar.Familiar(k, s) : new Entity(k, s);
            if (s.get("spell") != null) e.spell = spells.get(n(s.get("spell")));
            if (s.get("autocast") != null) e.autocast = spells.get(n(s.get("autocast")));
            entities.add(e);
        }
        for (Entity e : entities) e.bind();
        for (Entity e : entities) if (e.attrs.get("combat-stop") instanceof Long) e.attrs.put("combat-stop", n(e.attrs.get("combat-stop")));
        for (Entity e : entities) if (e.spec.get("lastVictim") != null) setField(CombatPulse.class, e.getProperties().getCombatPulse(), "lastVictim", entity(e.spec.get("lastVictim")));
        for (Object o : (List) c.getOrDefault("handlers", Collections.emptyList())) handlers.add(make((Map) o));
        RangeWeapon.getRangeWeapons().clear();
        Ammunition.getAmmunition().clear();
        Map range = (Map) c.getOrDefault("range", Collections.emptyMap());
        for (Object o : (List) range.getOrDefault("weapons", Collections.emptyList())) {
            Map w = (Map) o;
            List<Integer> ammo = new ArrayList<>();
            for (Object a : (List) w.get("ammunition")) ammo.add(n(a));
            Animation animation = w.get("animation") == null ? null : new Animation(n(w.get("animation")), Priority.HIGH);
            RangeWeapon.getRangeWeapons().put(n(w.get("itemId")), new RangeWeapon(n(w.get("itemId")), animation, i(w, "attackSpeed", 4), n(w.get("slot")), n(w.get("type")), !Boolean.FALSE.equals(w.get("dropAmmo")), ammo));
        }
        for (Object o : (List) range.getOrDefault("ammo", Collections.emptyList())) {
            Map a = (Map) o;
            Projectile projectile = a.get("projectile") == null ? null : new Projectile(n(a.get("projectile")));
            Ammunition ammo = new Ammunition(n(a.get("itemId")), graphics(a.get("start")), graphics(a.get("darkBow")), projectile, i(a, "poisonDamage", 0));
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
    static CombatSwingHandler handler(Map op) { return handlers.get(op.get("h") == null ? 0 : n(op.get("h"))); }

    static BattleState state(Map s) {
        if (s == null) s = Collections.emptyMap();
        BattleState b = Boolean.FALSE.equals(s.get("ctor")) ? new BattleState() : new BattleState(entity(s.get("attacker")), entity(s.get("victim")));
        if (s.containsKey("estimatedHit")) b.setEstimatedHit(n(s.get("estimatedHit")));
        if (s.containsKey("maximumHit")) b.setMaximumHit(n(s.get("maximumHit")));
        if (s.containsKey("secondaryHit")) b.setSecondaryHit(n(s.get("secondaryHit")));
        if (s.get("style") != null) b.setStyle(style(s.get("style")));
        if (s.get("spell") != null) b.setSpell(spells.get(n(s.get("spell"))));
        if (s.containsKey("targets")) {
            List l = (List) s.get("targets");
            BattleState[] t = new BattleState[l.size()];
            for (int k = 0; k < t.length; k++) t[k] = l.get(k) == null ? null : "this".equals(l.get(k)) ? b : state((Map) l.get(k));
            b.setTargets(t);
        }
        if (s.get("weapon") != null) b.setWeapon(new Weapon(new Item(n(s.get("weapon")))));
        if (s.get("ammunition") != null) b.setAmmunition(Ammunition.get(n(s.get("ammunition"))));
        return b;
    }

    static Object item(Item i) { return i == null ? null : Arrays.asList(i.ref, i.getId(), i.getAmount(), i.getCharge(), i.getName(), i.getSlot()); }

    static Map dump(BattleState b) {
        Map<String, Object> m = new TreeMap<>();
        m.put("attacker", tag(b.getAttacker()));
        m.put("victim", tag(b.getVictim()));
        m.put("estimatedHit", b.getEstimatedHit());
        m.put("maximumHit", b.getMaximumHit());
        m.put("secondaryHit", b.getSecondaryHit());
        List<Object> targets = null;
        if (b.getTargets() != null) {
            targets = new ArrayList<>();
            for (BattleState s : b.getTargets()) targets.add(s == null ? null : s == b ? "this" : dump(s));
        }
        m.put("targets", targets);
        m.put("style", b.getStyle() == null ? null : b.getStyle().name());
        m.put("armourEffect", b.getArmourEffect() == null ? null : b.getArmourEffect().name());
        Weapon w = b.getWeapon();
        m.put("weapon", w == null ? null : Arrays.asList(w.getId(), w.getAmmunitionSlot(), w.getAmmunition() == null ? null : (Object) w.getAmmunition().getId(), w.getType() == null ? null : w.getType().name()));
        m.put("ammunition", b.getAmmunition() == null ? null : (Object) b.getAmmunition().getItemId());
        m.put("rangeWeapon", b.getRangeWeapon() == null ? null : (Object) b.getRangeWeapon().getItemId());
        return m;
    }

    static Object impact(ImpactHandler.Impact i) {
        if (i == null) return null;
        return Arrays.asList(tag(i.getSource()), i.getAmount(), i.getStyle() == null ? null : i.getStyle().name(), i.getType() == null ? null : i.getType().name());
    }

    static Object handlerIndex(CombatSwingHandler h) {
        if (h == null) return null;
        for (int k = 0; k < handlers.size(); k++) if (handlers.get(k) == h) return k;
        throw new IllegalStateException("unknown handler " + h.getClass().getName());
    }

    static Object items(Map<Integer, Item> held, Set<Integer> emptied) {
        List<Object> out = new ArrayList<>();
        for (Map.Entry<Integer, Item> e : new TreeMap<>(held).entrySet()) out.add(Arrays.asList(e.getKey(), item(e.getValue())));
        for (Integer slot : new TreeSet<>(emptied)) out.add(Arrays.asList(slot, null));
        return out;
    }

    /** Everything the ported classes own, after an operation. Calls no recording stand-in. */
    static Map snapshot() throws Exception {
        Map<String, Object> m = new TreeMap<>();
        List<Object> pulses = new ArrayList<>(), impacts = new ArrayList<>(), tasks = new ArrayList<>(), settings = new ArrayList<>(), held = new ArrayList<>();
        for (Entity e : entities) {
            CombatPulse p = e.getProperties().getCombatPulse();
            List<Object> checks = new ArrayList<>();
            for (Object c : (Object[]) field(Pulse.class, p, "checks")) checks.add(tag((Entity) c));
            pulses.add(Arrays.asList(tag(p.getVictim()), tag(p.getLastVictim()), p.getNextAttack(), field(CombatPulse.class, p, "combatTimeOut"),
                p.getStyle().name(), p.isRunning(), p.getDelay(), p.getTicksPassed(), handlerIndex(p.getTemporaryHandler()), checks));
            ImpactHandler ih = e.getImpactHandler();
            List<Object> q = new ArrayList<>(), npc = new ArrayList<>(), players = new ArrayList<>();
            for (ImpactHandler.Impact i : ih.getImpactQueue()) q.add(impact(i));
            for (Map.Entry<Entity, Integer> en : ih.getNpcImpactLog().entrySet()) npc.add(Arrays.asList(tag(en.getKey()), en.getValue()));
            for (Map.Entry<Integer, Integer> en : ih.getPlayerImpactLog().entrySet()) players.add(Arrays.asList(en.getKey(), en.getValue()));
            impacts.add(Arrays.asList(ih.getDisabledTicks(), q, npc, players));
            if (e instanceof Player) {
                Player pl = (Player) e;
                Settings s = pl.getSettings();
                settings.add(Arrays.asList(field(Settings.class, s, "specialEnergy"), field(Settings.class, s, "specialToggled")));
                Object eq = field(core.game.container.impl.EquipmentContainer.class, pl.getEquipment(), "items");
                Object eqEmpty = field(core.game.container.impl.EquipmentContainer.class, pl.getEquipment(), "emptied");
                Player.Inventory inv = pl.getInventory();
                Object invItems = field(Player.Inventory.class, inv, "items");
                Object invEmpty = field(Player.Inventory.class, inv, "emptied");
                held.add(Arrays.asList(items((Map) eq, (Set) eqEmpty), items((Map) invItems, (Set) invEmpty)));
            } else {
                settings.add(null);
                held.add(null);
            }
        }
        for (Pulse p : queue) {
            String label = label(p);
            Object shown = null;
            if (label.equals("impact")) shown = field(p.getClass(), p, "impact");
            tasks.add(Arrays.asList(label, p.isRunning(), p.getDelay(), p.getTicksPassed(), shown));
        }
        List<Object> types = new ArrayList<>();
        for (CombatSwingHandler h : handlers) types.add(h.getType() == null ? null : h.getType().name());
        m.put("pulses", pulses);
        m.put("impacts", impacts);
        m.put("queue", tasks);
        m.put("settings", settings);
        m.put("items", held);
        m.put("types", types);
        m.put("salamander", ((CombatStyle) field(SalamanderSwingHandler.class, SalamanderSwingHandler.Companion.getINSTANCE(), "style")).name());
        m.put("attrs", attributes());
        m.put("xpMod", hex(CombatSwingHandler.EXPERIENCE_MOD));
        return m;
    }

    static final String[] KEYS = {"hunter", "/save:sheep-shearer:is-penguin-sheep-sheared"};

    /** The attributes these classes store, per entity. */
    static Object attributes() {
        List<Object> out = new ArrayList<>();
        for (Entity e : entities) {
            List<Object> row = new ArrayList<>();
            for (String k : KEYS) row.add(e.attrs.containsKey(k) ? e.attrs.get(k) : "absent");
            out.add(row);
        }
        return out;
    }

    static Map registry() {
        Map<String, Object> m = new TreeMap<>();
        m.put("items", new ArrayList<>(EquipmentDegrader.StaticDegrader.getItemList()));
        List<Object> sets = new ArrayList<>();
        for (ArrayList<Integer> s : EquipmentDegrader.StaticDegrader.getSetList()) sets.add(new ArrayList<>(s));
        m.put("sets", sets);
        List<Object> charges = new ArrayList<>();
        for (Map.Entry<Integer, Integer> e : new TreeMap<>(EquipmentDegrader.StaticDegrader.getItemCharges()).entrySet()) charges.add(Arrays.asList(e.getKey(), e.getValue()));
        m.put("charges", charges);
        return m;
    }

    static Object barrows(Map op) {
        int id = n(op.get("id")), amount = i(op, "amount", 1), charge = i(op, "charge", 1000);
        Item item = new Item("barrows", id, amount, charge, "piece", null, null);
        BarrowsEquipment b = BarrowsEquipment.INSTANCE;
        BarrowsEquipment.BarrowsItemDefinition def = BarrowsEquipment.getDefinition(id);
        List<Object> shown = null;
        if (def != null) {
            List<Object> states = new ArrayList<>(def.getDegradationStates());
            shown = Arrays.asList(def.getBrother(), def.getEquipmentType(), def.getItemName(), def.getBaseRepairCost(), states,
                def.getRepairedId(), def.getBrokenId(), def.getDegradationIndex(id), def.contains(id));
        }
        Item reduced = BarrowsEquipment.graveDeathDurabilityReduction(item);
        Object reduction = reduced == null ? null : reduced == item ? "same" : Arrays.asList(reduced.getId(), reduced.getAmount(), reduced.getCharge());
        return Arrays.asList(BarrowsEquipment.isBarrowsItem(id), BarrowsEquipment.isFullyRepaired(id), BarrowsEquipment.isBroken(id),
            b.getRepairCost(item), shown, reduction);
    }

    static Object barrowsLists() {
        List<Object> sets = new ArrayList<>();
        for (int[] s : BarrowsEquipment.INSTANCE.getAllEquipmentSets()) sets.add(list(s));
        return Arrays.asList(sets, new ArrayList<>(BarrowsEquipment.INSTANCE.getAllRepairableBarrowsIds()), BarrowsEquipment.DEGRADATION_TICKS_PER_TIER);
    }

    static BattleState current;

    static BattleState given(Map op) {
        current = "none".equals(op.get("state")) ? null : state((Map) op.get("state"));
        return current;
    }

    static void ticks(int count) {
        for (int k = 0; k < count; k++) {
            mark("tick", tick);
            int size = queue.size();
            for (int at = 0; at < size; at++) {
                Pulse p = queue.poll();
                String label = label(p);
                try {
                    boolean done = p.update();
                    mark("update", label, done);
                    if (!done && p.isRunning()) queue.add(p);
                } catch (Exception e) {
                    mark("runnerError", label, classify(e));
                }
            }
            tick++;
        }
    }

    static Item held(Map op) {
        Player p = (Player) ent(op, "e");
        int slot = n(op.get("slot"));
        return "inventory".equals(op.get("where")) ? p.getInventory().at(slot) : p.getEquipment().peek(slot);
    }

    /** A node argument: an inventory item of the player ({"slot"}), a case entity ({"entity"}), a scenery ({"scenery"}) or null. */
    static Node node(Player p, Object o) {
        if (o == null) return null;
        Map m = (Map) o;
        if (m.containsKey("slot")) return p.getInventory().at(n(m.get("slot")));
        if (m.containsKey("scenery")) return new Scenery(n(m.get("scenery")));
        return entity(m.get("entity"));
    }

    @SuppressWarnings("unchecked")
    static Object run(Map op) throws Throwable {
        String name = (String) op.get("op");
        Entity e = ent(op, "e"), v = ent(op, "v");
        switch (name) {
            case "swing": return CombatPulse.Companion.swing(e, v, handler(op));
            case "handlerSwing": return handler(op).swing(e, v, "null".equals(op.get("state")) ? null : given(op));
            case "visualize": handler(op).visualize(e, v, given(op)); return null;
            case "visualizeImpact": handler(op).visualizeImpact(e, v, given(op)); return null;
            case "impact": handler(op).impact(e, v, given(op)); return null;
            case "adjust": handler(op).adjustBattleState(e, v, given(op)); return null;
            case "canSwing": { InteractionType t = handler(op).canSwing(e, v); return t == null ? null : t.name(); }
            case "accuracy": return handler(op).calculateAccuracy(e);
            case "hit": return handler(op).calculateHit(e, v, bits(op.get("modifier")));
            case "defence": return handler(op).calculateDefence(e, v);
            case "setMultiplier": return hex(handler(op).getSetMultiplier(e, n(op.get("skill"))));
            case "addExperience": handler(op).addExperience(e, v, "null".equals(op.get("state")) ? null : given(op)); return null;
            case "postSwing": handler(op).postSwing(e, v, given(op)); return null;
            case "armourSet": { ArmourSet s = handler(op).getArmourSet(e); return s == null ? null : s.name(); }
            case "swingHandler": return handlerIndex(e.getSwingHandler(Boolean.TRUE.equals(op.get("swing"))));
            case "attack": e.getProperties().getCombatPulse().attack(v); return null;
            case "setStyle": e.getProperties().getCombatPulse().setStyle(style(op.get("style"))); return null;
            case "tick": ticks(n(op.get("n"))); return null;
            // BarrowsEquipment and the degrader.
            case "barrows": return barrows(op);
            case "barrowsLists": return barrowsLists();
            case "registry": return registry();
            case "register": for (Object o : (List) op.get("names")) register((String) o); return registry();
            case "armourDegrades": { Player p = (Player) e; if (p == null) new EquipmentDegrader().checkArmourDegrades(null); else p.degrader.checkArmourDegrades(p); return null; }
            case "weaponDegrades": { Player p = (Player) e; if (p == null) new EquipmentDegrader().checkWeaponDegrades(null); else p.degrader.checkWeaponDegrades(p); return null; }
            // Listeners and their handlers.
            case "define": {
                String listener = (String) op.get("listener");
                switch (listener) {
                    case "hasta": new HastaEquipListener().defineListeners(); break;
                    case "sheep": new SheepBehavior().defineListeners(); break;
                    case "dog": new ShooAwayStrayDogListener().defineListeners(); break;
                    case "decant": new DecantListener().defineListeners(); break;
                    default: throw new AssertionError(listener);
                }
                return null;
            }
            case "equipHandler": return ((kotlin.jvm.functions.Function2<Object, Object, Object>) equips.get(n(op.get("k")))).invoke(e, node((Player) e, op.get("node")));
            case "npcOption": return ((kotlin.jvm.functions.Function2<Object, Object, Object>) listeners.get((String) op.get("option"))).invoke(e, v);
            case "useWith": return ((kotlin.jvm.functions.Function3<Object, Object, Object, Object>) useWiths.get(n(op.get("k")))).invoke(e, node((Player) e, op.get("used")), node((Player) e, op.get("with")));
            case "behaviourIds": return Arrays.asList(list(new SheepBehavior().getIds()), list(new ChinchompaDeathListener().getIds()), list(new ShooAwayStrayDogListener().getIds()));
            case "sheepTick": return new SheepBehavior().tick((NPC) e);
            case "deathFinished": new ChinchompaDeathListener().onDeathFinished((NPC) e, v); return null;
            case "respawn": new ChinchompaDeathListener().onRespawn((NPC) e); return null;
            case "cowPlugin": cow = new CowMilkingPlugin(); cow.newInstance(null); return null;
            case "cowHandle": { if (cow == null) cow = new CowMilkingPlugin(); return cow.handle((Player) e, node((Player) e, op.get("node")), (String) op.get("option")); }
            case "bucket": { if (cow == null) cow = new CowMilkingPlugin(); CowMilkingPlugin.BucketHandler b = cow.new BucketHandler(); return b.handle(new NodeUsageEvent((Player) e, 0, node((Player) e, op.get("used")), node((Player) e, op.get("with")))); }
            case "decantDialogue": {
                DecantListener.DecantingDialogue file = new DecantListener.DecantingDialogue();
                dialogueTag = e.tag;
                List<Object> stages = new ArrayList<>();
                for (int k = 0; k < n(op.get("n")); k++) { file.handle(0, 0); stages.add(file.getStage()); }
                return stages;
            }
            // World-model inputs between operations.
            case "set": {
                String key = (String) op.get("key");
                Object value = op.get("value");
                if (value == null) e.spec.remove(key); else e.spec.put(key, value);
                if (key.equals("location")) e.relocate(place(e.spec));
                if (key.equals("lifepoints")) e.lifepoints = n(value);
                if (key.equals("dead")) e.dead = Boolean.TRUE.equals(value);
                return null;
            }
            case "setAttr": { Object value = op.get("value"); if (value == null) e.attrs.remove((String) op.get("key")); else e.attrs.put((String) op.get("key"), value instanceof Long ? (Object) n(value) : value); return null; }
            case "setTick": tick = n(op.get("tick")); return null;
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
        installErr();
        PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), false, "UTF-8");
        try (BufferedReader r = new BufferedReader(new InputStreamReader(System.in, java.nio.charset.StandardCharsets.UTF_8))) {
            String line;
            while ((line = r.readLine()) != null) {
                Map c = (Map) new JSONParser().parse(line);
                world(c);
                events.clear();
                String last = null;
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
                    try { result = run(op); } catch (Throwable x) { error = classify(x); }
                    failAt = -1;
                    row.put("result", result);
                    row.put("error", error);
                    row.put("events", new ArrayList<>(events));
                    if (current != null) row.put("state", dump(current));
                    Map world = snapshot();
                    String shown = JSONValue.toJSONString(world);
                    if (!shown.equals(last)) row.put("world", world);
                    last = shown;
                    results.add(row);
                }
                out.println(JSONValue.toJSONString(results));
            }
        } catch (Throwable fatal) {
            fatal.printStackTrace(realErr);
            realErr.flush();
            System.exit(2);
        }
        out.flush();
    }
}
