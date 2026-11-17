package oracle;

import content.global.handlers.iface.tabs.PrayerTabInterface;
import core.game.component.Component;
import core.game.event.Event;
import core.game.event.PrayerActivatedEvent;
import core.game.event.PrayerDeactivatedEvent;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.equipment.ArmourSet;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.prayer.Prayer;
import core.game.node.entity.player.link.prayer.PrayerType;
import core.game.node.entity.skill.SkillBonus;
import core.game.node.entity.skill.Skills;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import core.tools.RandomFunction;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.*;
import kotlin.jvm.functions.Function6;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/** Reads one case per line, runs its operations against the unchanged prayer classes and prints what they did. */
public class Support {
    static Map input;
    static List<Object> events;
    static int calls, failAt;
    static List<Entity> entities;
    static Map<Entity, Map> specs;
    static List<Pulse> pulses;
    static int registeredComponent;
    static Function6 handler;

    public static class Failure extends RuntimeException {}

    static class Rng extends Random {
        Rng(long seed) { super(seed); }
        @Override public int nextInt(int bound) { int v = super.nextInt(bound); emit("next_int", bound, v); return v; }
    }

    static String hex(double v) { return Long.toHexString(Double.doubleToRawLongBits(v)); }

    public static void emit(String name, Object... args) {
        List row = new ArrayList();
        row.add(name);
        for (Object a : args) row.add(a);
        events.add(row);
        calls++;
        if (calls == failAt) throw new Failure();
    }

    public static Integer ix(Object e) {
        if (e == null) return null;
        for (int i = 0; i < entities.size(); i++) if (entities.get(i) == e) return i;
        throw new IllegalStateException("unknown entity");
    }

    public static List coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }

    static Location loc(Object o) {
        if (o == null) return null;
        List a = (List) o;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }

    static int n(Object o) { return ((Number) o).intValue(); }

    public static boolean flag(Entity e, String key, boolean fallback) {
        Object v = specs.get(e).get(key);
        return v == null ? fallback : Boolean.TRUE.equals(v);
    }

    public static boolean perk(Player p, String name) {
        Object v = specs.get(p).get("perks");
        return v != null && ((List) v).contains(name);
    }

    public static void queue(Pulse pulse) { pulses.add(pulse); }

    public static void register(int component, Function6 h) { registeredComponent = component; handler = h; }

    public static void event(Player p, Event e) {
        PrayerType t = e instanceof PrayerActivatedEvent ? ((PrayerActivatedEvent) e).getType() : ((PrayerDeactivatedEvent) e).getType();
        emit("dispatch", ix(p), e.getClass().getSimpleName(), t.name());
    }

    public static <T> List<T> locals(String key, Class<T> kind) {
        List<T> out = new ArrayList<>();
        Object v = input.get(key);
        if (v == null) {
            for (Entity e : entities) if (kind.isInstance(e)) out.add(kind.cast(e));
        } else {
            for (Object o : (List) v) out.add(kind.cast(entities.get(n(o))));
        }
        return out;
    }

    static Field field(Class<?> c, String name) {
        try { Field f = c.getDeclaredField(name); f.setAccessible(true); return f; } catch (Exception e) { throw new IllegalStateException(e); }
    }

    static PrayerType type(Object o) { return o == null ? null : PrayerType.valueOf((String) o); }

    static CombatStyle style(Object o) { return o == null ? null : CombatStyle.valueOf((String) o); }

    static Entity entity(Object o) { return o == null ? null : entities.get(n(o)); }

    static List names(PrayerType[] types) {
        List out = new ArrayList();
        for (PrayerType t : types) out.add(t == null ? null : t.name());
        return out;
    }

    static void apply(Entity e, Map m) {
        Map spec = specs.get(e);
        for (Object o : m.entrySet()) {
            Map.Entry en = (Map.Entry) o;
            String key = (String) en.getKey();
            Object v = en.getValue();
            switch (key) {
                case "location": e.location = loc(v); break;
                case "static":
                    for (Object s : ((Map) v).entrySet()) {
                        Map.Entry se = (Map.Entry) s;
                        e.getSkills().staticLevels[Integer.parseInt((String) se.getKey())] = n(se.getValue());
                    }
                    break;
                case "points": e.getSkills().prayerPoints = Double.longBitsToDouble(Long.parseUnsignedLong((String) v, 16)); break;
                case "lifepoints": e.getSkills().lifepoints = n(v); break;
                case "bonuses":
                    if (v == null) e.getProperties().bonuses = null;
                    else {
                        List l = (List) v;
                        int[] b = new int[l.size()];
                        for (int i = 0; i < b.length; i++) b[i] = n(l.get(i));
                        e.getProperties().bonuses = b;
                    }
                    break;
                case "protect": e.getProperties().protectStyle = style(v); break;
                case "headIcon": ((Player) e).getAppearance().headIcon = n(v); break;
                case "varbit3909": ((Player) e).varbits.put(3909, n(v)); break;
                default: spec.put(key, v);
            }
        }
    }

    static Map snapshot(Entity e) {
        Map m = new LinkedHashMap();
        if (!(e instanceof Player)) {
            m.put("npc", true);
            return m;
        }
        Player p = (Player) e;
        List active = new ArrayList();
        for (PrayerType t : p.getPrayer().getActive()) active.add(t.name());
        m.put("active", active);
        try {
            m.put("ticks", field(Prayer.class, "prayerActiveTicks").getInt(p.getPrayer()));
        } catch (IllegalAccessException x) { throw new IllegalStateException(x); }
        Skills s = p.getSkills();
        m.put("points", hex(s.prayerPoints));
        m.put("lifepoints", s.lifepoints);
        m.put("lifepointsUpdate", s.lifepointsUpdate);
        m.put("headIcon", p.getAppearance().headIcon);
        List varps = new ArrayList();
        for (Integer k : new TreeSet<>(p.varpMap.keySet())) varps.add(Arrays.asList(k, p.varpMap.get(k)));
        m.put("varps", varps);
        List saved = new ArrayList();
        for (Integer k : new TreeSet<>(p.saveVarp.keySet())) saved.add(Arrays.asList(k, p.saveVarp.get(k)));
        m.put("saved", saved);
        m.put("varbit3909", p.varbits.getOrDefault(3909, 0));
        return m;
    }

    static List table() {
        List rows = new ArrayList();
        for (PrayerType t : PrayerType.values()) {
            Map m = new LinkedHashMap();
            m.put("name", t.name());
            m.put("ordinal", t.ordinal());
            m.put("level", t.getLevel());
            m.put("drain", t.getDrain());
            m.put("config", t.getConfig());
            m.put("button", t.getButton());
            m.put("restriction", t.getRestriction().name());
            m.put("secondRestriction", t.getSecondRestriction() == null ? null : t.getSecondRestriction().name());
            m.put("sound", Arrays.asList(t.getSound().id, t.getSound().delay, t.getSound().loops, t.getSound().radius));
            m.put("defenceReq", t.getDefenceReq());
            List bonuses = new ArrayList();
            for (SkillBonus b : t.getBonuses()) bonuses.add(Arrays.asList(b.getSkillId(), hex(b.getBonus()), b.getBaseBonus()));
            m.put("bonuses", bonuses);
            rows.add(m);
        }
        List styles = new ArrayList();
        for (CombatStyle s : CombatStyle.values()) styles.add(Arrays.asList(s.name(), s.getProtectionPrayer().name()));
        return Arrays.asList(rows, styles, registeredComponent);
    }

    static Object run(Map op) {
        String name = (String) op.get("op");
        Entity e = entities.get(op.containsKey("p") ? n(op.get("p")) : 0);
        Player p = e instanceof Player ? (Player) e : null;
        switch (name) {
            case "toggle": return p.getPrayer().toggle(type(op.get("type")));
            case "button": return handler.invoke(p, new Component(), 0, n(op.get("button")), 0, -1);
            case "type_toggle": return type(op.get("type")).toggle(p, Boolean.TRUE.equals(op.get("on")));
            case "permitted": return type(op.get("type")).permitted(p);
            case "flag": type(op.get("type")).flag(p, type(op.get("type"))); return null;
            case "find_next_icon": type(op.get("type")).findNextIcon(p); return null;
            case "reset": p.getPrayer().reset(); return null;
            case "tick": p.getPrayer().tick(); return null;
            case "pulse": {
                List<Pulse> due = new ArrayList<>(pulses);
                pulses.clear();
                List out = new ArrayList();
                for (Pulse pulse : due) out.add(pulse.pulse());
                return out;
            }
            case "redemption": p.getPrayer().startRedemption(); return null;
            case "retribution": p.getPrayer().startRetribution(entity(op.get("killer"))); return null;
            case "commence_death": p.commenceDeath(entity(op.get("killer"))); return null;
            case "full_restore": p.fullRestore(); return null;
            case "skill_bonus": return hex(p.getPrayer().getSkillBonus(n(op.get("skill"))));
            case "effective": {
                double level = n(op.get("level"));
                return hex(Math.floor(level + (p.getPrayer().getSkillBonus(n(op.get("skill"))) * level)));
            }
            case "get": return p.getPrayer().get(type(op.get("type")));
            case "icon": return type(op.get("type")).getIcon(p, type(op.get("type")));
            case "has_icon": return PrayerType.THICK_SKIN.hasIcon(p);
            case "by_bonus": {
                List ids = (List) op.get("ids");
                int[] a = new int[ids.size()];
                for (int i = 0; i < a.length; i++) a[i] = n(ids.get(i));
                return names(PrayerType.getByBonus(a));
            }
            case "melee_types": return names(PrayerType.getMeleeTypes());
            case "range_types": return names(PrayerType.getRangeTypes());
            case "magic_types": return names(PrayerType.getMagicTypes());
            case "for_button": { PrayerType t = PrayerType.get(n(op.get("button"))); return t == null ? null : t.name(); }
            case "protection": return entity(op.get("e")).hasProtectionPrayer(style(op.get("style")));
            case "formatted_hit": {
                BattleState s = new BattleState();
                s.attacker = entity(op.get("attacker"));
                s.victim = entity(op.get("victim"));
                s.style = style(op.get("style"));
                s.armourEffect = op.get("armour") == null ? null : ArmourSet.valueOf((String) op.get("armour"));
                return hex(entity(op.get("on")).getFormattedHit(s, n(op.get("hit"))));
            }
            case "set": apply(entity(op.get("e")), (Map) op.get("fields")); return null;
            case "table": return table();
            default: throw new IllegalStateException("unknown op " + name);
        }
    }

    static String error(Throwable t) {
        if (t instanceof Failure) return "Binding";
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        if (t instanceof ConcurrentModificationException) return "ConcurrentModification";
        return t.getClass().getName();
    }

    public static void main(String[] args) throws Exception {
        new PrayerTabInterface().defineInterfaceListeners();
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        JSONParser parser = new JSONParser();
        String line;
        StringBuilder out = new StringBuilder();
        while ((line = reader.readLine()) != null) {
            if (line.isEmpty()) continue;
            input = (Map) parser.parse(line);
            events = new ArrayList<>();
            entities = new ArrayList<>();
            specs = new IdentityHashMap<>();
            pulses = new ArrayList<>();
            RandomFunction.RANDOM = new Rng(((Number) input.getOrDefault("seed", 0L)).longValue());
            for (Object o : (List) input.get("entities")) {
                Map spec = (Map) o;
                Entity e = "npc".equals(spec.get("kind")) ? new NPC() : new Player();
                entities.add(e);
                specs.put(e, new HashMap());
                e.getSkills().staticLevels[Skills.HITPOINTS] = 10;
                for (int i = 0; i < 24; i++) if (i != Skills.HITPOINTS) e.getSkills().staticLevels[i] = 1;
                apply(e, spec);
            }
            List results = new ArrayList();
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
                for (Entity e : entities) states.add(snapshot(e));
                step.put("state", states);
                step.put("pulses", pulses.size());
                results.add(step);
            }
            Map row = new LinkedHashMap();
            row.put("input", input);
            row.put("expected", results);
            out.append(JSONValue.toJSONString(row)).append('\n');
        }
        System.out.write(out.toString().getBytes(StandardCharsets.UTF_8));
        System.out.flush();
    }
}
