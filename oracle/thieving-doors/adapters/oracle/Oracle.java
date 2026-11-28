package oracle;

import content.global.skill.thieving.PickableDoorHandler;
import core.game.node.entity.player.Player;
import core.game.node.scenery.Scenery;
import core.game.world.map.Location;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.List;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/** Drives the unmodified pickable door source over the recording world, one JSON case per line. */
public final class Oracle {
    private static final Player PLAYER = new Player();

    public static void main(String[] args) throws Throwable {
        JSONParser parser = new JSONParser();
        new PickableDoorHandler().newInstance(null);
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        PrintStream out = new PrintStream(System.out, false, "UTF-8");
        for (String line; (line = in.readLine()) != null; ) {
            if (line.isEmpty()) continue;
            JSONObject c = (JSONObject) parser.parse(line);
            c.put("expected", run(c));
            out.println(c.toJSONString());
        }
        out.flush();
    }

    @SuppressWarnings("unchecked")
    private static JSONArray run(JSONObject c) throws Exception {
        W.state = (JSONObject) c.get("state");
        W.draws = (JSONArray) W.state.get("draws");
        W.drawn = 0;
        JSONArray results = new JSONArray();
        for (Object o : (JSONArray) c.get("ops")) {
            JSONObject op = (JSONObject) o;
            W.trace = new JSONArray();
            JSONObject result = new JSONObject();
            try {
                result.put("result", step(op));
            } catch (RuntimeException e) {
                result.put("throws", failure(e));
            }
            result.put("trace", W.trace);
            results.add(result);
        }
        return results;
    }

    private static String failure(RuntimeException e) {
        if (e instanceof NullPointerException) return "Null";
        if (e instanceof ClassCastException) return "Cast";
        return e.getClass().getName();
    }

    private static Scenery scenery(JSONObject op) {
        JSONArray at = (JSONArray) op.get("at");
        Location location = Location.create(((Number) at.get(0)).intValue(), ((Number) at.get(1)).intValue(), ((Number) at.get(2)).intValue());
        return new Scenery(((Number) op.get("node")).intValue(), location, ((Number) op.get("rotation")).intValue());
    }

    private static Object field(Object target, String name) throws Exception {
        Field f = target.getClass().getDeclaredField(name);
        f.setAccessible(true);
        return f.get(target);
    }

    @SuppressWarnings("unchecked")
    private static Object step(JSONObject op) throws Exception {
        String kind = (String) op.get("op");
        switch (kind) {
            case "registrations":
                return W.registrations;
            case "doors": {
                // pickableDoors, in the order newInstance added them (private fields read by reflection).
                Field list = PickableDoorHandler.class.getDeclaredField("pickableDoors");
                list.setAccessible(true);
                JSONArray all = new JSONArray();
                for (Object o : (List<?>) list.get(null)) {
                    PickableDoorHandler.PickableDoor door = (PickableDoorHandler.PickableDoor) o;
                    JSONObject row = new JSONObject();
                    JSONArray locations = new JSONArray();
                    for (Location l : door.getLocations()) {
                        JSONArray at = new JSONArray();
                        at.add(l.getX()); at.add(l.getY()); at.add(l.getZ());
                        locations.add(at);
                    }
                    row.put("locations", locations); row.put("level", door.getLevel());
                    row.put("experience", W.bits(door.getExperience())); row.put("lockpick", door.isLockpick());
                    row.put("flipped", field(door, "flipped"));
                    all.add(row);
                }
                return all;
            }
            case "set":
                W.state.putAll((JSONObject) op.get("state"));
                return null;
            case "handle":
                // The registered plugin, with a scenery of the id, tile and rotation the operation names.
                return W.handler.handle(PLAYER, scenery(op), (String) op.get("option"));
            case "destination": {
                Location at = W.handler.getDestination(PLAYER, scenery(op));
                if (at == null) return null;
                JSONArray out = new JSONArray();
                out.add(at.getX()); out.add(at.getY()); out.add(at.getZ());
                return out;
            }
            default:
                throw new IllegalArgumentException(kind);
        }
    }
}
