package oracle;

import java.io.*;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
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
import core.plugin.Plugin;
import core.tools.RandomFunction;

/**
 * Drives the unmodified special attack handlers (on the unmodified combat
 * handlers, CombatPulse, ImpactHandler and Pulse) and MultihitTargets.kt for
 * one JSON case per input line. Every call they make outside their slice
 * arrives here as an ordered event; random draws are events too. The world
 * model (ticks, the pulse queue and what the stand-ins keep per entity) is an
 * oracle input. Doubles cross the boundary as 16 hex digits of raw bits.
 */
public class Support {
    /** The plugin classes in the order every case instantiates and registers them. */
    public static final String[] SPECIALS = {
        "AncientMaceSpecialHandler", "BackstabSpecialHandler", "ChainhitSpecialHandler", "CleaveSpecialHandler",
        "ClobberSpecialHandler", "DescentOfDarknessSpecialHandler", "EnergyDrainSpecialHandler", "ExcaliburSpecialHandler",
        "FeintSpecialHandler", "HamstringSpecialHandler", "HealingBladeSpecialHandler", "IceCleaveSpecialHandler",
        "ImpaleSpecialHandler", "JudgementSpecialHandler", "LiquefySpecialHandler", "PhantomStrikeSpecialHandler",
        "PowershotSpecialHandler", "PowerstabSpecialHandler", "PunctureSpecialHandler", "QuickSmashSpecialHandler",
        "RampageSpecialHandler", "SaradominsLightningHandler", "SeercullSpecialHandler", "SeverSpecialHandler",
        "ShatterSpecialHandler", "ShoveSpecialHandler", "SliceAndDiceSpecialHandler", "SmashSpecialHandler",
        "SnapshotSpecialHandler", "SnipeSpecialHandler", "SpearWallSpecialHandler", "SweepSpecialHandler",
        "WarstrikeSpecialHandler", "WeakenSpecialHandler",
    };
    public static Map input;
    public static final List ZERO15 = Collections.nCopies(15, 0L);
    static final List<Object> events = new ArrayList<>();
    static int calls, failAt = -1;
    static List<Entity> entities;
    static List<CombatSpell> spells;
    public static List<CombatSwingHandler> handlers;
    public static final LinkedList<Pulse> queue = new LinkedList<>();
    public static int tick;
    public static Map clip, pclip, uids;
    public static List inactive;
    public static ArrayList<Object> duelAmmo;
    /** Every timer handed to TimerManager.registerTimer, with its entity. */
    static final List<Object[]> timers = new ArrayList<>();
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
    public static boolean timer(Entity e, String name) {
        if (!name.equals("Miasmic")) throw unsupported("hasTimerActive<" + name + ">");
        emit("miasmic", e.tag);
        return Boolean.TRUE.equals(e.spec.get("miasmic"));
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

    /** A timer the registry answers: its identifier and arguments. */
    public static class Spawned extends RSTimer {
        public Spawned(String identifier) { super(0, identifier, false, false, new core.game.system.timer.TimerFlag[0]); }
        @Override public boolean run(Entity e) { throw unsupported("spawned timer run"); }
    }
    static List<Object> args(Object[] args) {
        List<Object> out = new ArrayList<>();
        for (Object a : args) {
            if (a instanceof Integer) out.add(a);
            else if (a instanceof Boolean) out.add(a);
            else throw unsupported("timer argument " + a);
        }
        return out;
    }
    public static RSTimer spawnTimer(String identifier, Object[] args) {
        emit("spawnTimer", identifier, args(args));
        return has(input, "nullTimers", identifier) ? null : new Spawned(identifier);
    }
    static String timerKind(RSTimer t) {
        if (t instanceof Spawned) return "spawned";
        String name = t.getClass().getName();
        if (name.equals("content.global.handlers.item.equipment.special.HamstringSpecialHandler$1")) return "hamstrung";
        if (name.equals("content.global.handlers.item.equipment.special.PhantomStrikeSpecialHandler$1")) return "phantomStrike";
        throw new IllegalStateException("unknown timer " + name);
    }
    public static void registerTimer(Entity e, RSTimer t) {
        List<Object> flags = new ArrayList<>();
        for (core.game.system.timer.TimerFlag f : t.getFlags()) flags.add(f.name());
        emit("registerTimer", e.tag, timerKind(t), t.getIdentifier(), t.getRunInterval(), t.isSoft(), t.isAuto(), flags);
        timers.add(new Object[] {e, t});
    }

    public static String label(Pulse p) {
        if (p instanceof CombatPulse) return "combat:" + ((CombatPulse) p).getEntity().tag;
        String name = p.getClass().getName();
        if (name.startsWith("core.game.node.entity.combat.CombatPulse$Companion$swing$")) return "impact";
        if (name.startsWith("core.game.node.entity.combat.RangeSwingHandler$impact$")) return "secondHit";
        if (name.startsWith("core.game.node.entity.combat.ImpactHandler$")) return "manualHit";
        String special = "content.global.handlers.item.equipment.special.";
        if (name.equals(special + "ChainhitSpecialHandler$1")) return "chainDoor";
        if (name.equals(special + "ChainhitSpecialHandler$2")) return "chainImpact";
        if (name.equals(special + "ChainhitSpecialHandler$3")) return "chainBounce";
        if (name.equals(special + "SnapshotSpecialHandler$1")) return "snapshotSecond";
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

    static void world(Map c) throws Throwable {
        input = c;
        entities = new ArrayList<>();
        spells = new ArrayList<>();
        queue.clear();
        timers.clear();
        duelAmmo = new ArrayList<>();
        tick = i(c, "tick", 0);
        clip = (Map) c.get("clip");
        pclip = (Map) c.get("pclip");
        uids = (Map) c.get("uids");
        inactive = (List) c.getOrDefault("inactiveRegions", Collections.emptyList());
        core.ServerConstants.DRAGON_AXE_USE_OSRS_SPEC = Boolean.TRUE.equals(c.get("osrsDragonAxe"));
        CombatSwingHandler.EXPERIENCE_MOD = 4.0;
        // The three CombatStyle singletons are process state: every case starts them fresh.
        handlers = new ArrayList<>();
        for (CombatStyle s : CombatStyle.values()) {
            CombatSwingHandler h = s.getSwingHandler();
            h.setType(s);
            h.getFlags().clear();
            setField(CombatSwingHandler.class, h, "specialHandlers", null);
            handlers.add(h);
        }
        // Every case makes and registers the plugins again, as start-up does.
        for (String name : SPECIALS) {
            CombatSwingHandler h = (CombatSwingHandler) Class.forName("content.global.handlers.item.equipment.special." + name).getDeclaredConstructor().newInstance();
            handlers.add(h);
            ((Plugin<Object>) (Object) h).newInstance(null);
        }
        content.global.handlers.item.equipment.special.ChinchompaSwingHandler.INSTANCE = new content.global.handlers.item.equipment.special.ChinchompaSwingHandler();
        handlers.add(content.global.handlers.item.equipment.special.ChinchompaSwingHandler.INSTANCE);
        content.global.handlers.item.equipment.special.SalamanderSwingHandler.setINSTANCE(new content.global.handlers.item.equipment.special.SalamanderSwingHandler());
        handlers.add(content.global.handlers.item.equipment.special.SalamanderSwingHandler.getINSTANCE());
        for (Object o : (List) c.getOrDefault("handlers", Collections.emptyList())) handlers.add(make((Map) o));
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
        for (Entity e : entities) e.bind();
        // The tick an attribute stores is an Integer in the original.
        for (Entity e : entities) if (e.attrs.get("combat-stop") instanceof Long) e.attrs.put("combat-stop", n(e.attrs.get("combat-stop")));
        for (Entity e : entities) if (e.spec.get("lastVictim") != null) setField(CombatPulse.class, e.getProperties().getCombatPulse(), "lastVictim", entity(e.spec.get("lastVictim")));
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
    static Item worn(Object o) { if (o == null) return null; List a = (List) o; return ((Player) entity(a.get(0))).getEquipment().peek(n(a.get(1))); }

    static BattleState state(Map s) {
        if (s == null) s = Collections.emptyMap();
        BattleState b = Boolean.FALSE.equals(s.get("ctor")) ? new BattleState() : new BattleState(entity(s.get("attacker")), entity(s.get("victim")));
        if (s.containsKey("estimatedHit")) b.setEstimatedHit(n(s.get("estimatedHit")));
        if (s.containsKey("maximumHit")) b.setMaximumHit(n(s.get("maximumHit")));
        if (s.containsKey("secondaryHit")) b.setSecondaryHit(n(s.get("secondaryHit")));
        if (s.containsKey("frozen")) b.setFrozen(Boolean.TRUE.equals(s.get("frozen")));
        if (s.get("style") != null) b.setStyle(style(s.get("style")));
        if (s.get("rangeWeapon") != null) b.setRangeWeapon(RangeWeapon.get(n(s.get("rangeWeapon"))));
        if (s.get("ammunition") != null) b.setAmmunition(Ammunition.get(n(s.get("ammunition"))));
        if (s.get("weapon") != null) {
            Map w = (Map) s.get("weapon");
            Weapon weapon = w.containsKey("slot") ? new Weapon(worn(w.get("item")), n(w.get("slot")), worn(w.get("ammo"))) : new Weapon(worn(w.get("item")));
            if (w.get("type") != null) weapon.setType(Weapon.WeaponType.valueOf((String) w.get("type")));
            b.setWeapon(weapon);
        }
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

    static Object impact(ImpactHandler.Impact i) {
        if (i == null) return null;
        return Arrays.asList(tag(i.getSource()), i.getAmount(), i.getStyle() == null ? null : i.getStyle().name(), i.getType() == null ? null : i.getType().name());
    }

    static Object handlerIndex(CombatSwingHandler h) {
        if (h == null) return null;
        for (int k = 0; k < handlers.size(); k++) if (handlers.get(k) == h) return k;
        throw new IllegalStateException("unknown handler");
    }

    static Object tags(List<? extends Entity> list) {
        if (list == null) return null;
        List<Object> out = new ArrayList<>();
        for (Entity e : list) out.add(tag(e));
        return out;
    }

    /** Everything the ported classes own, after an operation. Calls no recording stand-in. */
    static Map snapshot() throws Exception {
        Map<String, Object> m = new TreeMap<>();
        List<Object> pulses = new ArrayList<>(), impacts = new ArrayList<>(), tasks = new ArrayList<>(), settings = new ArrayList<>(), chains = new ArrayList<>();
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
                Settings s = ((Player) e).getSettings();
                settings.add(Arrays.asList(field(Settings.class, s, "specialEnergy"), field(Settings.class, s, "specialToggled")));
            } else settings.add(null);
            Object chain = e.attrs.get("chain-hit_v");
            chains.add(chain instanceof List && !((List) chain).isEmpty() && ((List) chain).get(0) instanceof Entity ? tags((List<Entity>) chain) : chain);
        }
        for (Pulse p : queue) {
            String label = label(p);
            Object shown = null;
            if (label.equals("impact")) shown = field(p.getClass(), p, "impact");
            tasks.add(Arrays.asList(label, p.isRunning(), p.getDelay(), p.getTicksPassed(), shown));
        }
        m.put("pulses", pulses);
        m.put("impacts", impacts);
        m.put("queue", tasks);
        m.put("settings", settings);
        m.put("chainHit", chains);
        m.put("xpMod", hex(CombatSwingHandler.EXPERIENCE_MOD));
        return m;
    }

    static BattleState current;

    static BattleState given(Map op) {
        current = "none".equals(op.get("state")) ? null : state((Map) op.get("state"));
        return current;
    }

    static ImpactHandler.HitsplatType splat(Object o) { return o == null ? null : ImpactHandler.HitsplatType.valueOf((String) o); }

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

    static Object fired(Object o) {
        if (o == null) return "null";
        if (o == kotlin.Unit.INSTANCE) return "unit";
        if (o == Boolean.TRUE) return true;
        throw new IllegalStateException("fireEvent answered " + o);
    }

    static Object run(Map op) throws Throwable {
        String name = (String) op.get("op");
        Entity e = ent(op, "e"), v = ent(op, "v");
        switch (name) {
            case "swing": return CombatPulse.Companion.swing(e, v, handler(op));
            case "specialSwing": return handler(op).swing(e, v, "null".equals(op.get("state")) ? null : given(op));
            case "visualize": handler(op).visualize(e, v, given(op)); return null;
            case "visualizeImpact": handler(op).visualizeImpact(e, v, given(op)); return null;
            case "impact": handler(op).impact(e, v, given(op)); return null;
            case "adjust": handler(op).adjustBattleState(e, v, given(op)); return null;
            case "formattedHit": {
                Method m = CombatSwingHandler.class.getDeclaredMethod("getFormattedHit", Entity.class, Entity.class, BattleState.class, int.class);
                m.setAccessible(true);
                try { return m.invoke(handler(op), e, v, given(op), n(op.get("hit"))); } catch (InvocationTargetException x) { throw x.getCause(); }
            }
            case "getSwingHandler": return handlerIndex(e.getSwingHandler(Boolean.TRUE.equals(op.get("swing"))));
            case "getSpecial": return handlerIndex(handler(op).getSpecial(n(op.get("itemId"))));
            case "newInstance": ((Plugin<Object>) (Object) handler(op)).newInstance(null); return null;
            case "fireEvent": return fired(((Plugin<Object>) (Object) handler(op)).fireEvent((String) op.get("id"), e));
            // CombatTabInterface.handleInstantSpec's body.
            case "instant": return handler(op).swing(e, e.getProperties().getCombatPulse().getVictim(), null);
            case "timerRegister": { Object[] t = timers.get(n(op.get("timer"))); ((RSTimer) t[1]).onRegister((Entity) t[0]); return null; }
            case "timerRun": { Object[] t = timers.get(n(op.get("timer"))); return ((RSTimer) t[1]).run((Entity) t[0]); }
            case "multihit": {
                String kind = (String) op.get("kind");
                if (kind.equals("targets")) return tags(MultihitTargetsKt.findMultihitTargets(v, e, style(op.get("style"))));
                if (kind.equals("halberd")) return tags(MultihitTargetsKt.findMultihitTargetsForDragonHalberd(v, e));
                if (kind.equals("d2h")) return tags(MultihitTargetsKt.findMultihitTargetsForD2h(v, e, style(op.get("style"))));
                return tags(MultihitTargetsKt.findMultihitTargetsForChinchompa(v, e));
            }
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
                if (key.equals("prayerPoints")) e.prayerPoints = bits(value);
                if (key.equals("specialEnergy")) setField(Settings.class, ((Player) e).getSettings(), "specialEnergy", n(value));
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
