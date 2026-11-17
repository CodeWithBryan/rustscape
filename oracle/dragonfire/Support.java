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
import core.game.system.task.Pulse;
import core.game.system.timer.RSTimer;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import core.tools.RandomFunction;
import content.global.handlers.item.equipment.special.DragonfireSwingHandler;
import content.global.handlers.item.DragonfireShieldListener;
import content.global.handlers.item.DFSItemPlugin;
import content.global.handlers.npc.ChromaticDragonNPC;
import content.global.handlers.npc.MetalDragonNPC;
import content.global.handlers.npc.ChromaticDragonBehavior;
import core.game.node.entity.npc.AbstractNPC;

/**
 * Drives the unmodified DragonfireSwingHandler, FireType, SwitchAttack,
 * MultiSwingHandler, MixedSwingHandlers and DragonfireShieldListener (on the
 * unmodified combat handlers, CombatPulse, ImpactHandler and Pulse) for one
 * JSON case per input line. Every call they make outside their slice arrives
 * here as an ordered event; random draws are events too. The world model
 * (ticks, the pulse queue and what the stand-ins keep per entity) is an oracle
 * input. Doubles cross the boundary as 16 hex digits of raw bits.
 */
public class Support {
    public static Map input;
    public static final List ZERO15 = Collections.nCopies(15, 0L);
    static final List<Object> events = new ArrayList<>();
    static int calls, failAt = -1;
    static List<Entity> entities;
    static List<CombatSpell> spells;
    public static List<CombatSwingHandler> handlers;
    /** The attack each DragonfireSwingHandler.get answered, by its handler's index. */
    static Map<Integer, SwitchAttack> made;
    static DragonfireShieldListener listener;
    static Map<String, Object> listeners;
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

    /** InteractionListener.on(ids, type, options, handler): recorded, the handler kept per option. */
    public static void listen(int[] ids, int type, String[] options, Object handler) {
        List<Object> shown = new ArrayList<>();
        for (int id : ids) shown.add(id);
        emit("on", shown, type, new ArrayList<>(Arrays.asList(options)));
        for (String o : options) listeners.put(o, handler);
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

    static Projectile projectile(Object o) {
        if (o == null) return null;
        List p = (List) o;
        return Projectile.create(entity(p.get(0)), entity(p.get(1)), n(p.get(2)), n(p.get(3)), n(p.get(4)), n(p.get(5)), n(p.get(6)), n(p.get(7)), n(p.get(8)));
    }
    static Animation animation(Object o) { return o == null ? null : new Animation(n(o), Priority.HIGH); }

    /** One SwitchAttack of a case: a get's attack, the style constructor, or the full constructor. */
    static SwitchAttack attack(Map a) {
        if (a.containsKey("from")) return made.get(n(a.get("from")));
        if (a.containsKey("style")) return new SwitchAttack(CombatStyle.valueOf((String) a.get("style")));
        CombatSwingHandler h = a.get("handler") == null ? null : handlers.get(n(a.get("handler")));
        SwitchAttack s = new SwitchAttack(h, animation(a.get("animation")), graphics(a.get("start")), graphics(a.get("end")), projectile(a.get("projectile")));
        if (a.containsKey("useHandler")) s.setUseHandler(Boolean.TRUE.equals(a.get("useHandler")));
        if (a.containsKey("maximumHit")) s.setMaximumHit(n(a.get("maximumHit")));
        return s;
    }

    static void addHandler(Map h) {
        String kind = (String) h.get("kind");
        if (kind.equals("dragonfire")) {
            String via = (String) h.get("via");
            boolean melee = Boolean.TRUE.equals(h.get("meleeRange"));
            int max = n(h.get("maximumHit"));
            boolean fire = !Boolean.FALSE.equals(h.get("fire"));
            if (via.equals("new")) {
                handlers.add(new DragonfireSwingHandler(melee, max, h.get("attack") == null ? null : attack((Map) h.get("attack")), fire));
                return;
            }
            Animation anim = animation(h.get("animation"));
            SwitchAttack a = via.equals("get6")
                ? DragonfireSwingHandler.get(melee, max, anim, graphics(h.get("start")), graphics(h.get("end")), projectile(h.get("projectile")))
                : DragonfireSwingHandler.get(melee, max, anim, graphics(h.get("start")), graphics(h.get("end")), projectile(h.get("projectile")), fire);
            made.put(handlers.size(), a);
            handlers.add(a.getHandler());
            return;
        }
        if (kind.equals("multi")) {
            List l = (List) h.get("attacks");
            SwitchAttack[] attacks = new SwitchAttack[l.size()];
            for (int k = 0; k < attacks.length; k++) attacks[k] = attack((Map) l.get(k));
            handlers.add(h.containsKey("meleeDistance") ? new MultiSwingHandler(Boolean.TRUE.equals(h.get("meleeDistance")), attacks) : new MultiSwingHandler(attacks));
            return;
        }
        handlers.add(make(h));
    }

    static void reset(CombatSwingHandler h, CombatStyle type) throws Exception {
        h.setType(type);
        h.getFlags().clear();
        setField(CombatSwingHandler.class, h, "specialHandlers", null);
        handlers.add(h);
    }

    static void world(Map c) throws Throwable {
        input = c;
        entities = new ArrayList<>();
        spells = new ArrayList<>();
        queue.clear();
        duelAmmo = new ArrayList<>();
        made = new HashMap<>();
        listeners = new HashMap<>();
        tick = i(c, "tick", 0);
        clip = (Map) c.get("clip");
        pclip = (Map) c.get("pclip");
        uids = (Map) c.get("uids");
        inactive = (List) c.getOrDefault("inactiveRegions", Collections.emptyList());
        core.ServerConstants.DRAGON_AXE_USE_OSRS_SPEC = false;
        core.ServerConstants.BETTER_DFS = Boolean.TRUE.equals(c.get("betterDfs"));
        CombatSwingHandler.EXPERIENCE_MOD = 4.0;
        // The CombatStyle singletons and the Kotlin objects are process state: every case starts them fresh.
        handlers = new ArrayList<>();
        for (CombatStyle s : CombatStyle.values()) reset(s.getSwingHandler(), s);
        reset(MagicalRangedSwingHandler.INSTANCE, CombatStyle.RANGE);
        reset(MagicalMeleeSwingHandler.INSTANCE, CombatStyle.MELEE);
        reset(RangedMeleeSwingHandler.INSTANCE, CombatStyle.MELEE);
        content.global.handlers.item.equipment.special.ChinchompaSwingHandler.INSTANCE = new content.global.handlers.item.equipment.special.ChinchompaSwingHandler();
        handlers.add(content.global.handlers.item.equipment.special.ChinchompaSwingHandler.INSTANCE);
        content.global.handlers.item.equipment.special.SalamanderSwingHandler.setINSTANCE(new content.global.handlers.item.equipment.special.SalamanderSwingHandler());
        handlers.add(content.global.handlers.item.equipment.special.SalamanderSwingHandler.getINSTANCE());
        List specs = (List) c.getOrDefault("spells", Collections.emptyList());
        for (int k = 0; k < specs.size(); k++) spells.add(new CombatSpell(k, (Map) specs.get(k)));
        List es = (List) c.getOrDefault("entities", Collections.emptyList());
        for (int k = 0; k < es.size(); k++) {
            Map s = (Map) es.get(k);
            String kind = (String) s.getOrDefault("kind", "player");
            pendingTag = k;
            pendingSpec = s;
            Object dragon = s.get("class");
            Entity e = "chromatic".equals(dragon) ? new ChromaticDragonNPC(i(s, "id", 941), place(s)) : "metal".equals(dragon) ? new MetalDragonNPC(i(s, "id", 1590), place(s))
                : kind.equals("player") ? new Player(k, s) : kind.equals("npc") ? new NPC(k, s)
                : kind.equals("familiar") ? new content.global.skill.summoning.familiar.Familiar(k, s) : new Entity(k, s);
            if (s.get("spell") != null) e.spell = spells.get(n(s.get("spell")));
            if (s.get("autocast") != null) e.autocast = spells.get(n(s.get("autocast")));
            entities.add(e);
        }
        for (Entity e : entities) e.bind();
        for (Entity e : entities) if (e.attrs.get("combat-stop") instanceof Long) e.attrs.put("combat-stop", n(e.attrs.get("combat-stop")));
        for (Entity e : entities) if (e.spec.get("lastVictim") != null) setField(CombatPulse.class, e.getProperties().getCombatPulse(), "lastVictim", entity(e.spec.get("lastVictim")));
        // Each dragon class's static dragonfire handler (once), then each dragon's own combat action.
        for (Entity e : entities) {
            if (!(e instanceof ChromaticDragonNPC) && !(e instanceof MetalDragonNPC)) continue;
            SwitchAttack fire = (SwitchAttack) field(e.getClass(), null, "DRAGONFIRE");
            if (!handlers.contains(fire.getHandler())) handlers.add(fire.getHandler());
            handlers.add((CombatSwingHandler) field(e.getClass(), e, "combatAction"));
        }
        ChromaticDragonBehavior.Companion.setEGG_RATE(1000);
        for (Object o : (List) c.getOrDefault("handlers", Collections.emptyList())) addHandler((Map) o);
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
        listener = new DragonfireShieldListener();
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
        return m;
    }

    static Object impact(ImpactHandler.Impact i) {
        if (i == null) return null;
        return Arrays.asList(tag(i.getSource()), i.getAmount(), i.getStyle() == null ? null : i.getStyle().name(), i.getType() == null ? null : i.getType().name());
    }

    static Object handlerIndex(CombatSwingHandler h) {
        if (h == null) return null;
        for (int k = 0; k < handlers.size(); k++) if (handlers.get(k) == h) return k;
        if (h.getClass().getName().startsWith("content.global.handlers.item.DragonfireShieldListener$")) return "dfs";
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
        List<Object> types = new ArrayList<>(), multis = new ArrayList<>();
        for (CombatSwingHandler h : handlers) {
            types.add(h.getType() == null ? null : h.getType().name());
            if (h instanceof MultiSwingHandler) {
                MultiSwingHandler mh = (MultiSwingHandler) h;
                SwitchAttack[] all = mh.getAttacks();
                Object cur = field(MultiSwingHandler.class, mh, "current"), next = field(MultiSwingHandler.class, mh, "next");
                int ci = -1, ni = -1;
                for (int k = all.length - 1; k >= 0; k--) { if (all[k] == cur) ci = k; if (all[k] == next) ni = k; }
                multis.add(Arrays.asList(handlerIndex(h), ci, ni, mh.isMeleeDistance()));
            }
        }
        m.put("pulses", pulses);
        m.put("impacts", impacts);
        m.put("queue", tasks);
        m.put("settings", settings);
        m.put("items", held);
        m.put("types", types);
        m.put("multi", multis);
        m.put("attrs", attributes());
        m.put("xpMod", hex(CombatSwingHandler.EXPERIENCE_MOD));
        return m;
    }

    /** The attributes these classes store, per entity. */
    static Object attributes() {
        List<Object> out = new ArrayList<>();
        for (Entity e : entities) {
            List<Object> row = new ArrayList<>();
            for (String k : new String[] {"fireBreath", "dfs_spec", "fire_resistance"}) row.add(e.attrs.get(k));
            out.add(row);
        }
        return out;
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
            case "formattedHit": {
                Method m = CombatSwingHandler.class.getDeclaredMethod("getFormattedHit", Entity.class, Entity.class, BattleState.class, int.class);
                m.setAccessible(true);
                try { return m.invoke(handler(op), e, v, given(op), n(op.get("hit"))); } catch (InvocationTargetException x) { throw x.getCause(); }
            }
            case "canSwing": { InteractionType t = handler(op).canSwing(e, v); return t == null ? null : t.name(); }
            case "accuracy": return handler(op).calculateAccuracy(e);
            case "hit": return handler(op).calculateHit(e, v, bits(op.get("modifier")));
            case "defence": return handler(op).calculateDefence(e, v);
            case "setMultiplier": return hex(handler(op).getSetMultiplier(e, n(op.get("skill"))));
            case "addExperience": handler(op).addExperience(e, v, "null".equals(op.get("state")) ? null : given(op)); return null;
            case "postSwing": handler(op).postSwing(e, v, given(op)); return null;
            case "armourSet": { ArmourSet s = handler(op).getArmourSet(e); return s == null ? null : s.name(); }
            case "maxHit": return core.api.ContentAPIKt.calculateDragonfireMaxHit(e, n(op.get("max")), Boolean.TRUE.equals(op.get("wyvern")), n(op.get("unprotectable")), Boolean.TRUE.equals(op.get("message")));
            case "shieldProtection": return core.api.ContentAPIKt.hasDragonfireShieldProtection((Player) e, Boolean.TRUE.equals(op.get("wyvern")));
            case "protection": return e.getDragonfireProtection(Boolean.TRUE.equals(op.get("fire")));
            case "fireType": {
                FireType t = FireType.valueOf((String) op.get("type"));
                List others = (List) op.getOrDefault("others", Collections.emptyList());
                Node[] rest = new Node[others.size()];
                for (int k = 0; k < rest.length; k++) rest[k] = entity(others.get(k));
                int animation = t.getAnimation().getId(), projectile = t.getProjectileId();
                return Arrays.asList(animation, projectile, t.getTask().exec(e, rest));
            }
            case "listen": listener.defineListeners(); return null;
            case "option": {
                Object h = listeners.get((String) op.get("option"));
                return ((kotlin.jvm.functions.Function2<Object, Object, Object>) h).invoke(e, held(op));
            }
            case "dragonInfo": {
                AbstractNPC d = (AbstractNPC) e;
                List<Object> ids = new ArrayList<>();
                for (int id : d.getIds()) ids.add(id);
                return Arrays.asList(ids, d.getDragonfireProtection(true), d.getDragonfireProtection(false), handlerIndex(d.getSwingHandler(true)), handlerIndex(d.getSwingHandler(false)));
            }
            case "dropTable": {
                ArrayList<Item> drops = new ArrayList<>();
                for (Object id : (List) op.get("drops")) drops.add(new Item(n(id)));
                if (op.containsKey("eggRate")) ChromaticDragonBehavior.Companion.setEGG_RATE(n(op.get("eggRate")));
                ChromaticDragonBehavior behavior = new ChromaticDragonBehavior();
                behavior.onDropTableRolled((NPC) e, v, drops);
                List<Object> out = new ArrayList<>();
                for (Item i : drops) out.add(Arrays.asList(i.getId(), i.getAmount(), i.getCharge()));
                List<Object> ids = new ArrayList<>();
                for (int id : behavior.getIds()) ids.add(id);
                return Arrays.asList(out, ids);
            }
            case "deathItem": return item(new DFSItemPlugin().getDeathItem(held(op)));
            case "itemPlugin": new DFSItemPlugin().newInstance(null); return null;
            case "attack": e.getProperties().getCombatPulse().attack(v); return null;
            case "setVictim": e.getProperties().getCombatPulse().setVictim(v); return null;
            case "setStyle": e.getProperties().getCombatPulse().setStyle(style(op.get("style"))); return null;
            case "pulse": return e.getProperties().getCombatPulse().pulse();
            case "tick": ticks(n(op.get("n"))); return null;
            case "setTemp": e.getProperties().getCombatPulse().setTemporaryHandler(op.get("h") == null ? null : handler(op)); return null;
            // World-model inputs between operations.
            case "set": {
                String key = (String) op.get("key");
                Object value = op.get("value");
                if (value == null) e.spec.remove(key); else e.spec.put(key, value);
                if (key.equals("location")) e.relocate(place(e.spec));
                if (key.equals("lifepoints")) e.lifepoints = n(value);
                if (key.equals("dead")) e.dead = Boolean.TRUE.equals(value);
                if (key.equals("specialToggled")) setField(Settings.class, ((Player) e).getSettings(), "specialToggled", Boolean.TRUE.equals(value));
                return null;
            }
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
