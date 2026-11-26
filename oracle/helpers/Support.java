package oracle;

import core.game.node.Node;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.node.entity.combat.graves.Grave;
import core.game.node.entity.combat.graves.GraveController;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.Rights;
import core.game.world.map.Location;
import kotlin.jvm.functions.Function2;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

import java.util.*;

/**
 * Runs one helpers case (Boot gives every case a class loader of its own): the unchanged AttackListener registered
 * through a recording InteractionListener and its handler run on an NPC; the verbatim Player.toggleDebug, debug and
 * isDebug; the verbatim GraveController.hasGraveAt over the unchanged Location. {@code failAt} makes the n-th
 * recorded call throw after it is recorded.
 */
public final class Support {
    public static final class Failure extends IllegalStateException {
        public Failure() { super("injected"); }
    }

    public static List<Object> events = new ArrayList<>();
    static int calls, failAt;
    public static Player player;
    public static final List<Node> nodes = new ArrayList<>();
    public static final Map<String, Object> attributes = new HashMap<>();
    public static CombatSwingHandler handler;
    public static Rights rights;
    public static Function2<Player, Node, Boolean> attack;

    static int num(Object o) { return ((Number) o).intValue(); }

    public static Object value(Object v) {
        if (v == null || v instanceof Integer || v instanceof Boolean || v instanceof String) return v;
        if (v instanceof Enum) return ((Enum<?>) v).name();
        if (v instanceof Object[]) { List<Object> l = new ArrayList<>(); for (Object x : (Object[]) v) l.add(value(x)); return l; }
        if (v == player) return "player";
        for (int i = 0; i < nodes.size(); i++) if (nodes.get(i) == v) return "n" + i;
        return "?" + v.getClass().getSimpleName();
    }

    public static void ev(String name, Object... args) {
        List<Object> row = new ArrayList<>();
        row.add(name);
        for (Object a : args) row.add(value(a));
        events.add(row);
        calls++;
        if (calls == failAt) throw new Failure();
    }

    /** {@code InteractionListener.on(type, options, handler)}. */
    public static void registered(Object type, String[] options, Function2<Player, Node, Boolean> handler) {
        ev("InteractionListener.on", type, options);
        attack = handler;
    }

    static Location location(Object o) {
        if (o == null) return null;
        JSONArray a = (JSONArray) o;
        return new Location(num(a.get(0)), num(a.get(1)), num(a.get(2)));
    }

    /** A stored attribute as the case gives it: booleans, ints (Integer) and text. */
    static Object stored(Object o) {
        return o instanceof Long ? (Object) ((Long) o).intValue() : o;
    }

    static void style(Object s) {
        if ("absent".equals(s)) handler = null;
        else handler = new CombatSwingHandler(s == null ? null : CombatStyle.valueOf((String) s));
    }

    static Object op(JSONArray o) throws Exception {
        String name = (String) o.get(0);
        switch (name) {
            case "define":
                ((core.game.interaction.InteractionListener) Class.forName("content.global.skill.AttackListener").getDeclaredConstructor().newInstance()).defineListeners();
                return null;
            case "node": {
                Node n = Boolean.TRUE.equals(o.get(2)) ? new NPC(num(o.get(1))) : new Node(num(o.get(1)));
                nodes.add(n);
                return value(n);
            }
            case "attack": return attack.invoke(player, nodes.get(num(o.get(1))));
            case "style": style(o.get(1)); return null;
            case "toggleDebug": player.toggleDebug(); return null;
            case "debug": player.debug((String) o.get(1)); return null;
            case "isDebug": return player.isDebug();
            case "attribute":
                if (o.get(2) == null) attributes.remove((String) o.get(1)); else attributes.put((String) o.get(1), stored(o.get(2)));
                return null;
            case "rights": rights = o.get(1) == null ? null : Rights.valueOf((String) o.get(1)); return null;
            case "grave": {
                Grave g = new Grave(location(o.get(2)));
                GraveController.Companion.getActiveGraves().put(num(o.get(1)), g);
                return null;
            }
            case "graveGone": GraveController.Companion.getActiveGraves().remove(num(o.get(1))); return null;
            case "hasGraveAt": return GraveController.hasGraveAt(location(o.get(1)));
            default: throw new IllegalArgumentException(name);
        }
    }

    static String error(Throwable t) {
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof ClassCastException) return "Cast";
        if (t instanceof IllegalStateException) return "Binding";
        return t.getClass().getName() + ": " + t.getMessage();
    }

    @SuppressWarnings("unchecked")
    public static String runCase(String line) throws Exception {
        JSONObject c = (JSONObject) new JSONParser().parse(line);
        failAt = c.containsKey("failAt") ? num(c.get("failAt")) : -1;
        player = new Player();
        style(c.get("style"));
        rights = c.get("rights") == null ? null : Rights.valueOf((String) c.get("rights"));
        if (c.get("attributes") != null) for (Object e : ((JSONObject) c.get("attributes")).entrySet()) {
            Map.Entry<String, Object> entry = (Map.Entry<String, Object>) e;
            attributes.put(entry.getKey(), stored(entry.getValue()));
        }
        List<Object> steps = new ArrayList<>();
        for (Object o : (JSONArray) c.get("ops")) {
            Map<String, Object> step = new TreeMap<>();
            int before = events.size();
            try {
                step.put("result", op((JSONArray) o));
            } catch (Failure | NullPointerException | ClassCastException f) {
                step.put("error", error(f));
            }
            step.put("events", new ArrayList<>(events.subList(before, events.size())));
            step.put("attributes", new TreeMap<>(attributes));
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
