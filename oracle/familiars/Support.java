package oracle;

import content.global.skill.summoning.familiar.BurdenBeast;
import content.global.skill.summoning.familiar.BurdenContainerListener;
import content.global.skill.summoning.familiar.Familiar;
import content.global.skill.summoning.familiar.FamiliarManager;
import content.global.skill.summoning.familiar.FamiliarSpecial;
import content.global.skill.summoning.familiar.Forager;
import content.global.skill.summoning.pet.Pet;
import content.global.skill.summoning.pet.PetDetails;
import content.global.skill.summoning.pet.Pets;
import core.game.component.CloseEvent;
import core.game.component.Component;
import core.game.container.Container;
import core.game.container.ContainerEvent;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import java.lang.reflect.Field;
import java.util.*;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Runs one case (Boot gives every case a class loader of its own, so FamiliarManager.FAMILIARS and the Pets maps
 * start fresh) against the unmodified familiar framework and returns what it did: every recorded call of every
 * operation, its result or failure class, and a snapshot of the manager, the familiars seen so far and the registry.
 */
public class Support {
    public static class Failure extends RuntimeException {}

    static Map input;
    static List<Object> events;
    static Map<String, Integer> calls;
    static int opCalls, failAt, npcs, containers, states;
    public static boolean quiet;
    public static Map<String, Node> nodes;
    public static Player player;
    static Map answers;
    static Map<String, Integer> answerCalls;
    public static List<Pulse> pulses;
    public static List<Object[]> closeEvents;
    static List<Familiar> seen;
    static IdentityHashMap<Object, Integer> labels;

    static int n(Object o) { return ((Number) o).intValue(); }
    public static Map input() { return input; }
    public static Object valueOf(Object v) { return value(v); }
    static Map spec(String k) { Map m = (Map) input.get(k); return m == null ? Collections.emptyMap() : m; }

    // ------------------------------------------------------------ recording
    public static void emit(String name, Object... args) {
        if (quiet) return;
        List<Object> row = new ArrayList<>();
        row.add(name);
        for (Object a : args) row.add(a);
        events.add(row);
        opCalls++;
        int count = calls.merge(name, 1, Integer::sum);
        for (Object h : (List) input.getOrDefault("hooks", Collections.emptyList())) {
            Map hook = (Map) h;
            if (name.equals(hook.get("event")) && count == n(hook.getOrDefault("at", 1L))) {
                for (Object action : (List) hook.get("actions")) {
                    Map a = (Map) action;
                    if ("fail".equals(a.get("op"))) throw new Failure();
                    try { op((Map) a.get("do")); } catch (RuntimeException e) { throw e; } catch (Throwable t) { throw new RuntimeException(t); }
                }
            }
        }
        if (opCalls == failAt) throw new Failure();
    }

    /** The case's answer for a call: `name@receiver`, then `name`; a list answers one element per call. */
    public static Object answer(String name, String receiver, Object fallback) {
        String key = receiver != null && answers.containsKey(name + "@" + receiver) ? name + "@" + receiver : name;
        if (!answers.containsKey(key)) return fallback;
        Object v = answers.get(key);
        if (v instanceof List) {
            List l = (List) v;
            int i = answerCalls.merge(key, 1, Integer::sum) - 1;
            v = l.get(Math.min(i, l.size() - 1));
        }
        return v instanceof Long ? (Object) ((Long) v).intValue() : v;
    }
    public static boolean bool(String name, String receiver, boolean fallback) { return Boolean.TRUE.equals(answer(name, receiver, fallback)); }
    public static int integer(String name, String receiver, int fallback) { return n(answer(name, receiver, fallback)); }

    // ------------------------------------------------------------ values
    public static String key(Object o) {
        if (o == null) return null;
        if (o instanceof Node && ((Node) o).key != null) return ((Node) o).key;
        throw new IllegalStateException("key of " + o.getClass());
    }
    public static Object bits(double d) { Map m = new TreeMap(); m.put("double", Double.doubleToRawLongBits(d)); return m; }
    public static Object item(Item i) {
        if (i == null) return null;
        Map m = new TreeMap(); m.put("item", Arrays.asList(i.getId(), i.getAmount(), i.getSlot())); return m;
    }
    public static List items(Item[] a) { if (a == null) return null; List l = new ArrayList(); for (Item i : a) l.add(item(i)); return l; }
    public static List coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    public static Location location(Object v) {
        if (v == null) return null;
        List a = (List) v;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }
    public static Item itemOf(Object v) {
        if (v == null) return null;
        List a = (List) v;
        Item i = new Item(n(a.get(0)), n(a.get(1)));
        if (a.size() > 2) i.setSlot(n(a.get(2)));
        return i;
    }
    public static Node node(Object k) { return k == null ? null : nodes.get((String) k); }
    static Object value(Object v) {
        if (v == null || v instanceof Boolean || v instanceof Integer || v instanceof String) return v;
        if (v instanceof Double) return bits((Double) v);
        if (v instanceof Item) return item((Item) v);
        if (v instanceof Node) return key(v);
        if (v instanceof Location) return coord((Location) v);
        if (v instanceof Enum) return ((Enum) v).name();
        if (v instanceof CombatSwingHandler) return ((CombatSwingHandler) v).key;
        throw new IllegalStateException("value of " + v.getClass());
    }

    // ------------------------------------------------------------ case setup
    public static int npcKey() { return ++npcs; }
    public static int containerKey() { return ++containers; }
    public static int stateKey() { return ++states; }
    public static Map classSpec(int id) {
        Map classes = spec("classes");
        Map m = (Map) classes.get(Integer.toString(id));
        if (m != null) return m;
        // Otherwise the class listing the id among its getIds().
        for (Object k : new TreeSet<Object>(classes.keySet())) {
            Map c = (Map) classes.get(k);
            for (Object i : (List) c.get("ids")) if (n(i) == id) return c;
        }
        throw new IllegalStateException("no class for " + id);
    }
    public static String npcName(int id) { Object v = spec("npcNames").get(Integer.toString(id)); return v == null ? "null" : (String) v; }
    public static Map itemSpec(int id) { Map m = (Map) spec("items").get(Integer.toString(id)); return m == null ? Collections.emptyMap() : m; }

    static Familiar create(int id, Player owner) {
        String type = (String) classSpec(id).get("type");
        switch (type) {
            case "plain": return new CaseFamiliar(owner, id);
            case "beast": return new CaseBeast(owner, id);
            case "forager": return new CaseForager(owner, id);
            default: throw new IllegalStateException(type);
        }
    }
    public static void kept(Familiar f) { if (!seen.contains(f)) seen.add(f); }

    // ------------------------------------------------------------ operations
    static Familiar target(Map op) {
        Object k = op.get("fam");
        if (k == null) return player.getFamiliarManager().getFamiliar();
        Node n = nodes.get((String) k);
        return (Familiar) n;
    }

    static Object op(Map op) throws Throwable {
        FamiliarManager m = player.getFamiliarManager();
        String name = (String) op.get("op");
        switch (name) {
            // ---- setup, not calls
            case "setLocation": { Node n = node(op.get("node")); n.setLocationQuiet(location(op.get("location"))); return null; }
            case "setInvisible": { Node n = node(op.get("node")); n.setInvisibleQuiet(Boolean.TRUE.equals(op.get("value"))); return null; }
            case "setAttribute": { Entity e = (Entity) node(op.get("node")); Object v = op.get("value"); e.attributes.put((String) op.get("key"), v instanceof Long ? ((Long) v).intValue() : v instanceof Map ? node(((Map) v).get("node")) : v); return null; }
            // ---- registry and construction
            case "register": {
                Familiar proto = create(n(op.get("id")), op.get("owner") == null ? null : (Player) node(op.get("owner")));
                Object r = proto.newInstance(null);
                return r == null ? null : key(r);
            }
            case "newFamiliar": { Familiar f = create(n(op.get("id")), (Player) node(op.getOrDefault("owner", "p"))); kept(f); return key(f); }
            case "construct": {
                Familiar proto = FamiliarManager.getFamiliars().get(n(op.get("id")));
                Familiar f = proto.construct((Player) node(op.getOrDefault("owner", "p")), n(op.get("id")));
                kept(f); return key(f);
            }
            case "newPet": {
                PetDetails d = new PetDetails(((Number) op.getOrDefault("growth", 0L)).doubleValue());
                Pet p = new Pet(player, d, n(op.get("item")), n(op.get("id")));
                kept(p); return key(p);
            }
            case "setFamiliar": { m.setFamiliar(op.get("fam") == null ? null : (Familiar) node(op.get("fam"))); return null; }
            // ---- FamiliarManager
            case "parse": { m.parse((JSONObject) new JSONParser().parse((String) op.get("json"))); return null; }
            case "login": m.login(); return null;
            case "summon":
                if (op.containsKey("delete")) m.summon(itemOf(op.get("item")), Boolean.TRUE.equals(op.get("pet")), Boolean.TRUE.equals(op.get("delete")));
                else m.summon(itemOf(op.get("item")), Boolean.TRUE.equals(op.get("pet")));
                return null;
            case "morphPet": m.morphPet(itemOf(op.get("item")), Boolean.TRUE.equals(op.get("delete")), location(op.get("location")), ((Number) op.get("hunger")).doubleValue(), ((Number) op.get("growth")).doubleValue()); return null;
            case "morphFamiliar": m.morphFamiliar(location(op.get("location"))); return null;
            case "spawnFamiliar": m.spawnFamiliar(); return null;
            case "eat": m.eat(n(op.get("food")), (Pet) target(op)); return null;
            case "pickup": m.pickup(); return null;
            case "adjustBattleState": m.adjustBattleState(new BattleState(player, (Entity) node(op.get("victim")))); return null;
            case "getBoost": return m.getBoost(n(op.get("skill")));
            case "hasFamiliar": return m.hasFamiliar();
            case "hasPet": return m.hasPet();
            case "dismiss": m.dismiss(); return null;
            case "addDetails": {
                PetDetails d;
                if (op.containsKey("from")) { Pet p = (Pet) node(op.get("from")); d = p.getDetails(); }
                else d = new PetDetails(((Number) op.getOrDefault("growth", 0L)).doubleValue());
                m.addDetails(n(op.get("item")), d); return null;
            }
            case "removeDetails": m.removeDetails(n(op.get("item"))); return null;
            case "isOwner": return m.isOwner((Familiar) node(op.get("fam")));
            case "isUsingSummoning": return m.isUsingSummoning();
            case "setHasPouch": m.setHasPouch(Boolean.TRUE.equals(op.get("value"))); return m.isHasPouch();
            case "setCombatLevel": m.setSummoningCombatLevel(n(op.get("value"))); return m.getSummoningCombatLevel();
            // ---- Familiar
            case "tick": target(op).handleTickActions(); return null;
            case "init": target(op).init(); return null;
            case "initAt": target(op).init(location(op.get("location")), Boolean.TRUE.equals(op.get("call"))); return null;
            case "isAttackable": return target(op).isAttackable((Entity) node(op.get("entity")), CombatStyle.valueOf((String) op.get("style")), Boolean.TRUE.equals(op.get("message")));
            case "continueAttack": return target(op).continueAttack((Entity) node(op.get("entity")), CombatStyle.valueOf((String) op.get("style")), Boolean.TRUE.equals(op.get("message")));
            case "regionInactivity": target(op).onRegionInactivity(); return null;
            case "swingHandler": return value(target(op).getSwingHandler(Boolean.TRUE.equals(op.get("swing"))));
            case "setCombatHandler": target(op).setCombatHandler(op.get("handler") == null ? null : new CombatSwingHandler((String) op.get("handler"), null)); return null;
            case "transform": target(op).transform(); return null;
            case "refreshTimer": target(op).refreshTimer(); return null;
            case "specialMove": {
                Node node = node(op.get("node"));
                FamiliarSpecial s = op.containsKey("iface") ? new FamiliarSpecial(node, n(op.get("iface")), n(op.get("component")), itemOf(op.get("item"))) : new FamiliarSpecial(node);
                return target(op).executeSpecialMove(s);
            }
            case "specialTarget": { FamiliarSpecial s = new FamiliarSpecial(node(op.get("node"))); return value(s.getTarget()); }
            case "visualize": target(op).visualizeSpecialMove(); return null;
            case "sendHit":
                if (op.containsKey("gfx")) target(op).sendFamiliarHit((Entity) node(op.get("target")), n(op.get("max")), core.game.world.update.flag.context.Graphics.create(n(op.get("gfx"))));
                else target(op).sendFamiliarHit((Entity) node(op.get("target")), n(op.get("max")));
                return null;
            case "projectile": target(op).projectile((Entity) node(op.get("target")), n(op.get("id"))); return null;
            case "canAttack":
                if (op.containsKey("message")) return target(op).canAttack((Entity) node(op.get("target")), Boolean.TRUE.equals(op.get("message")));
                return target(op).canAttack((Entity) node(op.get("target")));
            case "canCombatSpecial":
                if (op.containsKey("message")) return target(op).canCombatSpecial((Entity) node(op.get("target")), Boolean.TRUE.equals(op.get("message")));
                return target(op).canCombatSpecial((Entity) node(op.get("target")));
            case "isOwnerAttackable": return target(op).isOwnerAttackable();
            case "startFollowing": target(op).startFollowing(); return null;
            case "finalizeDeath": target(op).finalizeDeath(null); return null;
            case "isCombatFamiliar": return target(op).isCombatFamiliar();
            case "sendConfiguration": target(op).sendConfiguration(); return null;
            case "call": return target(op).call();
            case "spawnLocation": return coord(target(op).getSpawnLocation());
            case "dismissFamiliar": target(op).dismiss(); return null;
            case "updateSpecialPoints": target(op).updateSpecialPoints(n(op.get("diff"))); return null;
            case "isCharged": return target(op).isCharged();
            case "charge": target(op).charge(); return null;
            case "setCharged": target(op).setCharged(Boolean.TRUE.equals(op.get("value"))); return null;
            case "familiarBoost": return target(op).getBoost(n(op.get("skill")));
            case "isPeaceful": return target(op).isPeacefulFamiliar();
            case "isBurdenBeast": return target(op).isBurdenBeast();
            case "poisonImmune": return ((BurdenBeast) target(op)).isPoisonImmune();
            case "setOwner": target(op).setOwner((Player) node(op.get("owner"))); return null;
            case "getAttackStyle": return target(op).getAttackStyle();
            // ---- BurdenBeast and Forager
            case "isAllowed": return ((BurdenBeast) target(op)).isAllowed((Player) node(op.getOrDefault("owner", "p")), itemOf(op.get("item")));
            case "transfer": ((BurdenBeast) target(op)).transfer(itemOf(op.get("item")), n(op.get("amount")), Boolean.TRUE.equals(op.get("withdraw"))); return null;
            case "withdrawAll": ((BurdenBeast) target(op)).withdrawAll(); return null;
            case "openInterface": ((BurdenBeast) target(op)).openInterface(); return null;
            case "produceItem": return ((Forager) target(op)).produceItem(itemOf(op.get("item")));
            case "produceRandom": return ((Forager) target(op)).produceItem();
            case "setRandomPassive": ((Forager) target(op)).setRandomPassive(); return null;
            case "passiveAction": ((Forager) target(op)).handlePassiveAction(); return null;
            case "listenerUpdate": {
                List its = (List) op.get("items"); Item[] a = new Item[its.size()];
                for (int i = 0; i < a.length; i++) a[i] = itemOf(its.get(i));
                List sl = (List) op.get("slots"); int[] s = new int[sl.size()];
                for (int i = 0; i < s.length; i++) s[i] = n(sl.get(i));
                new BurdenContainerListener(player).update(null, new ContainerEvent(a, s)); return null;
            }
            case "listenerRefresh": new BurdenContainerListener(player).refresh(((BurdenBeast) target(op)).getContainer()); return null;
            case "closeEvent": { Object[] e = closeEvents.get(n(op.get("index"))); return ((CloseEvent) e[0]).close(player, (Component) e[1]); }
            case "pulse": return pulses.get(n(op.get("index"))).pulse();
            // ---- Pet
            case "growNextStage": ((Pet) target(op)).growNextStage(); return null;
            case "setHasWarned": ((Pet) target(op)).setHasWarned(n(op.get("value"))); return null;
            case "petIds": { List l = new ArrayList(); for (int id : new Pet(player, new PetDetails(0), 1555, 761).getIds()) l.add(id); return l; }
            // ---- Pets
            case "petsForId": { Pets p = Pets.forId(n(op.get("item"))); return p == null ? null : p.name(); }
            case "petsNpcId": return Pets.valueOf((String) op.get("pet")).getNpcId(n(op.get("item")));
            case "petsNextStage": return Pets.valueOf((String) op.get("pet")).getNextStageItemId(n(op.get("item")));
            case "petsKitten": return Pets.valueOf((String) op.get("pet")).isKitten(n(op.get("item")));
            case "petsGrownCat": return Pets.valueOf((String) op.get("pet")).isGrownCat(n(op.get("item")));
            case "petsHasPet": return Pets.hasPet(player);
            case "petsDump": {
                List out = new ArrayList();
                for (Pets p : Pets.values()) {
                    List food = new ArrayList(); for (int f : p.getFood()) food.add(f);
                    out.add(Arrays.asList(p.name(), p.getBabyItemId(), p.getGrownItemId(), p.getOvergrownItemId(), p.getBabyNpcId(),
                            p.getGrownNpcId(), p.getOvergrownNpcId(), bits(p.getGrowthRate()), p.getSummoningLevel(), food));
                }
                return out;
            }
            default: throw new IllegalArgumentException(name);
        }
    }

    // ------------------------------------------------------------ snapshot
    static Object field(Object o, Class<?> c, String name) {
        try { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(o); }
        catch (ReflectiveOperationException e) { throw new IllegalStateException(e); }
    }
    static Object label(Object o) {
        if (o == null) return null;
        Integer i = labels.get(o);
        if (i == null) labels.put(o, i = labels.size() + 1);
        return i;
    }
    static List details(PetDetails d) { return Arrays.asList(label(d), bits(d.getHunger()), bits(d.getGrowth())); }

    static Map familiarState(Familiar f) {
        Map s = new TreeMap();
        Class<?> c = Familiar.class;
        s.put("owner", key(field(f, c, "owner")));
        s.put("ticks", field(f, c, "ticks"));
        s.put("max", field(f, c, "maximumTicks"));
        s.put("special", field(f, c, "specialPoints"));
        s.put("pouch", field(f, c, "pouchId"));
        s.put("cost", field(f, c, "specialCost"));
        s.put("hasPouch", field(f, c, "pouch") != null);
        Object handler = field(f, c, "combatHandler");
        s.put("handler", handler == null ? null : ((CombatSwingHandler) handler).key);
        s.put("combat", field(f, c, "combatFamiliar"));
        s.put("charged", field(f, c, "charged"));
        List boosts = new ArrayList();
        for (Object b : (List) field(f, c, "boosts")) {
            core.game.node.entity.skill.SkillBonus sb = (core.game.node.entity.skill.SkillBonus) b;
            boosts.add(Arrays.asList(sb.getSkillId(), bits(sb.getBonus())));
        }
        s.put("boosts", boosts);
        s.put("style", field(f, c, "attackStyle"));
        s.put("perTick", bits((Double) field(f, c, "pointsPerTick")));
        s.put("frac", bits((Double) field(f, c, "fracDrain")));
        s.put("first", field(f, c, "firstCall"));
        s.put("npc", Arrays.asList(f.getId(), f.getOriginalId(), coord(f.getLocation()), f.isInvisible(), new TreeMap(attributes(f))));
        if (f instanceof BurdenBeast) {
            s.put("container", ((BurdenBeast) f).getContainer().key);
            if (f instanceof Forager) {
                Item[] its = (Item[]) field(f, Forager.class, "items");
                s.put("forager", Arrays.asList(items(its), field(f, Forager.class, "passiveDelay")));
            }
        }
        if (f instanceof Pet) {
            Pet p = (Pet) f;
            s.put("pet", Arrays.asList(p.getItemId(), details(p.getDetails()), bits((Double) field(p, Pet.class, "growthRate")), p.getPet().name(), p.getHasWarned()));
        }
        s.put("kind", f instanceof Pet ? "pet" : f instanceof Forager ? "forager" : f instanceof BurdenBeast ? "beast" : "plain");
        return s;
    }
    static Map attributes(Entity e) {
        Map out = new TreeMap();
        for (Object en : e.attributes.entrySet()) {
            Map.Entry entry = (Map.Entry) en;
            out.put(entry.getKey(), value(entry.getValue()));
        }
        return out;
    }

    static Map snapshot() {
        FamiliarManager m = player.getFamiliarManager();
        Map s = new TreeMap();
        Familiar current = m.getFamiliar();
        if (current != null) kept(current);
        Map manager = new TreeMap();
        manager.put("familiar", current == null ? null : key(current));
        manager.put("hasPouch", m.isHasPouch());
        manager.put("level", m.getSummoningCombatLevel());
        List pets = new ArrayList();
        for (Map.Entry<Integer, ArrayList<PetDetails>> e : m.getPetDetails().entrySet()) {
            List list = new ArrayList();
            for (PetDetails d : e.getValue()) list.add(details(d));
            pets.add(Arrays.asList(e.getKey(), list));
        }
        manager.put("pets", pets);
        s.put("manager", manager);
        List registry = new ArrayList();
        for (Map.Entry<Integer, Familiar> e : new TreeMap<>(FamiliarManager.getFamiliars()).entrySet()) {
            kept(e.getValue());
            registry.add(Arrays.asList(e.getKey(), key(e.getValue())));
        }
        s.put("registry", registry);
        List familiars = new ArrayList();
        for (Familiar f : seen) familiars.add(Arrays.asList(key(f), familiarState(f)));
        s.put("familiars", familiars);
        s.put("player", Arrays.asList(coord(player.getLocation()), new TreeMap(attributes(player))));
        return s;
    }

    // ------------------------------------------------------------ the case
    public static String runCase(String line) throws Exception {
        input = (Map) new JSONParser().parse(line);
        events = new ArrayList<>();
        calls = new HashMap<>();
        nodes = new HashMap<>();
        answers = (Map) input.getOrDefault("answers", new HashMap());
        answerCalls = new HashMap<>();
        pulses = new ArrayList<>();
        closeEvents = new ArrayList<>();
        seen = new ArrayList<>();
        labels = new IdentityHashMap<>();
        npcs = 0; containers = 0; states = 0;
        quiet = true;
        Map p = spec("player");
        player = new Player("p");
        player.version = n(p.getOrDefault("version", 2L));
        player.setLocationQuiet(location(p.getOrDefault("location", Arrays.asList(3200L, 3200L, 0L))));
        for (Object e : spec("attributes").entrySet()) {
            Map.Entry en = (Map.Entry) e; Object v = en.getValue();
            player.attributes.put((String) en.getKey(), v instanceof Long ? ((Long) v).intValue() : v);
        }
        nodes.put("p", player);
        for (Object e : spec("nodes").entrySet()) {
            Map.Entry en = (Map.Entry) e; Map d = (Map) en.getValue(); String k = (String) en.getKey();
            Node node;
            String kind = (String) d.get("kind");
            if ("player".equals(kind)) node = new Player(k);
            else if ("npc".equals(kind)) { NPC npc = new NPC(n(d.getOrDefault("id", 1L)), null); npc.key = k; node = npc; }
            else { Item i = new Item(n(d.getOrDefault("id", 995L))); i.key = k; node = i; }
            node.setLocationQuiet(location(d.get("location")));
            node.setInvisibleQuiet(Boolean.TRUE.equals(d.get("invisible")));
            nodes.put(k, node);
        }
        // Attribute values naming nodes are resolved once every node exists.
        for (Object e : spec("nodes").entrySet()) {
            Map.Entry en = (Map.Entry) e; Map d = (Map) en.getValue(); Node node = nodes.get(en.getKey());
            Map attrs = (Map) d.get("attributes");
            if (attrs != null && node instanceof Entity) for (Object a : attrs.entrySet()) {
                Map.Entry ae = (Map.Entry) a; Object v = ae.getValue();
                ((Entity) node).attributes.put((String) ae.getKey(), v instanceof Long ? ((Long) v).intValue() : v instanceof Map ? nodes.get(((Map) v).get("node")) : v);
            }
        }
        npcs = 0;
        quiet = false;
        List out = new ArrayList();
        for (Object o : (List) input.get("ops")) {
            Map op = (Map) o;
            failAt = n(op.getOrDefault("failAt", 0L));
            opCalls = 0;
            events = new ArrayList<>();
            Map row = new TreeMap();
            Object result = null;
            String error = null;
            try {
                result = op(op);
            } catch (Failure f) {
                error = "Binding";
            } catch (NullPointerException e) {
                error = "Null";
            } catch (ClassCastException e) {
                error = "Cast";
            } catch (NumberFormatException e) {
                error = "Number";
            } catch (IndexOutOfBoundsException e) {
                error = "Index";
            } catch (Throwable t) {
                error = t.getClass().getName() + ": " + t.getMessage();
            }
            row.put("events", events);
            row.put("result", result instanceof Double ? bits((Double) result) : result);
            row.put("error", error);
            row.put("state", snapshot());
            out.add(row);
        }
        Map record = new LinkedHashMap(input);
        record.put("expected", out);
        return JSONValue.toJSONString(record);
    }
}
