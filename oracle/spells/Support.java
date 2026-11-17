package oracle;

import content.global.skill.magic.SpellUtils;
import core.game.event.Event;
import core.game.event.SpellCastEvent;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.node.entity.combat.equipment.WeaponInterface;
import core.game.node.entity.combat.spell.*;
import core.game.node.entity.impl.Projectile;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.SpellBookManager;
import core.game.node.entity.player.link.SpellBookManager.SpellBook;
import core.game.node.item.Item;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.util.*;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/** Reads one case per line, runs its operations against the unchanged spell classes and prints what they did. */
public class Support {
    static final String[] CLASSES = {
        "content.global.skill.magic.modern.AirSpell", "content.global.skill.magic.modern.WaterSpell",
        "content.global.skill.magic.modern.EarthSpell", "content.global.skill.magic.modern.FireSpell",
        "content.global.skill.magic.modern.BindSpell", "content.global.skill.magic.modern.CurseSpells",
        "content.global.skill.magic.modern.GodSpells", "content.global.skill.magic.modern.CrumbleUndead",
        "content.global.skill.magic.modern.IbanBlast", "content.global.skill.magic.modern.MagicDart",
        "content.global.skill.magic.modern.TeleblockSpell", "content.global.skill.magic.modern.SonicWaveSpell",
        "content.global.skill.magic.ancient.IceSpells", "content.global.skill.magic.ancient.BloodSpells",
        "content.global.skill.magic.ancient.ShadowSpells", "content.global.skill.magic.ancient.SmokeSpells",
        "content.global.skill.magic.ancient.MiasmicSpells",
    };

    static Map input;
    static Map itemNames;
    static List<Object> events;
    static int calls, failAt;
    static boolean quiet;
    static List<Node> nodes;
    static Map<Node, Map> specs;
    static Map<Node, List<int[]>> inventories;
    static List<CombatSpell> defaults;
    static List<Object> mathScript;
    static int mathAt;
    static Random mathBase;
    static Double forced;

    public static class Failure extends RuntimeException {}

    static class MathRng extends Random {
        @Override public double nextDouble() {
            double v;
            if (forced != null) v = forced;
            else if (mathScript != null) {
                if (mathAt >= mathScript.size()) throw new IllegalStateException("math draws exhausted");
                v = bits(mathScript.get(mathAt++));
            } else v = mathBase.nextDouble();
            if (!quiet) emit("math_random", hex(v));
            return v;
        }
    }

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

    public static String hex(double v) { return String.format("%016x", Double.doubleToRawLongBits(v)); }
    static double bits(Object hex) { return Double.longBitsToDouble(Long.parseUnsignedLong((String) hex, 16)); }
    static int n(Object o) { return ((Number) o).intValue(); }

    public static void emit(String name, Object... args) {
        if (quiet) return;
        List row = new ArrayList();
        row.add(name);
        for (Object a : args) row.add(a);
        events.add(row);
        calls++;
        if (calls == failAt) throw new Failure();
    }

    public static Integer ix(Object node) {
        if (node == null) return null;
        for (int i = 0; i < nodes.size(); i++) if (nodes.get(i) == node) return i;
        if (node instanceof Item) return -1 - ((Item) node).getId();
        throw new IllegalStateException("unknown node");
    }

    public static List coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    static Location loc(Object o) { if (o == null) return null; List a = (List) o; return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2))); }
    static Map spec(Object node) { return specs.get(node); }

    public static List gfx(Graphics g) { return g == null ? null : Arrays.asList(g.id, g.height, g.delay); }
    public static List anim(Animation a) { return a == null ? null : Arrays.asList(a.id, a.delay, a.priority == null ? null : a.priority.name()); }
    static Graphics graphics(Object o) { if (o == null) return null; List a = (List) o; return new Graphics(n(a.get(0)), n(a.get(1)), n(a.get(2))); }
    static Animation animation(Object o) {
        if (o == null) return null;
        List a = (List) o;
        return new Animation(n(a.get(0)), n(a.get(1)), core.game.node.entity.impl.Animator.Priority.valueOf((String) a.get(2)));
    }

    public static Object value(Object v) {
        if (v == null || v instanceof Integer || v instanceof Boolean || v instanceof String) return v;
        if (v instanceof List) {
            List out = new ArrayList();
            for (Object o : (List) v) { Item i = (Item) o; out.add(Arrays.asList(i.getId(), i.getAmount())); }
            Map m = new LinkedHashMap();
            m.put("items", out);
            return m;
        }
        throw new IllegalStateException("attribute value " + v.getClass());
    }

    static Object unvalue(Object v) {
        if (v instanceof Map) {
            ArrayList<Item> out = new ArrayList<>();
            for (Object o : (List) ((Map) v).get("items")) { List a = (List) o; out.add(new Item(n(a.get(0)), n(a.get(1)))); }
            return out;
        }
        if (v instanceof Long) return ((Long) v).intValue();
        return v;
    }

    public static boolean flag(Object node, String key, boolean fallback) {
        Object v = spec(node).get(key);
        return v == null ? fallback : Boolean.TRUE.equals(v);
    }
    public static int ticks() { return n(input.getOrDefault("ticks", 0L)); }
    public static boolean devMode() { return Boolean.TRUE.equals(input.get("devMode")); }
    public static String itemName(int id) {
        Map names = itemNames;
        Object v = names == null ? null : names.get(String.valueOf(id));
        return v == null ? "null" : (String) v;
    }
    public static int level(Entity e, int slot) {
        Map l = (Map) spec(e).get("levels");
        Object v = l == null ? null : l.get(String.valueOf(slot));
        return v == null ? 1 : n(v);
    }
    public static Integer worn(Entity e, int slot) {
        Map w = (Map) spec(e).get("worn");
        Object v = w == null ? null : w.get(String.valueOf(slot));
        return v == null ? null : n(v);
    }
    public static int inventoryAmount(Entity e, int id) {
        int total = 0;
        for (int[] slot : inventories.get(e)) if (slot[0] == id) total += slot[1];
        return total;
    }
    /** The stand-in removal: from the first slot holding the id onwards, emptied slots leave the list. */
    public static boolean inventoryRemove(Entity e, int id, int amount) {
        List<int[]> slots = inventories.get(e);
        int left = amount;
        for (Iterator<int[]> it = slots.iterator(); it.hasNext() && left > 0;) {
            int[] slot = it.next();
            if (slot[0] != id) continue;
            int take = Math.min(left, slot[1]);
            slot[1] -= take;
            left -= take;
            if (slot[1] == 0) it.remove();
        }
        return left == 0;
    }
    public static Integer attackStyle(Entity e) { Object v = spec(e).get("style"); return v == null ? null : n(v); }
    public static int styleIndex(Entity e) { return n(spec(e).getOrDefault("styleIndex", 0L)); }
    public static Animation magicAnimation(Entity e) { return animation(spec(e).get("magicAnimation")); }
    public static Animation attackAnimation(Entity e) { return animation(spec(e).get("attackAnimation")); }
    public static int[] bonuses(Entity e) {
        Object v = spec(e).get("bonuses");
        if (v == null) return null;
        List l = (List) v;
        int[] b = new int[l.size()];
        for (int i = 0; i < b.length; i++) b[i] = n(l.get(i));
        return b;
    }
    public static boolean inZone(Entity e, String name) { List z = (List) spec(e).get("zones"); return z != null && z.contains(name); }
    public static boolean timer(Entity e, String name) { List z = (List) spec(e).get("timers"); return z != null && z.contains(name); }
    public static int[] godCasts(Entity e) { return (int[]) spec(e).get("godCastsArray"); }
    public static NPC.Task task(NPC npc) { Object v = spec(npc).get("task"); return v == null ? null : new NPC.Task(Boolean.TRUE.equals(v)); }
    public static Graphics[] combatGraphics(NPC npc) {
        List l = (List) spec(npc).get("combatGraphics");
        Graphics[] g = new Graphics[l.size()];
        for (int i = 0; i < g.length; i++) g[i] = graphics(l.get(i));
        return g;
    }
    public static List<Entity> multiTargets(Entity target) {
        List<Entity> out = new ArrayList<>();
        for (Object o : (List) spec(target).getOrDefault("multiTargets", Collections.emptyList())) out.add((Entity) nodes.get(n(o)));
        return out;
    }

    public static void event(Player p, Event e) {
        SpellCastEvent s = (SpellCastEvent) e;
        emit("dispatch", ix(p), "SpellCastEvent", s.getSpellBook().name(), s.getSpellId(), ix(s.getTarget()));
    }

    /** A spell's identity: its book and id, or its index among this case's default spells. */
    public static Object label(MagicSpell spell) {
        if (spell == null) return null;
        int k = defaults.indexOf(spell);
        if (k >= 0) return Arrays.asList("DEFAULT", k);
        return Arrays.asList(spell.getBook().name(), spell.getSpellId());
    }

    static MagicSpell spell(Object o) {
        if (o == null) return null;
        List l = (List) o;
        if ("DEFAULT".equals(l.get(0))) return defaults.get(n(l.get(1)));
        return SpellBook.valueOf((String) l.get(0)).getSpell(n(l.get(1)));
    }

    static Node node(Object o) {
        if (o == null) return null;
        if (n(o) < 0 || n(o) >= nodes.size()) throw new IllegalStateException("corpus node index " + o);
        return nodes.get(n(o));
    }

    static Object field(Object target, Class<?> c, String name) {
        try { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(target); } catch (Exception e) { throw new IllegalStateException(e); }
    }

    /** Names each projectile template by the static field of its class that holds it, else by its constructor. */
    static void labelProjectiles() throws Exception {
        for (SpellBook book : SpellBook.values()) {
            for (int button = 0; button < 70000; button++) {
                MagicSpell s = book.getSpell(button);
                if (!(s instanceof CombatSpell)) continue;
                Projectile p = (Projectile) field(s, CombatSpell.class, "projectile");
                if (p == null || !"unlabelled".equals(p.label)) continue;
                p.label = s.getClass().getSimpleName() + ".new";
                for (Field f : s.getClass().getDeclaredFields()) {
                    if (!Modifier.isStatic(f.getModifiers()) || f.getType() != Projectile.class) continue;
                    f.setAccessible(true);
                    if (f.get(null) == p) p.label = s.getClass().getSimpleName() + "." + f.getName();
                }
            }
        }
    }

    static List items(Item[] runes) {
        if (runes == null) return null;
        List out = new ArrayList();
        for (Item i : runes) out.add(i == null ? null : Arrays.asList(i.getId(), i.getAmount()));
        return out;
    }

    static Map row(MagicSpell s) {
        Map m = new LinkedHashMap();
        m.put("class", s.getClass().getSimpleName());
        m.put("book", s.getBook().name());
        m.put("spellId", s.getSpellId());
        m.put("level", s.getLevel());
        m.put("levelRequirement", s.levelRequirement());
        m.put("experience", hex(s.getExperience()));
        m.put("delay", s.getDelay());
        m.put("runes", items(s.getCastRunes()));
        m.put("animation", anim((Animation) field(s, MagicSpell.class, "animation")));
        m.put("graphic", gfx((Graphics) field(s, MagicSpell.class, "graphic")));
        core.game.node.entity.player.link.audio.Audio a = s.getAudio();
        m.put("audio", a == null ? null : Arrays.asList(a.id, a.delay, a.loops, a.radius));
        if (s instanceof CombatSpell) {
            CombatSpell c = (CombatSpell) s;
            m.put("type", c.getType().name());
            m.put("accuracy", hex(c.getAccuracyMod()));
            m.put("impactAudio", field(s, CombatSpell.class, "impactAudio"));
            m.put("endGraphic", gfx((Graphics) field(s, CombatSpell.class, "endGraphic")));
            Projectile p = (Projectile) field(s, CombatSpell.class, "projectile");
            m.put("projectile", p == null ? null : Arrays.asList(p.label, p.projectileId, p.startHeight, p.endHeight, p.startDelay, p.speed, p.angle, p.distance));
        }
        return m;
    }

    static List table() {
        List runes = new ArrayList();
        for (Runes r : Runes.values()) runes.add(Arrays.asList(r.name(), r.getId(), r.transform().getId(), r.transform().getAmount(), r.getItem(7).getAmount()));
        List combos = new ArrayList();
        for (CombinationRune c : CombinationRune.values()) {
            List types = new ArrayList();
            for (Runes r : c.types) types.add(r.name());
            combos.add(Arrays.asList(c.name(), c.id, types));
        }
        List staves = new ArrayList();
        for (MagicStaff s : MagicStaff.values()) {
            List ids = new ArrayList();
            for (int id : s.getStaves()) ids.add(id);
            staves.add(Arrays.asList(s.name(), s.getRuneId(), ids));
        }
        List types = new ArrayList();
        for (SpellType t : SpellType.values()) types.add(Arrays.asList(t.name(), t.ordinal(), hex(t.getAccuracyMod())));
        List books = new ArrayList();
        for (SpellBook b : SpellBook.values()) {
            List spells = new ArrayList();
            for (int button = 0; button < 70000; button++) {
                MagicSpell s = b.getSpell(button);
                if (s != null) spells.add(Arrays.asList(button, row(s)));
            }
            books.add(Arrays.asList(b.name(), b.ordinal(), b.getInterfaceId(), spells));
        }
        List constants = Arrays.asList(gfx(CombatSpell.SPLASH_GRAPHIC), anim(CombatSpell.AUTOCAST_ANIMATION), Arrays.asList(SpellBookManager.SpellbookChangeSource.values().length));
        return Arrays.asList(runes, combos, staves, types, books, constants);
    }

    static Item[] runeItems(Object o) {
        if (o == null) return null;
        List l = (List) o;
        Item[] out = new Item[l.size()];
        for (int i = 0; i < out.length; i++) { List a = (List) l.get(i); out[i] = new Item(n(a.get(0)), n(a.get(1))); }
        return out;
    }

    static SpellType[] typeArray(Object o) {
        if (o == null) return null;
        List l = (List) o;
        SpellType[] out = new SpellType[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = SpellType.valueOf((String) l.get(i));
        return out;
    }

    static Object run(Map op) throws Throwable {
        String name = (String) op.get("op");
        if (op.containsKey("experienceMod")) CombatSwingHandler.EXPERIENCE_MOD = bits(op.get("experienceMod"));
        switch (name) {
            case "table": return table();
            case "runes_for_id": { Runes r = Runes.forId(n(op.get("id"))); return r == null ? null : r.name(); }
            case "is_infinite": {
                Runes r = op.get("rune") == null ? null : Runes.valueOf((String) op.get("rune"));
                Item w = op.get("weapon") == null ? null : new Item(n(op.get("weapon")));
                return Runes.isInfinite(r, w, typeArray(op.get("types")));
            }
            case "staff_for_id": {
                MagicStaff s = MagicStaff.forId(n(op.get("rune")));
                return s == null ? null : s.name();
            }
            case "eligible_for": {
                List out = new ArrayList();
                for (CombinationRune c : CombinationRune.eligibleFor(op.get("rune") == null ? null : Runes.valueOf((String) op.get("rune")))) out.add(c.name());
                return out;
            }
            case "type_impact":
                return SpellType.valueOf((String) op.get("type")).getImpactAmount((Entity) node(op.get("e")), (Entity) node(op.get("victim")), n(op.get("base")));
            case "using_staff": return spell(op.get("spell")).usingStaff((Player) node(op.get("p")), n(op.get("rune")));
            case "utils_using_staff": return SpellUtils.INSTANCE.usingStaff((Player) node(op.get("p")), n(op.get("rune")));
            case "has_runes": {
                Item missing = SpellUtils.hasRunes((Player) node(op.get("p")), runeItems(op.get("runes")));
                return missing == null ? null : Arrays.asList(missing.getId(), missing.getAmount());
            }
            case "meets": return spell(op.get("spell")).meetsRequirements((Entity) node(op.get("e")), Boolean.TRUE.equals(op.get("message")), Boolean.TRUE.equals(op.get("remove")));
            case "check_level": return spell(op.get("spell")).checkLevelRequirement((Entity) node(op.get("e")), Boolean.TRUE.equals(op.get("message")));
            case "add_experience": spell(op.get("spell")).addExperience((Entity) node(op.get("e")), n(op.get("hit"))); return null;
            case "visualize": spell(op.get("spell")).visualize((Entity) node(op.get("e")), node(op.get("target"))); return null;
            case "visualize_impact": {
                BattleState s = new BattleState((Entity) node(op.get("e")), (Entity) node(op.get("target")));
                s.estimatedHit = n(op.get("hit"));
                s.frozen = Boolean.TRUE.equals(op.get("frozen"));
                ((CombatSpell) spell(op.get("spell"))).visualizeImpact((Entity) node(op.get("e")), (Entity) node(op.get("target")), s);
                return Arrays.asList(s.estimatedHit, s.frozen);
            }
            case "cast": return spell(op.get("spell")).cast((Entity) node(op.get("e")), node(op.get("target")));
            case "cast_spell":
                return MagicSpell.castSpell((Player) node(op.get("p")), SpellBook.valueOf((String) op.get("book")), n(op.get("button")), node(op.get("target")));
            case "max_impact": {
                Entity e = (Entity) node(op.get("e"));
                Entity v = (Entity) node(op.get("victim"));
                return ((CombatSpell) spell(op.get("spell"))).getMaximumImpact(e, v, new BattleState(e, v));
            }
            case "targets": {
                BattleState[] t = ((CombatSpell) spell(op.get("spell"))).getTargets((Entity) node(op.get("e")), (Entity) node(op.get("target")));
                if (t == null) return null;
                List out = new ArrayList();
                for (BattleState s : t) out.add(s == null ? null : Arrays.asList(ix(s.entity), ix(s.victim)));
                return out;
            }
            case "accuracy": return hex(((CombatSpell) spell(op.get("spell"))).getAccuracyMod());
            case "experience": { Node p = node(op.get("p")); return hex(spell(op.get("spell")).getExperience(p instanceof Player ? (Player) p : null)); }
            case "make_default": {
                defaults.add(new DefaultCombatSpell((NPC) node(op.get("npc"))));
                return defaults.size() - 1;
            }
            case "set_book": ((Player) node(op.get("p"))).getSpellBookManager().setSpellBook(SpellBook.valueOf((String) op.get("book"))); return null;
            case "book_update": ((Player) node(op.get("p"))).getSpellBookManager().update((Player) node(op.get("p"))); return null;
            case "for_interface": { SpellBook b = SpellBook.forInterface(n(op.get("id"))); return b == null ? null : b.name(); }
            case "get_spell": return label(SpellBook.valueOf((String) op.get("book")).getSpell(n(op.get("button"))));
            case "block_register": SpellBlocks.register(n(op.get("spellId")), node(op.get("node"))); return null;
            case "is_blocked": return SpellBlocks.isBlocked(n(op.get("spellId")), node(op.get("node")));
            case "autospell_id": return weapon(op).getAutospellId(n(op.get("spellId")));
            case "select_auto": weapon(op).selectAutoSpell(n(op.get("button")), Boolean.TRUE.equals(op.get("adjust")), Boolean.TRUE.equals(op.get("forget"))); return null;
            case "can_autocast": return weapon(op).canAutocast(Boolean.TRUE.equals(op.get("message")));
            case "set": apply(node(op.get("e")), (Map) op.get("fields")); return null;
            case "world": {
                if (op.containsKey("ticks")) input.put("ticks", op.get("ticks"));
                if (op.containsKey("devMode")) input.put("devMode", op.get("devMode"));
                return null;
            }
            default: throw new IllegalStateException("unknown op " + name);
        }
    }

    static WeaponInterface weapon(Map op) {
        Player p = (Player) node(op.get("p"));
        return new WeaponInterface(p, flag(p, "staffInterface", true));
    }

    static void apply(Node node, Map m) {
        Map spec = specs.get(node);
        for (Object o : m.entrySet()) {
            Map.Entry en = (Map.Entry) o;
            String key = (String) en.getKey();
            Object v = en.getValue();
            switch (key) {
                case "name": node.name = (String) v; break;
                case "location": node.location = loc(v); break;
                case "inventory": {
                    List<int[]> slots = new ArrayList<>();
                    for (Object s : (List) v) { List a = (List) s; slots.add(new int[] { n(a.get(0)), n(a.get(1)) }); }
                    inventories.put(node, slots);
                    break;
                }
                case "attributes":
                    for (Object a : ((Map) v).entrySet()) {
                        Map.Entry ae = (Map.Entry) a;
                        if (ae.getValue() == null) ((Entity) node).attributes.remove(ae.getKey());
                        else ((Entity) node).attributes.put((String) ae.getKey(), unvalue(ae.getValue()));
                    }
                    break;
                case "autocast": ((Entity) node).getProperties().autocastSpell = (CombatSpell) spell(v); break;
                case "book": ((Player) node).getSpellBookManager().setSpellBook(SpellBook.valueOf((String) v)); break;
                case "npcId": ((NPC) node).id = n(v); break;
                case "godCasts": {
                    List l = (List) v;
                    int[] a = new int[l.size()];
                    for (int i = 0; i < a.length; i++) a[i] = n(l.get(i));
                    spec.put("godCastsArray", a);
                    break;
                }
                default: spec.put(key, v);
            }
        }
    }

    static Map snapshot(Node node) {
        Map m = new LinkedHashMap();
        if (!(node instanceof Entity)) return m;
        Entity e = (Entity) node;
        Map attributes = new TreeMap();
        for (Map.Entry<String, Object> a : e.attributes.entrySet()) attributes.put(a.getKey(), value(a.getValue()));
        m.put("attributes", attributes);
        m.put("autocast", label(e.getProperties().autocastSpell));
        if (inventories.containsKey(e)) {
            List inv = new ArrayList();
            for (int[] s : inventories.get(e)) inv.add(Arrays.asList(s[0], s[1]));
            m.put("inventory", inv);
        }
        if (e instanceof Player) m.put("book", ((Player) e).getSpellBookManager().getSpellBook());
        int[] casts = (int[]) specs.get(e).get("godCastsArray");
        if (casts != null) {
            List c = new ArrayList();
            for (int x : casts) c.add(x);
            m.put("godCasts", c);
        }
        return m;
    }

    static String error(Throwable t) {
        if (t instanceof Failure) return "Binding";
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        if (t instanceof ClassCastException) return "Cast";
        return t.getClass().getName() + ": " + t.getMessage();
    }

    @SuppressWarnings("unchecked")
    static void reset() throws Exception {
        Field blocks = SpellBlocks.class.getDeclaredField("blocks");
        blocks.setAccessible(true);
        ((Map) blocks.get(null)).clear();
    }

    public static void main(String[] args) throws Exception {
        installMath();
        itemNames = (Map) new JSONParser().parse(new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get(args[0])), StandardCharsets.UTF_8));
        events = new ArrayList<>();
        quiet = true;
        for (String c : CLASSES) {
            Object instance = Class.forName(c).getDeclaredConstructor().newInstance();
            try { ((core.plugin.Plugin) instance).newInstance(null); } catch (Throwable t) { throw new IllegalStateException(t); }
        }
        labelProjectiles();
        quiet = false;
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        JSONParser parser = new JSONParser();
        String line;
        StringBuilder out = new StringBuilder();
        while ((line = reader.readLine()) != null) {
            if (line.isEmpty()) continue;
            input = (Map) parser.parse(line);
            Map original = (Map) parser.parse(line);
            events = new ArrayList<>();
            nodes = new ArrayList<>();
            specs = new IdentityHashMap<>();
            inventories = new IdentityHashMap<>();
            defaults = new ArrayList<>();
            reset();
            CombatSwingHandler.EXPERIENCE_MOD = input.get("experienceMod") == null ? 4.0 : bits(input.get("experienceMod"));
            mathScript = (List) input.get("math");
            mathAt = 0;
            mathBase = new Random(((Number) input.getOrDefault("mathSeed", 0L)).longValue());
            quiet = true;
            for (Object o : (List) input.get("entities")) {
                Map spec = (Map) o;
                String kind = (String) spec.getOrDefault("kind", "player");
                Node e = kind.equals("npc") ? new NPC() : kind.equals("node") ? new TargetNode() : new Player();
                nodes.add(e);
                specs.put(e, new HashMap());
                if (e instanceof Player) inventories.put(e, new ArrayList<>());
                e.location = Location.create(3200, 3200, 0);
                apply(e, spec);
            }
            quiet = false;
            List results = new ArrayList();
            List previous = new ArrayList();
            for (Node e : nodes) previous.add(snapshot(e));
            List initial = previous;
            for (Object o : (List) input.get("ops")) {
                Map op = (Map) o;
                events = new ArrayList<>();
                calls = 0;
                failAt = op.containsKey("failAt") ? n(op.get("failAt")) : -1;
                Map step = new LinkedHashMap();
                Object result = null;
                String error = null;
                try {
                    result = run(op);
                } catch (Throwable t) {
                    error = error(t);
                }
                step.put("result", result);
                step.put("error", error);
                step.put("events", events);
                List states = new ArrayList();
                for (Node e : nodes) states.add(snapshot(e));
                // The whole state after the operation, or null when it equals the state before it.
                step.put("state", states.equals(previous) ? null : states);
                previous = states;
                step.put("experienceMod", hex(CombatSwingHandler.EXPERIENCE_MOD));
                step.put("mathDraws", mathAt);
                results.add(step);
            }
            Map row = new LinkedHashMap();
            row.put("input", original);
            row.put("expected", results);
            row.put("initial", initial);
            out.append(JSONValue.toJSONString(row)).append('\n');
        }
        System.out.write(out.toString().getBytes(StandardCharsets.UTF_8));
        System.out.flush();
    }
}
