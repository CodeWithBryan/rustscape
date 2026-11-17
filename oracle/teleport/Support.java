package oracle;

import core.cache.def.impl.AnimationDefinition;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.impl.Animator;
import core.game.node.entity.impl.WalkingQueue;
import core.game.node.entity.lock.ActionLocks;
import core.game.node.entity.lock.Lock;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.TeleportManager;
import core.game.node.entity.player.link.TeleportManager.TeleportType;
import core.game.system.task.Pulse;
import core.game.system.timer.RSTimer;
import core.game.world.map.Location;
import core.game.world.map.zone.MapZone;
import core.game.world.map.zone.RegionZone;
import core.game.world.update.flag.EntityFlag;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.*;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/** Reads one case per line, runs its operations against the unchanged teleport classes and prints what they did. */
public class Support {
    static Map input;
    static List<Object> events;
    static int calls, failAt, ticks;
    static List<Entity> entities;
    static Map<Entity, Map> specs;
    static List<Location> dests;
    static List<Pulse> pulses;
    static ArrayDeque<Pulse> queue;
    static List<MapZone> zones;
    static List<List<Boolean>> zoneAnswers;
    static Map<String, Integer> counts;
    static Set<Integer> fired;
    static boolean quiet;

    public static class Failure extends RuntimeException {}

    /** The attribute map of GameAttributes: every get, put and remove is recorded, except the walking queue's routedraw read. */
    public static class Attributes extends HashMap<String, Object> {
        private final Entity entity;
        public Attributes(Entity entity) { this.entity = entity; }
        @Override public Object get(Object key) {
            if (!quiet && !"routedraw".equals(key)) emit("get_attribute", ix(entity), key);
            return super.get(key);
        }
        @Override public Object put(String key, Object value) {
            if (!quiet) emit("set_attribute", ix(entity), key, label(value));
            return super.put(key, value);
        }
        @Override public Object remove(Object key) {
            if (!quiet) emit("remove_attribute", ix(entity), key);
            return super.remove(key);
        }
    }

    /** Captures Throwable.printStackTrace: its first line is the throwable itself. */
    static class Err extends PrintStream {
        Err() { super(new ByteArrayOutputStream()); }
        @Override public void println(Object x) {
            if (x instanceof Throwable && events != null) events.add(Arrays.asList("print_stack_trace", error((Throwable) x)));
        }
    }

    public static void emit(String name, Object... args) {
        List row = new ArrayList();
        row.add(name);
        Collections.addAll(row, args);
        events.add(row);
        calls++;
        int count = counts.merge(name, 1, Integer::sum);
        List hooks = (List) input.get("hooks");
        if (hooks != null) {
            for (int i = 0; i < hooks.size(); i++) {
                Map hook = (Map) hooks.get(i);
                if (name.equals(hook.get("event")) && n(hook.getOrDefault("at", 1L)) == count && fired.add(i)) {
                    for (Object action : (List) hook.get("actions")) run((Map) action);
                }
            }
        }
        if (calls == failAt) throw new Failure();
    }

    static int n(Object o) { return ((Number) o).intValue(); }

    public static Integer ix(Object e) {
        if (e == null) return null;
        for (int i = 0; i < entities.size(); i++) if (entities.get(i) == e) return i;
        throw new IllegalStateException("unknown entity");
    }

    public static List coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }

    static Integer dest(Location l) {
        if (l == null) return null;
        for (int i = 0; i < dests.size(); i++) if (dests.get(i) == l) return i;
        return null;
    }

    public static Integer pulseIx(Pulse p) {
        if (p == null) return null;
        for (int i = 0; i < pulses.size(); i++) if (pulses.get(i) == p) return i;
        pulses.add(p);
        return pulses.size() - 1;
    }

    static Object label(Object value) {
        if (value instanceof Pulse) return Arrays.asList("pulse", pulseIx((Pulse) value));
        if (value == null || value instanceof Boolean || value instanceof Number || value instanceof String) return value;
        return Arrays.asList("object", value.getClass().getName());
    }

    public static int ticks() { emit("ticks"); return ticks; }

    public static void submit(Pulse pulse) {
        emit("submit", pulseIx(pulse));
        queue.add(pulse);
    }

    public static int number(Entity e, String key, int fallback) {
        Object v = specs.get(e).get(key);
        return v == null ? fallback : n(v);
    }

    public static RSTimer timer(Entity e, String name) {
        emit("get_timer", ix(e), name);
        List active = (List) specs.get(e).get("timers");
        return active != null && active.contains(name) ? new RSTimer() : null;
    }

    /** A record that is neither a failure point nor a hook trigger: the original forId catches every Throwable. */
    static void record(String name, Object... args) {
        List row = new ArrayList();
        row.add(name);
        Collections.addAll(row, args);
        events.add(row);
    }

    public static AnimationDefinition definition(int id) {
        record("definition", id);
        Map table = (Map) input.get("durations");
        String key = Integer.toString(id);
        if (table != null && table.containsKey(key)) {
            Object v = table.get(key);
            if ("missing".equals(v)) return null;
            AnimationDefinition d = new AnimationDefinition();
            if (v != null) {
                List l = (List) v;
                d.durations = new int[l.size()];
                for (int i = 0; i < l.size(); i++) d.durations[i] = n(l.get(i));
            }
            return d;
        }
        return new AnimationDefinition();
    }

    public static void register(Entity e, EntityFlag flag, Object context) {
        if (context instanceof Animation) {
            Animation a = (Animation) context;
            emit("register", ix(e), flag.name(), a.getId(), a.getDelay(), a.getPriority() == null ? null : a.getPriority().name());
        } else {
            Graphics g = (Graphics) context;
            emit("register", ix(e), flag.name(), g.getId(), g.getHeight(), g.getDelay());
        }
    }

    public static boolean zoneTeleport(MapZone zone, Entity e, int type, Node node) {
        emit("zone_teleport", zone.index, ix(e), type, node == null ? null : "node");
        List<Boolean> answers = zoneAnswers.get(zone.index);
        return answers.isEmpty() || answers.remove(0);
    }

    public static void dispatch(Entity e, core.game.event.Event event) {
        if (event instanceof core.game.event.AttributeSetEvent) {
            core.game.event.AttributeSetEvent s = (core.game.event.AttributeSetEvent) event;
            emit("dispatch", ix(e), "AttributeSetEvent", s.getAttribute(), label(s.getValue()));
        } else {
            emit("dispatch", ix(e), "AttributeRemoveEvent", ((core.game.event.AttributeRemoveEvent) event).getAttribute());
        }
    }

    public static boolean zoneUpdate(Entity e, Location last) {
        emit("zone_update", ix(e), coord(last));
        return true;
    }

    static Field field(Class<?> c, String name) {
        try { Field f = c.getDeclaredField(name); f.setAccessible(true); return f; } catch (Exception x) { throw new IllegalStateException(x); }
    }

    static Object get(Class<?> c, String name, Object target) {
        try { return field(c, name).get(target); } catch (IllegalAccessException x) { throw new IllegalStateException(x); }
    }

    static TeleportType type(Object o) { return o == null ? null : TeleportType.valueOf((String) o); }

    static Location loc(Object o) {
        if (o == null) return null;
        List a = (List) o;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }

    static Entity entity(Map op) { return entities.get(op.containsKey("e") ? n(op.get("e")) : 0); }

    static List table() throws Exception {
        List rows = new ArrayList();
        for (TeleportType t : TeleportType.values()) {
            Object s = t.getSettings();
            List row = new ArrayList();
            row.add(t.name());
            row.add(t.ordinal());
            for (String getter : new String[] {"getStartEmote", "getEndEmote", "getStartGfx", "getEndGfx"}) {
                Method m = s.getClass().getDeclaredMethod(getter);
                m.setAccessible(true);
                row.add(m.invoke(s));
            }
            rows.add(row);
        }
        return Arrays.asList(rows, TeleportManager.WILDY_TELEPORT);
    }

    static void quietly(Runnable r) {
        boolean was = quiet;
        quiet = true;
        try { r.run(); } finally { quiet = was; }
    }

    static Object run(Map op) {
        String name = (String) op.get("op");
        Entity e = entity(op);
        switch (name) {
            case "send": {
                Location l = op.get("loc") == null ? null : dests.get(n(op.get("loc")));
                int form = n(op.get("form"));
                if (form == 1) return e.getTeleporter().send(l);
                if (form == 2) return e.getTeleporter().send(l, type(op.get("type")));
                return e.getTeleporter().send(l, type(op.get("type")), n(op.get("tt")));
            }
            case "pulse": {
                int count = queue.size();
                for (int i = 0; i < count; i++) {
                    Pulse p = queue.poll();
                    try {
                        if (!p.update() && p.isRunning()) queue.add(p);
                    } catch (Exception x) {
                        emit("log", "Pulse execution error. Stack trace below.");
                        x.printStackTrace();
                    }
                }
                return null;
            }
            case "move":
                for (Entity each : entities) each.getWalkingQueue().update();
                return null;
            case "end":
                for (Entity each : entities) {
                    each.getProperties().setTeleporting(false);
                    if (each instanceof Player) {
                        Player.Flags f = ((Player) each).getPlayerFlags();
                        if (f.update) f.last = each.getLocation();
                        f.setUpdateSceneGraph(false);
                    }
                }
                ticks++;
                return null;
            case "stop": e.getTeleporter().getCurrentTeleport().stop(); return null;
            case "start": e.getTeleporter().getCurrentTeleport().start(); return null;
            case "stop_pulse": pulses.get(n(op.get("pulse"))).stop(); return null;
            case "teleporting": return e.isTeleporting();
            case "get_type": return TeleportManager.getType((Player) e).name();
            case "teleport_type": return e.getTeleporter().getTeleportType();
            case "set_teleport_type": e.getTeleporter().setTeleportType(n(op.get("value"))); return null;
            case "lock_teleport": e.getLocks().lockTeleport(n(op.get("ticks"))); return null;
            case "lock": e.lock(n(op.get("ticks"))); return null;
            case "lock_forever": e.lock(); return null;
            case "unlock": e.unlock(); return null;
            case "clear": e.getPulseManager().clear(); return null;
            case "timer": {
                List active = (List) specs.get(e).computeIfAbsent("timers", k -> new ArrayList());
                active.remove(op.get("name"));
                if (Boolean.TRUE.equals(op.get("on"))) active.add(op.get("name"));
                return null;
            }
            case "attr":
                quietly(() -> {
                    if (Boolean.TRUE.equals(op.get("remove"))) e.getAttributes().getAttributes().remove(op.get("key"));
                    else e.getAttributes().getAttributes().put((String) op.get("key"), op.get("value") instanceof Long ? (Object) n(op.get("value")) : op.get("value"));
                });
                return null;
            case "spellbook": specs.get(e).put("spellbook", op.get("value")); return null;
            case "zone_add": e.getZoneMonitor().getZones().add(new RegionZone(zones.get(n(op.get("zone"))))); return null;
            case "zone_remove": {
                List<RegionZone> list = e.getZoneMonitor().getZones();
                for (int i = 0; i < list.size(); i++) {
                    if (list.get(i).getZone() == zones.get(n(op.get("zone")))) { list.remove(i); break; }
                }
                return null;
            }
            case "mutate": {
                Location l = dests.get(n(op.get("d")));
                List v = (List) op.get("to");
                l.setX(n(v.get(0)));
                l.setY(n(v.get(1)));
                l.setZ(n(v.get(2)));
                return null;
            }
            case "table":
                try { return table(); } catch (Exception x) { throw new IllegalStateException(x); }
            default: throw new IllegalStateException("unknown op " + name);
        }
    }

    static List lock(Lock l) { return Arrays.asList(get(Lock.class, "expiration", l)); }

    static Map snapshot(Entity e) {
        Map m = new LinkedHashMap();
        m.put("location", coord(e.getLocation()));
        m.put("locationDest", dest(e.getLocation()));
        m.put("teleportLocation", coord(e.getProperties().getTeleportLocation()));
        m.put("teleportDest", dest(e.getProperties().getTeleportLocation()));
        m.put("teleporting", e.getProperties().isTeleporting());
        WalkingQueue q = e.getWalkingQueue();
        m.put("footprint", coord(q.getFootPrint()));
        m.put("points", q.getQueue().size());
        m.put("walkDir", q.getWalkDir());
        ActionLocks locks = e.getLocks();
        List l = new ArrayList();
        for (String f : new String[] {"movementLock", "teleportLock", "interactionLock"}) l.add(get(Lock.class, "expiration", get(ActionLocks.class, f, locks)));
        m.put("locks", l);
        Animator a = e.getAnimator();
        Animation an = (Animation) get(Animator.class, "animation", a);
        Graphics g = (Graphics) get(Animator.class, "graphics", a);
        Object pr = get(Animator.class, "priority", a);
        m.put("animation", an == null ? null : Arrays.asList(an.getId(), an.getDelay(), an.getPriority() == null ? null : an.getPriority().name()));
        m.put("graphics", g == null ? null : Arrays.asList(g.getId(), g.getHeight(), g.getDelay()));
        m.put("priority", pr == null ? null : pr.toString());
        m.put("animatorTicks", get(Animator.class, "ticks", a));
        m.put("animationEnd", e.clocks[1]);
        m.put("disabledTicks", e.getImpactHandler().getDisabledTicks());
        if (e instanceof Player) {
            m.put("scriptDelay", e.scripts.getDelay());
            Player.Flags f = ((Player) e).getPlayerFlags();
            m.put("scene", f.update);
            m.put("lastScene", coord(f.last));
        }
        Map attrs = new TreeMap();
        for (Map.Entry<String, Object> en : e.getAttributes().getAttributes().entrySet()) attrs.put(en.getKey(), label(en.getValue()));
        m.put("attributes", attrs);
        TeleportManager t = e.getTeleporter();
        m.put("current", pulseIx(t.getCurrentTeleport()));
        m.put("last", pulseIx((Pulse) get(TeleportManager.class, "lastTeleport", t)));
        m.put("teleportType", t.getTeleportType());
        List z = new ArrayList();
        for (RegionZone r : e.getZoneMonitor().getZones()) z.add(r.getZone().index);
        m.put("zones", z);
        return m;
    }

    static List pulseStates() {
        List out = new ArrayList();
        for (Pulse p : pulses) {
            Object counter = null;
            try { counter = get(p.getClass(), "delay", p); } catch (IllegalStateException x) { counter = null; }
            out.add(Arrays.asList(p.getClass().getName(), p.isRunning(), p.getTicksPassed(), p.getDelay(), counter));
        }
        return out;
    }

    static String error(Throwable t) {
        if (t instanceof Failure) return "Binding";
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof ClassCastException) return "ClassCast";
        if (t instanceof ConcurrentModificationException) return "ConcurrentModification";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        return t.getClass().getName();
    }

    public static void main(String[] args) throws Exception {
        System.setErr(new Err());
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        JSONParser parser = new JSONParser();
        String text;
        StringBuilder out = new StringBuilder();
        while ((text = reader.readLine()) != null) {
            if (text.isEmpty()) continue;
            input = (Map) parser.parse(text);
            events = new ArrayList<>();
            calls = 0;
            failAt = -1;
            counts = new HashMap<>();
            fired = new HashSet<>();
            quiet = false;
            ticks = n(input.getOrDefault("ticks", 0L));
            entities = new ArrayList<>();
            specs = new IdentityHashMap<>();
            pulses = new ArrayList<>();
            queue = new ArrayDeque<>();
            dests = new ArrayList<>();
            zones = new ArrayList<>();
            zoneAnswers = new ArrayList<>();
            for (Object o : (List) input.get("dests")) dests.add(loc(o));
            List zoneSpecs = (List) input.getOrDefault("zones", new ArrayList());
            for (int i = 0; i < zoneSpecs.size(); i++) {
                Map z = (Map) zoneSpecs.get(i);
                zones.add(new MapZone(i, n(z.getOrDefault("restriction", 0L))));
                List<Boolean> answers = new ArrayList<>();
                for (Object b : (List) z.getOrDefault("answers", new ArrayList())) answers.add((Boolean) b);
                zoneAnswers.add(answers);
            }
            for (Object o : (List) input.get("entities")) {
                Map spec = (Map) o;
                Entity e = "npc".equals(spec.get("kind")) ? new NPC() : new Player();
                entities.add(e);
                Map state = new HashMap(spec);
                if (state.get("timers") != null) state.put("timers", new ArrayList((List) state.get("timers")));
                specs.put(e, state);
                e.location = loc(spec.get("loc"));
                e.init();
                if (e instanceof Player && spec.get("last") != null) ((Player) e).getPlayerFlags().last = loc(spec.get("last"));
                Map attrs = (Map) spec.get("attrs");
                if (attrs != null) {
                    quietly(() -> {
                        for (Object en : new TreeMap(attrs).entrySet()) {
                            Map.Entry entry = (Map.Entry) en;
                            Object v = entry.getValue();
                            e.getAttributes().getAttributes().put((String) entry.getKey(), v instanceof Long ? (Object) n(v) : v);
                        }
                    });
                }
                for (Object zi : (List) spec.getOrDefault("zones", new ArrayList())) e.getZoneMonitor().getZones().add(new RegionZone(zones.get(n(zi))));
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
                failAt = -1;
                step.put("result", result);
                step.put("error", error);
                step.put("calls", calls);
                step.put("events", events);
                quiet = true;
                List states = new ArrayList();
                for (Entity each : entities) states.add(snapshot(each));
                quiet = false;
                step.put("state", states);
                step.put("pulses", pulseStates());
                List q = new ArrayList();
                for (Pulse p : queue) q.add(pulseIx(p));
                step.put("queue", q);
                step.put("ticks", ticks);
                results.add(step);
            }
            events = null;
            Map row = new LinkedHashMap();
            row.put("input", input);
            row.put("expected", results);
            out.append(JSONValue.toJSONString(row)).append('\n');
        }
        System.out.write(out.toString().getBytes(StandardCharsets.UTF_8));
        System.out.flush();
    }
}
