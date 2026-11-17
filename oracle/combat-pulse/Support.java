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
import core.game.node.entity.impl.Animator.Priority;
import core.game.node.entity.impl.Projectile;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import core.tools.RandomFunction;

/**
 * Drives the unmodified combat handlers, CombatPulse, ImpactHandler and Pulse for
 * one JSON case per input line. Every call they make outside their slice arrives
 * here as an ordered event; random draws are events too. The world model (ticks,
 * the pulse queue and what the stand-ins keep per entity) is an oracle input.
 * Doubles cross the boundary as 16 hex digits of raw bits.
 */
public class Support {
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

    /** A driver marker: recorded, never counted as a service call. */
    static void mark(Object... args) {
        events.add(new ArrayList<>(Arrays.asList(args)));
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
    public static Object graphic(Graphics g) { return g == null ? null : Arrays.asList(g.getId(), g.getHeight(), g.getDelay()); }
    static Graphics graphics(Object o) { if (o == null) return null; List g = (List) o; return new Graphics(n(g.get(0)), n(g.get(1)), n(g.get(2))); }
    public static Entity entity(Object index) {
        if (index == null) return null;
        // A case naming an entity it does not define is a corpus mistake, never an original failure.
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
    /** sendImpact/checkImpact stand-ins may overwrite hits, as the original methods can. */
    public static void script(BattleState s, Object spec) {
        if (spec == null) return;
        Map m = (Map) spec;
        if (m.containsKey("estimatedHit")) s.setEstimatedHit(n(m.get("estimatedHit")));
        if (m.containsKey("secondaryHit")) s.setSecondaryHit(n(m.get("secondaryHit")));
    }
    public static String label(Pulse p) {
        if (p instanceof CombatPulse) return "combat:" + ((CombatPulse) p).getEntity().tag;
        String name = p.getClass().getName();
        if (name.startsWith("core.game.node.entity.combat.CombatPulse$Companion$swing$")) return "impact";
        if (name.startsWith("core.game.node.entity.combat.RangeSwingHandler$impact$")) return "secondHit";
        if (name.startsWith("core.game.node.entity.combat.ImpactHandler$")) return "manualHit";
        throw new IllegalStateException("unknown pulse " + name);
    }

    /** Math.random's private generator: the seeded or scripted source of this case. */
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
        mathScript = Collections.singletonList(hex(0.25));
        mathAt = 0;
        if (Math.random() != 0.25) throw new IllegalStateException("Math.random is not bound to the oracle generator");
        events.clear();
    }

    /** Pulse.update prints a caught exception: Throwable.printStackTrace starts with println(this). */
    static void installErr() {
        realErr = System.err;
        System.setErr(new PrintStream(new OutputStream() { @Override public void write(int b) {} }) {
            @Override public void println(Object x) { if (x instanceof Throwable) emit("printStackTrace", classify((Throwable) x)); }
        });
    }

    static Object field(Class<?> c, Object target, String name) throws Exception { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(target); }

    static CombatSwingHandler make(Map h) {
        List fl = (List) h.getOrDefault("flags", Collections.emptyList());
        SwingHandlerFlag[] flags = new SwingHandlerFlag[fl.size()];
        for (int k = 0; k < flags.length; k++) flags[k] = SwingHandlerFlag.valueOf((String) fl.get(k));
        String kind = (String) h.get("kind");
        if (kind.equals("salamander")) return new content.global.handlers.item.equipment.special.SalamanderSwingHandler();
        return kind.equals("melee") ? new MeleeSwingHandler(flags) : kind.equals("range") ? new RangeSwingHandler(flags) : new MagicSwingHandler(flags);
    }

    static void world(Map c) throws Exception {
        input = c;
        entities = new ArrayList<>();
        spells = new ArrayList<>();
        queue.clear();
        duelAmmo = new ArrayList<>();
        tick = i(c, "tick", 0);
        clip = (Map) c.get("clip");
        pclip = (Map) c.get("pclip");
        uids = (Map) c.get("uids");
        inactive = (List) c.getOrDefault("inactiveRegions", Collections.emptyList());
        CombatSwingHandler.EXPERIENCE_MOD = 4.0;
        // The three CombatStyle singletons are process state: every case starts them fresh.
        handlers = new ArrayList<>();
        for (CombatStyle s : CombatStyle.values()) {
            CombatSwingHandler h = s.getSwingHandler();
            h.setType(s);
            h.getFlags().clear();
            Field f = CombatSwingHandler.class.getDeclaredField("specialHandlers");
            f.setAccessible(true);
            f.set(h, null);
            handlers.add(h);
        }
        for (Object o : (List) c.getOrDefault("handlers", Collections.emptyList())) handlers.add(make((Map) o));
        List specs = (List) c.getOrDefault("spells", Collections.emptyList());
        for (int k = 0; k < specs.size(); k++) spells.add(new CombatSpell(k, (Map) specs.get(k)));
        List es = (List) c.getOrDefault("entities", Collections.emptyList());
        for (int k = 0; k < es.size(); k++) {
            Map s = (Map) es.get(k);
            String kind = (String) s.getOrDefault("kind", "player");
            Entity e = kind.equals("player") ? new Player(k, s) : kind.equals("aiplayer") ? new core.game.bots.AIPlayer(k, s)
                : kind.equals("npc") ? new NPC(k, s) : kind.equals("randomEvent") ? new content.global.ame.RandomEventNPC(k, s)
                : kind.equals("familiar") ? new content.global.skill.summoning.familiar.Familiar(k, s) : new Entity(k, s);
            if (s.get("spell") != null) e.spell = spells.get(n(s.get("spell")));
            if (s.get("autocast") != null) e.autocast = spells.get(n(s.get("autocast")));
            entities.add(e);
        }
        for (Entity e : entities) e.bind();
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
    static CombatSwingHandler handler(Map op) { return handlers.get(op.get("h") == null ? 0 : n(op.get("h"))); }
    static Item worn(Object o) { if (o == null) return null; List a = (List) o; return ((Player) entity(a.get(0))).getEquipment().peek(n(a.get(1))); }

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
        if (s.get("spell") != null) b.setSpell(spells.get(n(s.get("spell"))));
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

    /** Everything the ported classes own, after an operation. Calls no recording stand-in. */
    static Map snapshot() throws Exception {
        Map<String, Object> m = new TreeMap<>();
        List<Object> pulses = new ArrayList<>(), impacts = new ArrayList<>(), tasks = new ArrayList<>(), hs = new ArrayList<>();
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
        }
        for (Pulse p : queue) {
            String label = label(p);
            Object shown = null;
            if (label.equals("impact")) shown = field(p.getClass(), p, "impact");
            tasks.add(Arrays.asList(label, p.isRunning(), p.getDelay(), p.getTicksPassed(), shown));
        }
        for (CombatSwingHandler h : handlers) {
            List<Object> flags = new ArrayList<>();
            for (SwingHandlerFlag f : h.getFlags()) flags.add(f.name());
            hs.add(Arrays.asList(h.getType() == null ? null : h.getType().name(), flags));
        }
        m.put("pulses", pulses);
        m.put("impacts", impacts);
        m.put("queue", tasks);
        m.put("handlers", hs);
        m.put("xpMod", hex(CombatSwingHandler.EXPERIENCE_MOD));
        return m;
    }

    static BattleState current;

    static BattleState given(Map op) {
        current = "none".equals(op.get("state")) ? null : state((Map) op.get("state"));
        return current;
    }

    static ImpactHandler.HitsplatType splat(Object o) { return o == null ? null : ImpactHandler.HitsplatType.valueOf((String) o); }

    /** The modelled runner: a fixed prefix of the queue per tick, unfinished running pulses back at the tail. */
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

    static Object run(Map op) throws Exception {
        String name = (String) op.get("op");
        Entity e = ent(op, "e"), v = ent(op, "v");
        switch (name) {
            case "canSwing": { InteractionType t = handler(op).canSwing(e, v); return t == null ? null : t.name(); }
            case "isAttackable": { InteractionType t = handler(op).isAttackable(e, v); return t == null ? null : t.name(); }
            case "canStepTowards": return handler(op).canStepTowards(e, v).name();
            case "canMelee": return MeleeSwingHandler.Companion.canMelee(e, v, n(op.get("distance")));
            case "projectile": return CombatSwingHandler.isProjectileClipped(e, v, Boolean.TRUE.equals(op.get("close")));
            case "adjust": handler(op).adjustBattleState(e, v, given(op)); return null;
            case "addXp": handler(op).addExperience(e, v, given(op)); return null;
            case "postSwing": handler(op).postSwing(e, v, given(op)); return null;
            case "useAmmo": RangeSwingHandler.Companion.useAmmo(e, given(op), loc(op.get("location"))); return null;
            case "visualize": handler(op).visualize(e, v, given(op)); return null;
            case "visualizeImpact": handler(op).visualizeImpact(e, v, given(op)); return null;
            case "visualizeAudio": handler(op).visualizeAudio(e, v, given(op)); return null;
            case "impact": handler(op).impact(e, v, given(op)); return null;
            case "onImpact": handler(op).onImpact(e, v, given(op)); return null;
            case "attackAnimation": return handler(op).getAttackAnimation(e, style(op.get("style"))).getId();
            case "register": return handler(op).register(n(op.get("itemId")), handlers.get(n(op.get("special"))));
            case "getSpecial": return handlerIndex(handler(op).getSpecial(n(op.get("itemId"))));
            case "setEffect": return ArmourSet.valueOf((String) op.get("set")).effect(e, v, given(op));
            case "setVisualize": ArmourSet.valueOf((String) op.get("set")).visualize(e, v); return null;
            case "setXpMod": CombatSwingHandler.EXPERIENCE_MOD = bits(op.get("mod")); return null;
            case "setType": handler(op).setType(style(op.get("style"))); return null;
            case "attack": e.getProperties().getCombatPulse().attack(v); return null;
            case "setVictim": e.getProperties().getCombatPulse().setVictim(v); return null;
            case "stop": e.getProperties().getCombatPulse().stop(); return null;
            case "start": e.getProperties().getCombatPulse().start(); return null;
            case "updateStyle": e.getProperties().getCombatPulse().updateStyle(); return null;
            case "setNextAttack": e.getProperties().getCombatPulse().setNextAttack(n(op.get("ticks"))); return null;
            case "delayNextAttack": e.getProperties().getCombatPulse().delayNextAttack(n(op.get("ticks"))); return null;
            case "canInteract": { InteractionType t = e.getProperties().getCombatPulse().canInteract(); return t == null ? null : t.name(); }
            case "isAttacking": return e.getProperties().getCombatPulse().isAttacking();
            case "isInCombat": return e.getProperties().getCombatPulse().isInCombat();
            case "setCombatFlags": e.getProperties().getCombatPulse().setCombatFlags(v); return null;
            case "setTemp": e.getProperties().getCombatPulse().setTemporaryHandler(op.get("h") == null ? null : handler(op)); return null;
            case "swing": return CombatPulse.Companion.swing(e, v, handler(op));
            case "pulse": return e.getProperties().getCombatPulse().pulse();
            case "update": return e.getProperties().getCombatPulse().update();
            case "tick": ticks(n(op.get("n"))); return null;
            case "handleImpact": return impact(e.getImpactHandler().handleImpact(ent(op, "source"), n(op.get("hit")), style(op.get("style")), null, splat(op.get("type")), Boolean.TRUE.equals(op.get("secondary"))));
            case "manualHit":
                if (op.containsKey("ticks")) return impact(e.getImpactHandler().manualHit(ent(op, "source"), n(op.get("hit")), splat(op.get("type")), n(op.get("ticks"))));
                return impact(e.getImpactHandler().manualHit(ent(op, "source"), n(op.get("hit")), splat(op.get("type"))));
            case "recoil": e.getImpactHandler().handleRecoilEffect(ent(op, "attacker"), n(op.get("hit"))); return null;
            case "mostDamage": return tag(e.getImpactHandler().getMostDamageEntity(ent(op, "killer")));
            case "setDisabled": e.getImpactHandler().setDisabledTicks(n(op.get("ticks"))); return null;
            case "isHitUpdate": return e.getImpactHandler().isHitUpdate();
            case "pollImpacts": { int size = e.getImpactHandler().getImpactQueue().size(); e.getImpactHandler().getImpactQueue().clear(); return size; }
            case "clearLogs": e.getImpactHandler().getNpcImpactLog().clear(); e.getImpactHandler().getPlayerImpactLog().clear(); return null;
            // World-model inputs between operations.
            case "set": {
                String key = (String) op.get("key");
                Object value = op.get("value");
                if (value == null) e.spec.remove(key); else e.spec.put(key, value);
                if (key.equals("location")) e.relocate(place(e.spec));
                if (key.equals("lifepoints")) e.lifepoints = n(value);
                if (key.equals("dead")) e.dead = Boolean.TRUE.equals(value);
                if (key.equals("prayerPoints")) e.prayerPoints = bits(value);
                return null;
            }
            case "setAttr": { String key = (String) op.get("key"); if (op.get("value") == null) e.attrs.remove(key); else e.attrs.put(key, op.get("value")); return null; }
            case "setSpell": e.spell = op.get("spell") == null ? null : spells.get(n(op.get("spell"))); e.autocast = op.get("autocast") == null ? null : spells.get(n(op.get("autocast"))); return null;
            case "setTick": tick = n(op.get("tick")); return null;
            case "tables": {
                Map<String, Object> out = new TreeMap<>();
                List<Object> types = new ArrayList<>();
                for (ImpactHandler.HitsplatType t : ImpactHandler.HitsplatType.values()) types.add(t.name());
                out.put("hitsplats", types);
                out.put("safeZone", core.game.world.map.zone.ZoneType.SAFE.getId());
                return out;
            }
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
                    try { result = run(op); } catch (Throwable e) { error = classify(e); }
                    failAt = -1;
                    row.put("result", result);
                    row.put("error", error);
                    row.put("events", new ArrayList<>(events));
                    if (current != null) row.put("state", dump(current));
                    // What the ported classes own is written whenever an operation changed it.
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
