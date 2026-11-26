package oracle;

import core.cache.def.impl.NPCDefinition;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import core.game.world.repository.Repository;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

import java.util.*;

/**
 * Runs one npc-runtime case (Boot gives every case a class loader of its own): the verbatim NPC constructors,
 * NPC.transform, transformWithHpCarryover, reTransform, NPC.clear, Entity.init and Entity.clear over recording
 * adapters, with the unchanged NodeList as Repository.getNpcs(). Every call the originals make outside the object is
 * recorded in order; field writes show in the snapshot after each operation. {@code failAt} makes the n-th recorded
 * call throw after it is recorded.
 */
public final class Support {
    public static final class Failure extends IllegalStateException {
        public Failure() { super("injected"); }
    }

    public static List<Object> events = new ArrayList<>();
    static int calls, failAt;
    public static final List<Entity> entities = new ArrayList<>();
    public static final Map<Integer, NPCDefinition> definitions = new HashMap<>();
    static int definitionSerial;
    static List<Object> ordinals = new ArrayList<>();
    static int ordinalCalls;
    public static final Map<Entity, Integer> lifepoints = new IdentityHashMap<>();
    public static final List<Entity> renderable = new ArrayList<>();

    static int num(Object o) { return ((Number) o).intValue(); }

    public static String label(Object o) {
        if (o == null) return null;
        for (int i = 0; i < entities.size(); i++) if (entities.get(i) == o) return "n" + i;
        if (o instanceof NPCDefinition) return ((NPCDefinition) o).label;
        return "?" + o.getClass().getSimpleName();
    }

    public static Object value(Object v) {
        if (v == null || v instanceof Integer || v instanceof Boolean || v instanceof String) return v;
        if (v instanceof Location) {
            Location l = (Location) v;
            return Arrays.asList(l.getX(), l.getY(), l.getZ());
        }
        if (v instanceof Enum) return ((Enum<?>) v).name();
        if (v instanceof List) return v;
        return label(v);
    }

    public static void ev(String name, Object... args) {
        List<Object> row = new ArrayList<>();
        row.add(name);
        for (Object a : args) row.add(value(a));
        events.add(row);
        calls++;
        if (calls == failAt) throw new Failure();
    }

    /** {@code super(name, location)}: the entity object exists from here on. */
    public static void allocated(Entity e) {
        ev("Entity.<init>", e.getName(), e.getLocation());
        entities.add(e);
    }

    static JSONArray hooks = new JSONArray();
    static int lookups;

    /** {@code NPCDefinition.forId}: a case may change the table right after its n-th lookup answered. */
    public static NPCDefinition definition(int id) {
        ev("NPCDefinition.forId", id);
        NPCDefinition found = definitions.get(id);
        lookups++;
        for (Object h : hooks) {
            JSONObject hook = (JSONObject) h;
            if (num(hook.get("at")) == lookups) define((JSONObject) hook.get("def"));
        }
        return found;
    }

    public static int ordinal() {
        ev("EntityFlags.getOrdinal", "NPC", "TypeSwap");
        if (ordinals.isEmpty()) return 5;
        return num(ordinals.get(Math.min(ordinalCalls++, ordinals.size() - 1)));
    }

    static void define(JSONObject d) {
        int id = num(d.get("id"));
        if (Boolean.TRUE.equals(d.get("absent"))) {
            definitions.remove(id);
            return;
        }
        NPCDefinition def = new NPCDefinition();
        def.label = "d" + (definitionSerial++);
        def.name = (String) d.get("name");
        def.size = num(d.get("size"));
        definitions.put(id, def);
    }

    static Location location(Object o) {
        if (o == null) return null;
        JSONArray a = (JSONArray) o;
        return new Location(num(a.get(0)), num(a.get(1)), num(a.get(2)));
    }

    static Entity entity(Object o) { return entities.get(num(o)); }
    static NPC npc(Object o) { return (NPC) entities.get(num(o)); }

    static Object op(JSONArray o) {
        String name = (String) o.get(0);
        switch (name) {
            case "new1": return label(new NPC(num(o.get(1))));
            case "new2": return label(new NPC(num(o.get(1)), location(o.get(2))));
            case "new3": return label(NPC.oracleNew(num(o.get(1)), location(o.get(2)), o.get(3) == null ? null : Direction.valueOf((String) o.get(3))));
            case "player": return label(new Player((String) o.get(1), location(o.get(2))));
            case "entityInit": entity(o.get(1)).init(); return null;
            case "clear": entity(o.get(1)).clear(); return null;
            case "transform": return label(npc(o.get(1)).transform(num(o.get(2))));
            case "transformHp": return label(npc(o.get(1)).transformWithHpCarryover(num(o.get(2))));
            case "reTransform": npc(o.get(1)).reTransform(); return null;
            case "add": return Repository.getNpcs().add(npc(o.get(1)));
            case "renderable": Repository.addRenderableNPC(npc(o.get(1))); return null;
            case "move": entity(o.get(1)).oracleLocation(location(o.get(2))); return null;
            case "def": define((JSONObject) o.get(1)); return null;
            case "lifepoints": lifepoints.put(entity(o.get(1)), num(o.get(2))); return null;
            case "behavior": npc(o.get(1)).behavior = null; return null;
            default: throw new IllegalArgumentException(name);
        }
    }

    static Map<String, Object> state(Entity e) {
        Map<String, Object> m = new TreeMap<>();
        m.put("kind", e instanceof Player ? "player" : "npc");
        m.put("name", e.getName());
        m.put("location", value(e.getLocation()));
        m.put("index", e.getIndex());
        m.put("size", e.size());
        m.put("active", e.isActive());
        m.put("direction", value(e.oracleDirection()));
        m.put("plugin", e.oraclePlugin());
        m.put("lifepoints", lifepoints.getOrDefault(e, 0));
        m.put("renderable", renderable.contains(e));
        if (e instanceof NPC) {
            NPC n = (NPC) e;
            m.put("id", n.oracleId());
            m.put("originalId", n.oracleOriginalId());
            m.put("definition", label(n.oracleDefinition()));
            m.put("behavior", n.behavior == null ? null : n.behavior.id);
        }
        return m;
    }

    static String error(Throwable t) {
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof IllegalStateException) return "Binding";
        if (t instanceof IndexOutOfBoundsException || t instanceof ClassCastException) return "Index";
        return t.getClass().getName() + ": " + t.getMessage();
    }

    @SuppressWarnings("unchecked")
    public static String runCase(String line) throws Exception {
        JSONObject c = (JSONObject) new JSONParser().parse(line);
        failAt = c.containsKey("failAt") ? num(c.get("failAt")) : -1;
        Repository.oracleCapacity(c.containsKey("capacity") ? num(c.get("capacity")) : 8);
        if (c.containsKey("ordinals")) ordinals = (JSONArray) c.get("ordinals");
        if (c.containsKey("hooks")) hooks = (JSONArray) c.get("hooks");
        for (Object d : (JSONArray) c.get("defs")) define((JSONObject) d);
        List<Object> steps = new ArrayList<>();
        for (Object o : (JSONArray) c.get("ops")) {
            Map<String, Object> step = new TreeMap<>();
            int before = events.size();
            try {
                step.put("result", op((JSONArray) o));
            } catch (Failure | NullPointerException | IndexOutOfBoundsException | ClassCastException f) {
                step.put("error", error(f));
            }
            step.put("events", new ArrayList<>(events.subList(before, events.size())));
            List<Object> all = new ArrayList<>();
            for (Entity e : entities) all.add(state(e));
            step.put("entities", all);
            List<Object> slots = new ArrayList<>();
            for (int i = 1; i < Repository.getNpcs().length(); i++) slots.add(label(Repository.getNpcs().get(i)));
            step.put("repository", slots);
            step.put("repositorySize", Repository.getNpcs().size());
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
}
