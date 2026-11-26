package oracle;

import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.impl.Projectile;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import core.game.world.update.flag.chunk.ProjectileUpdateFlag;
import core.net.packet.IoBuffer;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

import java.util.*;

/**
 * Runs one projectiles case (Boot gives every case a class loader of its own). The case names entities (players,
 * NPCs or plain entities with a size, a location and an index) and a list of operations on the unchanged
 * Projectile, ProjectileUpdateFlag and UpdateFlag. Every call the originals make on a node or the region manager is
 * recorded in order; field reads (Node.getCenterLocation reads size and location directly) are not calls.
 * {@code failAt} makes the n-th recorded call throw after it is recorded.
 */
public final class Support {
    public static final class Failure extends IllegalStateException {
        public Failure() { super("injected"); }
    }

    public static List<Object> events = new ArrayList<>();
    static int calls, failAt;
    static final List<Entity> entities = new ArrayList<>();
    static final List<Projectile> projectiles = new ArrayList<>();
    static final List<ProjectileUpdateFlag> flags = new ArrayList<>();

    static int num(Object o) { return ((Number) o).intValue(); }

    public static String label(Object o) {
        if (o == null) return null;
        for (int i = 0; i < entities.size(); i++) if (entities.get(i) == o) return "e" + i;
        for (int i = 0; i < projectiles.size(); i++) if (projectiles.get(i) == o) return "p" + i;
        return "?" + o.getClass().getSimpleName();
    }

    public static Object value(Object v) {
        if (v == null || v instanceof Integer || v instanceof Boolean || v instanceof String) return v;
        if (v instanceof Location) {
            Location l = (Location) v;
            return Arrays.asList(l.getX(), l.getY(), l.getZ());
        }
        if (v instanceof Double) { Map<String, Object> m = new TreeMap<>(); m.put("double", v); return m; }
        if (v instanceof Described) return value(((Described) v).describe());
        return label(v);
    }

    /** Objects that record as a value of their own (a region chunk records the location it was asked for). */
    public interface Described { Object describe(); }

    /** A recorded call; the n-th one fails when the case asks for it. */
    public static void ev(String name, Object... args) {
        List<Object> row = new ArrayList<>();
        row.add(name);
        for (Object a : args) row.add(value(a));
        events.add(row);
        calls++;
        if (calls == failAt) throw new Failure();
    }

    static Entity entity(Object o) {
        if (o == null) return null;
        int i = num(o);
        return i < 0 ? null : entities.get(i);
    }

    static Location location(Object o) {
        if (o == null) return null;
        JSONArray a = (JSONArray) o;
        if ("ent".equals(a.get(0))) return entities.get(num(a.get(1))).getLocation0();
        if ("src".equals(a.get(0))) return projectiles.get(num(a.get(1))).getSourceLocation();
        if ("end".equals(a.get(0))) return projectiles.get(num(a.get(1))).getEndLocation();
        return new Location(num(a.get(0)), num(a.get(1)), num(a.get(2)));
    }

    static Projectile slot(Object o) { return projectiles.get(num(o)); }

    static Object keep(Projectile p) {
        for (int i = 0; i < projectiles.size(); i++) if (projectiles.get(i) == p) return "p" + i;
        projectiles.add(p);
        return "p" + (projectiles.size() - 1);
    }

    static Object op(JSONArray o) {
        String name = (String) o.get(0);
        switch (name) {
            case "create": {
                int arity = num(o.get(1));
                Entity s = entity(o.get(2)), v = entity(o.get(3));
                int id = num(o.get(4));
                switch (arity) {
                    case 3: return keep(Projectile.create(s, v, id));
                    case 5: return keep(Projectile.create(s, v, id, num(o.get(5)), num(o.get(6))));
                    case 6: return keep(Projectile.create(s, v, id, num(o.get(5)), num(o.get(6)), num(o.get(7))));
                    case 7: return keep(Projectile.create(s, v, id, num(o.get(5)), num(o.get(6)), num(o.get(7)), num(o.get(8))));
                    case 8: return keep(Projectile.create(s, v, id, num(o.get(5)), num(o.get(6)), num(o.get(7)), num(o.get(8)), num(o.get(9))));
                    case 9: return keep(Projectile.create(s, v, id, num(o.get(5)), num(o.get(6)), num(o.get(7)), num(o.get(8)), num(o.get(9)), num(o.get(10))));
                    default: throw new IllegalArgumentException("arity");
                }
            }
            case "createLoc":
                return keep(Projectile.create(location(o.get(1)), location(o.get(2)), num(o.get(3)), num(o.get(4)), num(o.get(5)), num(o.get(6)), num(o.get(7)), num(o.get(8)), num(o.get(9))));
            case "magic":
                return keep(Projectile.magic(entity(o.get(1)), entity(o.get(2)), num(o.get(3)), num(o.get(4)), num(o.get(5)), num(o.get(6)), num(o.get(7))));
            case "ranged":
                return keep(Projectile.ranged(entity(o.get(1)), entity(o.get(2)), num(o.get(3)), num(o.get(4)), num(o.get(5)), num(o.get(6)), num(o.get(7))));
            case "empty": return keep(new Projectile());
            case "getSpeed": return Projectile.getSpeed(entity(o.get(1)), location(o.get(2)));
            case "getLocation": return value(Projectile.getLocation(entity(o.get(1))));
            case "transform": return keep(slot(o.get(1)).transform(entity(o.get(2)), entity(o.get(3))));
            case "transform5": return keep(slot(o.get(1)).transform(entity(o.get(2)), entity(o.get(3)), (Boolean) o.get(4), num(o.get(5)), num(o.get(6))));
            case "transformLoc": return keep(slot(o.get(1)).transform(entity(o.get(2)), location(o.get(3))));
            case "transformLoc5": return keep(slot(o.get(1)).transform(entity(o.get(2)), location(o.get(3)), (Boolean) o.get(4), num(o.get(5)), num(o.get(6))));
            case "copy": return keep(slot(o.get(1)).copy(entity(o.get(2)), entity(o.get(3)), ((Number) o.get(4)).doubleValue()));
            case "send": slot(o.get(1)).send(); return null;
            case "sendStatic": Projectile.send(slot(o.get(1))); return null;
            case "sendTo": Projectile.send(entity(o.get(1)), entity(o.get(2)), slot(o.get(3))); return null;
            case "sendAt": Projectile.send(entity(o.get(1)), location(o.get(2)), slot(o.get(3))); return null;
            case "set": {
                Projectile p = slot(o.get(1));
                Object v = o.get(3);
                switch ((String) o.get(2)) {
                    case "source": p.setSource(entity(v)); break;
                    case "sourceLocation": p.setSourceLocation(location(v)); break;
                    case "victim": p.setVictim(entity(v)); break;
                    case "projectileId": p.setProjectileId(num(v)); break;
                    case "startHeight": p.setStartHeight(num(v)); break;
                    case "endHeight": p.setEndHeight(num(v)); break;
                    case "startDelay": p.setStartDelay(num(v)); break;
                    case "speed": p.setSpeed(num(v)); break;
                    case "angle": p.setAngle(num(v)); break;
                    case "distance": p.setDistance(num(v)); break;
                    case "endLocation": p.setEndLocation(location(v)); break;
                    default: throw new IllegalArgumentException("field");
                }
                return null;
            }
            case "write": {
                IoBuffer buffer = new IoBuffer();
                ProjectileUpdateFlag flag = flags.get(num(o.get(1)));
                Map<String, Object> out = new TreeMap<>();
                try {
                    flag.write(buffer);
                } finally {
                    out.put("bytes", buffer.oracleBytes());
                    lastBytes = buffer.oracleBytes();
                }
                out.put("data", flag.data());
                out.put("ordinal", flag.ordinal());
                return out;
            }
            case "compare": return flags.get(num(o.get(1))).compareTo(flags.get(num(o.get(2))));
            case "equals": return flags.get(num(o.get(1))).equals(flags.get(num(o.get(2))));
            case "move": entities.get(num(o.get(1))).oracleLocation(location(o.get(2))); return null;
            case "setLoc": {
                Location l = entities.get(num(o.get(1))).getLocation0();
                l.setX(num(o.get(2)));
                l.setY(num(o.get(3)));
                l.setZ(num(o.get(4)));
                return null;
            }
            case "index": entities.get(num(o.get(1))).oracleIndex(num(o.get(2))); return null;
            case "size": entities.get(num(o.get(1))).oracleSize(num(o.get(2))); return null;
            default: throw new IllegalArgumentException(name);
        }
    }

    static List<Object> lastBytes;

    static Map<String, Object> state(Projectile p) {
        Map<String, Object> m = new TreeMap<>();
        m.put("source", value(p.getSource()));
        m.put("sourceLocation", value(p.getSourceLocation()));
        m.put("victim", value(p.getVictim()));
        m.put("projectileId", p.getProjectileId());
        m.put("startHeight", p.getStartHeight());
        m.put("endHeight", p.getEndHeight());
        m.put("startDelay", p.getStartDelay());
        m.put("speed", p.getSpeed());
        m.put("angle", p.getAngle());
        m.put("distance", p.getDistance());
        m.put("endLocation", value(p.getEndLocation()));
        m.put("locationBased", p.isLocationBased());
        return m;
    }

    static String error(Throwable t) {
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof IllegalStateException) return "Binding";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        return t.getClass().getName() + ": " + t.getMessage();
    }

    @SuppressWarnings("unchecked")
    public static String runCase(String line) throws Exception {
        JSONObject c = (JSONObject) new JSONParser().parse(line);
        failAt = c.containsKey("failAt") ? num(c.get("failAt")) : -1;
        for (Object o : (JSONArray) c.get("entities")) {
            JSONObject e = (JSONObject) o;
            String kind = (String) e.get("kind");
            Entity entity = "player".equals(kind) ? new Player() : "npc".equals(kind) ? new NPC() : new Entity();
            entity.oracleSize(num(e.get("size")));
            entity.oracleIndex(num(e.get("index")));
            entity.oracleLocation(location(e.get("loc")));
            entities.add(entity);
        }
        List<Object> steps = new ArrayList<>();
        for (Object o : (JSONArray) c.get("ops")) {
            Map<String, Object> step = new TreeMap<>();
            int before = events.size();
            lastBytes = null;
            try {
                step.put("result", op((JSONArray) o));
            } catch (Failure | NullPointerException | IndexOutOfBoundsException f) {
                step.put("error", error(f));
                if (lastBytes != null) step.put("bytes", lastBytes);
            }
            step.put("events", new ArrayList<>(events.subList(before, events.size())));
            List<Object> slots = new ArrayList<>();
            for (Projectile p : projectiles) slots.add(state(p));
            step.put("projectiles", slots);
            List<Object> flagged = new ArrayList<>();
            for (ProjectileUpdateFlag f : flags) flagged.add(value(f.getContext()));
            step.put("flags", flagged);
            steps.add(step);
        }
        JSONObject out = new JSONObject();
        out.putAll(c);
        Map<String, Object> expected = new TreeMap<>();
        expected.put("steps", steps);
        expected.put("calls", calls);
        out.put("expected", expected);
        return out.toJSONString();
    }

    /** {@code RegionChunk.flag}: the flag list of the chunk the case observes. */
    public static void flagged(Object chunk, Object flag) {
        ev("RegionChunk.flag", chunk, ((ProjectileUpdateFlag) flag).getContext());
        flags.add((ProjectileUpdateFlag) flag);
    }
}
